use crate::utils::wait_for_content;
use crate::Peertest;
use ethportal_api::jsonrpsee::http_client::HttpClient;
use ethportal_api::PossibleHistoryContentValue;
use ethportal_api::{HistoryContentKey, HistoryContentValue};
use portal_bridge::bridge::Bridge;
use portal_bridge::mode::BridgeMode;
use serde_json::Value;
use tokio::time::{sleep, Duration};
use trin_validation::accumulator::MasterAccumulator;
use trin_validation::oracle::HeaderOracle;

pub async fn test_bridge(peertest: &Peertest, target: &HttpClient) {
    let master_acc = MasterAccumulator::default();
    let header_oracle = HeaderOracle::new(master_acc);
    let portal_clients = vec![target.clone()];
    let epoch_acc_path = "validation_assets/epoch_acc.bin".into();
    let bridge_data_path = "./test_assets/portalnet/bridge_data.json";
    let mode = BridgeMode::Test(bridge_data_path.into());
    // Wait for bootnode to start
    sleep(Duration::from_secs(1)).await;
    let bridge = Bridge::new(mode, portal_clients, header_oracle, epoch_acc_path);
    bridge.launch().await;

    let test_item = std::fs::read_to_string(bridge_data_path).unwrap();
    let test_item: Value = serde_json::from_str(&test_item).unwrap();
    let test_item = test_item.as_array().unwrap()[0].as_object().unwrap();
    let content_key = test_item.get("content_key").unwrap();
    let content_key = serde_json::from_value::<HistoryContentKey>(content_key.clone()).unwrap();
    let content_value = test_item.get("content_value").unwrap();
    let content_value =
        serde_json::from_value::<HistoryContentValue>(content_value.clone()).unwrap();
    // Check if the stored content value in bootnode's DB matches the offered
    let response = wait_for_content(&peertest.bootnode.ipc_client, content_key).await;
    let received_content_value = match response {
        PossibleHistoryContentValue::ContentPresent(c) => c,
        PossibleHistoryContentValue::ContentAbsent => panic!("Expected content to be found"),
    };
    assert_eq!(
        content_value, received_content_value,
        "The received content {received_content_value:?}, must match the expected {content_value:?}",
    );
}
