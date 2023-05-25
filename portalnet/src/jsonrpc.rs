use std::sync::Arc;

use ethportal_api::utils::bytes::hex_encode;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tracing::error;

use discv5::enr::NodeId;
use ethportal_api::types::constants::CONTENT_ABSENT;
use ethportal_api::types::content_key::RawContentKey;
use ethportal_api::types::query_trace::QueryTrace;
use ethportal_api::types::portal::PongInfo;
use ethportal_api::types::portal::AcceptInfo;
use ethportal_api::types::portal::FindNodesInfo;
use ethportal_api::types::portal::TraceContentInfo;

// /// Handles History network JSON-RPC requests
// pub struct HistoryRequestHandler {
//     pub network: Arc<RwLock<HistoryNetwork>>,
//     pub history_rx: Arc<Mutex<mpsc::UnboundedReceiver<HistoryJsonRpcRequest>>>,
// }
//
// impl HistoryRequestHandler {
//     /// Complete RPC requests for the History network.
//     pub async fn handle_client_queries(&self) {
//         let history_rx = self.history_rx.clone();
//         while let Some(request) = history_rx.lock().await.recv().await {
//             let network = self.network.clone();
//             tokio::spawn(async move { complete_request(network, request).await });
//         }
//     }
// }
//
// /// Generates a response for a given request and sends it to the receiver.
// async fn complete_request<Network, JsonRpcRequest, Endpoint>(network: Arc<RwLock<Network>>, request: JsonRpcRequest) {
//     let response: Result<Value, String> = match request.endpoint {
//         Endpoint::LocalContent(content_key) => local_content(network, content_key).await,
//         Endpoint::PaginateLocalContentKeys(offset, limit) => {
//             paginate_local_content_keys(network, offset, limit).await
//         }
//         Endpoint::Store(content_key, content_value) => {
//             store(network, content_key, content_value).await
//         }
//         Endpoint::RecursiveFindContent(content_key) => {
//             recursive_find_content(network, content_key, false).await
//         }
//         Endpoint::TraceRecursiveFindContent(content_key) => {
//             recursive_find_content(network, content_key, true).await
//         }
//         Endpoint::AddEnr(enr) => add_enr(network, enr).await,
//         Endpoint::DataRadius => {
//             let radius = network.read().await.overlay.data_radius();
//             Ok(json!(*radius))
//         }
//         Endpoint::DeleteEnr(node_id) => delete_enr(network, node_id).await,
//         Endpoint::FindContent(enr, content_key) => {
//             find_content(network, enr, content_key).await
//         }
//         Endpoint::FindNodes(enr, distances) => find_nodes(network, enr, distances).await,
//         Endpoint::GetEnr(node_id) => get_enr(network, node_id).await,
//         Endpoint::Gossip(content_key, content_value) => {
//             gossip(network, content_key, content_value).await
//         }
//         Endpoint::LookupEnr(node_id) => lookup_enr(network, node_id).await,
//         Endpoint::Offer(enr, content_key, content_value) => {
//             offer(network, enr, content_key, content_value).await
//         }
//         Endpoint::Ping(enr) => ping(network, enr).await,
//         Endpoint::RoutingTableInfo => Ok(bucket_entries_to_json(
//             network.read().await.overlay.bucket_entries(),
//         )),
//         Endpoint::RecursiveFindNodes(node_id) => {
//             recursive_find_nodes(network, node_id).await
//         }
//     };
//     let _ = request.resp.send(response);
// }

