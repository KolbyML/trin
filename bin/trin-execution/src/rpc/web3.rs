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

use crate::{
    storage::execution_position::ExecutionPositionV2,
    sync::era::execution_payload::{
        ExecutionPayloadV3WithBeaconBlockHash, ProcessExecutionPayload,
    },
};

/// Engine Api JSON-RPC endpoints
#[rpc(client, server, namespace = "web3")]
pub trait Web3Api {
    #[method(name = "clientVersion")]
    async fn client_version(&self) -> RpcResult<String>;

    #[method(name = "sha3")]
    fn sha3(&self, input: Bytes) -> RpcResult<B256>;
}

pub struct Web3RPCServer {}

#[async_trait]
impl Web3ApiServer for Web3RPCServer {
    async fn client_version(&self) -> RpcResult<String> {
        info!("Received client_version request");
        Ok("test test test".to_string())
    }

    fn sha3(&self, input: Bytes) -> RpcResult<B256> {
        info!("Received sha3 request");
        Ok(B256::ZERO)
    }
}
