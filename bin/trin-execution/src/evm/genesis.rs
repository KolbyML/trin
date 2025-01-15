use alloy::{
    consensus::{constants::EMPTY_OMMER_ROOT_HASH, EMPTY_ROOT_HASH},
    genesis::Genesis,
    primitives::{keccak256, Bloom, U64},
};
use eth_trie::Trie;
use ethportal_api::{types::state_trie::account_state::AccountState, Header};
use revm::{db::State, Evm};
use revm_primitives::{B256, U256};

use crate::storage::{
    block::{Block, BlockStorage},
    state::evm_db::EvmDB,
};

pub fn import_genesis<'a>(
    evm_db: &mut EvmDB,
    genesis: &Genesis,
    store_blocks: bool,
) -> anyhow::Result<Header> {
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

    let block = Block {
        header: Header {
            parent_hash: B256::ZERO,
            uncles_hash: EMPTY_OMMER_ROOT_HASH,
            author: genesis.coinbase,
            state_root: evm_db.trie.lock().root_hash()?,
            transactions_root: EMPTY_ROOT_HASH,
            receipts_root: EMPTY_ROOT_HASH,
            logs_bloom: Bloom::default(),
            difficulty: genesis.difficulty,
            number: 0,
            gas_limit: U256::from(genesis.gas_limit),
            gas_used: U256::ZERO,
            timestamp: genesis.timestamp,
            extra_data: genesis.extra_data.to_vec(),
            mix_hash: Some(genesis.mix_hash),
            nonce: Some(genesis.nonce.into()),
            base_fee_per_gas: genesis.base_fee_per_gas.map(U256::from),
            withdrawals_root: None,
            blob_gas_used: genesis.blob_gas_used.map(U64::from),
            excess_blob_gas: genesis.excess_blob_gas.map(U64::from),
            parent_beacon_block_root: None,
        },
        transactions: vec![],
        uncles: None,
        withdrawals: None,
    };

    if store_blocks {
        let block_storage = BlockStorage::new(evm_db.db.clone());
        let _ = block_storage.store_block(&block)?;
    }

    Ok(block.header)
}
