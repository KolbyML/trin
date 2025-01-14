use anyhow::ensure;
use e2store::{
    e2store::types::{Entry, Header},
    era::{get_beacon_fork, CompressedSignedBeaconBlock, SlotIndexBlockEntry, SlotIndexStateEntry},
};
use reqwest::Client;

use crate::sync::era::utils::download_range_from_file;

pub async fn get_latest_block_number_and_slot_available_from_era(
    era_path: String,
    http_client: &Client,
) -> anyhow::Result<(u64, u64)> {
    {
        // download era file length
        let response = http_client.head(&era_path).send().await?;
        let era_file_length = response
            .headers()
            .get("Content-Length")
            .expect("to be able to get content length")
            .to_str()?
            .parse::<usize>()?;

        // download slot-index-block
        let start_offset = era_file_length
            - (SlotIndexBlockEntry::SERIALIZED_SIZE + SlotIndexStateEntry::SERIALIZED_SIZE);
        let end_offset = era_file_length - SlotIndexStateEntry::SERIALIZED_SIZE - 1;

        let raw_slot_index_block =
            download_range_from_file(&era_path, start_offset, end_offset, http_client).await?;

        ensure!(
            raw_slot_index_block.len() == SlotIndexBlockEntry::SERIALIZED_SIZE,
            "Downloaded slot-index-block was not the expected size got {} expected {}",
            raw_slot_index_block.len(),
            SlotIndexBlockEntry::SERIALIZED_SIZE
        );

        let slot_index_block = Entry::deserialize(&raw_slot_index_block)?;
        let slot_index_block = SlotIndexBlockEntry::try_from(&slot_index_block)?;

        // find the position of the last block in the era file
        let beginning_of_file_offset = -(era_file_length as i64);
        let mut last_slot_number = u64::MAX;
        let mut last_offset = i64::MAX;
        for (i, offset) in slot_index_block.slot_index.indices.iter().enumerate() {
            if *offset != beginning_of_file_offset {
                last_slot_number = slot_index_block.slot_index.starting_slot + i as u64;
                last_offset = *offset;
            }
        }
        ensure!(
            last_slot_number != u64::MAX,
            "Could not find the last block in the era file"
        );
        // download last compressed beacon block in era file
        let header_start_offset = start_offset - (-last_offset) as usize;
        let raw_header = download_range_from_file(
            &era_path,
            header_start_offset,
            header_start_offset + 7,
            http_client,
        )
        .await?;

        let body_length = Header::deserialize(&raw_header)?.length as usize;

        let compressed_beacon_block = download_range_from_file(
            &era_path,
            header_start_offset,
            header_start_offset + 7 + body_length,
            http_client,
        )
        .await?;

        // process the compressed beacon block and collect the block number
        let entry = Entry::deserialize(&compressed_beacon_block)?;
        let fork = get_beacon_fork(last_slot_number);
        let beacon_block = CompressedSignedBeaconBlock::try_from(&entry, fork)?;

        Ok((
            beacon_block.block.execution_block_number(),
            beacon_block.block.slot(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use e2store::utils::get_era_files;
    use tracing::info;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_get_latest_block_number_and_slot_available_from_era() -> anyhow::Result<()> {
        let http_client = Client::new();
        let era_files = get_era_files(&http_client).await?;
        let (_, latest_era_file) = era_files.iter().max_by_key(|(key, _)| *key).unwrap();

        let block_number_and_slot = get_latest_block_number_and_slot_available_from_era(
            latest_era_file.to_string(),
            &http_client,
        )
        .await
        .unwrap();

        info!(
            "Latest block number in era file: {:?}",
            block_number_and_slot
        );

        Ok(())
    }
}
