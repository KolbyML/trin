use std::sync::Arc;

use alloy_consensus::EMPTY_ROOT_HASH;
use alloy_rlp::{Decodable, RlpDecodable, RlpEncodable};
use ethportal_api::Header;
use revm_primitives::B256;
use rocksdb::DB as RocksDB;
use serde::{Deserialize, Serialize};
use tracing::info;

// The location in the database which describes the current execution position.
pub const EXECUTION_POSITION_DB_KEY: &[u8; 18] = b"EXECUTION_POSITION";

// todo: remove this when me and Milos upgrade
#[derive(Debug, Clone, Serialize, Deserialize, RlpDecodable, RlpEncodable)]
struct ExecutionPositionV0 {
    /// Version of the current execution state struct
    version: u8,

    state_root: B256,

    /// The next block number to be executed.
    next_block_number: u64,
}

impl ExecutionPositionV0 {
    fn initialize_from_db(db: Arc<RocksDB>) -> anyhow::Result<Self> {
        Ok(match db.get(EXECUTION_POSITION_DB_KEY)? {
            Some(raw_execution_position) => {
                Decodable::decode(&mut raw_execution_position.as_slice())?
            }
            None => Self::default(),
        })
    }
}

impl From<ExecutionPositionV0> for ExecutionPositionV1 {
    fn from(execution_position_v0: ExecutionPositionV0) -> Self {
        Self {
            version: 1,
            state_root: execution_position_v0.state_root,
            next_block_number: execution_position_v0.next_block_number,
            starting_block_number: execution_position_v0.next_block_number,
            latest_block_number: execution_position_v0.next_block_number,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, RlpDecodable, RlpEncodable)]
pub struct ExecutionPositionV1 {
    /// Version of the current execution state struct
    version: u8,

    state_root: B256,

    /// The next block number to be executed.
    next_block_number: u64,

    /// Positions needed by eth_syncing
    starting_block_number: u64,

    latest_block_number: u64,
}

impl ExecutionPositionV1 {
    pub fn initialize_from_db(db: Arc<RocksDB>) -> anyhow::Result<Self> {
        Ok(match db.get(EXECUTION_POSITION_DB_KEY)? {
            Some(raw_execution_position) => {
                match Decodable::decode(&mut raw_execution_position.as_slice()) {
                    Ok(execution_position_1) => {
                        let mut execution_position_1: ExecutionPositionV1 = execution_position_1;
                        execution_position_1.starting_block_number =
                            execution_position_1.next_block_number;
                        execution_position_1
                    }
                    Err(_) => {
                        info!("Failed to decode ExecutionPositionV1, trying ExecutionPositionV0");
                        ExecutionPositionV0::initialize_from_db(db)?.into()
                    }
                }
            }
            None => Self {
                version: 0,
                next_block_number: 0,
                state_root: EMPTY_ROOT_HASH,
                starting_block_number: 0,
                latest_block_number: 0,
            },
        })
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn starting_block_number(&self) -> u64 {
        self.starting_block_number
    }

    pub fn next_block_number(&self) -> u64 {
        self.next_block_number
    }

    pub fn latest_block_number(&self) -> u64 {
        self.latest_block_number
    }

    pub fn state_root(&self) -> B256 {
        self.state_root
    }

    pub fn update_position(&mut self, db: Arc<RocksDB>, header: &Header) -> anyhow::Result<()> {
        self.next_block_number = header.number + 1;
        self.state_root = header.state_root;
        db.put(EXECUTION_POSITION_DB_KEY, alloy_rlp::encode(self))?;
        Ok(())
    }

    pub fn set_latest_known_block_number(&mut self, latest_block_number: u64) {
        self.latest_block_number = latest_block_number;
    }
}

impl Default for ExecutionPositionV0 {
    fn default() -> Self {
        Self {
            version: 0,
            next_block_number: 0,
            state_root: EMPTY_ROOT_HASH,
        }
    }
}

impl Default for ExecutionPositionV1 {
    fn default() -> Self {
        Self {
            version: 1,
            next_block_number: 0,
            state_root: EMPTY_ROOT_HASH,
            starting_block_number: 0,
            latest_block_number: 0,
        }
    }
}
