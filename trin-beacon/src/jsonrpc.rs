use std::sync::Arc;

use discv5::enr::NodeId;
use ethportal_api::types::content_key::{BeaconContentKey, OverlayContentKey};
use ethportal_api::types::content_value::{BeaconContentValue, ContentValue};
use ethportal_api::types::jsonrpc::endpoints::BeaconEndpoint;
use ethportal_api::types::jsonrpc::request::BeaconJsonRpcRequest;
use ethportal_api::types::portal::{AcceptInfo, FindNodesInfo, PongInfo, TraceContentInfo};
use ethportal_api::types::{
    constants::CONTENT_ABSENT,
    content_key::RawContentKey,
    distance::{Metric, XorMetric},
    enr::Enr,
    query_trace::QueryTrace
};
use ethportal_api::utils::bytes::hex_encode;
use portalnet::storage::ContentStore;
use serde_json::{json, Value};
use ssz::Encode;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::error;
use portalnet::jsonrpc;

use crate::network::BeaconNetwork;

/// Handles Beacon network JSON-RPC requests
pub struct BeaconRequestHandler {
    pub network: Arc<RwLock<BeaconNetwork>>,
    pub rpc_rx: Arc<Mutex<mpsc::UnboundedReceiver<BeaconJsonRpcRequest>>>,
}

impl BeaconRequestHandler {
    /// Complete RPC requests for the Beacon network.
    pub async fn handle_client_queries(&self) {
        let rpc_rx = self.rpc_rx.clone();
        while let Some(request) = rpc_rx.lock().await.recv().await {
            let network = self.network.clone();
            tokio::spawn(async move { complete_request(network, request).await });
        }
    }
}

/// Generates a response for a given request and sends it to the receiver.
async fn complete_request(network: Arc<RwLock<BeaconNetwork>>, request: BeaconJsonRpcRequest) {
    let response: Result<Value, String> = match request.endpoint {
        BeaconEndpoint::LocalContent(content_key) => local_content(network, content_key).await,
        BeaconEndpoint::PaginateLocalContentKeys(offset, limit) => {
            jsonrpc::paginate_local_content_keys(network, offset, limit).await
        }
        BeaconEndpoint::Store(content_key, content_value) => {
            jsonrpc::store(network, content_key, content_value).await
        }
        BeaconEndpoint::RecursiveFindContent(content_key) => {
            jsonrpc::recursive_find_content(network, content_key, false).await
        }
        BeaconEndpoint::TraceRecursiveFindContent(content_key) => {
            jsonrpc::recursive_find_content(network, content_key, true).await
        }
        BeaconEndpoint::AddEnr(enr) => jsonrpc::add_enr(network, enr).await,
        BeaconEndpoint::DataRadius => {
            let radius = network.read().await.overlay.data_radius();
            Ok(json!(*radius))
        }
        BeaconEndpoint::DeleteEnr(node_id) => delete_enr(network, node_id).await,
        BeaconEndpoint::FindContent(enr, content_key) => {
            jsonrpc::find_content(network, enr, content_key).await
        }
        BeaconEndpoint::FindNodes(enr, distances) => jsonrpc::find_nodes(network, enr, distances).await,
        BeaconEndpoint::GetEnr(node_id) => jsonrpc::get_enr(network, node_id).await,
        BeaconEndpoint::Gossip(content_key, content_value) => {
            jsonrpc::gossip(network, content_key, content_value).await
        }
        BeaconEndpoint::LookupEnr(node_id) => jsonrpc::lookup_enr(network, node_id).await,
        BeaconEndpoint::Offer(enr, content_key, content_value) => {
            jsonrpc::offer(network, enr, content_key, content_value).await
        }
        BeaconEndpoint::Ping(enr) => jsonrpc::ping(network, enr).await,
        BeaconEndpoint::RoutingTableInfo => Ok(json!("Not implemented")), // TODO: implement this when refactor trin_history utils
        BeaconEndpoint::RecursiveFindNodes(node_id) => jsonrpc::recursive_find_nodes(network, node_id).await,
    };
    let _ = request.resp.send(response);
}