// /// Constructs a JSON call for the RecursiveFindContent method.
// pub async fn recursive_find_content<Network: crate::network::Network, ContentKey: std::fmt::Display + std::clone::Clone + ethportal_api::OverlayContentKey>(
//     network: Arc<RwLock<Network>>,
//     content_key: ContentKey,
//     is_trace: bool,
// ) -> Result<Value, String> {
//     // Check whether we have the data locally.
//     let overlay = network.read().await.overlay();
//     let local_content: Option<Vec<u8>> = match network.read().await.store.read().get(&content_key) {
//         Ok(Some(data)) => Some(data),
//         Ok(None) => None,
//         Err(err) => {
//             error!(
//                 error = %err,
//                 content.key = %content_key,
//                 "Error checking data store for content",
//             );
//             None
//         }
//     };
//     let (possible_content_bytes, trace) = match local_content {
//         Some(val) => {
//             let local_enr = overlay.local_enr();
//             let mut trace = QueryTrace::new(
//                 &overlay.local_enr(),
//                 NodeId::new(&content_key.content_id()).into(),
//             );
//             trace.node_responded_with_content(&local_enr);
//             (Some(val), if is_trace { Some(trace) } else { None })
//         }
//         None => overlay.lookup_content(content_key.clone(), is_trace).await,
//     };
//
//     // Format as string.
//     let content_response_string = match possible_content_bytes {
//         Some(bytes) => Value::String(hex_encode(bytes)),
//         None => Value::String(CONTENT_ABSENT.to_string()), // "0x"
//     };
//
//     // If tracing is not required, return content.
//     if !is_trace {
//         return Ok(content_response_string);
//     }
//     if let Some(trace) = trace {
//         Ok(json!(TraceContentInfo {
//             content: serde_json::from_value(content_response_string).map_err(|e| e.to_string())?,
//             trace,
//         }))
//     } else {
//         Err("Content query trace requested but none provided.".to_owned())
//     }
// }

// /// Constructs a JSON call for the LocalContent method.
// async fn local_content<Network: crate::network::Network, ContentKey: std::fmt::Debug>(
//     network: Arc<RwLock<Network>>,
//     content_key: ContentKey,
// ) -> Result<Value, String> {
//     let store = network.read().await.overlay().store.clone();
//     let response = match store.read().get(&content_key)
//         {
//             Ok(val) => match val {
//                 Some(val) => {
//                     Ok(Value::String(hex_encode(val)))
//                 }
//                 None => {
//                     Ok(Value::String(CONTENT_ABSENT.to_string()))
//                 }
//             },
//             Err(err) => Err(format!(
//                 "Database error while looking for content key in local storage: {content_key:?}, with error: {err}",
//             )),
//         };
//     response
// }

// /// Constructs a JSON call for the PaginateLocalContentKeys method.
// pub async fn paginate_local_content_keys<Network: crate::network::Network>(
//     network: Arc<RwLock<Network>>,
//     offset: u64,
//     limit: u64,
// ) -> Result<Value, String> {
//     let store = network.read().await.overlay().store.clone();
//     let response = match store.read().paginate(&offset, &limit)
//         {
//             Ok(val) => Ok(json!(val)),
//             Err(err) => Err(format!(
//                 "Database error while paginating local content keys with offset: {offset:?}, limit: {limit:?}. Error message: {err}"
//             )),
//         };
//     response
// }

// /// Constructs a JSON call for the Store method.
// pub async fn store<Network: crate::network::Network, ContentKey, ContentValue: ethportal_api::ContentValue>(
//     network: Arc<RwLock<Network>>,
//     content_key: ContentKey,
//     content_value: ContentValue,
// ) -> Result<Value, String> {
//     let data = content_value.encode();
//     let store = network.read().await.overlay().store.clone();
//     let response = match store
//         .write()
//         .put::<ContentKey, Vec<u8>>(content_key, data)
//     {
//         Ok(_) => Ok(Value::Bool(true)),
//         Err(err) => Ok(Value::String(err.to_string())),
//     };
//     response
// }

/// Constructs a JSON call for the AddEnr method.
pub async fn add_enr<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    enr: discv5::enr::Enr<discv5::enr::CombinedKey>,
) -> Result<Value, String> {
    match network.read().await.add_enr(enr) {
        Ok(_) => Ok(json!(true)),
        Err(err) => Err(format!("AddEnr failed: {err:?}")),
    }
}

/// Constructs a JSON call for the GetEnr method.
pub async fn get_enr<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    node_id: ethportal_api::NodeId,
) -> Result<Value, String> {
    let node_id = discv5::enr::NodeId::from(node_id.0);
    match network.read().await.get_enr(node_id) {
        Ok(enr) => Ok(json!(enr)),
        Err(err) => Err(format!("GetEnr failed: {err:?}")),
    }
}

/// Constructs a JSON call for the deleteEnr method.
async fn delete_enr<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    node_id: ethportal_api::NodeId,
) -> Result<Value, String> {
    let node_id = discv5::enr::NodeId::from(node_id.0);
    let is_deleted = network.read().await.delete_enr(node_id);
    Ok(json!(is_deleted))
}

