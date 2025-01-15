use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use alloy::rlp::Decodable;
use discv5::Enr;
use eth_trie::{decode_node, node::Node, RootWithTrieDiff};
use ethportal_api::{
    types::{
        network::Subnetwork, portal_wire::OfferTrace, state_trie::account_state::AccountState,
    },
    ContentValue, OverlayContentKey, StateContentKey, StateContentValue,
};
use futures::select;
use portal_bridge::{bridge::history::SERVE_BLOCK_TIMEOUT, census::Census, cli::BridgeId};
use revm::State;
use revm_primitives::{keccak256, Bytecode, B256};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        OwnedSemaphorePermit, RwLock, Semaphore,
    },
    task::JoinHandle,
    time::timeout,
};
use tracing::{error, info, warn};
use trin_validation::oracle::HeaderOracle;

use super::{
    channel::{StateBridgeMessage, StateDiffBundle},
    offer_report::{GlobalOfferReport, GranularOfferReport, OfferReport},
    rpc::RpcHeaderOracle,
};
use crate::{
    content::{
        create_account_content_key, create_account_content_value, create_contract_content_key,
        create_contract_content_value, create_storage_content_key, create_storage_content_value,
    },
    storage::state::evm_db::{self, EvmDB},
    sync::syncer::Syncer,
    trie_walker::TrieWalker,
    types::trie_proof::TrieProof,
    utils::full_nibble_path_to_address_hash,
};

pub struct StateBridge {
    portal_client: Arc<RwLock<HeaderOracle>>,
    /// Semaphore used to limit the amount of active offer transfers
    /// to make sure we don't overwhelm the trin client
    offer_semaphore: Arc<Semaphore>,
    /// Used to request all interested enrs in the network.
    census: Census<RpcHeaderOracle>,
    /// Global offer report for tallying total performance of state bridge
    global_offer_report: Arc<Mutex<GlobalOfferReport>>,
    /// Bridge id used to determine which content keys to gossip
    bridge_id: BridgeId,
    /// Database used to store newly created contracts and storage trie diffs
    evm_db: EvmDB,
}

impl StateBridge {
    pub async fn new(
        portal_client: Arc<RwLock<HeaderOracle>>,
        offer_limit: usize,
        census: Census<RpcHeaderOracle>,
        bridge_id: BridgeId,
        evm_db: EvmDB,
    ) -> anyhow::Result<Self> {
        let offer_semaphore = Arc::new(Semaphore::new(offer_limit));
        let global_offer_report = GlobalOfferReport::default();
        Ok(Self {
            portal_client,
            offer_semaphore,
            census,
            global_offer_report: Arc::new(Mutex::new(global_offer_report)),
            bridge_id,
            evm_db,
        })
    }

    pub async fn launch(
        self,
        shutdown_signal: broadcast::Receiver<()>,
        census_join_handle: JoinHandle<anyhow::Result<()>>,
    ) -> anyhow::Result<(
        UnboundedSender<StateBridgeMessage>,
        JoinHandle<anyhow::Result<()>>,
    )> {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<StateBridgeMessage>();

        let thread_handle = tokio::spawn(async move {
            let mut shutdown_signal = shutdown_signal;
            loop {
                tokio::select! {
                    _  = shutdown_signal.recv() => {
                        census_join_handle.abort();
                        break;
                    }
                    Some(command) = command_rx.recv() => {
                        self.gossip_trie_diff(command.state_diff_bundle).await?;
                        if let Err(err) = command.sender.send(Ok(()))  {
                            error!("Failed to send reply to state bridge command: {err:?}");
                        }
                    }
                }
            }
            Ok(())
        });

        Ok((command_tx, thread_handle))
    }

