use std::sync::Arc;

use anyhow::anyhow;
use tokio::{
    sync::{broadcast::Receiver, mpsc::UnboundedSender, Mutex},
    task::JoinHandle,
};
use tracing::error;
use url::Url;

use crate::{
    storage::execution_position::ExecutionPositionV2,
    sync::{
        consensus_block_fetcher::ConsensusBlockFetcher,
        era::{
            execution_payload::ProcessExecutionPayload,
            manager::EraManager,
            types::{ProcessedBlock, SyncStatus},
        },
    },
};

use super::event::BlockEvent;

/// Block service is responsible for managing the block fetching
/// For syncing we must fetch blocks from different sources initally we will fetch blocks from
/// Era1/Era files using EraManager We can sync up to the HEAD minus ~8000-50000 blocks, then we
/// must switch to fetching blocks from the consensus layer client
#[derive(Clone)]
pub struct BlockService {
    era_manager: Arc<Mutex<EraManager>>,
    consensus_block_fetcher: Arc<Mutex<ConsensusBlockFetcher>>,
    next_slot_to_check: Arc<Mutex<Option<u64>>>,
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
}

impl BlockService {
    pub async fn new(
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        beacon_api_endpoint: Url,
    ) -> anyhow::Result<Self> {
        let next_block_number = execution_position.lock().await.next_block_number();
        let era_manager = Arc::new(Mutex::new(EraManager::new(next_block_number).await?));
        let consensus_block_fetcher =
            Arc::new(Mutex::new(ConsensusBlockFetcher::new(beacon_api_endpoint)));

        Ok(BlockService {
            era_manager,
            consensus_block_fetcher,
            next_slot_to_check: Arc::new(Mutex::new(None)),
            execution_position,
        })
    }

    pub async fn spawn(
        &self,
        shutdown_signal: Receiver<()>,
    ) -> anyhow::Result<(JoinHandle<anyhow::Result<()>>, UnboundedSender<BlockEvent>)> {
        let (send_channel, mut receive_channel) = tokio::sync::mpsc::unbounded_channel();

        let block_service = self.clone();
        let mut shutdown_signal = shutdown_signal;
        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_signal.recv() => {
                        return Ok(());
                    }
                    Some(event) = receive_channel.recv() => {
                        match event {
                            BlockEvent::FetchNextBlock(sender) => {
                                let last_next_slot = *block_service.next_slot_to_check.lock().await;
                                match block_service.get_next_block().await {
                                    Ok(block) => {
                                        if let Err(err) = sender.send(Ok(block)) {
                                            error!("Block service: failed to send beacon block: {:?}", err);
                                            *block_service.next_slot_to_check.lock().await = last_next_slot;

                                        }
                                    },
                                    Err(err) => {
                                        if let Err(err) = sender.send(Err(err)) {
                                            error!("Block service: failed to send error: {:?}", err);
                                        }
                                        *block_service.next_slot_to_check.lock().await = last_next_slot;
                                    }
                                }
                            },
                            BlockEvent::LatestBlockNumber(sender) => {
                                let latest_block_number = block_service.era_manager.lock().await.last_available_block_number().await.map_err(|err| {
                                    anyhow!("Block service: failed to get latest block: {err:?}" )
                                });
                                if let Err(err) = sender.send(latest_block_number) {
                                    error!("Block service: failed to send latest block number: {:?}", err);
                                }
                            }
                        }
                    }
                }
            }
        });
        Ok((join_handle, send_channel))
    }

    async fn fetch_next_block_from_beacon_endpoint(
        &self,
        next_slot_to_check: u64,
    ) -> anyhow::Result<(ProcessedBlock, u64)> {
        let mut next_slot_to_check = next_slot_to_check;
        let beacon_block = loop {
            if let Some(beacon_block) = self
                .consensus_block_fetcher
                .lock()
                .await
                .get_block_by_slot(next_slot_to_check)
                .await
                .map_err(|err| anyhow!("Block service: failed to get block by slot: {err:?}"))?
            {
                break beacon_block; // Exit the loop when Some is returned
            }
            next_slot_to_check += 1;
        };

        let processed_block = beacon_block.process_execution_payload()?;
        Ok((processed_block, next_slot_to_check + 1))
    }

    async fn get_next_block(&self) -> anyhow::Result<SyncStatus> {
        let is_era_manager_out_of_blocks = self
            .era_manager
            .lock()
            .await
            .is_era_manager_out_of_blocks()
            .await?;

        // If era manager runs out of blocks we switch to fetching blocks from the consensus layer client
        match is_era_manager_out_of_blocks {
            false => self
                .era_manager
                .lock()
                .await
                .get_next_block()
                .await
                .map_err(|err| anyhow!("Block service: failed to get next block: {err:?}")),
            true => {
                let next_slot_to_check = *self.next_slot_to_check.lock().await;
                let next_slot = match next_slot_to_check {
                    Some(slot) => slot,
                    None => {
                        self.era_manager
                            .lock()
                            .await
                            .last_available_slot_number()
                            .await?
                    }
                };

                let (beacon_block, next_slot) = self.fetch_next_block_from_beacon_endpoint(next_slot).await.map_err(|err| {
                    anyhow!("Block service: failed to fetch next block from beacon endpoint: {err:?}")
                })?;
                *self.next_slot_to_check.lock().await = Some(next_slot);

                // Check if the block is the finalized block, if it is the block is finished syncing
                // and the `Blockchain` struct will keep up with the chain
                if beacon_block.header.hash()
                    == self.execution_position.lock().await.finalized_block_hash()
                {
                    return Ok(SyncStatus::Finished);
                }

                Ok(SyncStatus::Syncing(beacon_block))
            }
        }
    }
}
