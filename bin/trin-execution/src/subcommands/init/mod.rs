use std::{path::Path, sync::Arc};

use anyhow::{ensure, Error};
use e2store::era2::{AccountEntry, AccountOrStorageEntry, Era2Reader, StorageItem};
use eth_trie::{EthTrie, Trie};
use ethportal_api::Header;
use parking_lot::Mutex;
use revm_primitives::{keccak256, B256, U256};
use tracing::info;

use crate::{
    chain_spec::ChainSpec,
    cli::ImportStateConfig,
    config::StateConfig,
    evm::{block_executor::BLOCKHASH_SERVE_WINDOW, genesis::import_genesis},
    storage::{
        block::BlockStorage,
        execution_position::ExecutionPositionV2,
        state::{account_db::AccountDB, evm_db::EvmDB},
        utils::setup_rocksdb,
    },
    subcommands::era2::utils::percentage_from_address_hash,
    sync::era::{manager::EraManager, types::SyncStatus},
};

pub struct InitState {
    evm_db: EvmDB,
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
}

impl InitState {
    pub fn new(data_dir: &Path) -> anyhow::Result<Self> {
        let rocks_db = Arc::new(setup_rocksdb(data_dir)?);

        let execution_position = Arc::new(Mutex::new(ExecutionPositionV2::initialize_from_db(
            rocks_db.clone(),
        )?));
        ensure!(
            execution_position.lock().next_block_number() == 0,
            "Cannot import genesis file, database is not empty",
        );

        let evm_db = EvmDB::new(
            StateConfig::default(),
            rocks_db.clone(),
            execution_position.lock().state_root(),
        )
        .expect("Failed to create EVM database");

        Ok(Self {
            evm_db,
            execution_position,
        })
    }

    pub fn run(&mut self, chain_spec: Arc<ChainSpec>, save_blocks: bool) -> anyhow::Result<()> {
        let header = import_genesis(&mut self.evm_db, &chain_spec.genesis, save_blocks)?;
        self.execution_position
            .lock()
            .update_position(self.evm_db.db.clone(), &header)?;
        Ok(())
    }
}
