use std::{sync::Arc, time::Instant};

use e2store::era1::BlockTuple;
use ethportal_api::{
    BlockBodyLegacy, Discv5ApiClient, Enr, HistoryContentKey, HistoryContentValue,
    HistoryNetworkApiClient, Receipts,
};
use futures::future::join_all;
use humanize_duration::{prelude::DurationExt, Truncate};
use jsonrpsee::http_client::HttpClient;
use portal_bridge::{api::execution::construct_proof, bridge::utils::lookup_epoch_acc};
use tokio::sync::Semaphore;
use tracing::info;
use trin_validation::{constants::EPOCH_SIZE, oracle::HeaderOracle};

use crate::{benchmarks::utils::spawn_serve_history_content, cli::TrinBenchConfig};

pub struct BenchPut {
    pub send_node_client: HttpClient,
    pub receiver_node_client: HttpClient,
    pub blocks: Vec<BlockTuple>,
    pub trin_bench_config: TrinBenchConfig,
}

impl BenchPut {
    pub async fn run(&self) -> anyhow::Result<()> {
        let receiver_node_enr = match self.receiver_node_client.node_info().await {
            Ok(node_info) => node_info.enr,
            Err(err) => panic!("Error getting receiver_node_enr: {err:?}"),
        };

        // ping receiver node, to exchange radius's, as if we just start with offers, the other node
        // will assume a 100% radius by default
        HistoryNetworkApiClient::ping(&self.send_node_client, receiver_node_enr.clone()).await?;

        let start_timer = Instant::now();
        let mut offer_count = 0;

        // gossip blocks to receiver node
        let gossip_semaphore = Arc::new(Semaphore::new(self.trin_bench_config.offer_concurrency));
        let header_oracle = HeaderOracle::default();

        let mut epoch_acc = None;
        let mut serve_full_block_handles = vec![];
        let mut current_epoch_index = u64::MAX;
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
            let content_value = if let Some(epoch_acc) = &epoch_acc {
                // Construct HeaderWithProof
                let header_with_proof =
                    construct_proof(block.header.header.clone(), epoch_acc).await?;
                HistoryContentValue::BlockHeaderWithProof(header_with_proof)
            } else {
                unreachable!("epoch_acc should be Some(epoch_acc) at this point");
            };
            serve_full_block_handles.push(spawn_serve_history_content(
                self.send_node_client.clone(),
                receiver_node_enr.clone(),
                content_key,
                content_value,
                Some(permit),
            ));
            offer_count += 1;
        }

        // gossip bodies
        for block in self.blocks.clone() {
            let permit = gossip_semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("to be able to acquire semaphore");

            let content_key = HistoryContentKey::new_block_body(block.header.header.hash());
            let content_value =
                HistoryContentValue::BlockBody(ethportal_api::BlockBody::Legacy(BlockBodyLegacy {
                    txs: block.body.body.transactions().to_vec(),
                    uncles: block.body.body.uncles().to_vec(),
                }));
            serve_full_block_handles.push(spawn_serve_history_content(
                self.send_node_client.clone(),
                receiver_node_enr.clone(),
                content_key,
                content_value,
                Some(permit),
            ));
            offer_count += 1;
        }

        // gossip receipts
        for block in self.blocks.clone() {
            let permit = gossip_semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("to be able to acquire semaphore");

            let content_key = HistoryContentKey::new_block_receipts(block.header.header.hash());
            let content_value = HistoryContentValue::Receipts(Receipts {
                receipt_list: block.receipts.receipts.receipt_list,
            });
            serve_full_block_handles.push(spawn_serve_history_content(
                self.send_node_client.clone(),
                receiver_node_enr.clone(),
                content_key,
                content_value,
                Some(permit),
            ));
            offer_count += 1;
        }

        // Wait till all blocks are done gossiping.
        // This can't deadlock, because the tokio::spawn has a timeout.
        join_all(serve_full_block_handles).await;

        info!(
            "Finished gossiping blocks in {}, with {} offers",
            start_timer.elapsed().human(Truncate::Second),
            offer_count
        );

        Ok(())
    }
}
