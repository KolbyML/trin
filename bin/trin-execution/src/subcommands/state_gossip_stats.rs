use std::{path::Path, sync::Arc, time::Instant};

use alloy::consensus::EMPTY_ROOT_HASH;
use alloy_rlp::Decodable;
use anyhow::{ensure, Ok};
use eth_trie::{decode_node, node::Node, EthTrie, Trie};
use ethportal_api::{
    types::{
        content_value::state::{ContractBytecode, TrieNode},
        state_trie::account_state::AccountState,
    },
    ContentValue, OverlayContentKey, StateContentKey, StateContentValue,
};
use parking_lot::Mutex;
use revm::{Database, DatabaseRef};
use revm_primitives::keccak256;
use tracing::info;

use crate::{
    config::StateConfig,
    content::{
        create_account_content_key, create_account_content_value, create_contract_content_key,
        create_contract_content_value, create_storage_content_key, create_storage_content_value,
    },
    storage::{
        execution_position::ExecutionPositionV2,
        state::{account_db::AccountDB, evm_db::EvmDB},
        utils::setup_rocksdb,
    },
    trie_walker::TrieWalker,
    utils::full_nibble_path_to_address_hash,
};

#[derive(Default, Debug)]
pub struct Stats {
    content_count: usize,
    gossip_size: usize,
    storage_size: usize,
    account_trie_count: usize,
    contract_storage_trie_count: usize,
    contract_bytecode_count: usize,
}

impl Stats {
    /// Panics if either is `Err`
    pub fn check_content(&mut self, key: &StateContentKey, value: &StateContentValue) {
        let value_without_proof = match &value {
            StateContentValue::AccountTrieNodeWithProof(account_trie_node_with_proof) => {
                self.account_trie_count += 1;
                StateContentValue::TrieNode(TrieNode {
                    node: account_trie_node_with_proof
                        .proof
                        .last()
                        .expect("Account trie proof must have at least one trie node")
                        .clone(),
                })
            }
            StateContentValue::ContractStorageTrieNodeWithProof(
                contract_storage_trie_node_with_proof,
            ) => {
                self.contract_storage_trie_count += 1;
                StateContentValue::TrieNode(TrieNode {
                    node: contract_storage_trie_node_with_proof
                        .storage_proof
                        .last()
                        .expect("Storage trie proof much have at least one trie node")
                        .clone(),
                })
            }
            StateContentValue::ContractBytecodeWithProof(contract_bytecode_with_proof) => {
                self.contract_bytecode_count += 1;
                StateContentValue::ContractBytecode(ContractBytecode {
                    code: contract_bytecode_with_proof.code.clone(),
                })
            }
            _ => panic!("Content value doesn't contain proof!"),
        };
        let gossip_size = key.to_bytes().len() + value.encode().len();
        let storage_size = 32 + key.to_bytes().len() + value_without_proof.encode().len();
        self.content_count += 1;
        self.gossip_size += gossip_size;
        self.storage_size += storage_size;
    }
}

pub struct StateGossipStats {
    evm_db: EvmDB,
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
}

impl StateGossipStats {
    pub fn new(data_dir: &Path) -> anyhow::Result<Self> {
        let rocks_db = Arc::new(setup_rocksdb(data_dir)?);

        let execution_position = Arc::new(Mutex::new(ExecutionPositionV2::initialize_from_db(
            rocks_db.clone(),
        )?));
        ensure!(
            execution_position.lock().next_block_number() > 0,
            "Trin execution not initialized!"
        );

        let evm_db = EvmDB::new(
            StateConfig::default(),
            rocks_db,
            execution_position.lock().state_root(),
        )
        .expect("Failed to create EVM database");

        Ok(Self {
            evm_db,
            execution_position,
        })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut stats = Stats::default();
        let start = Instant::now();

        let number = self.execution_position.lock().next_block_number() - 1;
        info!("Starting stats for block number: {}", number);
        let block_hash = self.evm_db.block_hash(number)?;

        let mut leaf_count = 0;

        let root_hash = self.evm_db.trie.lock().root_hash()?;
        let state_walker = TrieWalker::new(root_hash, self.evm_db.trie.lock().db.clone(), None)?;
        for proof in state_walker {
            // check account content key/value
            let content_key =
                create_account_content_key(&proof).expect("Content key should be present");
            let content_value = create_account_content_value(block_hash, &proof)
                .expect("Content key should be present");
            stats.check_content(&content_key, &content_value);

            let Some(encoded_last_node) = proof.proof.last() else {
                panic!("Account proof is empty");
            };

            let Node::Leaf(leaf) = decode_node(&mut encoded_last_node.as_ref())? else {
                continue;
            };

            let account: AccountState = Decodable::decode(&mut leaf.value.as_slice())?;

            // reconstruct the address hash from the path so that we can fetch the
            // address from the database
            let mut partial_key_path = leaf.key.get_data().to_vec();
            partial_key_path.pop();
            let full_key_path = [&proof.path.clone(), partial_key_path.as_slice()].concat();
            let address_hash = full_nibble_path_to_address_hash(&full_key_path);

            leaf_count += 1;
            if leaf_count % 10000 == 0 {
                // we can assume that the address hashes are evenly distributed due to keccak256
                // hash's properties
                let mut be_bytes = [0u8; 4];
                be_bytes.copy_from_slice(&address_hash.0[..4]);
                let address_hash_bytes = u32::from_be_bytes(be_bytes);
                let percentage_done = address_hash_bytes as f64 / u32::MAX as f64 * 100.0;
                info!("Processed {leaf_count} leaves, last address_hash processed: {address_hash}, {percentage_done:.2}% done");
            }

            // check contract code content key/value
            if account.code_hash != keccak256([]) {
                let code: revm_primitives::Bytecode =
                    self.evm_db.code_by_hash_ref(account.code_hash)?;

                let content_key = create_contract_content_key(address_hash, account.code_hash)
                    .expect("Content key should be present");
                let content_value = create_contract_content_value(block_hash, &proof, code)
                    .expect("Content key should be present");
                stats.check_content(&content_key, &content_value);
            }

            // check contract storage content key/value
            if account.storage_root != EMPTY_ROOT_HASH {
                let account_db = AccountDB::new(address_hash, self.evm_db.db.clone());
                let trie = EthTrie::from(Arc::new(account_db), account.storage_root)?.db;

                let storage_walker = TrieWalker::new(account.storage_root, trie, None)?;
                for storage_proof in storage_walker {
                    let content_key = create_storage_content_key(&storage_proof, address_hash)
                        .expect("Content key should be present");
                    let content_value =
                        create_storage_content_value(block_hash, &proof, &storage_proof)
                            .expect("Content key should be present");
                    stats.check_content(&content_key, &content_value);
                }
            }
        }

        info!("Finished stats: {stats:?}");
        info!("Took {} seconds to complete", start.elapsed().as_secs());

        Ok(())
    }
}
