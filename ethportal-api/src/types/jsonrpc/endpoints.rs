use crate::{
    types::enr::Enr, BeaconContentKey, BeaconContentValue, HistoryContentKey, HistoryContentValue,
    StateContentKey, StateContentValue,
};
use discv5::enr::NodeId;

/// Discv5 JSON-RPC endpoints. Start with "discv5_" prefix
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Discv5Endpoint {
    NodeInfo,
    RoutingTableInfo,
}

/// State network JSON-RPC endpoints. Start with "portal_state" prefix
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum StateEndpoint {
    /// params: None
    RoutingTableInfo,
    /// params: [enr]
    Ping(Enr),
    /// params: [enr]
    AddEnr(Enr),
    /// params: [node_id]
    DeleteEnr(NodeId),
    /// params: [node_id]
    GetEnr(NodeId),
    /// params: [node_id]
    LookupEnr(NodeId),
    /// params: [enr, distances]
    FindNodes(Enr, Vec<u16>),
    /// params: [node_id]
    RecursiveFindNodes(NodeId),
    /// params: None
    DataRadius,
    /// params: content_key
    LocalContent(StateContentKey),
    /// params: [enr, content_key]
    FindContent(Enr, StateContentKey),
    /// params: content_key
    RecursiveFindContent(StateContentKey),
    /// params: content_key
    TraceRecursiveFindContent(StateContentKey),
    /// params: [content_key, content_value]
    Store(StateContentKey, StateContentValue),
    /// WireOffer is not supported in the state network, since locally
    /// stored values do not contain the proofs necessary for valid gossip.
    /// params: [enr, content_key, content_value]
    Offer(Enr, StateContentKey, StateContentValue),
    /// params: [enr, content_key, content_value]
    TraceOffer(Enr, StateContentKey, StateContentValue),
    /// params: [enr, content_key, content_value]
    Gossip(StateContentKey, StateContentValue),
    /// params: [content_key, content_value]
    TraceGossip(StateContentKey, StateContentValue),
    /// params: [offset, limit]
    PaginateLocalContentKeys(u64, u64),
}

/// History network JSON-RPC endpoints. Start with "portal_history" prefix
#[derive(Debug, PartialEq, Clone)]
pub enum HistoryEndpoint {
    /// params: [enr]
    AddEnr(Enr),
    /// params: None
    DataRadius,
    /// params: [node_id]
    DeleteEnr(NodeId),
    /// params: [enr, content_key]
    FindContent(Enr, HistoryContentKey),
    /// params: [enr, distances]
    FindNodes(Enr, Vec<u16>),
    /// params: [node_id]
    GetEnr(NodeId),
    /// params: content_key
    LocalContent(HistoryContentKey),
    /// params: [node_id]
    LookupEnr(NodeId),
    /// params: [content_key, content_value]
    Gossip(HistoryContentKey, HistoryContentValue),
    /// params: [content_key, content_value]
    TraceGossip(HistoryContentKey, HistoryContentValue),
    /// params: [enr, content_key, content_value]
    Offer(Enr, HistoryContentKey, HistoryContentValue),
    /// params: [enr, content_key, content_value]
    TraceOffer(Enr, HistoryContentKey, HistoryContentValue),
    /// params: [enr, [content_key]]
    WireOffer(Enr, Vec<HistoryContentKey>),
    /// params: [enr]
    Ping(Enr),
    /// params: content_key
    RecursiveFindContent(HistoryContentKey),
    /// params: content_key
    TraceRecursiveFindContent(HistoryContentKey),
    /// params: [content_key, content_value]
    Store(HistoryContentKey, HistoryContentValue),
    /// params: None
    RoutingTableInfo,
    // This endpoint is not History network specific
    /// params: [offset, limit]
    PaginateLocalContentKeys(u64, u64),
    /// params: [node_id]
    RecursiveFindNodes(NodeId),
}

/// Beacon network JSON-RPC endpoints. Start with "portal_beacon" prefix
#[derive(Debug, PartialEq, Clone)]
pub enum BeaconEndpoint {
    /// params: enr
    AddEnr(Enr),
    /// params: None
    DataRadius,
    /// params: node_id
    DeleteEnr(NodeId),
    /// params: None
    OptimisticStateRoot,
    /// params: [enr, content_key]
    FindContent(Enr, BeaconContentKey),
    /// params: [enr, distances]
    FindNodes(Enr, Vec<u16>),
    /// params: None
    FinalizedStateRoot,
    /// params: node_id
    GetEnr(NodeId),
    /// params: content_key
    LocalContent(BeaconContentKey),
    /// params: node_id
    LookupEnr(NodeId),
    /// params: [content_key, content_value]
    Gossip(BeaconContentKey, BeaconContentValue),
    /// params: [content_key, content_value]
    TraceGossip(BeaconContentKey, BeaconContentValue),
    /// params: [enr, content_key, content_value]
    Offer(Enr, BeaconContentKey, BeaconContentValue),
    /// params: [enr, content_key, content_value]
    TraceOffer(Enr, BeaconContentKey, BeaconContentValue),
    /// params: [enr, [content_key]]
    WireOffer(Enr, Vec<BeaconContentKey>),
    /// params: enr
    Ping(Enr),
    /// params: content_key
    RecursiveFindContent(BeaconContentKey),
    /// params: content_key
    TraceRecursiveFindContent(BeaconContentKey),
    /// params: [content_key, content_value]
    Store(BeaconContentKey, BeaconContentValue),
    /// params: None
    RoutingTableInfo,
    /// params: [offset, limit]
    PaginateLocalContentKeys(u64, u64),
    /// params: [node_id]
    RecursiveFindNodes(NodeId),
}

/// The common functionality of subnetwork endpoints.
pub trait SubnetworkEndpoint {
    /// The subnetwork name.
    fn subnetwork() -> &'static str;
}

impl SubnetworkEndpoint for StateEndpoint {
    fn subnetwork() -> &'static str {
        "state"
    }
}

impl SubnetworkEndpoint for HistoryEndpoint {
    fn subnetwork() -> &'static str {
        "history"
    }
}

impl SubnetworkEndpoint for BeaconEndpoint {
    fn subnetwork() -> &'static str {
        "beacon"
    }
}
