use ethportal_api::consensus::beacon_block::BeaconBlockDeneb;
use reqwest::Client;
use url::Url;

pub struct ConsensusBlockFetcher {
    http_client: Client,
    beacon_api_endpoint: Url,
}

impl ConsensusBlockFetcher {
    pub fn new(beacon_api_endpoint: Url) -> Self {
        let http_client = Client::new();
        Self {
            http_client,
            beacon_api_endpoint,
        }
    }

    pub async fn check_if_consensus_client_is_synced(&self) -> anyhow::Result<bool> {
        let url = self.beacon_api_endpoint.join("/eth/v1/node/syncing")?;
        let response = self.http_client.get(url).send().await?;
        let response_json: serde_json::Value = response.json().await?;
        let Some(is_syncing) = response_json["data"]["is_syncing"].as_bool() else {
            return Err(anyhow::anyhow!(
                "Failed to get is_syncing from consensus client"
            ));
        };

        Ok(is_syncing)
    }

    pub async fn get_block_by_slot(&self, slot: u64) -> anyhow::Result<Option<BeaconBlockDeneb>> {
        let url = self
            .beacon_api_endpoint
            .join(&format!("/eth/v2/beacon/blocks/{}", slot))?;
        let response = self.http_client.get(url).send().await?;

        // The consensus client will return a 404 error if the slot was skipped.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response_json: serde_json::Value = response.json().await?;
        let beacon_block: BeaconBlockDeneb = serde_json::from_value(response_json)?;

        Ok(Some(beacon_block))
    }
}
