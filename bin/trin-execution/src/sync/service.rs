use std::{sync::Arc, thread::JoinHandle};

use parking_lot::Mutex;
use tracing::{error, info};

use super::blocking_syncer::BlockingSyncer;

#[derive(Clone)]
pub struct SyncService {
    blocking_syncer: BlockingSyncer,
    is_synced: Arc<Mutex<bool>>,
}

impl SyncService {
    pub fn new(blocking_syncer: BlockingSyncer) -> Self {
        let is_synced = Arc::new(Mutex::new(false));
        Self {
            blocking_syncer,
            is_synced,
        }
    }

    pub fn is_synced(&self) -> bool {
        *self.is_synced.lock()
    }

    pub fn spawn_background_syncer(
        &self,
        debug_last_block: Option<u64>,
    ) -> JoinHandle<anyhow::Result<()>> {
        let mut blocking_syncer = self.blocking_syncer.clone();
        let is_synced = self.is_synced.clone();
        std::thread::spawn(
            move || match blocking_syncer.sync_blocks(debug_last_block) {
                Ok(_) => {
                    *is_synced.lock() = true;
                    info!("Sync service has finished syncing");
                    Ok(())
                }
                Err(err) => {
                    error!("Sync service has failed to sync: {err:?}");
                    Err(err)
                }
            },
        )
    }
}
