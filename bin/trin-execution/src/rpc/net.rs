use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use tracing::info;

/// Engine Api JSON-RPC endpoints
#[rpc(client, server, namespace = "net")]
pub trait NetApi {
    #[method(name = "clientVersion")]
    async fn client_version(&self) -> RpcResult<String>;
}

pub struct NetRPCServer {}

#[async_trait]
impl NetApiServer for NetRPCServer {
    async fn client_version(&self) -> RpcResult<String> {
        info!("Received client_version request");
        Ok("test test test".to_string())
    }
}
