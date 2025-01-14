pub mod types;

use std::cmp::Ordering;

use anyhow::anyhow;
use ethportal_api::consensus::beacon_block::{BeaconBlockDeneb, SignedBeaconBlockDeneb};
use reqwest::Client;
use tracing::info;
use types::Syncing;
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

    pub async fn fetch_syncing_status(&self) -> anyhow::Result<Syncing> {
        let url = self.beacon_api_endpoint.join("/eth/v1/node/syncing")?;
        let response = self.http_client.get(url).send().await?;
        let mut response_json: serde_json::Value = response.json().await?;
        let syncing: Syncing = serde_json::from_value(response_json["data"].take())?;

        Ok(syncing)
    }

    pub async fn get_block_by_slot(&self, slot: u64) -> anyhow::Result<Option<BeaconBlockDeneb>> {
        let url = self
            .beacon_api_endpoint
            .join(&format!("/eth/v2/beacon/blocks/{}", slot))
            .map_err(|err| anyhow!("failed to generate url {err}"))?;
        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|err| anyhow!("failed to get response from consensus client {err}"))?;

        // The consensus client will return a 404 error if the slot was skipped.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|err| anyhow!("failed to decode json {err}"))?;
        let beacon_block: SignedBeaconBlockDeneb =
            serde_json::from_value(response_json["data"].clone())
                .map_err(|err| anyhow!("failed to decode deneb block {err} {response_json}"))?;
        // .map_err(|err| anyhow!("failed to decode deneb block {err}"))?;

        Ok(Some(beacon_block.message))
    }

    pub async fn get_slot_number_from_execution_block_number(
        &self,
        starting_slot_number: u64,
        block_number: u64,
    ) -> anyhow::Result<u64> {
        let mut start_slot_number = starting_slot_number;
        let mut end_slot_number = self.fetch_syncing_status().await?.head_slot;
        info!(
            "Fetching block number {} between slots {} and {}",
            block_number, start_slot_number, end_slot_number
        );

        while start_slot_number <= end_slot_number {
            let mid = (start_slot_number + end_slot_number) / 2;
            match self.get_block_by_slot(mid).await {
                Ok(Some(block)) => {
                    match block.body.execution_payload.block_number.cmp(&block_number) {
                        Ordering::Equal => return Ok(mid),
                        Ordering::Greater => end_slot_number = mid - 1,
                        Ordering::Less => start_slot_number = mid + 1,
                    }
                }
                Ok(None) => return Err(anyhow!("Block not found")),
                Err(_) => {
                    start_slot_number += 1;
                }
            }
        }

        Err(anyhow!("Block not found"))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    #[ignore = "requires a running beacon node"]
    async fn test_get_slot_number_from_execution_block_number() {
        let http_client = Client::new();
        let beacon_api_endpoint = Url::parse("http://127.0.0.1:3500").unwrap();
        let consensus_block_fetcher = ConsensusBlockFetcher {
            http_client,
            beacon_api_endpoint,
        };
        let starting_slot_number = 10039226;
        let block_number = 21048889;
        let slot_number = consensus_block_fetcher
            .get_slot_number_from_execution_block_number(starting_slot_number, block_number)
            .await
            .unwrap();
        assert_eq!(slot_number, 10259226);
    }

    #[tokio::test]
    #[ignore = "requires a running beacon node"]
    async fn test_get_block_by_slot() {
        let http_client = Client::new();
        let beacon_api_endpoint = Url::parse("http://127.0.0.1:3500").unwrap();
        let consensus_block_fetcher = ConsensusBlockFetcher {
            http_client,
            beacon_api_endpoint,
        };

        let slot = 10232841;
        let _block = consensus_block_fetcher
            .get_block_by_slot(slot)
            .await
            .unwrap();
    }
}
