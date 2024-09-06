use alloy_primitives::{keccak256, Address, Bytes, B256, U256};
use alloy_rlp::Decodable;
use anyhow::{anyhow, bail, ensure};
use eth_trie::{RootWithTrieDiff, Trie};
use ethportal_api::types::state_trie::account_state::AccountState as AccountStateInfo;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    era::manager::EraManager,
    evm::execution_context::ExecutionContext,
    metrics::{start_timer_vec, stop_timer, BLOCK_PROCESSING_TIMES},
    storage::{
        account::Account,
        evm_db::EvmDB,
        execution_position::ExecutionPosition,
        utils::{get_default_data_dir, setup_rocksdb},
    },
};

use super::{config::StateConfig, types::trie_proof::TrieProof, utils::address_to_nibble_path};

#[derive(Debug, Serialize, Deserialize)]
struct AllocBalance {
    balance: U256,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenesisConfig {
    alloc: HashMap<Address, AllocBalance>,
    state_root: B256,
}

pub struct State {
    pub database: EvmDB,
    pub config: StateConfig,
    execution_position: ExecutionPosition,
    pub era_manager: Arc<Mutex<EraManager>>,
    pub node_data_directory: PathBuf,
}

const GENESIS_STATE_FILE: &str = "trin-execution/resources/genesis/mainnet.json";
const TEST_GENESIS_STATE_FILE: &str = "resources/genesis/mainnet.json";

impl State {
    pub async fn new(path: Option<PathBuf>, config: StateConfig) -> anyhow::Result<Self> {
        let node_data_directory = match path {
            Some(path_buf) => path_buf,
            None => get_default_data_dir()?,
        };
        let db = Arc::new(setup_rocksdb(node_data_directory.clone())?);
        let execution_position = ExecutionPosition::initialize_from_db(db.clone())?;

        let database = EvmDB::new(config.clone(), db, &execution_position)
            .expect("Failed to create EVM database");

        let era_manager = Arc::new(Mutex::new(
            EraManager::new(execution_position.next_block_number()).await?,
        ));

        Ok(State {
            execution_position,
            config,
            era_manager,
            database,
            node_data_directory,
        })
    }

    pub fn initialize_genesis(&mut self) -> anyhow::Result<RootWithTrieDiff> {
        ensure!(
            self.execution_position.next_block_number() == 0,
            "Trying to initialize genesis but received block {}",
            self.execution_position.next_block_number(),
        );

        let genesis_file = if Path::new(GENESIS_STATE_FILE).is_file() {
            File::open(GENESIS_STATE_FILE)?
        } else if Path::new(TEST_GENESIS_STATE_FILE).is_file() {
            File::open(TEST_GENESIS_STATE_FILE)?
        } else {
            bail!("Genesis file not found")
        };
        let genesis: GenesisConfig = serde_json::from_reader(BufReader::new(genesis_file))?;

        for (address, alloc_balance) in genesis.alloc {
            let mut account = Account::default();
            account.balance += alloc_balance.balance;
            self.database.trie.lock().insert(
                keccak256(address).as_ref(),
                &alloy_rlp::encode(AccountStateInfo::from(&account)),
            )?;
            self.database
                .db
                .put(keccak256(address.as_slice()), alloy_rlp::encode(account))?;
        }

        let root_with_trie_diff = self.get_root_with_trie_diff()?;
        ensure!(
            root_with_trie_diff.root == genesis.state_root,
            "Root doesn't match state root from genesis file"
        );

        self.execution_position.set_next_block_number(
            self.database.db.clone(),
            1,
            root_with_trie_diff.root,
        )?;

        Ok(root_with_trie_diff)
    }

    /// This is a lot faster then process_block() as it executes the range in memory, but we won't
    /// return the trie diff so you can use this to sync up to the block you want, then use
    /// `process_block()` to get the trie diff to gossip on the state bridge
    pub async fn process_range_of_blocks(
        &mut self,
        start: u64,
        end: u64,
    ) -> anyhow::Result<RootWithTrieDiff> {
        info!("Processing blocks from {} to {} (inclusive)", start, end);
        let mut execution_context = ExecutionContext::new(
            self.database.clone(),
            self.config.block_to_trace.clone(),
            self.node_data_directory.clone(),
        );
        let range_start = Instant::now();
        let mut last_state_root = self.execution_position.state_root();
        for block_number in start..=end {
            let timer = start_timer_vec(&BLOCK_PROCESSING_TIMES, &["fetching_block_from_era"]);
            let block = self
                .era_manager
                .lock()
                .await
                .get_next_block()
                .await?
                .clone();
            stop_timer(timer);

            execution_context.execute_block(&block)?;
            last_state_root = block.header.state_root;

            // Commit the bundle if we have reached the limits, to prevent to much memory usage
            // We won't use this during the dos attack to avoid writing empty accounts to disk
            if !(2_200_000..2_700_000).contains(&block_number)
                && should_we_commit_block_execution_early(
                    block_number - start,
                    execution_context.bundle_size_hint() as u64,
                    execution_context.cumulative_gas_used(),
                    range_start.elapsed(),
                )
            {
                break;
            }
        }

        let root_with_trie_diff = execution_context.commit_bundle(last_state_root)?;

        let timer = start_timer_vec(&BLOCK_PROCESSING_TIMES, &["set_block_execution_number"]);
        self.execution_position.set_next_block_number(
            self.database.db.clone(),
            execution_context.block_number() + 1,
            root_with_trie_diff.root,
        )?;
        stop_timer(timer);

        Ok(root_with_trie_diff)
    }

