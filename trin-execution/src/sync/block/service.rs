use std::sync::Arc;

use tokio::sync::{broadcast::Receiver, mpsc::UnboundedSender, Mutex};
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
    ) -> anyhow::Result<UnboundedSender<BlockEvent>> {
        let (send_channel, mut receive_channel) = tokio::sync::mpsc::unbounded_channel();

        let block_service = self.clone();
        let mut shutdown_signal = shutdown_signal;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_signal.recv() => {
                        break;
                    }
                    Some(event) = receive_channel.recv() => {
                        match event {
                            BlockEvent::FetchNextBlock(sender) => {
                                match block_service.era_manager.lock().await.get_next_block().await {
                                    Ok(next_block) => {
                                        if let Err(err) = sender.send(next_block) {
                                            error!("Block service: failed to send next block: {:?}", err);
                                        }
                                    },
                                    Err(err) =>  {
                                        error!("Block service: failed to get next block: {err:?}" );
                                    }
                                }
                            },
                        }
                    }
                }
            }
        });
        Ok(send_channel)
    }
}
