use std::path::{Path, PathBuf};

use e2store::{
    era1::{BlockTuple, Era1},
    utils::get_era1_files,
};
use ethportal_api::{
    types::{portal::FindContentInfo, portal_wire::OfferTrace},
    ContentValue, Enr, HistoryContentKey, HistoryContentValue, HistoryNetworkApiClient,
};
use jsonrpsee::http_client::HttpClient;
use portal_bridge::bridge::history::SERVE_BLOCK_TIMEOUT;
use reqwest::Client;
use tokio::{
    fs::{create_dir_all, read, File},
    io::AsyncWriteExt,
    sync::OwnedSemaphorePermit,
    task::JoinHandle,
    time::timeout,
};
use tracing::{debug, error, warn, Instrument};
use trin_execution::era::utils::download_raw_era;

pub async fn get_block_range_from_era1(
    start_era1: u16,
    end_era1: u16,
    local_cache_dir: PathBuf,
    http_client: Client,
) -> Result<Vec<BlockTuple>, anyhow::Error> {
    let mut blocks = vec![];
    for era1_index in start_era1..=end_era1 {
        let era1_files = get_era1_files(&http_client).await?;
        let local_cache_dir = "./logs/era1_cache"; // Define the local cache directory

        // Ensure the cache directory exists
        create_dir_all(local_cache_dir).await?;
        let era1_path = era1_files[&(era1_index as u64)].clone();
        let file_name = format!("era1_{}.bin", era1_index);
        let local_file_path = format!("{}/{}", local_cache_dir, file_name);

        let raw_era1 = if Path::new(&local_file_path).exists() {
            // Load from disk if already downloaded
            println!("Loading {} from disk", local_file_path);
            read(&local_file_path).await?
        } else {
            // Download and save to disk if not available
            println!("Downloading {}", era1_path);
            let data = download_raw_era(era1_path, http_client.clone()).await?;
            let mut file = File::create(&local_file_path).await?;
            file.write_all(&data).await?;
            data.to_vec()
        };

        let block_tuples = Era1::deserialize(&raw_era1)?;
        blocks.extend(block_tuples.block_tuples);
    }
    Ok(blocks)
}

pub fn spawn_serve_history_content(
    portal_client: HttpClient,
    enr: Enr,
    content_key: HistoryContentKey,
    content_value: HistoryContentValue,
    permit: Option<OwnedSemaphorePermit>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        match timeout(
            SERVE_BLOCK_TIMEOUT,
            offer_content(&portal_client, enr, content_key.clone(), content_value)
                .in_current_span(),
        )
        .await {
            Ok(result) => match result {
                Ok(_) => debug!("Successfully served block: {content_key:?}"),
                Err(msg) => warn!("Error serving block: {content_key:?}: {msg:?}"),
            },
            Err(_) => error!("serve_full_block() timed out on height {content_key:?}: this is an indication a bug is present")
        };
        if let Some(permit) = permit {
            drop(permit);
        }
    })
}

pub async fn offer_content(
    client: &HttpClient,
    enr: Enr,
    content_key: HistoryContentKey,
    content_value: HistoryContentValue,
) -> anyhow::Result<()> {
    let result = HistoryNetworkApiClient::trace_offer(
        client,
        enr.clone(),
        content_key.clone(),
        content_value.encode(),
    )
    .await?;
    if OfferTrace::Declined == result || OfferTrace::Failed == result {
        warn!("Error offering content: {enr:?} ||| {result:?}");
    }

    Ok(())
}

pub fn spawn_find_content(
    portal_client: HttpClient,
    enr: Enr,
    content_key: HistoryContentKey,
    permit: Option<OwnedSemaphorePermit>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        match timeout(
            SERVE_BLOCK_TIMEOUT,
            find_content(&portal_client, enr, content_key.clone())
                .in_current_span(),
        )
        .await {
            Ok(result) => match result {
                Ok(_) => debug!("Successfully served block: {content_key:?}"),
                Err(msg) => warn!("Error serving block: {content_key:?}: {msg:?}"),
            },
            Err(_) => error!("serve_full_block() timed out on height {content_key:?}: this is an indication a bug is present")
        };
        if let Some(permit) = permit {
            drop(permit);
        }
    })
}

pub async fn find_content(
    client: &HttpClient,
    enr: Enr,
    content_key: HistoryContentKey,
) -> anyhow::Result<()> {
    let result =
        HistoryNetworkApiClient::find_content(client, enr.clone(), content_key.clone()).await?;
    if let FindContentInfo::Enrs { .. } = result {
        warn!("Error finding content: {enr:?} ||| {result:?}");
    }

    Ok(())
}
