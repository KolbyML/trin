use std::{path::Path, sync::Arc};

use anyhow::ensure;
use parking_lot::Mutex;

use crate::{
    chain_spec::ChainSpec,
    config::StateConfig,
    evm::genesis::import_genesis,
    storage::{
        execution_position::ExecutionPositionV2, state::evm_db::EvmDB, utils::setup_rocksdb,
    },
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
