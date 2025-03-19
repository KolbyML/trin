use std::{
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

use e2store::era1::BlockTuple;
use ethportal_api::{
    BlockBodyLegacy, ContentValue, Discv5ApiClient, Enr, HistoryContentKey, HistoryContentValue,
    HistoryNetworkApiClient, Receipts,
};
use futures::future::join_all;
use humanize_duration::{prelude::DurationExt, Truncate};
use jsonrpsee::http_client::HttpClient;
use portal_bridge::{api::execution::construct_proof, bridge::utils::lookup_epoch_acc};
use tokio::sync::Semaphore;
use tracing::info;
use trin_validation::{constants::EPOCH_SIZE, oracle::HeaderOracle};

use crate::{
    benchmarks::utils::{spawn_find_content, spawn_serve_history_content},
    cli::TrinBenchConfig,
};

pub struct BenchGet {
    pub send_node_client: HttpClient,
    pub receiver_node_client: HttpClient,
    pub blocks: Vec<BlockTuple>,
    pub trin_bench_config: TrinBenchConfig,
}

impl BenchGet {
    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Preparring get benchmark");
        let sender_node_enr = match self.send_node_client.node_info().await {
            Ok(node_info) => node_info.enr,
            Err(err) => panic!("Error getting sender_node_enr: {err:?}"),
        };

        // ping receiver node, to exchange radius's, as if we just start with offers, the other node
        // will assume a 100% radius by default
        HistoryNetworkApiClient::ping(&self.send_node_client, sender_node_enr.clone()).await?;
        let mut stored_content = 0;

        let header_oracle = HeaderOracle::default();

        let mut epoch_acc = None;
        let mut current_epoch_index = u64::MAX;

        info!("Starting test 1");
        // gossip headers
        for block in self.blocks.clone() {
            let height = block.header.header.number;
            // Using epoch_size chunks & epoch boundaries ensures that every
            // "chunk" shares an epoch accumulator avoiding the need to
            // look up the epoch acc on a header by header basis
            if current_epoch_index != height / EPOCH_SIZE {
                current_epoch_index = height / EPOCH_SIZE;
                epoch_acc = match lookup_epoch_acc(
                    current_epoch_index,
                    &header_oracle.header_validator.pre_merge_acc,
                    &self.trin_bench_config.epoch_acc_path,
                )
                .await
                {
                    Ok(epoch_acc) => Some(epoch_acc),
                    Err(msg) => {
                        unreachable!("Error looking up epoch acc: {msg:?}");
                    }
                };
            }

            let content_key =
                HistoryContentKey::new_block_header_by_hash(block.header.header.hash());
            let content_value = if let Some(epoch_acc) = &epoch_acc {
                // Construct HeaderWithProof
                let header_with_proof =
                    construct_proof(block.header.header.clone(), epoch_acc).await?;
                // Double check that the proof is valid
                header_oracle
                    .header_validator
                    .validate_header_with_proof(&header_with_proof)?;
                HistoryContentValue::BlockHeaderWithProof(header_with_proof)
            } else {
                unreachable!("epoch_acc should be Some(epoch_acc) at this point");
            };

            HistoryNetworkApiClient::store(
                &self.send_node_client,
                content_key,
                content_value.encode(),
            )
            .await?;

            stored_content += 1;
        }

        info!("Starting test 2");

        // gossip bodies
        for block in self.blocks.clone() {
            let content_key = HistoryContentKey::new_block_body(block.header.header.hash());
            let content_value =
                HistoryContentValue::BlockBody(ethportal_api::BlockBody::Legacy(BlockBodyLegacy {
                    txs: block.body.body.transactions().to_vec(),
                    uncles: block.body.body.uncles().to_vec(),
                }));
            HistoryNetworkApiClient::store(
                &self.send_node_client,
                content_key,
                content_value.encode(),
            )
            .await?;
            stored_content += 1;
        }

        info!("Starting test 3");

        // gossip receipts
        for block in self.blocks.clone() {
            let content_key = HistoryContentKey::new_block_receipts(block.header.header.hash());
            let content_value = HistoryContentValue::Receipts(Receipts {
                receipt_list: block.receipts.receipts.receipt_list,
            });
            HistoryNetworkApiClient::store(
                &self.send_node_client,
                content_key,
                content_value.encode(),
            )
            .await?;
            stored_content += 1;
        }

        info!("Starting test 4");

        info!("Starting benchmark with {stored_content} stored content");

        let start_timer = Instant::now();

        let mut content_fetched = 0;

        // gossip blocks to receiver node
        let gossip_semaphore = Arc::new(Semaphore::new(self.trin_bench_config.offer_concurrency));

        let mut serve_full_block_handles = vec![];

        // gossip headers
        for block in self.blocks.clone() {
            let height = block.header.header.number;
            // Using epoch_size chunks & epoch boundaries ensures that every
            // "chunk" shares an epoch accumulator avoiding the need to
            // look up the epoch acc on a header by header basis
            if current_epoch_index != height / EPOCH_SIZE {
                current_epoch_index = height / EPOCH_SIZE;
                epoch_acc = match lookup_epoch_acc(
                    current_epoch_index,
                    &header_oracle.header_validator.pre_merge_acc,
                    &self.trin_bench_config.epoch_acc_path,
                )
                .await
                {
                    Ok(epoch_acc) => Some(epoch_acc),
                    Err(msg) => {
                        unreachable!("Error looking up epoch acc: {msg:?}");
                    }
                };
            }

            let permit = gossip_semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("to be able to acquire semaphore");

            let content_key =
                HistoryContentKey::new_block_header_by_hash(block.header.header.hash());
            serve_full_block_handles.push(spawn_find_content(
                self.receiver_node_client.clone(),
                sender_node_enr.clone(),
                content_key,
                Some(permit),
            ));
            content_fetched += 1;
        }

        // gossip bodies
        for block in self.blocks.clone() {
            let permit = gossip_semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("to be able to acquire semaphore");

            let content_key = HistoryContentKey::new_block_body(block.header.header.hash());
            serve_full_block_handles.push(spawn_find_content(
                self.receiver_node_client.clone(),
                sender_node_enr.clone(),
                content_key,
                Some(permit),
            ));
            content_fetched += 1;
        }

        // gossip receipts
        for block in self.blocks.clone() {
            let permit = gossip_semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("to be able to acquire semaphore");

            let content_key = HistoryContentKey::new_block_receipts(block.header.header.hash());
            serve_full_block_handles.push(spawn_find_content(
                self.receiver_node_client.clone(),
                sender_node_enr.clone(),
                content_key,
                Some(permit),
            ));
            content_fetched += 1;
        }

        // Wait till all blocks are done gossiping.
        // This can't deadlock, because the tokio::spawn has a timeout.
        join_all(serve_full_block_handles).await;

        info!(
            "Benchmark completed in {} with {} content fetched",
            start_timer.elapsed().human(Truncate::Second),
            content_fetched
        );

        sleep(Duration::from_secs(60));
        Ok(())
    }
}
