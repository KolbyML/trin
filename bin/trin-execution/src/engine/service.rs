use std::{path::Path, sync::Arc};

use alloy_rpc_types_engine::{
    ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadStatus, PayloadStatusEnum,
};
use ethportal_api::types::network::Subnetwork;
use portal_bridge::{
    census::Census,
    cli::{BridgeConfig, ClientType},
};
use revm_primitives::B256;
use rocksdb::DB as RocksDB;
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot::Sender,
    Mutex,
};
use tracing::{error, info};

use super::{command::NewPayloadMessage, thread_manager::ThreadManager};
use crate::{
    blockchain::Blockchain,
    bridge::{rpc::RpcHeaderOracle, state::StateBridge},
    cli::TrinExecutionConfig,
    engine::command::EngineCommand,
    networking::trin::run_trin,
    storage::{execution_position::ExecutionPositionV2, state::evm_db::EvmDB},
    sync::{block::service::BlockService, blocking_syncer::BlockingSyncer, service::SyncService},
};

pub struct EngineService {
    command_rx: UnboundedReceiver<EngineCommand>,
    latest_canonical_header_hash: Option<B256>,
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
    _sync_service: Option<SyncService>,
    db: Arc<RocksDB>,
    _blockchain: Blockchain,
    shutdown_signal: broadcast::Receiver<()>,
}

impl EngineService {
    pub async fn spawn(
        data_dir: &Path,
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        db: Arc<RocksDB>,
        evm_db: EvmDB,
        trin_execution_config: TrinExecutionConfig,
        thread_manager: &mut ThreadManager,
    ) -> UnboundedSender<EngineCommand> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let sync_service =
            if let Some(beacon_api_endpoint) = trin_execution_config.beacon_api_endpoint.clone() {
                let bridge_channel = if trin_execution_config.bridge_diffs {
                    // Start Trin for header oracle
                    let header_oracle = run_trin(data_dir.to_path_buf())
                        .await
                        .expect("Failed to create header oracle");

                    let mut bridge_config = BridgeConfig::default();
                    bridge_config.filter_clients = vec![ClientType::Ultralight];
                    bridge_config.bridge_id = trin_execution_config.bridge_id;
                    bridge_config.enr_offer_limit = 8;
                    let mut census = Census::new(
                        Arc::new(RpcHeaderOracle::new(header_oracle.clone())),
                        &bridge_config,
                    );
                    let census_handle = census
                        .init([Subnetwork::State])
                        .await
                        .expect("Failed to init census");

                    let state_bridge = StateBridge::new(
                        header_oracle,
                        bridge_config.offer_limit,
                        census,
                        bridge_config.bridge_id,
                        evm_db.clone(),
                    )
                    .await
                    .expect("Failed to create state bridge");

                    let (command_tx, thread_handle) = state_bridge
                        .launch(thread_manager.shutdown_signal_2_receiver(), census_handle)
                        .await
                        .expect("Failed to launch state bridge");
                    thread_manager.append_tokio_handle_2(thread_handle);
                    Some(command_tx)
                } else {
                    None
                };

                let block_service = BlockService::new(
                    execution_position.clone(),
                    beacon_api_endpoint,
                    trin_execution_config.execution_delay,
                )
                .await
                .expect("Failed to create block service");

                let (join_handle, block_requester) = block_service
                    .spawn(thread_manager.shutdown_signal_2_receiver())
                    .await
                    .expect("Failed to spawn block service");
                thread_manager.append_tokio_handle_2(join_handle);

                let blocking_syncer = BlockingSyncer::new(
                    data_dir,
                    block_requester,
                    execution_position.clone(),
                    evm_db.clone(),
                    evm_db.config.clone(),
                    Arc::new(Mutex::new(thread_manager.shutdown_signal_1_receiver())),
                    bridge_channel,
                )
                .expect("Failed to create blocking syncer");

                let sync_service = SyncService::new(blocking_syncer);
                info!("Starting sync service");
                let join_handle =
                    sync_service.spawn_background_syncer(trin_execution_config.debug_last_block);
                thread_manager.append_std_handle_1(join_handle);
                Some(sync_service)
            } else {
                None
            };

        let shutdown_signal = thread_manager.shutdown_signal_2_receiver();
        let join_handle = tokio::spawn(async move {
            let engine_service = EngineService {
                command_rx,
                _blockchain: Blockchain::new(
                    evm_db,
                    execution_position.clone(),
                    sync_service.clone(),
                ),
                execution_position,
                latest_canonical_header_hash: None,
                db,
                _sync_service: sync_service,
                shutdown_signal,
            };

            engine_service.start().await;
            Ok(())
        });
        thread_manager.append_tokio_handle_2(join_handle);

        command_tx
    }

    async fn start(mut self) {
        loop {
            tokio::select! {
                _ = self.shutdown_signal.recv() => {
                    break;
                }
                Some(command) = self.command_rx.recv() => {
                    match command {
                        EngineCommand::NewPayloadV1(new_payload_message) => self.handle_new_payload(new_payload_message).await,
                        EngineCommand::ForkChoice((fork_choice_state,payload_attributes,response_tx)) => self.handle_fork_choice(fork_choice_state,payload_attributes,response_tx).await,
                        EngineCommand::NewPayloadV2(new_payload_message) => todo!(),
                        EngineCommand::NewPayloadV3(new_payload_message) => todo!(),
                        EngineCommand::GetPayloadV1(new_payload_message) => todo!(),
                        EngineCommand::GetPayloadV2(new_payload_message) => todo!(),
                        EngineCommand::GetPayloadV3(new_payload_message) => todo!(),
                    }
                }
            }
        }
    }

    pub async fn handle_new_payload(&self, new_payload_message: NewPayloadMessage) {
        // todo: handle if data to validate payload is present.

        let NewPayloadMessage {
            sender: response_tx,
            ..
        } = new_payload_message;

        if let Err(err) =
            response_tx.send(Ok(PayloadStatus::from_status(PayloadStatusEnum::Syncing)))
        {
            error!("Failed to send response to new payload: {err:?}");
        }
    }

    pub async fn handle_fork_choice(
        &mut self,
        fork_choice_state: ForkchoiceState,
        _payload_attributes: Option<PayloadAttributes>,
        response_tx: Sender<anyhow::Result<ForkchoiceUpdated>>,
    ) {
        // update execution position with the new fork choice state.
        if let Err(err) = self
            .execution_position
            .lock()
            .await
            .update_fork_choice_state(self.db.clone(), fork_choice_state)
        {
            error!("Failed to update execution position with new fork choice state: {err:?}");
        };

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
