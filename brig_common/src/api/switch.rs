use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SwitchRequest {
    pub dataset: String,
    pub new_server: String,
}