    pub async fn gossip_trie_diff(&self, state_diff_bundle: StateDiffBundle) -> anyhow::Result<()> {
        let walk_diff = TrieWalker::new_partial_trie(
            state_diff_bundle.root_with_trie_diff.root,
            state_diff_bundle.root_with_trie_diff.trie_diff,
        )?;

        // gossip block's new state transitions
        let mut content_idx = 0;
        for account_proof in walk_diff {
            // gossip the account
            self.gossip_account(&account_proof, state_diff_bundle.block_hash, content_idx)
                .await?;
            content_idx += 1;

            let Some(encoded_last_node) = account_proof.proof.last() else {
                error!("Account proof is empty. This should never happen maybe there is a bug in trie_walker?");
                continue;
            };

            let decoded_node = decode_node(&mut encoded_last_node.as_ref())
                .expect("Should should only be passing valid encoded nodes");
            let Node::Leaf(leaf) = decoded_node else {
                continue;
            };
            let account: AccountState = Decodable::decode(&mut leaf.value.as_slice())?;

            // reconstruct the address hash from the path so that we can fetch the
            // address from the database
            let mut partial_key_path = leaf.key.get_data().to_vec();
            partial_key_path.pop();
            let full_key_path = [&account_proof.path.clone(), partial_key_path.as_slice()].concat();
            let address_hash = full_nibble_path_to_address_hash(&full_key_path);

            // if the code_hash is empty then then don't try to gossip the contract bytecode
            if account.code_hash != keccak256([]) {
                // gossip contract bytecode
                if let Some(code) = self.evm_db.get_newly_created_contract(account.code_hash) {
                    self.gossip_contract_bytecode(
                        address_hash,
                        &account_proof,
                        state_diff_bundle.block_hash,
                        account.code_hash,
                        code,
                        content_idx,
                    )
                    .await?;
                    content_idx += 1;
                }
            }

            // gossip contract storage
            let storage_changed_nodes = self.evm_db.get_storage_trie_diff(address_hash);

            let storage_walk_diff =
                TrieWalker::new_partial_trie(account.storage_root, storage_changed_nodes)?;

            for storage_proof in storage_walk_diff {
                self.gossip_storage(
                    &account_proof,
                    &storage_proof,
                    address_hash,
                    state_diff_bundle.block_hash,
                    content_idx,
                )
                .await?;
                content_idx += 1;
            }
        }

        // flush the database cache
        // This is used for gossiping storage trie diffs and newly created contracts
        self.evm_db.clear_contract_cache();

        Ok(())
    }

    async fn gossip_account(
        &self,
        account_proof: &TrieProof,
        block_hash: B256,
        content_idx: u64,
    ) -> anyhow::Result<()> {
        // check if the bridge should gossip the content key
        if !self.bridge_id.is_selected(content_idx) {
            return Ok(());
        }
        let content_key = create_account_content_key(account_proof)?;
        let content_value = create_account_content_value(block_hash, account_proof)?;
        self.spawn_offer_tasks(content_key, content_value).await;
        Ok(())
    }

    async fn gossip_contract_bytecode(
        &self,
        address_hash: B256,
        account_proof: &TrieProof,
        block_hash: B256,
        code_hash: B256,
        code: Bytecode,
        content_idx: u64,
    ) -> anyhow::Result<()> {
        // check if the bridge should gossip the content key
        if !self.bridge_id.is_selected(content_idx) {
            return Ok(());
        }
        let content_key = create_contract_content_key(address_hash, code_hash)?;
        let content_value = create_contract_content_value(block_hash, account_proof, code)?;
        self.spawn_offer_tasks(content_key, content_value).await;
        Ok(())
    }

    async fn gossip_storage(
        &self,
        account_proof: &TrieProof,
        storage_proof: &TrieProof,
        address_hash: B256,
        block_hash: B256,
        content_idx: u64,
    ) -> anyhow::Result<()> {
        // check if the bridge should gossip the content key
        if !self.bridge_id.is_selected(content_idx) {
            return Ok(());
        }
        let content_key = create_storage_content_key(storage_proof, address_hash)?;
        let content_value = create_storage_content_value(block_hash, account_proof, storage_proof)?;
        self.spawn_offer_tasks(content_key, content_value).await;
        Ok(())
    }

