use std::collections::BTreeMap;
use std::sync::Arc;
use discv5::{Enr, TalkRequest};
use discv5::enr::NodeId;
use discv5::kbucket::NodeStatus;

use parking_lot::RwLock as PLRwLock;
use tokio::sync::RwLock;
use utp_rs::socket::UtpSocket;
use ethportal_api::OverlayContentKey;

use crate::validation::BeaconValidator;
use ethportal_api::types::content_key::{BeaconContentKey, RawContentKey};
use ethportal_api::types::distance::{Distance, XorMetric};
use ethportal_api::types::query_trace::QueryTrace;
use portalnet::{
    discovery::{Discovery, UtpEnr},
    overlay::{OverlayConfig, OverlayProtocol},
    storage::{PortalStorage, PortalStorageConfig},
    types::messages::{PortalnetConfig, ProtocolId},
};
use portalnet::network::Network;
use portalnet::types::messages::{Accept, Content, Nodes, Pong, Request, Response};
use trin_validation::oracle::HeaderOracle;

pub(crate) type BucketEntry = (NodeId, Enr, NodeStatus, Distance, Option<String>);

#[async_trait::async_trait]
impl Network for BeaconNetwork {
    type Result = Arc<OverlayProtocol<BeaconContentKey, XorMetric, BeaconValidator, PortalStorage>>;

    fn protocol(&self) -> &ProtocolId {
        self.overlay.protocol()
    }

    fn local_enr(&self) -> ethportal_api::Enr {
        self.overlay.local_enr()
    }

    fn data_radius(&self) -> Distance {
        self.overlay.data_radius()
    }

    async fn process_one_request(&self, talk_request: &TalkRequest) -> Result<Response, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.process_one_request(talk_request).await
    }

    fn propagate_gossip<T as BeaconContentKey>(&self, content: Vec<(T as BeaconContentKey, Vec<u8>)>) -> usize {
        self.overlay.propagate_gossip(content)
    }

    fn table_entries_id(&self) -> Vec<NodeId> {
        self.overlay.table_entries_id()
    }

    fn table_entries_enr(&self) -> Vec<ethportal_api::Enr> {
        self.overlay.table_entries_enr()
    }

    fn bucket_entries(&self) -> BTreeMap<usize, Vec<BucketEntry>> {
        self.overlay.bucket_entries()
    }

    fn add_enr(&self, enr: ethportal_api::Enr) -> Result<(), portalnet::overlay_service::OverlayRequestError> {
        self.overlay.add_enr(enr)
    }

    fn get_enr(&self, node_id: NodeId) -> Result<ethportal_api::Enr, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.get_enr(node_id)
    }

    fn delete_enr(&self, node_id: NodeId) -> bool {
        self.overlay.delete_enr(node_id)
    }

    async fn lookup_enr(&self, node_id: NodeId) -> Result<ethportal_api::Enr, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.lookup_enr(node_id).await
    }

    async fn send_ping(&self, enr: ethportal_api::Enr) -> Result<Pong, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.send_ping(enr).await
    }

    async fn send_find_nodes(&self, enr: ethportal_api::Enr, distances: Vec<u16>) -> Result<Nodes, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.send_find_nodes(enr, distances).await
    }

    async fn send_find_content(&self, enr: ethportal_api::Enr, content_key: Vec<u8>) -> Result<Content, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.send_find_content(enr, content_key).await
    }

    async fn send_offer(&self, content_keys: Vec<RawContentKey>, enr: ethportal_api::Enr) -> Result<Accept, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.send_offer(content_keys, enr).await
    }

    async fn send_populated_offer(&self, enr: ethportal_api::Enr, content_key: RawContentKey, content_value: Vec<u8>) -> Result<Accept, portalnet::overlay_service::OverlayRequestError> {
        self.overlay.send_populated_offer(enr, content_key, content_value).await
    }

    async fn lookup_node(&self, target: NodeId) -> Vec<Enr> {
        self.overlay.lookup_node(target).await
    }

    async fn lookup_content<TContentKey>(&self, target: TContentKey, is_trace: bool) -> (Option<Vec<u8>>, Option<QueryTrace>) {
        self.overlay.lookup_content(target, is_trace).await
    }

    async fn ping_bootnodes(&self) {
       self.overlay.ping_bootnodes().await
    }

    fn get_summary_info(&self) -> String {
        self.overlay.get_summary_info()
    }

    fn overlay(&self) -> Arc<OverlayProtocol<BeaconContentKey, XorMetric, BeaconValidator, PortalStorage>> {
        self.overlay.clone()
    }
}

/// Beacon network layer on top of the overlay protocol. Encapsulates beacon network specific data and logic.
#[derive(Clone)]
pub struct BeaconNetwork {
    pub overlay: Arc<OverlayProtocol<BeaconContentKey, XorMetric, BeaconValidator, PortalStorage>>,
}

impl BeaconNetwork {
    pub async fn new(
        discovery: Arc<Discovery>,
        utp_socket: Arc<UtpSocket<UtpEnr>>,
        storage_config: PortalStorageConfig,
        portal_config: PortalnetConfig,
        header_oracle: Arc<RwLock<HeaderOracle>>,
    ) -> anyhow::Result<Self> {
        let config = OverlayConfig {
            bootnode_enrs: portal_config.bootnode_enrs.clone(),
            ..Default::default()
        };
        let storage = Arc::new(PLRwLock::new(PortalStorage::new(
            storage_config,
            ProtocolId::Beacon,
        )?));
        let validator = Arc::new(BeaconValidator { header_oracle });
        let overlay = OverlayProtocol::new(
            config,
            discovery,
            utp_socket,
            storage,
            ProtocolId::Beacon,
            validator,
        )
        .await;

        Ok(Self {
            overlay: Arc::new(overlay),
        })
    }
}
