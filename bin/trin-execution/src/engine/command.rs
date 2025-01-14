use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadStatus};
use revm_primitives::B256;
use tokio::sync::oneshot::Sender;

use crate::sync::era::types::ProcessedBlock;

pub struct NewPayloadMessage {
    pub processed_block: ProcessedBlock,

    /// The expected blob versioned hashes for the payload.
    /// Available only from `engine_newPayloadV3` and later.
    pub expected_blob_versioned_hashes: Option<Vec<B256>>,
    pub sender: Sender<anyhow::Result<PayloadStatus>>,
}

pub enum EngineCommand {
    NewPayload(NewPayloadMessage),
    ForkChoice((ForkchoiceState, Sender<anyhow::Result<ForkchoiceUpdated>>)),
}

impl EngineCommand {
    pub fn new_payload(
        processed_block: ProcessedBlock,
        expected_blob_versioned_hashes: Option<Vec<B256>>,
        sender: Sender<anyhow::Result<PayloadStatus>>,
    ) -> Self {
        Self::NewPayload(NewPayloadMessage {
            processed_block,
            expected_blob_versioned_hashes,
            sender,
        })
    }

    pub fn fork_choice(
        forkchoice_state: ForkchoiceState,
        sender: Sender<anyhow::Result<ForkchoiceUpdated>>,
    ) -> Self {
        Self::ForkChoice((forkchoice_state, sender))
    }
}
