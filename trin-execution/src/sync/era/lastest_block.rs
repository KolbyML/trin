use anyhow::ensure;
use e2store::{
    e2store::types::Entry,
    era::{
        get_beacon_fork, CompressedSignedBeaconBlock, Era, SlotIndexBlock, SlotIndexBlockEntry,
        SlotIndexState,
    },
};
use jsonrpsee::http_client::HttpClient;
use reqwest::Client;

pub async fn get_latest_block_number_available_from_era(
    era_path: String,
    http_client: &Client,
) -> anyhow::Result<u64> {
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
        let start_offset =
            era_file_length - SlotIndexBlock::SERIALIZED_SIZE - SlotIndexState::SERIALIZED_SIZE;
        let end_offset = era_file_length - SlotIndexState::SERIALIZED_SIZE;

        let raw_slot_index_block = http_client
            .get(&era_path)
            .header("Range", format!("bytes={start_offset}-{end_offset}",))
            .send()
            .await?
            .bytes()
            .await?;

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
        let compressed_beacon_block = http_client
            .get(&era_path)
            .header(
                "Range",
                format!("bytes={}-{start_offset}", last_offset * -1),
            )
            .send()
            .await?
            .bytes()
            .await?;

        // process the compressed beacon block and collect the block number
        let entry = Entry::deserialize(&compressed_beacon_block)?;
        let fork = get_beacon_fork(last_slot_number);
        let beacon_block = CompressedSignedBeaconBlock::try_from(&entry, fork)?;

        Ok(beacon_block.block.execution_block_number())
    }
}
