use std::sync::Arc;

use alloy::{
    primitives::bytes::Bytes,
    rpc::types::{
        engine::{
            ExecutionPayloadBodiesV1, ExecutionPayloadBodiesV2, ExecutionPayloadInputV2,
            ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, ExecutionPayloadV4,
            ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId, PayloadStatus,
        },
        Block, BlockId, Filter, Log, SyncInfo, SyncStatus, TransactionRequest,
    },
};
use alloy_rpc_types_engine::{ClientCode, ClientVersionV1, PayloadStatusEnum};
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc, types::ErrorObject};
use parking_lot::Mutex as ParkingMutex;
use revm_primitives::{Address, B256, U256};
use tokio::sync::{mpsc::UnboundedSender, oneshot, Mutex};
use tracing::info;

use super::{command::EngineCommand, error::EngineApiError};
use crate::{
    storage::execution_position::ExecutionPositionV2,
    sync::era::execution_payload::{
        ExecutionPayloadV3WithBeaconBlockHash, ProcessExecutionPayload,
    },
};

/// Engine Api JSON-RPC endpoints
#[rpc(client, server, namespace = "engine")]
pub trait EngineApi {
    #[method(name = "exchangeCapabilities")]
    async fn exchange_capabilities(
        &self,
        supported_capabilities: Vec<String>,
    ) -> RpcResult<Vec<String>>;

    #[method(name = "getClientVersionV1")]
    async fn get_client_version_v1(
        &self,
        client_version: ClientVersionV1,
    ) -> RpcResult<Vec<ClientVersionV1>>;

    #[method(name = "forkchoiceUpdatedV1")]
    async fn fork_choice_updated_v1(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated>;

    #[method(name = "forkchoiceUpdatedV2")]
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated>;

    #[method(name = "forkchoiceUpdatedV3")]
    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated>;

    #[method(name = "getPayloadBodiesByHashV1")]
    async fn get_payload_bodies_by_hash_v1(
        &self,
        block_hashes: Vec<B256>,
    ) -> RpcResult<ExecutionPayloadBodiesV1>;

    #[method(name = "getPayloadBodiesByHashV2")]
    async fn get_payload_bodies_by_hash_v2(
        &self,
        block_hashes: Vec<B256>,
    ) -> RpcResult<ExecutionPayloadBodiesV2>;

    #[method(name = "getPayloadBodiesByRangeV1")]
    async fn get_payload_bodies_by_range_v1(
        &self,
        start: u64,
        count: u64,
    ) -> RpcResult<ExecutionPayloadBodiesV1>;

    #[method(name = "getPayloadBodiesByRangeV2")]
    async fn get_payload_bodies_by_range_v2(
        &self,
        start: u64,
        count: u64,
    ) -> RpcResult<ExecutionPayloadBodiesV2>;

    #[method(name = "getPayloadV1")]
    async fn get_payload_v1(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadV1>;

    #[method(name = "getPayloadV2")]
    async fn get_payload_v2(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadV2>;

    #[method(name = "getPayloadV3")]
    async fn get_payload_v3(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadV3>;

    #[method(name = "getPayloadV4")]
    async fn get_payload_v4(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadV4>;

    #[method(name = "newPayloadV1")]
    async fn new_payload_v1(&self, payload: ExecutionPayloadV1) -> RpcResult<PayloadStatus>;

    #[method(name = "newPayloadV2")]
    async fn new_payload_v2(&self, payload: ExecutionPayloadInputV2) -> RpcResult<PayloadStatus>;

    #[method(name = "newPayloadV3")]
    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> RpcResult<PayloadStatus>;

    #[method(name = "newPayloadV4")]
    async fn new_payload_v4(
        &self,
        payload: ExecutionPayloadV4,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> RpcResult<PayloadStatus>;
}

/// A subset of Eth JSON-RPC endpoints under the Engine Api's JWT authentication
#[rpc(client, server, namespace = "eth")]
pub trait EngineEthApi {
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<String>;

    #[method(name = "call")]
    async fn call(&self, transaction: TransactionRequest, block: BlockId) -> RpcResult<Bytes>;

    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<U256>;

    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        block_hash: B256,
        hydrated_transactions: bool,
    ) -> RpcResult<Block>;

    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        block_number: u64,
        hydrated_transactions: bool,
    ) -> RpcResult<Block>;

    #[method(name = "getCode")]
    async fn get_code(&self, address: Address, block: BlockId) -> RpcResult<Bytes>;

    #[method(name = "getLogs")]
    async fn get_logs(&self, filter: Filter) -> RpcResult<Vec<Log>>;

    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<B256>;

    #[method(name = "syncing")]
    async fn syncing(&self) -> RpcResult<SyncStatus>;
}

const CAPABILITIES: [&str; 16] = [
    "engine_getClientVersionV1",
    "engine_forkchoiceUpdatedV1",
    "engine_forkchoiceUpdatedV2",
    "engine_forkchoiceUpdatedV3",
    "engine_getPayloadBodiesByHashV1",
    "engine_getPayloadBodiesByHashV2",
    "engine_getPayloadBodiesByRangeV1",
    "engine_getPayloadBodiesByRangeV2",
    "engine_getPayloadV1",
    "engine_getPayloadV2",
    "engine_getPayloadV3",
    "engine_getPayloadV4",
    "engine_newPayloadV1",
    "engine_newPayloadV2",
    "engine_newPayloadV3",
    "engine_newPayloadV4",
];

pub struct EngineRPCServer {
    consensus_capabilities: Arc<ParkingMutex<Vec<String>>>,
    engine_tx: UnboundedSender<EngineCommand>,
}

#[async_trait]
impl EngineApiServer for EngineRPCServer {
    async fn exchange_capabilities(
        &self,
        supported_capabilities: Vec<String>,
    ) -> RpcResult<Vec<String>> {
        info!("Received capabilities: {:?}", supported_capabilities);
        *self.consensus_capabilities.lock() = supported_capabilities;
        let capabilities: Vec<String> = CAPABILITIES
            .into_iter()
            .map(|method| method.to_string())
            .collect();
        Ok(capabilities)
    }

    async fn get_client_version_v1(
        &self,
        _client_version: ClientVersionV1,
    ) -> RpcResult<Vec<ClientVersionV1>> {
        info!("Received client version request {:?}", _client_version);
        Ok(vec![ClientVersionV1 {
            code: ClientCode::TE,
            name: "Trin Execution".to_string(),
            version: "todo!".to_string(),
            commit: "todo!".to_string(),
        }])
    }

    async fn fork_choice_updated_v1(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        info!(
            "Received fork choice update 1  request {:?}",
            fork_choice_state
        );

        let (command_tx, command_rx) = oneshot::channel();

        let command =
            EngineCommand::ForkChoice((fork_choice_state, payload_attributes, command_tx));

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive ForkchoiceUpdated result: {err:?}"
            ))
        })?;

        let result = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        Ok(result)
    }

    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        info!(
            "Received fork choice update 2  request {:?}",
            fork_choice_state
        );

