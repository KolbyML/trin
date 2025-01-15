use std::sync::Arc;

use anyhow::{anyhow, Ok};
use tokio::sync::Mutex;

use crate::{
    storage::{execution_position::ExecutionPositionV2, state::evm_db::EvmDB},
    sync::{era::types::ProcessedBlock, service::SyncService},
};

pub struct Blockchain {
    _evm_db: EvmDB,
    blocks: Vec<ProcessedBlock>,
    side_chains: Vec<Vec<ProcessedBlock>>,
    _execution_position: Arc<Mutex<ExecutionPositionV2>>,
    _sync_service: Option<SyncService>,
}

impl Blockchain {
    pub fn new(
        evm_db: EvmDB,
        execution_position: Arc<Mutex<ExecutionPositionV2>>,
        sync_service: Option<SyncService>,
    ) -> Self {
        Self {
            _evm_db: evm_db,
            blocks: Vec::new(),
            side_chains: Vec::new(),
            _execution_position: execution_position,
            _sync_service: sync_service,
        }
    }

    pub fn process_block(&mut self, block: ProcessedBlock) -> anyhow::Result<()> {
        match self.blocks.last() {
            Some(last_block) if last_block.header.hash() == block.header.parent_hash => {
                self.blocks.push(block);
            }
            _ => {
                self.process_side_chain_blocks(block)
                    .map_err(|err| anyhow!("Failed to process side chain: {err}"))?;
            }
        }
        Ok(())
    }

    fn process_side_chain_blocks(&mut self, block: ProcessedBlock) -> anyhow::Result<()> {
        // Remove empty side chains
        self.side_chains.retain(|side_chain| !side_chain.is_empty());

        // commented out because side_chains is not used yet
        // for side_chain in self.side_chains.iter_mut() {
        //     match side_chain.last() {
        //         // Append to an existing side chain
        //         Some(last_block) if last_block.header.hash() == block.header.parent_hash => {
        //             side_chain.push(block);
        //             return Ok(());
        //         }
        //         _ => return Err(anyhow::anyhow!("Side chains shouldn't be empty")),
        //     }
        // }

        // Found a new side chain
        self.side_chains.push(vec![block]);
        Ok(())
    }
}
