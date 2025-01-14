use std::sync::Arc;

use async_trait::async_trait;
use discv5::enr::NodeId;
use ethportal_api::{
    types::{network::Subnetwork, portal::PongInfo},
    Enr,
};
use portal_bridge::census::rpc::PortalCensusRpc;
use tokio::sync::RwLock;
use trin_validation::oracle::HeaderOracle;

#[derive(Clone)]
pub struct RpcHeaderOracle {
    pub header_oracle: Arc<RwLock<HeaderOracle>>,
}

impl RpcHeaderOracle {
    pub fn new(header_oracle: Arc<RwLock<HeaderOracle>>) -> Self {
        Self { header_oracle }
    }
}

#[async_trait]
impl PortalCensusRpc for RpcHeaderOracle {
    async fn ping(&self, enr: Enr, network: Subnetwork) -> anyhow::Result<PongInfo> {
        match network {
            Subnetwork::History => self.header_oracle.read().await.history_ping(enr).await,
            Subnetwork::State => self.header_oracle.read().await.state_ping(enr).await,
            Subnetwork::Beacon => self.header_oracle.read().await.beacon_ping(enr).await,
            _ => unreachable!("ping: unsupported subnetwork: {}", network),
        }
    }

    async fn find_nodes(
        &self,
        enr: Enr,
        distances: Vec<u16>,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>> {
        match network {
            Subnetwork::History => {
                self.header_oracle
                    .read()
                    .await
                    .history_find_nodes(enr, distances)
                    .await
            }
            Subnetwork::State => {
                self.header_oracle
                    .read()
                    .await
                    .state_find_nodes(enr, distances)
                    .await
            }
            Subnetwork::Beacon => {
                self.header_oracle
                    .read()
                    .await
                    .beacon_find_nodes(enr, distances)
                    .await
            }
            _ => unreachable!("find_nodes: unsupported subnetwork: {}", network),
        }
    }

    async fn recursive_find_nodes(
        &self,
        node_id: NodeId,
        network: Subnetwork,
    ) -> anyhow::Result<Vec<Enr>> {
        match network {
            Subnetwork::History => {
                self.header_oracle
                    .read()
                    .await
                    .history_recursive_find_nodes(node_id)
                    .await
            }
            Subnetwork::State => {
                self.header_oracle
                    .read()
                    .await
                    .state_recursive_find_nodes(node_id)
                    .await
            }
            Subnetwork::Beacon => {
                self.header_oracle
                    .read()
                    .await
                    .beacon_recursive_find_nodes(node_id)
                    .await
            }
            _ => unreachable!("recursive_find_nodes: unsupported subnetwork: {}", network),
        }
    }
}
