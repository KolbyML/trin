use alloy::{genesis::Genesis, primitives::keccak256};
use eth_trie::Trie;
use ethportal_api::types::state_trie::account_state::AccountState;
use revm::{db::State, Evm};

use crate::storage::evm_db::EvmDB;

pub fn import_genesis<'a>(evm_db: &mut EvmDB, genesis: &Genesis) -> anyhow::Result<()> {
    for (address, alloc_balance) in genesis.alloc.iter() {
        let address_hash = keccak256(address);
        let mut account = AccountState::default();
        account.balance += alloc_balance.balance;
        evm_db
            .trie
            .lock()
            .insert(address_hash.as_ref(), &alloy::rlp::encode(&account))?;
        evm_db.db.put(address_hash, alloy::rlp::encode(account))?;
    }

    Ok(())
}
