use alloy_rpc_types_engine::{
    ForkchoiceState, ForkchoiceUpdated, PayloadStatus, PayloadStatusEnum,
};
use revm_primitives::B256;
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot::Sender,
};
use tracing::error;

use crate::{
    blockchain_tree::BlockchainTree, engine::command::EngineCommand, era::types::ProcessedBlock,
    syncer::Syncer,
};

pub struct EngineService {
    command_rx: UnboundedReceiver<EngineCommand>,
    latest_canonical_header_hash: Option<B256>,
    // trin_execution: TrinExecution,
    syncer: Syncer,
    _blockchain_tree: BlockchainTree,
}

impl EngineService {
    pub async fn spawn(syncer: Syncer) -> UnboundedSender<EngineCommand> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let engine_service = EngineService {
                command_rx,
                _blockchain_tree: BlockchainTree {},
                latest_canonical_header_hash: None,
                syncer,
            };

            engine_service.start().await;
        });

        command_tx
    }

    async fn start(mut self) {
        loop {
            tokio::select! {
                Some(command) = self.command_rx.recv() => {
                    match command {
                        EngineCommand::NewPayload((processed_block, response_tx)) => self.handle_new_payload(processed_block, response_tx).await,
                        EngineCommand::ForkChoice((fork_choice_state, response_tx)) =>  self.handle_fork_choice(fork_choice_state, response_tx).await,
                    }
                }
            }
        }
    }

    pub async fn handle_new_payload(
        &self,
        _processed_block: ProcessedBlock,
        response_tx: Sender<anyhow::Result<PayloadStatus>>,
    ) {
        // todo: handle if data to validate payload is present.

        if let Err(err) =
            response_tx.send(Ok(PayloadStatus::from_status(PayloadStatusEnum::Syncing)))
        {
            error!("Failed to send response to new payload: {err:?}");
        }
    }

    pub async fn handle_fork_choice(
        &mut self,
        fork_choice_state: ForkchoiceState,
        response_tx: Sender<anyhow::Result<ForkchoiceUpdated>>,
    ) {
        let ForkchoiceState {
            head_block_hash,
            safe_block_hash,
            finalized_block_hash,
        } = fork_choice_state;

        self.latest_canonical_header_hash = Some(head_block_hash);

        if let Err(err) = response_tx.send(Ok(ForkchoiceUpdated::from_status(
            PayloadStatusEnum::Syncing,
        ))) {
            error!("Failed to send response to fork choice: {err:?}");
        }
    }
}
