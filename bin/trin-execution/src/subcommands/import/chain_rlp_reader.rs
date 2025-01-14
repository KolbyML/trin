use std::{fs, path::PathBuf};

use alloy::{
    consensus::{Header, TxEnvelope},
    eips::eip4895::Withdrawal,
};
use alloy_rlp::{Decodable, RlpDecodable, RlpEncodable};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::sync::era::types::{ProcessedBlock, TransactionsWithSender};

pub struct ChainRlpReader {
    pub blocks: Vec<ProcessedBlock>,
}

impl ChainRlpReader {
    pub fn new(path: &PathBuf) -> anyhow::Result<Self> {
        let chain_rlp = &mut &fs::read(PathBuf::from(path))?[..];

        let mut processed_blocks = Vec::new();
        while !chain_rlp.is_empty() {
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
    pub transactions: Vec<TxEnvelope>,
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
                    .recover_signer()
                    .map(|sender_address| TransactionsWithSender {
                        sender_address,
                        transaction,
                    })
                    .map_err(|err| anyhow::anyhow!("Failed to recover sender address: {err}"))
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
    use super::*;

    #[test]
    fn test_chain_rlp_reader() {
        let _chain_rlp = &mut &include_bytes!("../../../resources/chain.rlp")[..];
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
