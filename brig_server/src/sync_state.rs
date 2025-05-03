use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SyncState {
    pub dataset: String,
    pub src: String,
    pub dst: String,
    pub total_bytes: u64,
    pub sent_bytes: u64,
}
