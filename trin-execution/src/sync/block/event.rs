use tokio::sync::oneshot::Sender;

use crate::sync::era::types::SyncStatus;

pub enum BlockEvent {
    FetchNextBlock(Sender<SyncStatus>),
}
