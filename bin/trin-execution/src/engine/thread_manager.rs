use tokio::{
    sync::broadcast,
    task::{spawn_blocking, JoinHandle},
};
use tracing::{error, info};

/// This is for long running tasks that need to be spawned in the background, not short lived tasks
/// which return a result. Because certain tasks might depend on others for example the
/// BlockingSyncer task depends on the BlockService task. We will have a tier shutdown where we
/// shutdown tier 1 services first then tier 2 services.
pub struct ThreadManager {
    //
    shutdown_signal_1: broadcast::Sender<()>,
    std_handles_1: Vec<std::thread::JoinHandle<anyhow::Result<()>>>,
    shutdown_signal_2: broadcast::Sender<()>,
    tokio_handles_2: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl ThreadManager {
    pub fn new() -> Self {
        let (shutdown_signal_1, _) = tokio::sync::broadcast::channel(1);
        let (shutdown_signal_2, _) = tokio::sync::broadcast::channel(1);
        Self {
            shutdown_signal_1,
            std_handles_1: Vec::new(),
            shutdown_signal_2,
            tokio_handles_2: Vec::new(),
        }
    }

    pub fn shutdown_signal_1_receiver(&self) -> broadcast::Receiver<()> {
        self.shutdown_signal_1.subscribe()
    }

    pub fn append_std_handle_1(&mut self, handle: std::thread::JoinHandle<anyhow::Result<()>>) {
        self.std_handles_1.push(handle);
    }

    pub fn shutdown_signal_2_receiver(&self) -> broadcast::Receiver<()> {
        self.shutdown_signal_2.subscribe()
    }

    pub fn append_tokio_handle_2(&mut self, handle: JoinHandle<anyhow::Result<()>>) {
        self.tokio_handles_2.push(handle);
    }

    pub async fn shutdown_services(self) {
        info!("Shutting down Trin Execution Engine");

        // Shutdown tier 1 services
        if self.shutdown_signal_1.receiver_count() == 0 {
            info!("No tier 1 services to shutdown");
        } else if let Err(err) = self.shutdown_signal_1.send(()) {
            error!(
                "Failed to send shutdown signal 1: {err:?} received count: {}",
                self.shutdown_signal_1.receiver_count()
            );
        }

        for handle in self.std_handles_1 {
            // Spawn a blocking task to join the std thread as the join operation is blocking
            // and can not be done in an async context or a deadlock could occur
            spawn_blocking(move || {
                if let Err(err) = handle.join() {
                    info!("Std thread exited with result: {err:?}");
                }
            })
            .await
            .expect("Tokio spawn_blocking failed");
        }

        // Shutdown tier 2 services
        if self.shutdown_signal_2.receiver_count() == 0 {
            error!("No tier 2 services to shutdown, there should always be services in tier 2");
        } else if let Err(err) = self.shutdown_signal_2.send(()) {
            error!(
                "Failed to send shutdown signal 2: {err:?} received count: {}",
                self.shutdown_signal_2.receiver_count()
            );
        }

        for handle in self.tokio_handles_2 {
            if let Err(err) = handle.await.expect("Tokio thread panicked on join") {
                info!("Tokio thread exited with result: {err:?}");
            }
        }
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}
