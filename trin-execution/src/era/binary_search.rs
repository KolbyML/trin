use e2store::{
    e2store::types::{Entry, Header as E2StoreHeader},
    era::{get_beacon_fork, CompressedSignedBeaconBlock, Era, SlotIndexStateEntry},
    utils::get_era_file_download_links,
};
use surf::Client;

use super::{
    constants::FIRST_ERA_EPOCH_WITH_EXECUTION_PAYLOAD,
    types::ProcessedEra,
    utils::{download_raw_era, process_era_file},
};

pub struct EraBinarySearch {}

impl EraBinarySearch {
    /// For era files there isn't really a good way to know which era file has the block we want,
    /// well we could download all of them and check that isn't really feasible. We could also
    /// create a table which maps block numbers to era files, but that would be a lot of work and
    /// would need to be updated every time a new era file is added. So we will just binary
    /// search for the era file which contains the block we want. To make it so we don't have to
    /// download multiple era files because they are very big we will download the first beacon
    /// block estimate which blocks it contains and narrow our search till we find it.
    pub async fn find_era_file(
        http_client: Client,
        block_number: u64,
    ) -> anyhow::Result<ProcessedEra> {
        let era_links = get_era_file_download_links(&http_client).await?;
        let mut start_epoch_index = FIRST_ERA_EPOCH_WITH_EXECUTION_PAYLOAD;
        let mut end_epoch_index = era_links[era_links.len() - 1]
            .split('-')
            .nth(1)
            .expect("to be able to get epoch index")
            .parse::<u64>()
            .expect("to be able to parse epoch index");

        while start_epoch_index <= end_epoch_index {
            let mid = end_epoch_index + (start_epoch_index - end_epoch_index) / 2;
            let mid_block = EraBinarySearch::download_first_beacon_block_from_era(
                era_links[mid as usize].clone(),
                http_client.clone(),
            )
            .await?;
            let mid_block_number = mid_block.block.execution_block_number();

            // There are 2 cases to watch for
            // - if fetching a block from the first era, block_number will be 0
            // - for the rest we have no way to get the exact amount of blocks in an era file, so we just use 8192 as an estimate as it is the max possible value
            if mid_block_number == 0
                || (mid_block_number..mid_block_number + 8192).contains(&block_number)
            {
                let era_to_check =
                    download_raw_era(era_links[mid as usize].clone(), http_client.clone()).await?;

                let decoded_era = Era::deserialize(&era_to_check)?;
                if EraBinarySearch::does_era_contain_block(&decoded_era, block_number) {
                    return process_era_file(era_to_check, mid);
                }

                if mid + 1 > end_epoch_index {
                    return Err(anyhow::anyhow!("Block not found in any era file"));
                }

                // if the block is not contained in this era we know it is in the next one
                let next_era_index = mid + 1;
                let era_to_check = download_raw_era(
                    era_links[next_era_index as usize].clone(),
                    http_client.clone(),
                )
                .await?;

                let decoded_era = Era::deserialize(&era_to_check)?;
                if EraBinarySearch::does_era_contain_block(&decoded_era, block_number) {
                    return process_era_file(era_to_check, next_era_index);
                }
            }

            if mid_block_number < block_number {
                start_epoch_index = mid + 1;
            } else {
                end_epoch_index = mid - 1;
            }
        }

        Err(anyhow::anyhow!("Block not found in any era file"))
    }

    fn does_era_contain_block(era: &Era, block_number: u64) -> bool {
        for block in &era.blocks {
            if block.block.execution_block_number() == block_number {
                return true;
            }
        }
        false
    }

    async fn download_first_beacon_block_from_era(
        era_path: String,
        http_client: Client,
    ) -> anyhow::Result<CompressedSignedBeaconBlock> {
        let e2store_header = http_client
            .get(&era_path)
            .header("Range", "bytes=8-15")
            .recv_bytes()
            .await
            .expect("to be able to download e2store header");

        let e2store_header = E2StoreHeader::deserialize(&e2store_header)?;

        let compressed_beacon_block = http_client
            .get(&era_path)
            .header("Range", format!("bytes=8-{}", 15 + e2store_header.length))
            .recv_bytes()
            .await
            .expect("to be able to download compressed beacon block");

        let entry = Entry::deserialize(&compressed_beacon_block)?;

        // download slot_index_state to get the starting slot of the era file
        let response = surf::head(&era_path).await.expect("to be able to get head");
        let content_length = response
            .header("Content-Length")
            .map(|lengths| lengths.get(0))
            .expect("to be able to get content length")
            .map(|length| length.as_str().parse::<u64>())
            .expect("to be able to parse content length")
            .expect("to be able to get content length");
        let slot_index_state = http_client
            .get(&era_path)
            .header(
                "Range",
                format!("bytes={}-{}", content_length - 32, content_length),
            )
            .recv_bytes()
            .await
            .expect("to be able to download compressed beacon block");

        let slot_index_state = Entry::deserialize(&slot_index_state)?;
        let slot_index_state = SlotIndexStateEntry::try_from(&slot_index_state)?;
        let fork = get_beacon_fork(slot_index_state.slot_index.starting_slot);

        let beacon_block = CompressedSignedBeaconBlock::try_from(&entry, fork)?;

        Ok(beacon_block)
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::*;

    #[traced_test]
    #[tokio::test]
    async fn test_download_first_block() {
        let http_client = Client::new();
        let era_files = get_era_file_download_links(&http_client).await.unwrap();
        let epoch_index = 600;
        let era_file = era_files
            .iter()
            .find(|file| file.contains(&format!("mainnet-{epoch_index:05}-")))
            .expect("to be able to find era file")
            .clone();
        let beacon_block =
            EraBinarySearch::download_first_beacon_block_from_era(era_file, http_client)
                .await
                .unwrap();
        println!("bob {:?}", beacon_block.block.execution_block_number());
    }

    #[traced_test]
    #[tokio::test]
    async fn test_fetching_version_header_from_era() {
        let http_client = Client::new();
        let era_files = get_era_file_download_links(&http_client).await.unwrap();
        let epoch_index = 600;
        let era_file = era_files
            .iter()
            .find(|file| file.contains(&format!("mainnet-{epoch_index:05}-")))
            .expect("to be able to find era file")
            .clone();

        let raw_era1 = http_client.get(&era_file).recv_bytes().await.unwrap();
        let era = Era::deserialize(&raw_era1).unwrap();
        let epoch_index = 601;
        let era_file = era_files
            .iter()
            .find(|file| file.contains(&format!("mainnet-{epoch_index:05}-")))
            .expect("to be able to find era file")
            .clone();

        let raw_era1 = http_client.get(&era_file).recv_bytes().await.unwrap();
        let era1 = Era::deserialize(&raw_era1).unwrap();

        println!(
            "bob {:?} {:?}",
            era.blocks[era.blocks.len() - 1]
                .block
                .execution_block_number(),
            era1.blocks[0].block.execution_block_number()
        );
        let epoch_index = 602;
        let era_file = era_files
            .iter()
            .find(|file| file.contains(&format!("mainnet-{epoch_index:05}-")))
            .expect("to be able to find era file")
            .clone();

        let raw_era1 = http_client.get(&era_file).recv_bytes().await.unwrap();
        let era2 = Era::deserialize(&raw_era1).unwrap();
        println!(
            "bob2 {:?} {:?}",
            era1.blocks[era1.blocks.len() - 1]
                .block
                .execution_block_number(),
            era2.blocks[0].block.execution_block_number()
        );
    }
}
