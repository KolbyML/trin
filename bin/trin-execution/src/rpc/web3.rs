use alloy::primitives::bytes::Bytes;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use revm_primitives::B256;
use tracing::info;

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

    fn sha3(&self, _input: Bytes) -> RpcResult<B256> {
        info!("Received sha3 request");
        Ok(B256::ZERO)
    }
}