    pub async fn process_block(&mut self, block_number: u64) -> anyhow::Result<RootWithTrieDiff> {
        self.process_range_of_blocks(block_number, block_number)
            .await
    }

    pub fn next_block_number(&self) -> u64 {
        self.execution_position.next_block_number()
    }

    pub fn get_root(&mut self) -> anyhow::Result<B256> {
        Ok(self.database.trie.lock().root_hash()?)
    }

    pub fn get_root_with_trie_diff(&mut self) -> anyhow::Result<RootWithTrieDiff> {
        Ok(self.database.trie.lock().root_hash_with_changed_nodes()?)
    }

    pub fn get_account_state(&self, account: &Address) -> anyhow::Result<AccountStateInfo> {
        let account_state = self.database.db.get(keccak256(account))?;
        match account_state {
            Some(account) => {
                let account: Account = Decodable::decode(&mut account.as_slice())?;
                Ok(AccountStateInfo::from(&account))
            }
            None => Ok(AccountStateInfo::default()),
        }
    }

    pub fn get_proof(&mut self, address: Address) -> anyhow::Result<TrieProof> {
        let proof: Vec<Bytes> = self
            .database
            .trie
            .lock()
            .get_proof(keccak256(address).as_slice())?
            .into_iter()
            .map(Bytes::from)
            .collect();
        let last_node = proof.last().ok_or(anyhow!("Missing proof!"))?;

        let eth_trie::node::Node::Leaf(last_node) = eth_trie::decode_node(&mut last_node.as_ref())?
        else {
            bail!("Last node in the proof should be leaf!")
        };
        let mut last_node_nibbles = last_node.key.clone();
        if last_node_nibbles.is_leaf() {
            last_node_nibbles.pop();
        } else {
            bail!("Nibbles of the last node should have LEAF Marker")
        }

        let mut path = address_to_nibble_path(address);
        if path.ends_with(last_node_nibbles.get_data()) {
            path.truncate(path.len() - last_node_nibbles.len());
        } else {
            bail!("Path should have a suffix of last node's nibbles")
        }

        Ok(TrieProof { path, proof })
    }

    pub fn get_proofs(&mut self, accounts: &BTreeSet<Address>) -> anyhow::Result<Vec<TrieProof>> {
        accounts
            .iter()
            .map(|account| self.get_proof(*account))
            .collect()
    }
}

/// This function is used to determine if we should commit the block execution early.
/// We want this for a few reasons
/// - To prevent memory usage from getting too high
/// - To cap the amount of time it takes to commit everything to the database, the bigger the
///   changes the more time it takes The various limits are arbitrary and can be adjusted as needed,
///   but are based on the current state of the network and what we have seen so far
pub fn should_we_commit_block_execution_early(
    blocks_processed: u64,
    pending_state_changes: u64,
    cumulative_gas_used: u64,
    elapsed: Duration,
) -> bool {
    blocks_processed >= 500_000
        || pending_state_changes >= 5_000_000
        || cumulative_gas_used >= 30_000_000 * 50_000
        || elapsed >= Duration::from_secs(30 * 60)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        config::StateConfig, era::utils::process_era1_file, storage::utils::setup_temp_dir,
    };

    use super::State;
    use alloy_primitives::Address;
    use revm_primitives::hex::FromHex;

    #[tokio::test]
    async fn test_we_generate_the_correct_state_root_for_the_first_8192_blocks() {
        let temp_directory = setup_temp_dir().unwrap();
        let mut state = State::new(
            Some(temp_directory.path().to_path_buf()),
            StateConfig::default(),
        )
        .await
        .unwrap();
        let _ = state.initialize_genesis().unwrap();
        let raw_era1 = fs::read("../test_assets/era1/mainnet-00000-5ec1ffb8.era1").unwrap();
        let processed_era = process_era1_file(raw_era1, 0).unwrap();
        for block in processed_era.blocks {
            if block.header.number == 0 {
                // initialize genesis state processes this block so we skip it
                state
                    .era_manager
                    .lock()
                    .await
                    .get_next_block()
                    .await
                    .unwrap();
                continue;
            }
            state.process_block(block.header.number).await.unwrap();
            assert_eq!(state.get_root().unwrap(), block.header.state_root);
        }
    }

    #[tokio::test]
    async fn test_get_proof() {
        let temp_directory = setup_temp_dir().unwrap();
        let mut state = State::new(
            Some(temp_directory.path().to_path_buf()),
            StateConfig::default(),
        )
        .await
        .unwrap();
        let _ = state.initialize_genesis().unwrap();
        let valid_proof = state
            .get_proof(Address::from_hex("0x001d14804b399c6ef80e64576f657660804fec0b").unwrap())
            .unwrap();
        assert_eq!(valid_proof.path, [5, 9, 2, 13]);
        // the proof is already tested by eth-trie.rs
    }
}
