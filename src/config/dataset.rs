use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Dataset {
    pub name: String,
    pub owner: String,
    pub server: String,
}
