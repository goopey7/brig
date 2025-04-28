use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SyncState {
    pub dataset: String,
    pub src: String,
    pub dst: String,
    pub total: String,
    pub sent: String,
}
