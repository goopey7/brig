use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SyncRequest {
    pub datasets: Vec<String>,
}
