use alloy::primitives::B256;
use eth_trie::RootWithTrieDiff;
use tokio::sync::oneshot::Sender;

pub struct StateDiffBundle {
    pub root_with_trie_diff: RootWithTrieDiff,
    pub block_hash: B256,
}

pub struct StateBridgeMessage {
    pub state_diff_bundle: StateDiffBundle,
    pub sender: Sender<anyhow::Result<()>>,
}
