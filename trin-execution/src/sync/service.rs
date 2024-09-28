use std::thread::JoinHandle;

use tokio::sync::oneshot;

use super::syncer2::BlockingSyncer;

pub struct SyncService {
    blocking_syncer: BlockingSyncer,
}

impl SyncService {
    pub fn new(blocking_syncer: BlockingSyncer) -> Self {
        Self { blocking_syncer }
    }

    pub fn spawn_background_syncer(&self) -> (JoinHandle<anyhow::Result<()>>, oneshot::Sender<()>) {
        let mut blocking_syncer = self.blocking_syncer.clone();
        let (shutdown_signal_sender, shutdown_signal_receiver) = oneshot::channel();
        let join_handle = std::thread::spawn(move || {
            blocking_syncer
                .process_range_of_blocks(None, Some(shutdown_signal_receiver))
                .map(|_| ())
        });
        (join_handle, shutdown_signal_sender)
    }
}
