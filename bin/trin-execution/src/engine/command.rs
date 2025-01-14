use alloy_rpc_types_engine::{
    ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadStatus,
};
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
    NewPayloadV1(NewPayloadMessage),
    NewPayloadV2(NewPayloadMessage),
    NewPayloadV3(NewPayloadMessage),
    GetPayloadV1(NewPayloadMessage),
    GetPayloadV2(NewPayloadMessage),
    GetPayloadV3(NewPayloadMessage),
    ForkChoice(
        (
            ForkchoiceState,
            Option<PayloadAttributes>,
            Sender<anyhow::Result<ForkchoiceUpdated>>,
        ),
    ),
}

impl EngineCommand {
    pub fn new_payload(
        processed_block: ProcessedBlock,
        expected_blob_versioned_hashes: Option<Vec<B256>>,
        sender: Sender<anyhow::Result<PayloadStatus>>,
    ) -> Self {
        Self::NewPayloadV1(NewPayloadMessage {
            processed_block,
            expected_blob_versioned_hashes,
            sender,
        })
    }

    pub fn fork_choice(
        forkchoice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
        sender: Sender<anyhow::Result<ForkchoiceUpdated>>,
    ) -> Self {
        Self::ForkChoice((forkchoice_state, payload_attributes, sender))
    }
}
