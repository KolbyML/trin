use std::collections::BTreeMap;
use async_trait::async_trait;
use discv5::{enr::NodeId, kbucket::{
    Entry, FailureReason, Filter, InsertResult, KBucketsTable, Key, NodeStatus,
    MAX_NODES_PER_BUCKET,
}, ConnectionDirection, ConnectionState, TalkRequest, Enr};
use ethportal_api::OverlayContentKey;
use ethportal_api::types::content_key::RawContentKey;
use ethportal_api::types::distance::Distance;
use ethportal_api::types::query_trace::QueryTrace;
use crate::overlay_service::OverlayRequestError;
use crate::types::messages::{Accept, Content, Nodes, Pong, ProtocolId, Response};

pub(crate) type BucketEntry = (NodeId, Enr, NodeStatus, Distance, Option<String>);

/// An encodable portal network content value.
#[async_trait]
pub trait Network {
    type Result;
    /// Returns the subnetwork protocol of the overlay protocol.
    fn protocol(&self) -> &ProtocolId;

    /// Returns the ENR of the local node.
    fn local_enr(&self) -> Enr;

    /// Returns the data radius of the local node.
    fn data_radius(&self) -> Distance;

    /// Processes a single Discovery v5 TALKREQ message.
    async fn process_one_request(
        &self,
        talk_request: &TalkRequest,
    ) -> Result<Response, OverlayRequestError>;

    /// Propagate gossip accepted content via OFFER/ACCEPT, return number of peers propagated
    fn propagate_gossip<T>(&self, content: Vec<(T, Vec<u8>)>) -> usize  where T: 'static + OverlayContentKey + Send + Sync;

    /// Returns a vector of all ENR node IDs of nodes currently contained in the routing table.
    fn table_entries_id(&self) -> Vec<NodeId>;

    /// Returns a vector of all the ENRs of nodes currently contained in the routing table.
    fn table_entries_enr(&self) -> Vec<Enr>;

    /// Returns a map (BTree for its ordering guarantees) with:
    ///     key: usize representing bucket index
    ///     value: Vec of tuples, each tuple represents a node
    fn bucket_entries(&self) -> BTreeMap<usize, Vec<crate::overlay::BucketEntry>>;

    /// `AddEnr` adds requested `enr` to our kbucket.
    fn add_enr(&self, enr: Enr) -> Result<(), OverlayRequestError>;

    /// `GetEnr` gets requested `enr` from our kbucket.
    fn get_enr(&self, node_id: NodeId) -> Result<Enr, OverlayRequestError>;

    /// `DeleteEnr` deletes requested `enr` from our kbucket.
    fn delete_enr(&self, node_id: NodeId) -> bool;

    /// `LookupEnr` finds requested `enr` from our kbucket, FindNode, and RecursiveFindNode.
    async fn lookup_enr(&self, node_id: NodeId) -> Result<Enr, OverlayRequestError>;

    /// Sends a `Ping` request to `enr`.
    async fn send_ping(&self, enr: Enr) -> Result<Pong, OverlayRequestError>;

    /// Sends a `FindNodes` request to `enr`.
    async fn send_find_nodes(
        &self,
        enr: Enr,
        distances: Vec<u16>,
    ) -> Result<Nodes, OverlayRequestError>;

    /// Sends a `FindContent` request for `content_key` to `enr`.
    async fn send_find_content(
        &self,
        enr: Enr,
        content_key: Vec<u8>,
    ) -> Result<Content, OverlayRequestError>;

    /// Offer is sent in order to store content to k nodes with radii that contain content-id
    /// Offer is also sent to nodes after FindContent (POKE)
    async fn send_offer(
        &self,
        content_keys: Vec<RawContentKey>,
        enr: Enr,
    ) -> Result<Accept, OverlayRequestError>;

    /// Send Offer request without storing the content into db
    async fn send_populated_offer(
        &self,
        enr: Enr,
        content_key: RawContentKey,
        content_value: Vec<u8>,
    ) -> Result<Accept, OverlayRequestError>;

    async fn lookup_node(&self, target: NodeId) -> Vec<Enr>;

    /// Performs a content lookup for `target`.
    /// Returns the target content along with the peers traversed during content lookup.
    async fn lookup_content<TContentKey>(
        &self,
        target: TContentKey,
        is_trace: bool,
    ) -> (Option<Vec<u8>>, Option<QueryTrace>);

    async fn ping_bootnodes(&self);

    fn get_summary_info(&self) -> String;

    fn overlay(&self) -> Self::Result;
}
