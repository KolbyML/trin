pub mod admin;
pub mod debug;
pub mod engine;
pub mod eth;
pub mod net;
pub mod utils;
pub mod web3;

use std::{net::SocketAddr, path::Path, str::FromStr, sync::Arc};

use admin::{AdminApiServer, AdminRPCServer};
use alloy_rpc_types_engine::JwtSecret;
use debug::{DebugApiServer, DebugRPCServer};
use eth::{EthApiServer, EthRPCServer};
use jsonrpsee::{
    server::{Server, ServerHandle},
    RpcModule,
};
use net::{NetApiServer, NetRPCServer};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use web3::{Web3ApiServer, Web3RPCServer};

use crate::{
    chain_spec::ChainSpec,
    cli::TrinExecutionConfig,
    engine::{
        command::EngineCommand,
        rpc::{EngineApiServer, EngineEthApiServer, EngineEthRPCServer, EngineRPCServer},
    },
    rpc::utils::read_default_jwt_secret,
    storage::{block::BlockStorage, execution_position::ExecutionPositionV2},
};

#[derive(Debug, Clone)]
pub struct RpcNamespaces {
    pub namespaces: Vec<RpcNamespace>,
}

impl Default for RpcNamespaces {
    fn default() -> Self {
        Self {
            namespaces: vec![RpcNamespace::Eth],
        }
    }
}

impl FromStr for RpcNamespaces {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let namespaces = s
            .split(',')
            .map(|namespace| match namespace {
                "eth" => Ok(RpcNamespace::Eth),
                "net" => Ok(RpcNamespace::Net),
                "web3" => Ok(RpcNamespace::Web3),
                "debug" => Ok(RpcNamespace::Debug),
                "admin" => Ok(RpcNamespace::Admin),
                _ => Err(anyhow::anyhow!("Invalid namespace: {}", namespace)),
            })
            .collect::<Result<Vec<RpcNamespace>, anyhow::Error>>()?;

        Ok(Self { namespaces })
    }
}

impl RpcNamespaces {
    pub fn contains(&self, namespace: &RpcNamespace) -> bool {
        self.namespaces.contains(namespace)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcNamespace {
    Eth,
    Net,
    Web3,
    Debug,
    Admin,
}

pub struct RpcServer {}

impl RpcServer {
    pub async fn start(
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        trin_execution_config: TrinExecutionConfig,
        block_storage: Arc<BlockStorage>,
    ) -> anyhow::Result<ServerHandle> {
        let socket_address = SocketAddr::from((
            trin_execution_config.http_address,
            trin_execution_config.http_port,
        ));

        let mut module = RpcModule::new(());

        if trin_execution_config
            .http_enabled_namespaces
            .namespaces
            .contains(&RpcNamespace::Eth)
        {
            module.merge(
                EthRPCServer::new(
                    execution_position,
                    trin_execution_config.chain.clone(),
                    block_storage,
                )
                .into_rpc(),
            )?;
        }
        if trin_execution_config
            .http_enabled_namespaces
            .namespaces
            .contains(&RpcNamespace::Net)
        {
            module.merge(NetRPCServer {}.into_rpc())?;
        }
        if trin_execution_config
            .http_enabled_namespaces
            .namespaces
            .contains(&RpcNamespace::Web3)
        {
            module.merge(Web3RPCServer {}.into_rpc())?;
        }
        if trin_execution_config
            .http_enabled_namespaces
            .namespaces
            .contains(&RpcNamespace::Debug)
        {
            module.merge(DebugRPCServer {}.into_rpc())?;
        }
        if trin_execution_config
            .http_enabled_namespaces
            .namespaces
            .contains(&RpcNamespace::Admin)
        {
            module.merge(AdminRPCServer {}.into_rpc())?;
        }

        let server = Server::builder().build(socket_address).await?;

        let handle = server.start(module);
        Ok(handle)
    }
}
