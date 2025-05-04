use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Dataset {
    pub name: String,
    pub owner: String,
    pub server: String,
    pub snapshot_lifetime: String,
}
