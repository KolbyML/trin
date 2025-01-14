pub mod auth_middleware;

use std::{net::SocketAddr, path::Path, sync::Arc};

use alloy_rpc_types_engine::JwtSecret;
use auth_middleware::JwtAuthLayer;
use jsonrpsee::{
    server::{Server, ServerHandle},
    RpcModule,
};
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use crate::{
    cli::TrinExecutionConfig,
    engine::{
        command::EngineCommand,
        rpc::{EngineApiServer, EngineEthApiServer, EngineEthRPCServer, EngineRPCServer},
    },
    rpc::utils::read_default_jwt_secret,
    storage::execution_position::ExecutionPositionV2,
};

pub struct EngineAuthServer {}

impl EngineAuthServer {
    pub async fn start(
        engine_tx: UnboundedSender<EngineCommand>,
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        data_dir: &Path,
        trin_execution_config: TrinExecutionConfig,
    ) -> anyhow::Result<ServerHandle> {
        let jwt_secret = match trin_execution_config.rpc_authentication_jwt_secret_path {
            Some(path) => JwtSecret::from_file(&path)?,
            None => read_default_jwt_secret(data_dir)?,
        };

        let socket_address = SocketAddr::from((
            trin_execution_config.rpc_authentication_address,
            trin_execution_config.rpc_authentication_port,
        ));

        let mut module = RpcModule::new(());

        // Add EngineApi to the module
        module.merge(EngineRPCServer::new(engine_tx.clone()).into_rpc())?;

        // Add ETH subset required for the Engine
        module.merge(EngineEthRPCServer::new(execution_position.clone()).into_rpc())?;

        let middleware = tower::ServiceBuilder::new().layer(JwtAuthLayer::new(jwt_secret));

        let server = Server::builder()
            .set_http_middleware(middleware)
            .build(socket_address)
            .await?;

        let handle = server.start(module);
        Ok(handle)
    }
}