        let (command_tx, command_rx) = oneshot::channel();

        let command =
            EngineCommand::ForkChoice((fork_choice_state, payload_attributes, command_tx));

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive ForkchoiceUpdated result: {err:?}"
            ))
        })?;

        let result = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        Ok(result)
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        info!(
            "Received fork choice update 3 request {:?}",
            fork_choice_state
        );

        let (command_tx, command_rx) = oneshot::channel();

        let command =
            EngineCommand::ForkChoice((fork_choice_state, payload_attributes, command_tx));

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive ForkchoiceUpdated result: {err:?}"
            ))
        })?;

        let result = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::ForkChoice: {err:?}"
            ))
        })?;

        Ok(result)
    }

    async fn get_payload_bodies_by_hash_v1(
        &self,
        _block_hashes: Vec<B256>,
    ) -> RpcResult<ExecutionPayloadBodiesV1> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_bodies_by_hash_v2(
        &self,
        _block_hashes: Vec<B256>,
    ) -> RpcResult<ExecutionPayloadBodiesV2> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_bodies_by_range_v1(
        &self,
        _start: u64,
        _count: u64,
    ) -> RpcResult<ExecutionPayloadBodiesV1> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_bodies_by_range_v2(
        &self,
        _start: u64,
        _count: u64,
    ) -> RpcResult<ExecutionPayloadBodiesV2> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_v1(&self, _payload_id: PayloadId) -> RpcResult<ExecutionPayloadV1> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_v2(&self, _payload_id: PayloadId) -> RpcResult<ExecutionPayloadV2> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_v3(&self, _payload_id: PayloadId) -> RpcResult<ExecutionPayloadV3> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn get_payload_v4(&self, _payload_id: PayloadId) -> RpcResult<ExecutionPayloadV4> {
        Err(EngineApiError::ServerError(
            "Trin Execution doesn't support block building for the time being.".to_string(),
        )
        .into())
    }

    async fn new_payload_v1(&self, payload: ExecutionPayloadV1) -> RpcResult<PayloadStatus> {
        info!("Received new payload 1 request {:?}", payload);
        let block_hash = payload.block_hash;
        let processed_block = payload.process_execution_payload().map_err(|err| {
            EngineApiError::ServerError(format!("Failed to process execution payload: {err:?}"))
        })?;

        if block_hash != processed_block.header.hash() {
            return Err(EngineApiError::ServerError(format!(
                "Block hash mismatch: expected {block_hash} but got {}",
                processed_block.header.hash()
            ))
            .into());
        }

        let (command_tx, command_rx) = oneshot::channel();

        let command = EngineCommand::new_payload(processed_block, None, command_tx);

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive PayloadStatus result:
        {err:?}"
            ))
        })?;

        let payload_status = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        Ok(payload_status)
    }

    async fn new_payload_v2(&self, payload: ExecutionPayloadInputV2) -> RpcResult<PayloadStatus> {
        info!("Received new payload 2 request {:?}", payload);
        let block_hash = payload.execution_payload.block_hash;
        let processed_block = payload.process_execution_payload().map_err(|err| {
            EngineApiError::ServerError(format!("Failed to process execution payload: {err:?}"))
        })?;

        if block_hash != processed_block.header.hash() {
            return Err(EngineApiError::ServerError(format!(
                "Block hash mismatch: expected {block_hash} but got {}",
                processed_block.header.hash()
            ))
            .into());
        }

        let (command_tx, command_rx) = oneshot::channel();

        let command = EngineCommand::new_payload(processed_block, None, command_tx);

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive PayloadStatus result:
        {err:?}"
            ))
        })?;

        let payload_status = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        Ok(payload_status)
    }

    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> RpcResult<PayloadStatus> {
        info!("Received new payload 3 request");

        return Ok(PayloadStatus::from_status(PayloadStatusEnum::Syncing));

        let block_hash = payload.payload_inner.payload_inner.block_hash;

        let processed_block =
            ExecutionPayloadV3WithBeaconBlockHash::new(payload, parent_beacon_block_root)
                .process_execution_payload()
                .map_err(|err| {
                    EngineApiError::ServerError(format!(
                        "Failed to process execution payload: {err:?}"
                    ))
                })?;

        if block_hash != processed_block.header.hash() {
            return Err(EngineApiError::ServerError(format!(
                "Block hash mismatch: expected {block_hash} but got {}",
                processed_block.header.hash()
            ))
            .into());
        }

        let (command_tx, command_rx) = oneshot::channel();

        let command =
            EngineCommand::new_payload(processed_block, Some(versioned_hashes), command_tx);

        self.engine_tx.send(command).map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to send EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        let result = command_rx.await.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to receive PayloadStatus result:
        {err:?}"
            ))
        })?;

        let payload_status = result.map_err(|err| {
            EngineApiError::ServerError(format!(
                "Failed to process EngineCommand::NewPayload: {err:?}"
            ))
        })?;

        Ok(payload_status)
    }

    async fn new_payload_v4(
        &self,
        _payload: ExecutionPayloadV4,
        _versioned_hashes: Vec<B256>,
        _parent_beacon_block_root: B256,
    ) -> RpcResult<PayloadStatus> {
        info!("Received new payload 4 request {:?}", _payload);
        return Ok(PayloadStatus::from_status(PayloadStatusEnum::Syncing));
        // Err(EngineApiError::ServerError(
        //     "Trin Execution doesn't support Pectra for the time being".to_string(),
        // )
        // .into())
    }
}

