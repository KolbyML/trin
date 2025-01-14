use std::{
    path::{Path, PathBuf},
    sync::Arc,
    thread::sleep,
    time::Duration,
};

use alloy::primitives::B256;
use anyhow::{bail, ensure};
use eth_trie::{RootWithTrieDiff, Trie};
use ethportal_api::{types::execution::transaction::Transaction, Header};
use revm::inspectors::TracerEip3155;
use tokio::sync::{broadcast, mpsc::UnboundedSender, oneshot::error::TryRecvError, Mutex};
use tracing::{info, warn};

use super::block::event::BlockEvent;
use crate::{
    bridge::channel::{StateBridgeMessage, StateDiffBundle},
    config::StateConfig,
    evm::block_executor::BlockExecutor,
    metrics::{start_timer_vec, stop_timer, BLOCK_PROCESSING_TIMES},
    storage::{execution_position::ExecutionPositionV2, state::evm_db::EvmDB},
    sync::era::types::SyncStatus,
};

#[derive(Debug, Clone)]
pub struct BlockingSyncer {
    pub database: EvmDB,
    pub config: StateConfig,
    pub execution_position: Arc<Mutex<ExecutionPositionV2>>,
    pub block_requester: UnboundedSender<BlockEvent>,
    pub data_directory: PathBuf,
    stop_signal: Arc<Mutex<broadcast::Receiver<()>>>,
    bridge_channel: Option<UnboundedSender<StateBridgeMessage>>,
}

/// BlockingSyncer will panic if it is ran in an async runtime
impl BlockingSyncer {
    pub fn new(
        data_dir: &Path,
        block_requester: UnboundedSender<BlockEvent>,
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        database: EvmDB,
        config: StateConfig,
        stop_signal: Arc<Mutex<broadcast::Receiver<()>>>,
        bridge_channel: Option<UnboundedSender<StateBridgeMessage>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            execution_position,
            config,
            block_requester,
            database,
            data_directory: data_dir.to_path_buf(),
            stop_signal,
            bridge_channel,
        })
    }

    pub fn next_block_number(&self) -> u64 {
        self.execution_position.blocking_lock().next_block_number()
    }

    /// Syncs blocks until the block service says it is done
    pub fn sync_blocks(
        &mut self,
        debug_last_block: Option<u64>,
    ) -> anyhow::Result<RootWithTrieDiff> {
        let start_block = self.execution_position.blocking_lock().next_block_number();

        info!("Starting sync from block {start_block}");

        if debug_last_block.is_some() {
            info!("Debug option is set to sync until block {debug_last_block:?}");
        }

        let mut block_executor =
            BlockExecutor::new(self.database.clone(), false, self.config.save_blocks);

        let mut block = match self.fetch_next_block()? {
            SyncStatus::Block(processed_block) => processed_block,
            SyncStatus::Finished => bail!("No more blocks to process"),
            SyncStatus::ConsensusClientIsSyncing => {
                unreachable!("This case is handled in fetch_next_block")
            }
        };

        loop {
            block_executor
                .execute_block_with_tracer(&block, |tx| self.create_tracer(&block.header, tx))?;

            // Commit and return if we reached last block or stop signal is received.
            let stop_signal_received = self.stop_signal.blocking_lock().try_recv().is_ok();

            // Fetch next block
            let next_block = self.fetch_next_block()?;

            if Some(block.header.number) == debug_last_block
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

            // Commit early if we have reached the limits, to prevent too much memory usage.
            // We won't use this during the dos attack to avoid writing empty accounts to disk
            //
            // If we have a state bridge channel, we commit every block so we can gossip the state
            // bridge
            if (block_executor.should_commit()
                && !(2_200_000..2_700_000).contains(&block.header.number))
                || self.bridge_channel.is_some()
            {
                let root_with_trie_diff = self.commit(&block.header, block_executor)?;

                // If we have a bridge channel, send the root with diff to the bridge
                if let Some(bridge_channel) = &self.bridge_channel {
                    let (sender, mut rx) = tokio::sync::oneshot::channel();

                    let root_with_trie_diff_cloned = RootWithTrieDiff {
                        root: root_with_trie_diff.root.clone(),
                        trie_diff: root_with_trie_diff.trie_diff.clone(),
                    };

                    bridge_channel.send(StateBridgeMessage {
                        state_diff_bundle: StateDiffBundle {
                            root_with_trie_diff: root_with_trie_diff,
                            block_hash: block.header.hash(),
                        },
                        sender,
                    })?;

                    // loop to check if we have a stop signal
                    loop {
                        match rx.try_recv() {
                            Ok(response) => {
                                if let Err(err) = response {
                                    warn!("Waiting for state bridging failed: {err}");
                                }
                                break;
                            }
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Closed) => {
                                bail!("State bridge channel closed");
                            }
                        }

                        // loop to check if we have a stop signal
                        let stop_signal_received =
                            self.stop_signal.blocking_lock().try_recv().is_ok();
                        if stop_signal_received {
                            info!("Stop signal received. Stopping execution.");
                            return Ok(root_with_trie_diff_cloned);
                        }
                        sleep(Duration::from_millis(100));
                    }
                }

                block_executor =
                    BlockExecutor::new(self.database.clone(), false, self.config.save_blocks);
            }

            block = match next_block {
                SyncStatus::Block(processed_block) => processed_block,
                SyncStatus::Finished => panic!("We checked that SyncStatus is not Finished above, so if this panics it's a bug"),
                SyncStatus::ConsensusClientIsSyncing => unreachable!("This case is handled in fetch_next_block")
            };
        }
    }

    fn fetch_next_block(&self) -> anyhow::Result<SyncStatus> {
        let fetching_block_timer =
            start_timer_vec(&BLOCK_PROCESSING_TIMES, &["fetching_block_from_era"]);
        let block = loop {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.block_requester.send(BlockEvent::FetchNextBlock(tx))?;
            let block = match rx.blocking_recv()? {
                Ok(block) => block,
                Err(err) => {
                    warn!("Failed to fetch next block: {err}. Exiting sync.");
                    break SyncStatus::Finished;
                }
            };

            match block {
                SyncStatus::Block(_) => break block,
                SyncStatus::Finished => break block,
                SyncStatus::ConsensusClientIsSyncing => {
                    let stop_signal_received = self.stop_signal.blocking_lock().try_recv().is_ok();
                    if stop_signal_received {
                        break SyncStatus::Finished;
                    }
                    warn!("Consensus client is still syncing. Waiting for it to finish before continuing to sync.");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        };
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
