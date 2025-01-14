use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use tracing::info;

/// Engine Api JSON-RPC endpoints
#[rpc(client, server, namespace = "debug")]
pub trait DebugApi {
    #[method(name = "clientVersion")]
    async fn client_version(&self) -> RpcResult<String>;
}

pub struct DebugRPCServer {}

#[async_trait]
impl DebugApiServer for DebugRPCServer {
    async fn client_version(&self) -> RpcResult<String> {
        info!("Received client_version request");
        Ok("test test test".to_string())
    }
}
