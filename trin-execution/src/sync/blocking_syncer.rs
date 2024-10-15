use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use alloy_primitives::B256;
use anyhow::{bail, ensure};
use eth_trie::{RootWithTrieDiff, Trie};
use ethportal_api::{types::execution::transaction::Transaction, Header};
use revm::inspectors::TracerEip3155;
use tokio::sync::{broadcast, mpsc::UnboundedSender, Mutex};
use tracing::{info, warn};

use crate::{
    config::StateConfig,
    evm::block_executor::BlockExecutor,
    metrics::{start_timer_vec, stop_timer, BLOCK_PROCESSING_TIMES},
    storage::{evm_db::EvmDB, execution_position::ExecutionPositionV1},
    sync::era::types::SyncStatus,
};

use super::block::event::BlockEvent;

#[derive(Clone)]
pub struct BlockingSyncer {
    pub database: EvmDB,
    pub config: StateConfig,
    pub execution_position: Arc<Mutex<ExecutionPositionV1>>,
    pub block_requester: UnboundedSender<BlockEvent>,
    pub data_directory: PathBuf,
}

/// BlockingSyncer will panic if it is ran in an async runtime
impl BlockingSyncer {
    pub fn new(
        data_dir: &Path,
        block_requester: UnboundedSender<BlockEvent>,
        execution_position: Arc<Mutex<ExecutionPositionV1>>,
        database: EvmDB,
        config: StateConfig,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            execution_position,
            config,
            block_requester,
            database,
            data_directory: data_dir.to_path_buf(),
        })
    }

    pub fn next_block_number(&self) -> u64 {
        self.execution_position.blocking_lock().next_block_number()
    }

    pub fn process_next_block(&mut self) -> anyhow::Result<RootWithTrieDiff> {
        self.process_range_of_blocks(Some(self.next_block_number()), None)
    }

    /// Processes blocks up to last block number (inclusive) and returns the root with trie diff. If
    /// last block is None, we will process all blocks until we reach the latest block.
    ///
    /// If the state cache gets too big, we will commit the state and continue. Execution can be
    /// interrupted early by sending `stop_signal`, in which case we will commit and return.
    pub fn process_range_of_blocks(
        &mut self,
        last_block: Option<u64>,
        mut stop_signal: Option<broadcast::Receiver<()>>,
    ) -> anyhow::Result<RootWithTrieDiff> {
        let start_block = self.execution_position.blocking_lock().next_block_number();

        // Ensure that last block is greater than or equal to start block if specified.
        if last_block.is_some() {
            ensure!(
                last_block >= Some(start_block),
                "Last block number {last_block:?} is less than start block number {start_block}",
            );
        }

        info!("Processing blocks from {start_block} to {last_block:?} (inclusive)");

        let mut block_executor = BlockExecutor::new(self.database.clone());

        let mut block = match self.fetch_next_block()? {
            SyncStatus::Syncing(processed_block) => processed_block,
            SyncStatus::Finished => bail!("No more blocks to process"),
        };

        loop {
            block_executor
                .execute_block_with_tracer(&block, |tx| self.create_tracer(&block.header, tx))?;

            // Commit and return if we reached last block or stop signal is received.
            let stop_signal_received = stop_signal
                .as_mut()
                .is_some_and(|stop_signal| stop_signal.try_recv().is_ok());

            // Fetch next block
            let next_block = self.fetch_next_block()?;

            if Some(block.header.number) == last_block
                || stop_signal_received
                || next_block == SyncStatus::Finished
            {
                info!("Stop signal received or syncer reached last block. Committing now, please wait!");

                let result = self.commit(&block.header, block_executor)?;

                info!(
                    "Commit done. Stopping execution. Last block executed number: {} state root: {}",
                    block.header.number, block.header.state_root
                );

                return Ok(result);
            }

            block = match next_block {
                SyncStatus::Syncing(processed_block) => processed_block,
                SyncStatus::Finished => panic!("We checked that SyncStatus is not Finished above, so if this panics it's a bug"),
            };

            // Commit early if we have reached the limits, to prevent too much memory usage.
            // We won't use this during the dos attack to avoid writing empty accounts to disk
            if block_executor.should_commit()
                && !(2_200_000..2_700_000).contains(&block.header.number)
            {
                self.commit(&block.header, block_executor)?;
                block_executor = BlockExecutor::new(self.database.clone());
            }
        }
    }

    fn fetch_next_block(&self) -> anyhow::Result<SyncStatus> {
        let fetching_block_timer =
            start_timer_vec(&BLOCK_PROCESSING_TIMES, &["fetching_block_from_era"]);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.block_requester.send(BlockEvent::FetchNextBlock(tx))?;
        let block = rx.blocking_recv()?;
        stop_timer(fetching_block_timer);
        Ok(block)
    }

    pub fn get_root(&mut self) -> anyhow::Result<B256> {
        Ok(self.database.trie.lock().root_hash()?)
    }

    fn commit(
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
            .blocking_lock()
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