/// Constructs a JSON call for the LookupEnr method.
pub async fn lookup_enr<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    node_id: ethportal_api::NodeId,
) -> Result<Value, String> {
    let node_id = discv5::enr::NodeId::from(node_id.0);
    match network.read().await.lookup_enr(node_id).await {
        Ok(enr) => Ok(json!(enr)),
        Err(err) => Err(format!("LookupEnr failed: {err:?}")),
    }
}

/// Constructs a JSON call for the FindContent method.
pub async fn find_content<Network: crate::network::Network, ContentKey>(
    network: Arc<RwLock<Network>>,
    enr: discv5::enr::Enr<discv5::enr::CombinedKey>,
    content_key: ContentKey,
) -> Result<Value, String> where Vec<u8>: std::convert::From<ContentKey> {
    match network.read().await.send_find_content(enr, content_key.into()).await {
        Ok(content) => match content.try_into() {
            Ok(val) => Ok(val),
            Err(_) => Err("Content response decoding error".to_string()),
        },
        Err(msg) => Err(format!("FindContent request timeout: {msg:?}")),
    }
}

/// Constructs a JSON call for the FindNodes method.
pub async fn find_nodes<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    enr: discv5::enr::Enr<discv5::enr::CombinedKey>,
    distances: Vec<u16>,
) -> Result<Value, String> {
    match network.read().await.send_find_nodes(enr, distances).await {
        Ok(nodes) => Ok(json!(nodes
            .enrs
            .into_iter()
            .map(|enr| enr.into())
            .collect::<FindNodesInfo>())),
        Err(msg) => Err(format!("FindNodes request timeout: {msg:?}")),
    }
}

/// Constructs a JSON call for the Gossip method.
pub async fn gossip<Network: crate::network::Network, ContentKey, ContentValue: ethportal_api::ContentValue>(
    network: Arc<RwLock<Network>>,
    content_key: ContentKey,
    content_value: ContentValue,
) -> Result<Value, String> {
    let data: Vec<u8> = content_value.encode();
    let content_values = vec![(content_key, data)];
    let num_peers = network.read().await.propagate_gossip(content_values);
    Ok(num_peers.into())
}

/// Constructs a JSON call for the Offer method.
pub async fn offer<Network: crate::network::Network, ContentKey: ssz::Encode, ContentValue: ethportal_api::ContentValue>(
    network: Arc<RwLock<Network>>,
    enr: discv5::enr::Enr<discv5::enr::CombinedKey>,
    content_key: ContentKey,
    content_value: Option<ContentValue>,
) -> Result<Value, String> where std::vec::Vec<u8>: std::convert::From<ContentKey> {
    if let Some(content_value) = content_value {
        let content_value: Vec<u8> = content_value.encode();
        match network.read().await
            .send_populated_offer(enr, content_key.into(), content_value)
            .await
        {
            Ok(accept) => Ok(json!(AcceptInfo {
                content_keys: accept.content_keys,
            })),
            Err(msg) => Err(format!("Populated Offer request timeout: {msg:?}")),
        }
    } else {
        let content_key: Vec<RawContentKey> = vec![content_key.as_ssz_bytes()];
        match network.read().await.send_offer(content_key, enr).await {
            Ok(accept) => Ok(json!(AcceptInfo {
                content_keys: accept.content_keys,
            })),
            Err(msg) => Err(format!("Offer request timeout: {msg:?}")),
        }
    }
}

/// Constructs a JSON call for the Ping method.
pub async fn ping<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    enr: discv5::enr::Enr<discv5::enr::CombinedKey>,
) -> Result<Value, String> {
    match network.read().await.send_ping(enr).await {
        Ok(pong) => Ok(json!(PongInfo {
            enr_seq: pong.enr_seq as u32,
            data_radius: *network.read().await.data_radius(),
        })),
        Err(msg) => Err(format!("Ping request timeout: {msg:?}")),
    }
}

/// Constructs a JSON call for the RecursiveFindNodes method.
pub async fn recursive_find_nodes<Network: crate::network::Network>(
    network: Arc<RwLock<Network>>,
    node_id: ethportal_api::NodeId,
) -> Result<Value, String> {
    let node_id = discv5::enr::NodeId::from(node_id.0);
    let nodes = network.read().await.lookup_node(node_id).await;
    Ok(json!(nodes))
}
