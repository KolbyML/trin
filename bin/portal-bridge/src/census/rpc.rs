use anyhow::anyhow;
use async_trait::async_trait;
use discv5::enr::NodeId;
use ethportal_api::{
    types::{network::Subnetwork, portal::PongInfo},
    BeaconNetworkApiClient, Enr, HistoryNetworkApiClient, StateNetworkApiClient,
};
use jsonrpsee::http_client::HttpClient;

#[async_trait]
pub trait PortalCensusRpc: Clone + Send + Sync + 'static {
    async fn ping(&self, enr: Enr, network: Subnetwork) -> anyhow::Result<PongInfo>;

    async fn find_nodes(
        &self,
        enr: Enr,
        distances: Vec<u16>,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>>;

    async fn recursive_find_nodes(
        &self,
        node_id: NodeId,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>>;
}

#[derive(Clone)]

pub struct CensusHttpClient {
    client: HttpClient,
}

impl CensusHttpClient {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PortalCensusRpc for CensusHttpClient {
    async fn ping(&self, enr: Enr, network: Subnetwork) -> anyhow::Result<PongInfo> {
        match network {
            Subnetwork::History => HistoryNetworkApiClient::ping(&self.client, enr.clone()),
            Subnetwork::State => StateNetworkApiClient::ping(&self.client, enr.clone()),
            Subnetwork::Beacon => BeaconNetworkApiClient::ping(&self.client, enr.clone()),
            _ => unreachable!("ping: unsupported subnetwork: {}", network),
        }
        .await
        .map_err(|err| anyhow!(err))
    }

    async fn find_nodes(
        &self,
        enr: Enr,
        distances: Vec<u16>,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>> {
        match network {
            Subnetwork::History => {
                HistoryNetworkApiClient::find_nodes(&self.client, enr, distances)
            }
            Subnetwork::State => StateNetworkApiClient::find_nodes(&self.client, enr, distances),
            Subnetwork::Beacon => BeaconNetworkApiClient::find_nodes(&self.client, enr, distances),
            _ => unreachable!("find_nodes: unsupported subnetwork: {}", network),
        }
        .await
        .map_err(|err| anyhow!(err))
    }

    async fn recursive_find_nodes(
        &self,
        node_id: NodeId,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>> {
        match network {
            Subnetwork::History => {
                HistoryNetworkApiClient::recursive_find_nodes(&self.client, node_id)
            }
            Subnetwork::State => StateNetworkApiClient::recursive_find_nodes(&self.client, node_id),
            Subnetwork::Beacon => {
                BeaconNetworkApiClient::recursive_find_nodes(&self.client, node_id)
            }
            _ => unreachable!("recursive_find_nodes: unsupported subnetwork: {}", network),
        }
        .await
        .map_err(|err| anyhow!(err))
    }
}
