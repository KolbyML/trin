use serde::{Deserialize, Serialize};
use serde_this_or_that::as_u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Syncing {
    #[serde(deserialize_with = "as_u64")]
    pub head_slot: u64,
    #[serde(deserialize_with = "as_u64")]
    pub sync_distance: u64,
    pub is_syncing: bool,
    pub is_optimistic: bool,
    pub el_offline: bool,
}
