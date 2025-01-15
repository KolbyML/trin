use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use alloy::primitives::B256;
use anyhow::{bail, ensure};
use eth_trie::{RootWithTrieDiff, Trie};
use ethportal_api::{types::execution::transaction::Transaction, Header};
use parking_lot::Mutex as ParkingMutex;
use revm::inspectors::TracerEip3155;
use tokio::sync::{oneshot::Receiver, Mutex};
use tracing::{info, warn};

use crate::{
    config::StateConfig,
    evm::block_executor::BlockExecutor,
    metrics::{start_timer_vec, stop_timer, BLOCK_PROCESSING_TIMES},
    storage::{
        execution_position::ExecutionPositionV2, state::evm_db::EvmDB, utils::setup_rocksdb,
    },
    sync::era::{manager::EraManager, types::SyncStatus},
};

pub struct Syncer {
    pub database: EvmDB,
    pub config: StateConfig,
    pub execution_position: Arc<ParkingMutex<ExecutionPositionV2>>,
    pub era_manager: Arc<Mutex<EraManager>>,
    pub data_directory: PathBuf,
}

impl Syncer {
    pub async fn new(data_dir: &Path, config: StateConfig) -> anyhow::Result<Self> {
        let db = Arc::new(setup_rocksdb(data_dir)?);
        let execution_position = Arc::new(ParkingMutex::new(
            ExecutionPositionV2::initialize_from_db(db.clone())?,
        ));

        let database = EvmDB::new(config.clone(), db, execution_position.lock().state_root())
            .expect("Failed to create EVM database");

        let next_block_number = execution_position.lock().next_block_number();
        let era_manager = Arc::new(Mutex::new(EraManager::new(next_block_number).await?));

        Ok(Self {
            execution_position,
            config,
            era_manager,
            database,
            data_directory: data_dir.to_path_buf(),
        })
    }

    pub fn next_block_number(&self) -> u64 {
        self.execution_position.lock().next_block_number()
    }

    pub async fn process_next_block(&mut self) -> anyhow::Result<RootWithTrieDiff> {
        self.process_range_of_blocks(self.next_block_number(), None)
            .await
    }

    /// Processes blocks up to last block number (inclusive) and returns the root with trie diff.
    ///
    /// If the state cache gets too big, we will commit the state and continue. Execution can be
    /// interrupted early by sending `stop_signal`, in which case we will commit and return.
    pub async fn process_range_of_blocks(
        &mut self,
        last_block: u64,
        mut stop_signal: Option<Receiver<()>>,
    ) -> anyhow::Result<RootWithTrieDiff> {
        let start_block = self.execution_position.lock().next_block_number();
        ensure!(
            last_block >= start_block,
            "Last block number {last_block} is less than start block number {start_block}",
        );

        info!("Processing blocks from {start_block} to {last_block} (inclusive)");

        let mut block_executor = BlockExecutor::new(self.database.clone(), false, false);

        let mut block = match self.fetch_next_block().await? {
            SyncStatus::Block(processed_block) => processed_block,
            SyncStatus::Finished => bail!("No more blocks to process"),
            SyncStatus::ConsensusClientIsSyncing => {
                unreachable!("Consensus client is not supported in async syncer")
            }
        };

        loop {
            block_executor
                .execute_block_with_tracer(&block, |tx| self.create_tracer(&block.header, tx))?;

            // Commit and return if we reached last block or stop signal is received.
            let stop_signal_received = stop_signal
                .as_mut()
                .is_some_and(|stop_signal| stop_signal.try_recv().is_ok());

            if block.header.number == last_block || stop_signal_received {
                if stop_signal_received {
                    info!("Stop signal received. Committing now, please wait!");
                }

                let result = self.commit(&block.header, block_executor).await?;

                info!(
                    "Commit done. Stopping execution. Last block executed number: {} state root: {}",
                    block.header.number, block.header.state_root
                );

                return Ok(result);
            }

            // Fetch next block
            let next_block = self.fetch_next_block().await?;

            if next_block == SyncStatus::Finished {
                let result = self.commit(&block.header, block_executor).await?;

                info!(
                    "Commit done. Stopping execution. Last block executed number: {} state root: {}",
                    block.header.number, block.header.state_root
                );

                return Ok(result);
            }

            // Commit early if we have reached the limits, to prevent too much memory usage.
            // We won't use this during the dos attack to avoid writing empty accounts to disk
            if block_executor.should_commit()
                && !(2_200_000..2_700_000).contains(&block.header.number)
            {
                self.commit(&block.header, block_executor).await?;
                block_executor = BlockExecutor::new(self.database.clone(), false, false);
            }

            block = match next_block {
                SyncStatus::Block(processed_block) => processed_block,
                SyncStatus::Finished => panic!("We checked that SyncStatus is not Finished above, so if this panics it's a bug"),
                SyncStatus::ConsensusClientIsSyncing => {
                    unreachable!("Consensus client is not supported in async syncer")
                }
            };
        }
    }

    async fn fetch_next_block(&self) -> anyhow::Result<SyncStatus> {
        let fetching_block_timer =
            start_timer_vec(&BLOCK_PROCESSING_TIMES, &["fetching_block_from_era"]);
        let block = self.era_manager.lock().await.get_next_block().await?;
        stop_timer(fetching_block_timer);
        Ok(block)
    }

    pub fn get_root(&mut self) -> anyhow::Result<B256> {
        Ok(self.database.trie.lock().root_hash()?)
    }

    async fn commit(
        &mut self,
        header: &Header,
        block_executor: BlockExecutor<'_>,
    ) -> anyhow::Result<RootWithTrieDiff> {
        let root_with_trie_diff = block_executor.commit_bundle()?;
        ensure!(
            root_with_trie_diff.root == header.state_root,
            "State root doesn't match! Irreversible! Block number: {} | Generated root: {} | Expected root: {}",
            header.number,
            root_with_trie_diff.root,
            header.state_root
        );

        let update_execution_position_timer =
            start_timer_vec(&BLOCK_PROCESSING_TIMES, &["set_block_execution_number"]);
        self.execution_position
            .lock()
            .update_position(self.database.db.clone(), header)?;
        stop_timer(update_execution_position_timer);

        Ok(root_with_trie_diff)
    }

    fn create_tracer(&self, header: &Header, tx: &Transaction) -> Option<TracerEip3155> {
        self.config
            .block_to_trace
            .create_trace_writer(&self.data_directory, header, tx)
            .unwrap_or_else(|err| {
                warn!("Error while creating trace file: {err}. Skipping.");
                None
            })
            .map(TracerEip3155::new)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use trin_utils::dir::create_temp_test_dir;

    use super::*;
    use crate::sync::era::utils::process_era1_file;

    #[tokio::test]
    #[ignore = "This test downloads data from a remote server"]
    async fn test_we_generate_the_correct_state_root_for_the_first_8192_blocks() {
        let temp_directory = create_temp_test_dir().unwrap();
        let mut trin_execution = Syncer::new(temp_directory.path(), StateConfig::default())
            .await
            .unwrap();
        let raw_era1 = fs::read("../test_assets/era1/mainnet-00000-5ec1ffb8.era1").unwrap();
        let processed_era = process_era1_file(raw_era1, 0).unwrap();
        for block in processed_era.blocks {
            trin_execution.process_next_block().await.unwrap();
            assert_eq!(trin_execution.get_root().unwrap(), block.header.state_root);
        }
        temp_directory.close().unwrap();
    }
}
