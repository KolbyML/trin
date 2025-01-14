use std::sync::Arc;

use alloy::{
    consensus::EMPTY_ROOT_HASH,
    rlp::{Decodable, RlpDecodable, RlpEncodable},
};
use alloy_rpc_types_engine::ForkchoiceState;
use ethportal_api::Header;
use revm_primitives::B256;
use rocksdb::DB as RocksDB;
use serde::{Deserialize, Serialize};

// The location in the database which describes the current execution position.
pub const EXECUTION_POSITION_DB_KEY: &[u8; 18] = b"EXECUTION_POSITION";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, RlpDecodable, RlpEncodable)]
struct ExecutionPositionV1 {
    /// Version of the current execution state struct
    version: u8,

    state_root: B256,

    /// The next block number to be executed.
    next_block_number: u64,

    /// Positions needed by eth_syncing
    starting_block_number: u64,

    latest_block_number: u64,
}

impl From<ExecutionPositionV1> for ExecutionPositionV2 {
    fn from(execution_position_v1: ExecutionPositionV1) -> Self {
        Self {
            version: 2,
            state_root: execution_position_v1.state_root,
            next_block_number: execution_position_v1.next_block_number,
            starting_block_number: execution_position_v1.starting_block_number,
            latest_block_number: execution_position_v1.latest_block_number,
            head_block_hash: B256::ZERO,
            safe_block_hash: B256::ZERO,
            finalized_block_hash: B256::ZERO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, RlpDecodable, RlpEncodable)]
pub struct ExecutionPositionV2 {
    /// Version of the current execution state struct
    version: u64,

    state_root: B256,

    /// The next block number to be executed.
    next_block_number: u64,

    /// Positions needed by eth_syncing
    starting_block_number: u64,

    latest_block_number: u64,

    /// The next 3 fields are for the forkchoice state

    /// Hash of the head block.
    head_block_hash: B256,

    /// Hash of the safe block.
    safe_block_hash: B256,

    /// Hash of finalized block.
    finalized_block_hash: B256,
}

impl ExecutionPositionV2 {
    pub fn initialize_from_db(db: Arc<RocksDB>) -> anyhow::Result<Self> {
        Ok(match db.get(EXECUTION_POSITION_DB_KEY)? {
            Some(raw_execution_position) => {
                // Try to decode the version of the struct
                let version = Self::decode_version(&mut raw_execution_position.as_slice())?;
                match version {
                    1 => {
                        ExecutionPositionV1::decode(&mut raw_execution_position.as_slice())?.into()
                    }
                    2 => ExecutionPositionV2::decode(&mut raw_execution_position.as_slice())?,
                    _ => {
                        return Err(anyhow::anyhow!("Invalid version: {}", version));
                    }
                }
            }
            None => Self::default(),
        })
    }

