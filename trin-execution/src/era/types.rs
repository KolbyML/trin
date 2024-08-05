use ethportal_api::{types::execution::transaction::Transaction, Header};
use revm_primitives::{Address, SpecId};

use crate::spec_id::get_spec_block_number;

#[derive(Debug, Clone)]
pub struct TransactionsWithSenders {
    pub transaction: Transaction,
    pub senders_address: Address,
}

#[derive(Debug, Clone)]
pub struct BlockWithRecoveredSenders {
    pub header: Header,
    pub uncles: Option<Vec<Header>>,
    pub transactions: Vec<TransactionsWithSenders>,
}

pub struct ProcessedEra {
    pub blocks: Vec<BlockWithRecoveredSenders>,
    pub era_type: EraType,
    pub epoch_index: u64,
    pub first_block_number: u64,
    pub block_count: u64,
}

impl ProcessedEra {
    pub fn contains_block(&self, block_number: u64) -> bool {
        (self.first_block_number..self.first_block_number + self.block_count)
            .contains(&block_number)
    }

    pub fn get_block(&self, block_number: u64) -> &BlockWithRecoveredSenders {
        &self.blocks[block_number as usize - self.first_block_number as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EraType {
    Era,
    Era1,
}

pub fn get_era_type(block_number: u64) -> EraType {
    if block_number < get_spec_block_number(SpecId::MERGE) {
        EraType::Era1
    } else {
        EraType::Era
    }
}
