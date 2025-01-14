#![warn(clippy::unwrap_used)]
#![warn(clippy::uninlined_format_args)]

use std::{fs, path::PathBuf, sync::Arc};

#[cfg(windows)]
use ethportal_api::types::cli::Web3TransportType;
use ethportal_api::{
    types::{
        distance::Distance,
        network::{Network, Subnetwork},
        portal_wire::MAINNET,
    },
    utils::bytes::hex_encode,
    version::get_trin_version,
};
use portalnet::{
    bootnodes::Bootnodes,
    config::{PortalnetConfig, NODE_ADDR_CACHE_CAPACITY},
    constants::{DEFAULT_DISCOVERY_PORT, DEFAULT_UTP_TRANSFER_LIMIT},
    discovery::{Discovery, Discv5UdpSocket},
    events::PortalnetEvents,
    utils::db::configure_node_data_dir,
};
use tokio::sync::{mpsc, RwLock};
use tracing::info;
use tree_hash::TreeHash;
use trin_history::initialize_history_network;
use trin_state::initialize_state_network;
use trin_storage::{config::StorageCapacityConfig, PortalStorageConfigFactory};
use trin_validation::oracle::HeaderOracle;
use utp_rs::socket::UtpSocket;

pub async fn run_trin(
    data_dir: PathBuf,
) -> Result<Arc<RwLock<HeaderOracle>>, Box<dyn std::error::Error>> {
    let trin_version = get_trin_version();
    info!("Launching Trin: v{trin_version}");

    let network = Network::Mainnet;

    // Setup data directory
    let trin_data_dir = data_dir.join("trin");
    fs::create_dir_all(&trin_data_dir)?;

    // Configure node data dir based on the provided private key
    let (node_data_dir, private_key) = configure_node_data_dir(&trin_data_dir, None, network)?;

    let portalnet_config = PortalnetConfig {
        external_addr: None,
        private_key,
        listen_port: DEFAULT_DISCOVERY_PORT,
        bootnodes: Bootnodes::Default.to_enrs(network),
        no_stun: false,
        no_upnp: false,
        node_addr_cache_capacity: NODE_ADDR_CACHE_CAPACITY,
        disable_poke: false,
        trusted_block_root: None,
        utp_transfer_limit: DEFAULT_UTP_TRANSFER_LIMIT,
    };

    // Initialize base discovery protocol
    let mut discovery = Discovery::new(portalnet_config.clone(), MAINNET.clone())?;
    let talk_req_rx = discovery.start().await?;
    let discovery = Arc::new(discovery);

    // Initialize validation oracle
    let header_oracle = HeaderOracle::default();
    info!(
        hash_tree_root = %hex_encode(header_oracle.header_validator.pre_merge_acc.tree_hash_root().0),
        "Loaded pre-merge accumulator."
    );
    let header_oracle = Arc::new(RwLock::new(header_oracle));

    // Initialize and spawn uTP socket
    let (utp_talk_reqs_tx, utp_talk_reqs_rx) = mpsc::unbounded_channel();

    // Set the enr_cache_capacity to the maximum uTP limit between all active networks. This is
    // a trade off between memory usage and increased searches from the networks for each Enr.
    // utp_transfer_limit is 2x as it would be utp_transfer_limit for incoming and
    // utp_transfer_limit for outgoing
    let enr_cache_capacity = portalnet_config.utp_transfer_limit * 2 * 3;
    let discv5_utp_socket = Discv5UdpSocket::new(
        Arc::clone(&discovery),
        utp_talk_reqs_rx,
        header_oracle.clone(),
        enr_cache_capacity,
    );
    let utp_socket = UtpSocket::with_socket(discv5_utp_socket);
    let utp_socket = Arc::new(utp_socket);

    let storage_config_factory = PortalStorageConfigFactory::new(
        StorageCapacityConfig::Combined {
            total_mb: 0,
            subnetworks: vec![Subnetwork::State, Subnetwork::History],
        },
        discovery.local_enr().node_id(),
        node_data_dir,
    )?;

    // Initialize state sub-network service and event handlers, if selected
    let (state_handler, state_network_task, state_event_tx, _, state_event_stream) =
        initialize_state_network(
            &discovery,
            utp_socket.clone(),
            portalnet_config.clone(),
            storage_config_factory.create(&Subnetwork::State, Distance::ZERO)?,
            header_oracle.clone(),
        )
        .await?;

    // Initialize chain history sub-network service and event handlers, if selected
    let (history_handler, history_network_task, history_event_tx, _, history_event_stream) =
        initialize_history_network(
            &discovery,
            utp_socket.clone(),
            portalnet_config.clone(),
            storage_config_factory.create(&Subnetwork::History, Distance::ZERO)?,
            header_oracle.clone(),
        )
        .await?;

    if let Some(handler) = state_handler {
        tokio::spawn(async move { handler.handle_client_queries().await });
    }
    if let Some(handler) = history_handler {
        tokio::spawn(async move { handler.handle_client_queries().await });
    }

    // Spawn main portal events handler
    tokio::spawn(async move {
        let events = PortalnetEvents::new(
            talk_req_rx,
            (history_event_tx, history_event_stream),
            (state_event_tx, state_event_stream),
            (None, None),
            utp_talk_reqs_tx,
            MAINNET.clone(),
        )
        .await;
        events.start().await;
    });

    if let Some(network) = history_network_task {
        tokio::spawn(network);
    }
    if let Some(network) = state_network_task {
        tokio::spawn(network);
    }

    Ok(header_oracle)
}
