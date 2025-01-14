pub mod chain_rlp_reader;

use std::{path::Path, sync::Arc};

use anyhow::ensure;
use chain_rlp_reader::ChainRlpReader;
use parking_lot::Mutex;
use tracing::info;

use crate::{
    cli::ImportConfig,
    config::StateConfig,
    evm::block_executor::BlockExecutor,
    storage::{
        execution_position::ExecutionPositionV2, state::evm_db::EvmDB, utils::setup_rocksdb,
    },
};

pub struct ImportBlocks {
    evm_db: EvmDB,
}

impl ImportBlocks {
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
            rocks_db,
            execution_position.lock().state_root(),
        )
        .expect("Failed to create EVM database");

        Ok(Self { evm_db })
    }

    pub fn run(
        &mut self,
        import_blocks_config: ImportConfig,
        save_blocks: bool,
    ) -> anyhow::Result<()> {
        let mut block_executor = BlockExecutor::new(self.evm_db.clone(), false, save_blocks);

        let chain_rlp_reader = ChainRlpReader::new(&import_blocks_config.path)?;

        for block in chain_rlp_reader.blocks {
            info!(
                "Importing block number: {}, hash: {}",
                block.header.number,
                block.header.hash_slow()
            );

            block_executor.execute_block(&block)?;

            info!(
                "Block number: {}, hash: {} executed successfully",
                block.header.number,
                block.header.hash_slow()
            );
        }

        Ok(())
    }
}
