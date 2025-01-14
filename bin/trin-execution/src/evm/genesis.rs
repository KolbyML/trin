use alloy::{
    consensus::{constants::EMPTY_OMMER_ROOT_HASH, Header, EMPTY_ROOT_HASH},
    genesis::Genesis,
    primitives::{keccak256, Bloom},
};
use eth_trie::Trie;
use ethportal_api::types::state_trie::account_state::AccountState;
use revm_primitives::B256;

use crate::storage::{
    block::{Block, BlockStorage},
    state::evm_db::EvmDB,
};

pub fn import_genesis(
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
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: genesis.coinbase,
            state_root: evm_db.trie.lock().root_hash()?,
            transactions_root: EMPTY_ROOT_HASH,
            receipts_root: EMPTY_ROOT_HASH,
            logs_bloom: Bloom::default(),
            difficulty: genesis.difficulty,
            number: 0,
            gas_limit: genesis.gas_limit,
            gas_used: 0,
            timestamp: genesis.timestamp,
            extra_data: genesis.extra_data.clone(),
            mix_hash: genesis.mix_hash,
            nonce: genesis.nonce.into(),
            base_fee_per_gas: genesis
                .base_fee_per_gas
                .map(|base_fee_per_gas| base_fee_per_gas as u64),
            withdrawals_root: None,
            blob_gas_used: genesis.blob_gas_used,
            excess_blob_gas: genesis.excess_blob_gas,
            parent_beacon_block_root: None,
            requests_hash: None,
        },
        transactions: vec![],
        uncles: None,
        withdrawals: None,
    };

    if store_blocks {
        let block_storage = BlockStorage::new(evm_db.db.clone());
        block_storage.store_block(&block)?;
    }

    Ok(block.header)
}
