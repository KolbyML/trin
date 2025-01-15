use std::{path::Path, sync::Arc};

use rocksdb::DB as RocksDB;
use tokio::sync::Mutex;

use crate::{
    config::StateConfig,
    storage::{
        execution_position::ExecutionPositionV2, state::evm_db::EvmDB, utils::setup_rocksdb,
    },
};

pub async fn initialize_database(
    data_dir: &Path,
    state_config: StateConfig,
) -> anyhow::Result<(Arc<Mutex<ExecutionPositionV2>>, EvmDB, Arc<RocksDB>)> {
    let db = Arc::new(setup_rocksdb(data_dir)?);
    let execution_position = Arc::new(Mutex::new(ExecutionPositionV2::initialize_from_db(
        db.clone(),
    )?));

    let evm_db = EvmDB::new(
        state_config,
        db.clone(),
        execution_position.lock().await.state_root(),
    )
    .expect("Failed to create EVM database");

    Ok((execution_position, evm_db, db))
}
