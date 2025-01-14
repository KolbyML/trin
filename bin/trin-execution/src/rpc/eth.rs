use std::sync::Arc;

use alloy::{
    eips::BlockNumberOrTag,
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
    chain_spec::ChainSpec,
    storage::{block::BlockStorage, execution_position::ExecutionPositionV2},
    sync::era::execution_payload::{
        ExecutionPayloadV3WithBeaconBlockHash, ProcessExecutionPayload,
    },
};

/// Engine Api JSON-RPC endpoints
#[rpc(client, server, namespace = "eth")]
pub trait EthApi {
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
        block_number_or_tag: BlockNumberOrTag,
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

pub struct EthRPCServer {
    execution_position: Arc<Mutex<ExecutionPositionV2>>,
    chain_spec: Arc<ChainSpec>,
    block_storage: Arc<BlockStorage>,
}

impl EthRPCServer {
    pub fn new(
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        chain_spec: Arc<ChainSpec>,
        block_storage: Arc<BlockStorage>,
    ) -> Self {
        Self {
            execution_position,
            chain_spec,
            block_storage,
        }
    }
}

#[async_trait]
impl EthApiServer for EthRPCServer {
    async fn block_number(&self) -> RpcResult<String> {
        Ok(format!(
            "{:X}",
            self.execution_position.lock().await.previous_block_number()
        ))
    }

    async fn call(&self, _transaction: TransactionRequest, _block: BlockId) -> RpcResult<Bytes> {
        Err(ErrorObject::owned(
            -333333,
            "call is not implemented: depends on Portal Transaction Gossip Network",
            None::<()>,
        ))
    }

    async fn chain_id(&self) -> RpcResult<U256> {
        Ok(U256::from(self.chain_spec.chain.id()))
    }

    async fn get_block_by_hash(
        &self,
        block_hash: B256,
        hydrated_transactions: bool,
    ) -> RpcResult<Block> {
        info!("Received get_block_by_hash request");
        self.block_storage
            .get_block_by_hash(block_hash, hydrated_transactions)
            .map_err(|e| ErrorObject::owned(-333333, e.to_string(), None::<()>))
    }

    async fn get_block_by_number(
        &self,
        block_number_or_tag: BlockNumberOrTag,
        hydrated_transactions: bool,
    ) -> RpcResult<Block> {
        info!("Received get_block_by_number request");

        let block_number = match block_number_or_tag {
            BlockNumberOrTag::Number(block_number) => block_number,
            BlockNumberOrTag::Latest => {
                self.execution_position.lock().await.previous_block_number()
            }
            _ => {
                return Err(ErrorObject::owned(
                    -333333,
                    "This tag is not supported",
                    None::<()>,
                ))
            }
        };

        self.block_storage
            .get_block_by_number(block_number, hydrated_transactions)
            .map_err(|e| ErrorObject::owned(-333333, e.to_string(), None::<()>))
    }

    async fn get_code(&self, _address: Address, _block: BlockId) -> RpcResult<Bytes> {
        info!("Received get_code request");
        Err(ErrorObject::owned(
            -333333,
            "get_code is not implemented: depends on Portal Transaction Gossip Network",
            None::<()>,
        ))
    }

    async fn get_logs(&self, _filter: Filter) -> RpcResult<Vec<Log>> {
        info!("Received get_logs request");
        Err(ErrorObject::owned(
            -333333,
            "get_logs is not implemented: depends on Portal Transaction Gossip Network",
            None::<()>,
        ))
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
