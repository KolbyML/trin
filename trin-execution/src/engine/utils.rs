use std::{path::Path, sync::Arc};

use tokio::sync::Mutex;

use crate::{
    config::StateConfig,
    storage::{evm_db::EvmDB, execution_position::ExecutionPositionV1, utils::setup_rocksdb},
};

pub async fn initialize_database(
    data_dir: &Path,
    state_config: StateConfig,
) -> anyhow::Result<(Arc<Mutex<ExecutionPositionV1>>, EvmDB)> {
    let db = Arc::new(setup_rocksdb(data_dir)?);
    let execution_position = Arc::new(Mutex::new(ExecutionPositionV1::initialize_from_db(
        db.clone(),
    )?));

    let database = EvmDB::new(
        state_config,
        db,
        execution_position.lock().await.state_root(),
    )
    .expect("Failed to create EVM database");

    Ok((execution_position, database))
}
