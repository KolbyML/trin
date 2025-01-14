use std::sync::Arc;

use alloy::{
    consensus::{Block as ConsensusBlock, BlockBody, Header, TxEnvelope},
    eips::eip4895::Withdrawal,
    rpc::types::{Block as RpcBlock, BlockTransactions, Header as RpcHeader, Withdrawals},
};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use anyhow::bail;
use revm_primitives::{B256, U256};
use rocksdb::DB as RocksDB;

#[derive(Debug, Clone, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[rlp(trailing)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<TxEnvelope>,
    pub uncles: Option<Vec<Header>>,
    pub withdrawals: Option<Vec<Withdrawal>>,
}

pub struct BlockStorage {
    /// The underlying database.
    pub db: Arc<RocksDB>,
}

impl BlockStorage {
    /// Create a new block storage.
    pub fn new(db: Arc<RocksDB>) -> Self {
        Self { db }
    }

    pub fn get_block_by_number(
        &self,
        block_number: u64,
        hydrated_transactions: bool,
    ) -> anyhow::Result<RpcBlock> {
        let block_hash_key = Self::block_hash_key(block_number);
        let block_hash = match self.db.get(block_hash_key)? {
            Some(block_hash) => B256::from_slice(&block_hash),
            None => {
                return Err(anyhow::anyhow!(
                    "Block with number {} not found in the database",
                    block_number
                ))
            }
        };
        self.get_block_by_hash(block_hash, hydrated_transactions)
    }

    pub fn get_block_by_hash(
        &self,
        block_hash: B256,
        hydrated_transactions: bool,
    ) -> anyhow::Result<RpcBlock> {
        if hydrated_transactions {
            bail!("replying with all transaction bodies is not supported yet");
        }

        let key = Self::block_key(block_hash);
        let block = match self.db.get(key)? {
            Some(block) => Block::decode(&mut block.as_slice())?,
            None => {
                return Err(anyhow::anyhow!(
                    "Block with hash {} not found in the database",
                    block_hash
                ))
            }
        };

        // let transactions =
        //     BlockTransactions::Hashes(block.transactions.iter().map(Transaction::hash).
        // collect()); let uncles = block
        //     .uncles
        //     .unwrap_or_default()
        //     .iter()
        //     .map(|uncle| uncle.hash())
        //     .collect();
        // let withdrawals = block
        //     .withdrawals
        //     .map(|withdrawals| withdrawals.iter().map(RpcWithdrawal::from).collect())
        //     .map(Withdrawals::new);

        // Ok(RpcBlock {
        //     header: block.header.to_rpc_header(None),
        //     uncles,
        //     transactions,
        //     withdrawals,
        // })
        let body = BlockBody {
            transactions: block.transactions,
            ommers: block.uncles.unwrap_or_default(),
            withdrawals: block.withdrawals.map(Withdrawals::new),
        };

        let transactions = BlockTransactions::Hashes(
            body.transactions
                .clone()
                .into_iter()
                .map(|tx| *tx.hash())
                .collect::<Vec<_>>(),
        );
        let uncles = body
            .ommers
            .clone()
            .iter()
            .map(|uncle| uncle.hash_slow())
            .collect();

        // Calculate block size:
        //   len(rlp(header, transactions, uncles, withdrawals))
        // Note: transactions are encoded with header
        let size = ConsensusBlock::rlp_length_for(&block.header, &body);

        // Combine header and block body into the single json representation of the block.
        let block = RpcBlock {
            header: RpcHeader::new(block.header).with_size(Some(U256::from(size))),
            transactions,
            uncles,
            withdrawals: body.withdrawals,
        };
        Ok(block)
    }

    pub fn store_block(&self, block: &Block) -> anyhow::Result<()> {
        // store block number to block hash mapping
        let key = Self::block_hash_key(block.header.number);
        self.db.put(&key, block.header.hash_slow())?;

        // store block hash to block
        let key = Self::block_key(block.header.hash_slow());
        let mut encoded_block = vec![];
        block.encode(&mut encoded_block);
        self.db.put(&key, encoded_block)?;
        Ok(())
    }

    /// block number to block hash key
    fn block_hash_key(block_number: u64) -> Vec<u8> {
        ["block".as_bytes(), &block_number.to_be_bytes()].concat()
    }

    /// block hash to block
    fn block_key(block_hash: B256) -> Vec<u8> {
        ["block".as_bytes(), block_hash.as_slice()].concat()
    }
}
