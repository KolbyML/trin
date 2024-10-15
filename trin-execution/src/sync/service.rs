use std::thread::JoinHandle;

use tokio::sync::broadcast;

use super::blocking_syncer::BlockingSyncer;

pub struct SyncService {
    blocking_syncer: BlockingSyncer,
}

impl SyncService {
    pub fn new(blocking_syncer: BlockingSyncer) -> Self {
        Self { blocking_syncer }
    }

    pub fn spawn_background_syncer(
        &self,
        shutdown_signal: broadcast::Receiver<()>,
    ) -> JoinHandle<anyhow::Result<()>> {
        let mut blocking_syncer = self.blocking_syncer.clone();
        std::thread::spawn(move || {
            blocking_syncer
                .process_range_of_blocks(None, Some(shutdown_signal))
                .map(|_| ())
        })
    }
}
