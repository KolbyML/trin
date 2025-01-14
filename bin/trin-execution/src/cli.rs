use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use alloy::genesis::Genesis;
use anyhow::anyhow;
use clap::{Args, Parser, Subcommand};
use portal_bridge::cli::BridgeId;
use url::Url;

use crate::{
    chain_spec::{ChainSpec, MAINNET},
    rpc::{RpcNamespace, RpcNamespaces},
    types::block_to_trace::BlockToTrace,
};

pub const APP_NAME: &str = "trin-execution";
const DEFAULT_RPC_AUTHENTICATION_PORT: u16 = 8551;
const DEFAULT_HTTP_PORT: u16 = 8545;

#[derive(Parser, Debug, Clone)]
#[command(name = "Trin Execution", about = "Executing blocks with no devp2p")]
pub struct TrinExecutionConfig {
    #[arg(
        long,
        help = "The directory for storing application data. If used together with --ephemeral, new child directory will be created."
    )]
    pub data_dir: Option<PathBuf>,

    #[arg(
        long,
        short,
        help = "Use new data directory, located in OS temporary directory. If used together with --data-dir, new directory will be created there instead."
    )]
    pub ephemeral: bool,

    #[arg(
        long = "debug.last-block",
        help = "The last block that should be executed. This is useful if execution should stop early. This should be used for debugging purposes only."
    )]
    pub debug_last_block: Option<u64>,

    #[arg(
        long,
        default_value = "none",
        help = "The block traces will be dumped to the working directory: Configuration options ['none', 'block:<number>', 'all']."
    )]
    pub block_to_trace: BlockToTrace,

    #[arg(
        long,
        help = "Enable prometheus metrics reporting (provide local IP/Port from which your Prometheus server is configured to fetch metrics)"
    )]
    pub enable_metrics_with_url: Option<SocketAddr>,

    #[arg(long = "http", help = "Used to enable HTTP rpc.")]
    pub http: bool,

    #[arg(
        long = "http.addr",
        help = "Address used for authentication for the engine api RPC server",
        default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST),
        requires = "http"
    )]
    pub http_address: IpAddr,

    #[arg(
        long = "http.port",
        help = "Port used for authentication for the engine api RPC server",
        default_value_t = DEFAULT_HTTP_PORT,
        requires = "http"
    )]
    pub http_port: u16,

    #[arg(
        long = "http.api",
        help = "Namespaces enabled for the HTTP API",
        default_value = "eth",
        requires = "http"
    )]
    pub http_enabled_namespaces: RpcNamespaces,

    #[arg(
        long = "authrpc.addr",
        help = "Address used for authentication for the engine api RPC server",
        default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST)
    )]
    pub rpc_authentication_address: IpAddr,

    #[arg(
        long = "authrpc.port",
        help = "Port used for authentication for the engine api RPC server",
        default_value_t = DEFAULT_RPC_AUTHENTICATION_PORT
    )]
    pub rpc_authentication_port: u16,

    #[arg(
        long = "authrpc.jwtsecret",
        help = "Location of the jwt secret file used for authentication for the engine api RPC server. Defaults to the data directory."
    )]
    pub rpc_authentication_jwt_secret_path: Option<PathBuf>,

    #[arg(
        long,
        help = "Endpoint for the beacon node API of the running consensus layer client (Required for syncing state from the beacon node)"
    )]
    pub beacon_api_endpoint: Option<Url>,

    #[arg(
        long,
        help = "Gossip the state diffs between block execution to the Portal State Network, this will slow down the execution"
    )]
    pub bridge_diffs: bool,

    #[arg(
        long,
        help = "How many blocks to wait before executing the next block",
        default_value_t = 0
    )]
    pub execution_delay: u64,

    #[arg(
        help = "Bridge identifier: 'bridge_id/bridge_total' eg. '1/4' (STATE BRIDGE ONLY)",
        long = "bridge-id",
        default_value = "1/1"
    )]
    pub bridge_id: BridgeId,

    #[arg(long = "save-blocks", help = "Save blocks to disk")]
    pub save_blocks: bool,

    #[arg(
        help = "The chain Trin Execution is running on (mainnet, testnet, etc.) or a path to a genesis file",
        long,
        default_value = "mainnet",
        value_parser = chain_parser
    )]
    pub chain: Arc<ChainSpec>,

    #[command(subcommand)]
    pub command: Option<TrinExecutionSubCommands>,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum TrinExecutionSubCommands {
    /// Import genesis state from a file
    Init,
    /// Import chain data from file
    Import(ImportConfig),
    /// Import a era2 state snapshot from a file, useful for bootstrapping a new node quickly
    ImportState(ImportStateConfig),
    /// Export the current state of the node to a era2 file
    ExportState(ExportStateConfig),
    /// Print stats on what it takes to gossip the whole state onto the network
    StateGossipStats,
}

#[derive(Args, Debug, Default, Clone, PartialEq)]
pub struct ImportStateConfig {
    #[arg(long, help = "path to where the era2 state snapshot is located")]
    pub path_to_era2: PathBuf,
}

#[derive(Args, Debug, Default, Clone, PartialEq)]
pub struct ExportStateConfig {
    #[arg(long, help = "path to where the era2 state snapshot is located")]
    pub path_to_era2: PathBuf,
}

#[derive(Args, Debug, Default, Clone, PartialEq)]
pub struct ImportConfig {
    pub path: PathBuf,
}

pub fn chain_parser(chain_string: &str) -> Result<Arc<ChainSpec>, String> {
    match chain_string {
        "mainnet" => Ok(MAINNET.clone()),
        _ => {
            let json = fs::read_to_string(PathBuf::from(chain_string))
                .map_err(|err| format!("Error {err:?}"))?;

            let genesis: Genesis =
                serde_json::from_str(&json).map_err(|err| format!("Error {err:?}"))?;

            Ok(Arc::new(genesis.into()))
        }
    }
}
