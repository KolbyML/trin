use crate::{cli::TrinExecutionConfig, types::block_to_trace::BlockToTrace};

#[derive(Debug, Clone)]
pub struct StateConfig {
    /// This flag when enabled will storage all the trie keys from block execution in a cache, this
    /// is needed for gossiping the storage trie's changes. It is also needed for gossiping newly
    /// created contracts.
    pub cache_contract_changes: bool,
    pub block_to_trace: BlockToTrace,
    pub save_blocks: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for StateConfig {
    fn default() -> Self {
        Self {
            cache_contract_changes: false,
            block_to_trace: BlockToTrace::None,
            save_blocks: false,
        }
    }
}

impl From<TrinExecutionConfig> for StateConfig {
    fn from(cli_config: TrinExecutionConfig) -> Self {
        Self {
            cache_contract_changes: false,
            block_to_trace: cli_config.block_to_trace,
            save_blocks: cli_config.save_blocks,
        }
    }
}
