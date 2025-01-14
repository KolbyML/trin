use std::sync::Arc;

use alloy::genesis::Genesis;
use alloy_chains::Chain;
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct ChainSpec {
    pub chain: Chain,
    pub genesis: Genesis,
}

impl From<Genesis> for ChainSpec {
    fn from(genesis: Genesis) -> Self {
        Self {
            chain: genesis.config.chain_id.into(),
            genesis,
        }
    }
}

pub static MAINNET: Lazy<Arc<ChainSpec>> = Lazy::new(|| {
    let mut spec = ChainSpec {
        chain: Chain::mainnet(),
        genesis: serde_json::from_str(include_str!("../../resources/genesis/mainnet.json"))
            .expect("Can't deserialize Mainnet genesis json"),
    };
    spec.genesis.config.dao_fork_support = true;
    spec.into()
});
