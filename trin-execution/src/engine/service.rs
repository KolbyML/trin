use std::{path::Path, sync::Arc};

use alloy_rpc_types::engine::{
    ForkchoiceState, ForkchoiceUpdated, PayloadStatus, PayloadStatusEnum,
};
use revm_primitives::B256;
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot::Sender,
    Mutex,
};
use tracing::error;

use crate::{
    blockchain::Blockchain,
    engine::command::EngineCommand,
    storage::{evm_db::EvmDB, execution_position::ExecutionPositionV1},
    sync::{
        block::service::BlockService, era::types::ProcessedBlock, service::SyncService,
        syncer2::BlockingSyncer,
    },
};

pub struct EngineService {
    command_rx: UnboundedReceiver<EngineCommand>,
    latest_canonical_header_hash: Option<B256>,
    // trin_execution: TrinExecution,
    _sync_service: SyncService,
    _blockchain: Blockchain,
}

impl EngineService {
    pub async fn spawn(
        data_dir: &Path,
        execution_position: Arc<Mutex<ExecutionPositionV1>>,
        evm_db: EvmDB,
        shutdown_signal_sender: broadcast::Sender<()>,
    ) -> UnboundedSender<EngineCommand> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let block_service = BlockService::new(execution_position.clone())
            .await
            .expect("Failed to create block service");

        let block_requester = block_service
            .spawn(shutdown_signal_sender.subscribe())
            .await
            .expect("Failed to spawn block service");

        let blocking_syncer = BlockingSyncer::new(
            data_dir,
            block_requester,
            execution_position.clone(),
            evm_db.clone(),
            evm_db.config.clone(),
        )
        .expect("Failed to create blocking syncer");
        let sync_service = SyncService::new(blocking_syncer);
        let (_, sync_service_shutdown_sender) = sync_service.spawn_background_syncer();

        let mut shutdown_signal_receiver = shutdown_signal_sender.subscribe();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_signal_receiver.recv() => {
                    if let Err(err) = sync_service_shutdown_sender.send(()) {
                        error!("Failed to send shutdown signal to blocking syncer: {err:?}");
                    }
                }
            }
        });

        tokio::spawn(async move {
            let engine_service = EngineService {
                command_rx,
                _blockchain: Blockchain {},
                latest_canonical_header_hash: None,
                _sync_service: sync_service,
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
            head_block_hash, ..
        } = fork_choice_state;

        self.latest_canonical_header_hash = Some(head_block_hash);

        if let Err(err) = response_tx.send(Ok(ForkchoiceUpdated::from_status(
            PayloadStatusEnum::Syncing,
        ))) {
            error!("Failed to send response to fork choice: {err:?}");
        }
    }
}