    // spawn individual offer tasks of the content key for each interested enr found in Census
    async fn spawn_offer_tasks(
        &self,
        content_key: StateContentKey,
        content_value: StateContentValue,
    ) {
        let Ok(mut enrs) = self
            .census
            .select_peers(Subnetwork::State, &content_key.content_id())
        else {
            error!("Failed to request enrs for content key, skipping offer: {content_key:?}");
            return;
        };

        const BASE_TOTAL: usize = 2;

        let offer_report = Arc::new(Mutex::new(GranularOfferReport::new(
            content_key.clone(),
            BASE_TOTAL,
            enrs.len(),
        )));

        if enrs.len() < BASE_TOTAL {
            warn!("Not enough enrs to gossip content key: {content_key:?}. Skipping gossip. Must gossip to at least 2 peers, received: {}", enrs.len());
            return;
        }

        let enrs = Arc::new(Mutex::new(enrs));
        let state_gossiper = StateGossiper::new(
            self.offer_semaphore.clone(),
            self.census.clone(),
            self.portal_client.clone(),
            content_key.clone(),
            content_value.clone(),
            offer_report.clone(),
            self.global_offer_report.clone(),
            enrs,
        );

        let permit = self
            .offer_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("to be able to acquire semaphore");
        state_gossiper.clone().spawn_offer_task(permit);
        let permit = self
            .offer_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("to be able to acquire semaphore");
        state_gossiper.spawn_offer_task(permit);
    }
}

#[derive(Clone)]
pub struct StateGossiper {
    pub offer_semaphore: Arc<Semaphore>,
    pub census: Census<RpcHeaderOracle>,
    pub portal_client: Arc<RwLock<HeaderOracle>>,
    pub content_key: StateContentKey,
    pub content_value: StateContentValue,
    pub offer_report: Arc<Mutex<GranularOfferReport>>,
    pub global_offer_report: Arc<Mutex<GlobalOfferReport>>,
    pub enrs: Arc<Mutex<Vec<Enr>>>,
}

impl StateGossiper {
    pub fn new(
        offer_semaphore: Arc<Semaphore>,
        census: Census<RpcHeaderOracle>,
        portal_client: Arc<RwLock<HeaderOracle>>,
        content_key: StateContentKey,
        content_value: StateContentValue,
        offer_report: Arc<Mutex<GranularOfferReport>>,
        global_offer_report: Arc<Mutex<GlobalOfferReport>>,
        enrs: Arc<Mutex<Vec<Enr>>>,
    ) -> Self {
        Self {
            offer_semaphore,
            census,
            portal_client,
            content_key,
            content_value,
            offer_report,
            global_offer_report,
            enrs,
        }
    }

    pub fn spawn_offer_task(self, permit: OwnedSemaphorePermit) {
        tokio::spawn(async move {
            let enr = self
                .enrs
                .lock()
                .expect("to acquire lock")
                .pop()
                .expect("to have enr");

            let start_time = Instant::now();
            let content_value_size = self.content_value.encode().len();

            let result = timeout(
                SERVE_BLOCK_TIMEOUT,
                self.portal_client.read().await.state_trace_offer(
                    enr.clone(),
                    self.content_key.clone(),
                    self.content_value.clone(),
                ),
            )
            .await;

            let offer_trace = match &result {
                Ok(Ok(result)) => {
                    if matches!(result, &OfferTrace::Failed) {
                        warn!("Internal error offering to: {enr}");
                    }
                    result
                }
                Ok(Err(err)) => {
                    warn!("Error offering to: {enr}, error: {err:?}");
                    &OfferTrace::Failed
                }
                Err(_) => {
                    error!(
                        "trace_offer timed out on state proof {}: indicating a bug is present",
                        self.content_key
                    );
                    &OfferTrace::Failed
                }
            };

            self.census.record_offer_result(
                Subnetwork::State,
                enr.node_id(),
                content_value_size,
                start_time.elapsed(),
                offer_trace,
            );

            // Update report
            self.global_offer_report
                .lock()
                .expect("to acquire lock")
                .update(offer_trace);
            let gossip_again = self
                .offer_report
                .lock()
                .expect("to acquire lock")
                .update(&enr, offer_trace);

            if gossip_again {
                let state_gossiper = self.clone();
                let permit = state_gossiper
                    .offer_semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("to be able to acquire semaphore");
                state_gossiper.spawn_offer_task(permit);
            }

            // Release permit
            drop(permit);
        });
    }
}
