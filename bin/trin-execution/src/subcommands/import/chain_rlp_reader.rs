use std::{fs, path::PathBuf};

use alloy::hex;
use alloy_rlp::{Decodable, Header as RlpHeader, RlpDecodable, RlpEncodable};
use anyhow::anyhow;
use ethportal_api::{
    types::execution::{
        transaction::{Transaction, TransactionWithRlpHeader},
        withdrawal::Withdrawal,
    },
    BlockBody, Header,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::sync::era::types::{ProcessedBlock, TransactionsWithSender};

pub struct ChainRlpReader {
    pub blocks: Vec<ProcessedBlock>,
}

impl ChainRlpReader {
    pub fn new(path: &PathBuf) -> anyhow::Result<Self> {
        let chain_rlp = &mut &fs::read(PathBuf::from(path))?[..];

        let mut processed_blocks = Vec::new();
        while chain_rlp.len() > 0 {
            let block = Block::decode(chain_rlp)?;
            processed_blocks.push(block.try_into()?);
        }

        Ok(Self {
            blocks: processed_blocks,
        })
    }
}

/// Required for RLP decoding
#[derive(Debug, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
struct Block {
    pub header: Header,
    pub transactions: Vec<TransactionWithRlpHeader>,
    pub uncles: Vec<Header>,
    pub withdrawals: Option<Vec<Withdrawal>>,
}

impl TryFrom<Block> for ProcessedBlock {
    type Error = anyhow::Error;

    fn try_from(block: Block) -> Result<Self, Self::Error> {
        let transactions = block
            .transactions
            .into_par_iter()
            .map(|transaction| {
                transaction
                    .0
                    .get_transaction_sender_address()
                    .map(|sender_address| TransactionsWithSender {
                        sender_address,
                        transaction: transaction.0,
                    })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(ProcessedBlock {
            header: block.header,
            transactions,
            uncles: if block.uncles.is_empty() {
                None
            } else {
                Some(block.uncles)
            },
            withdrawals: block.withdrawals,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use alloy::hex;
    use alloy_rlp::{Buf, Decodable, Header as RlpHeader, Rlp};
    use alloy_rpc_types_engine::payload;
    use ethportal_api::Header;

    use super::*;
    use crate::sync::era::types::ProcessedBlock;

    #[test]
    fn test_chain_rlp_reader() {
        let chain_rlp = &mut &include_bytes!("../../../resources/chain.rlp")[..];
        // let header = RlpHeader::decode(&mut &chain_rlp[..]).unwrap();
        // for i in 0..1000 {
        //     let payload = &mut &chain_rlp[i..];
        //     let header2 = Header::decode(payload);
        //     if header2.is_ok() {
        //         panic!("{:?} {:?}", i, header2);
        //     }
        // }
        // let payload = &mut &chain_rlp[0..700];
        // println!("{:?}", hex::encode(payload.to_vec()));
        // let header2 = RlpHeader::decode(payload).unwrap();
        // payload.advance(header2.payload_length);

        let hi = ChainRlpReader::new(&PathBuf::from("resources/chain.rlp")).unwrap();

        panic!("{:?}", hi.blocks[0]);
    }
}
