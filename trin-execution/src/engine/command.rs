use alloy_rpc_types::engine::{ForkchoiceState, ForkchoiceUpdated, PayloadStatus};
use tokio::sync::oneshot::Sender;

use crate::sync::era::types::ProcessedBlock;

pub enum EngineCommand {
    NewPayload((ProcessedBlock, Sender<anyhow::Result<PayloadStatus>>)),
    ForkChoice((ForkchoiceState, Sender<anyhow::Result<ForkchoiceUpdated>>)),
}
