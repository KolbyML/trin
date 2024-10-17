use std::sync::Arc;

use anyhow::anyhow;
use tokio::{
    sync::{broadcast::Receiver, mpsc::UnboundedSender, Mutex},
    task::JoinHandle,
};
use tracing::error;

use crate::{storage::execution_position::ExecutionPositionV1, sync::era::manager::EraManager};

use super::event::BlockEvent;

#[derive(Clone)]
pub struct BlockService {
    era_manager: Arc<Mutex<EraManager>>,
}

impl BlockService {
    pub async fn new(execution_position: Arc<Mutex<ExecutionPositionV1>>) -> anyhow::Result<Self> {
        let next_block_number = execution_position.lock().await.next_block_number();
        let era_manager = Arc::new(Mutex::new(EraManager::new(next_block_number).await?));

        Ok(BlockService { era_manager })
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
                                let next_block = block_service.era_manager.lock().await.get_next_block().await.map_err(|err| {
                                    anyhow!("Block service: failed to get next block: {err:?}")
                                });

                                if let Err(err) = sender.send(next_block) {
                                    error!("Block service: failed to send next block: {:?}", err);
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
}