    fn decode_version(raw_execution_position: &mut &[u8]) -> anyhow::Result<u64> {
        // unwrap the first header which is the rlp list
        let _ = alloy_rlp::Header::decode(raw_execution_position).map_err(|err| {
            anyhow::anyhow!("Failed to decode execution_position header: {err:?}")
        })?;
        let version_header = alloy_rlp::Header::decode(raw_execution_position)
            .map_err(|err| anyhow::anyhow!("Failed to decode version header: {err:?}"))?;
        match version_header.payload_length {
            1 => {
                let value = &raw_execution_position[..version_header.payload_length];
                Ok(value[0] as u64)
            }
            8 => {
                let value = &raw_execution_position[..version_header.payload_length];
                Ok(u64::from_le_bytes(
                    value.try_into().expect("expected 8 bytes"),
                ))
            }
            _ => {
                panic!(
                    "Invalid version header length: {}",
                    version_header.payload_length
                );
            }
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn starting_block_number(&self) -> u64 {
        self.starting_block_number
    }

    pub fn next_block_number(&self) -> u64 {
        self.next_block_number
    }

    pub fn previous_block_number(&self) -> u64 {
        self.next_block_number.saturating_sub(1)
    }

    pub fn latest_block_number(&self) -> u64 {
        self.latest_block_number
    }

    pub fn head_block_hash(&self) -> B256 {
        self.head_block_hash
    }

    pub fn safe_block_hash(&self) -> B256 {
        self.safe_block_hash
    }

    pub fn finalized_block_hash(&self) -> B256 {
        self.finalized_block_hash
    }

    pub fn state_root(&self) -> B256 {
        self.state_root
    }

    pub fn update_position(&mut self, db: Arc<RocksDB>, header: &Header) -> anyhow::Result<()> {
        self.next_block_number = header.number + 1;
        self.state_root = header.state_root;
        db.put(EXECUTION_POSITION_DB_KEY, alloy::rlp::encode(self))?;
        Ok(())
    }

    pub fn update_fork_choice_state(
        &mut self,
        db: Arc<RocksDB>,
        fork_choice_state: ForkchoiceState,
    ) -> anyhow::Result<()> {
        let ForkchoiceState {
            head_block_hash,
            safe_block_hash,
            finalized_block_hash,
        } = fork_choice_state;
        self.head_block_hash = head_block_hash;
        self.safe_block_hash = safe_block_hash;
        self.finalized_block_hash = finalized_block_hash;
        db.put(EXECUTION_POSITION_DB_KEY, alloy_rlp::encode(self))?;
        Ok(())
    }

    pub fn set_latest_block_number(
        &mut self,
        db: Arc<RocksDB>,
        latest_block_number: u64,
    ) -> anyhow::Result<()> {
        self.latest_block_number = latest_block_number;
        db.put(EXECUTION_POSITION_DB_KEY, alloy_rlp::encode(self))?;
        Ok(())
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

impl Default for ExecutionPositionV2 {
    fn default() -> Self {
        Self {
            version: 2,
            next_block_number: 0,
            state_root: EMPTY_ROOT_HASH,
            starting_block_number: 0,
            latest_block_number: 0,
            head_block_hash: B256::ZERO,
            safe_block_hash: B256::ZERO,
            finalized_block_hash: B256::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, Bloom, B64, U256};
    use trin_utils::dir::create_temp_test_dir;

    use super::*;
    use crate::storage::utils::setup_rocksdb;

    #[test]
    fn test_execution_position_v1() {
        let execution_position = ExecutionPositionV1::default();
        let encoded = alloy_rlp::encode(&execution_position);
        let decoded = ExecutionPositionV1::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(execution_position, decoded);
    }

    #[test]
    fn test_execution_position_v2() {
        let execution_position = ExecutionPositionV2::default();
        let encoded = alloy_rlp::encode(&execution_position);
        let decoded = ExecutionPositionV2::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(execution_position, decoded);
    }

    #[test_log::test]
    fn test_execution_position_v2_db() {
        let temp_directory = create_temp_test_dir().unwrap();
        let db = Arc::new(setup_rocksdb(temp_directory.path()).unwrap());

        let mut execution_position = ExecutionPositionV2::initialize_from_db(db.clone()).unwrap();
        assert_eq!(execution_position.version(), 2);
        assert_eq!(execution_position.next_block_number(), 0);
        assert_eq!(execution_position.state_root(), EMPTY_ROOT_HASH);

        // create fake execution block header
        let header = Header {
            parent_hash: B256::default(),
            uncles_hash: B256::default(),
            author: Address::random(),
            state_root: B256::from([1; 32]),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 1,
            gas_limit: U256::default(),
            gas_used: U256::default(),
            timestamp: u64::default(),
            extra_data: Vec::default(),
            mix_hash: Some(B256::default()),
            nonce: Some(B64::default()),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
        };

        execution_position
            .update_position(db.clone(), &header)
            .unwrap();
        let mut execution_position = ExecutionPositionV2::initialize_from_db(db.clone()).unwrap();
        assert_eq!(execution_position.version(), 2);
        assert_eq!(execution_position.next_block_number(), 2);
        assert_eq!(execution_position.state_root(), B256::from([1; 32]));

        let fork_choice_state = ForkchoiceState {
            head_block_hash: B256::from([1; 32]),
            safe_block_hash: B256::from([2; 32]),
            finalized_block_hash: B256::from([3; 32]),
        };
        execution_position
            .update_fork_choice_state(db.clone(), fork_choice_state)
            .unwrap();
        let execution_position = ExecutionPositionV2::initialize_from_db(db.clone()).unwrap();
        assert_eq!(execution_position.head_block_hash(), B256::from([1; 32]));
        assert_eq!(execution_position.safe_block_hash(), B256::from([2; 32]));
        assert_eq!(
            execution_position.finalized_block_hash(),
            B256::from([3; 32])
        );
    }

    #[test_log::test]
    fn test_update_from_v1_to_v2_execution_position() {
        let temp_directory = create_temp_test_dir().unwrap();
        let db = Arc::new(setup_rocksdb(temp_directory.path()).unwrap());

        let execution_position_v1 = ExecutionPositionV1::default();
        db.put(
            EXECUTION_POSITION_DB_KEY,
            alloy_rlp::encode(&execution_position_v1),
        )
        .unwrap();

        let execution_position = ExecutionPositionV2::initialize_from_db(db.clone()).unwrap();
        assert_eq!(execution_position.version(), 2);
        assert_eq!(execution_position.next_block_number(), 0);
        assert_eq!(execution_position.state_root(), EMPTY_ROOT_HASH);
    }
}