impl EngineRPCServer {
    pub fn new(engine_tx: UnboundedSender<EngineCommand>) -> Self {
        let consensus_capabilities = Arc::new(ParkingMutex::new(Vec::new()));
        Self {
            consensus_capabilities,
            engine_tx,
        }
    }
}

pub struct EngineEthRPCServer {
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
}

impl EngineEthRPCServer {
    pub fn new(execution_position: Arc<Mutex<ExecutionPositionV2>>) -> Self {
        Self { execution_position }
    }
}

#[async_trait]
impl EngineEthApiServer for EngineEthRPCServer {
    async fn block_number(&self) -> RpcResult<String> {
        Ok(format!(
            "{:X}",
            self.execution_position.lock().await.latest_block_number()
        ))
    }

    async fn call(&self, _transaction: TransactionRequest, _block: BlockId) -> RpcResult<Bytes> {
        todo!()
    }

    async fn chain_id(&self) -> RpcResult<U256> {
        Ok(U256::from(1))
    }

    async fn get_block_by_hash(
        &self,
        _block_hash: B256,
        _hydrated_transactions: bool,
    ) -> RpcResult<Block> {
        todo!()
    }

    async fn get_block_by_number(
        &self,
        _block_number: u64,
        _hydrated_transactions: bool,
    ) -> RpcResult<Block> {
        todo!()
    }

    async fn get_code(&self, _address: Address, _block: BlockId) -> RpcResult<Bytes> {
        todo!()
    }

    async fn get_logs(&self, _filter: Filter) -> RpcResult<Vec<Log>> {
        todo!()
    }

    async fn send_raw_transaction(&self, _bytes: Bytes) -> RpcResult<B256> {
        Err(ErrorObject::owned(
            -333333,
            "send_raw_transaction is not implemented: depends on Portal Transaction Gossip Network",
            None::<()>,
        ))
    }

    async fn syncing(&self) -> RpcResult<SyncStatus> {
        let execution_position = self.execution_position.lock().await.clone();

        let current_block = execution_position.next_block_number().saturating_sub(1);

        if current_block == execution_position.latest_block_number() {
            return Ok(SyncStatus::None);
        }

        Ok(SyncStatus::Info(Box::new(SyncInfo {
            starting_block: U256::from(execution_position.starting_block_number()),
            current_block: U256::from(current_block),
            highest_block: U256::from(execution_position.latest_block_number()),
            warp_chunks_amount: None,
            warp_chunks_processed: None,
            stages: None,
        })))
    }
}
