use alloy_rlp::Decodable;
use anyhow::{ensure, Result};
use eth_trie::{decode_node, node::Node, RootWithTrieDiff};
use ethportal_api::types::state_trie::account_state::AccountState;
use revm_primitives::keccak256;
use tracing::info;
use tracing_test::traced_test;
use trin_execution::{
    cli::APP_NAME,
    config::StateConfig,
    content::{
        create_account_content_key, create_account_content_value, create_contract_content_key,
        create_contract_content_value, create_storage_content_key, create_storage_content_value,
    },
    subcommands::state_gossip_stats::Stats,
    sync::syncer::Syncer,
    trie_walker::TrieWalker,
    types::block_to_trace::BlockToTrace,
    utils::full_nibble_path_to_address_hash,
};
use trin_utils::dir::setup_data_dir;

/// Tests that we can execute and generate content up to a specified block.
///
/// The block is specified manually by set `blocks` variable.
///
/// Following command should be used for running:
///
/// ```
/// BLOCKS=1000000 cargo test --release -p trin-execution --test content_generation -- --include-ignored --nocapture
/// ```
#[tokio::test]
#[traced_test]
#[ignore = "takes too long"]
async fn test_we_can_generate_content_key_values_up_to_x2() -> Result<()> {
    let data_dir = setup_data_dir(APP_NAME, None, false)?;
    let mut trin_execution = Syncer::new(
        data_dir.as_path(),
        StateConfig {
            cache_contract_changes: true,
            block_to_trace: BlockToTrace::None,
            save_blocks: false,
        },
    )
    .await?;

    let mut stats = Stats::default();

    let start = std::time::Instant::now();
    let mut account_count = 0;
    let mut storage_count = 0;

    for _ in 1..=1000 {
        let mut block_stats = Stats::default();

        let RootWithTrieDiff {
            root: root_hash,
            trie_diff: changed_nodes,
        } = trin_execution.process_next_block().await?;
        let block = trin_execution
            .era_manager
            .lock()
            .await
            .last_fetched_block()
            .await?
            .clone();
        info!("Block {} started", block.header.number);
        ensure!(
            trin_execution.get_root()? == block.header.state_root,
            "State root doesn't match"
        );

        let walk_diff = TrieWalker::new_partial_trie(root_hash, changed_nodes)?;
        for account_proof in walk_diff {
            let block_hash = block.header.hash();

            // check account content key/value
            let content_key =
                create_account_content_key(&account_proof).expect("Content key should be present");
            let content_value = create_account_content_value(block_hash, &account_proof)
                .expect("Content key should be present");
            block_stats.check_content(&content_key, &content_value);
            stats.check_content(&content_key, &content_value);

            let Some(encoded_last_node) = account_proof.proof.last() else {
                panic!("Account proof is empty");
            };

            let Node::Leaf(leaf) = decode_node(&mut encoded_last_node.as_ref())? else {
                continue;
            };
            account_count += 1;
            let account: AccountState = Decodable::decode(&mut leaf.value.as_slice())?;

            // reconstruct the address hash from the path so that we can fetch the
            // address from the database
            let mut partial_key_path = leaf.key.get_data().to_vec();
            partial_key_path.pop();
            let full_key_path = [&account_proof.path.clone(), partial_key_path.as_slice()].concat();
            let address_hash = full_nibble_path_to_address_hash(&full_key_path);

            // check contract code content key/value
            if account.code_hash != keccak256([]) {
                if let Some(code) = trin_execution
                    .database
                    .get_newly_created_contract(account.code_hash)
                {
                    let content_key = create_contract_content_key(address_hash, account.code_hash)
                        .expect("Content key should be present");
                    let content_value =
                        create_contract_content_value(block_hash, &account_proof, code)
                            .expect("Content key should be present");
                    block_stats.check_content(&content_key, &content_value);
                    stats.check_content(&content_key, &content_value);
                }
            }

            // check contract storage content key/value
            let storage_changed_nodes = trin_execution.database.get_storage_trie_diff(address_hash);
            let storage_walk_diff =
                TrieWalker::new_partial_trie(account.storage_root, storage_changed_nodes)?;
            for storage_proof in storage_walk_diff {
                let content_key = create_storage_content_key(&storage_proof, address_hash)
                    .expect("Content key should be present");
                let content_value =
                    create_storage_content_value(block_hash, &account_proof, &storage_proof)
                        .expect("Content key should be present");
                block_stats.check_content(&content_key, &content_value);
                stats.check_content(&content_key, &content_value);

                let Some(encoded_last_node2) = storage_proof.proof.last() else {
                    panic!("Account proof is empty");
                };

                if let Node::Leaf(_) = decode_node(&mut encoded_last_node2.as_ref())? {
                    storage_count += 1;
                }
            }
        }

        // flush the database cache
        // This is used for gossiping storage trie diffs
        trin_execution.database.storage_cache.lock().clear();
        trin_execution
            .database
            .newly_created_contracts
            .lock()
            .clear();
        // info!(
        //     "Block {} finished: {block_stats:?} account count {}, storage count {}",
        //     block.header.number, account_count, storage_count
        // );
    }
    info!(
        "Finished all  blocks: {stats:?} time in seconds {} account count {account_count}, storage count {storage_count}",
        start.elapsed().as_secs()
    );
    Ok(())
}
