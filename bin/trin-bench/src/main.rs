use std::{path::Path, sync::Arc, thread::sleep, time::Instant};

use clap::Parser;
use e2store::{era1::Era1, utils::get_era1_files};
use ethportal_api::{
    types::portal_wire::OfferTrace, BlockBodyLegacy, ContentValue, Discv5ApiClient, Enr,
    HistoryContentKey, HistoryContentValue, HistoryNetworkApiClient, Receipts,
};
use futures::future::join_all;
use humanize_duration::{prelude::DurationExt, Truncate};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use portal_bridge::{
    api::execution::construct_proof,
    bridge::{history::SERVE_BLOCK_TIMEOUT, utils::lookup_epoch_acc},
};
use reqwest::Client;
use tokio::{
    fs::{create_dir_all, read, File},
    io::AsyncWriteExt,
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::timeout,
};
use tracing::{debug, error, info, warn, Instrument};
use trin_bench::{
    benchmarks::{get::BenchGet, put::BenchPut, utils::get_block_range_from_era1},
    cli::{BenchMode, TrinBenchConfig},
};
use trin_execution::era::utils::download_raw_era;
use trin_utils::log::init_tracing_logger;
use trin_validation::{constants::EPOCH_SIZE, oracle::HeaderOracle};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing_logger();

    // Sleep for a bit to allow the node to start up
    sleep(std::time::Duration::from_secs(10));

    let trin_bench_config = TrinBenchConfig::parse();
    let send_node_client = HttpClientBuilder::default()
        .build(trin_bench_config.web3_http_address_node_1.clone())
        .expect("Failed to build send_node_client");

    let receiver_node_client = HttpClientBuilder::default()
        .build(trin_bench_config.web3_http_address_node_2.clone())
        .expect("Failed to build receiver_node_client");

    let http_client = Client::new();
    let blocks = get_block_range_from_era1(
        trin_bench_config.start_era1,
        trin_bench_config.end_era1,
        trin_bench_config.epoch_acc_path.clone(),
        http_client.clone(),
    )
    .await?;

    info!("Beginning benchmark");

    match trin_bench_config.bench_mode {
        BenchMode::Put => {
            let bench_put = BenchPut {
                send_node_client,
                receiver_node_client,
                blocks,
                trin_bench_config,
            };
            bench_put.run().await?;
        }
        BenchMode::Get => {
            let bench_get = BenchGet {
                send_node_client,
                receiver_node_client,
                blocks,
                trin_bench_config,
            };
            bench_get.run().await?;
        }
    }

    Ok(())
}
