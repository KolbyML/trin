use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadStatus};
use tokio::sync::{mpsc::UnboundedSender, oneshot::Sender};

use crate::era::types::ProcessedBlock;

pub enum EngineCommand {
    NewPayload((ProcessedBlock, Sender<anyhow::Result<PayloadStatus>>)),
    ForkChoice((ForkchoiceState, Sender<anyhow::Result<ForkchoiceUpdated>>)),
}
