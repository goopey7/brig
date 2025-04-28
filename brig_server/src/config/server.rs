use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Server {
    pub name: String,
    pub user: String,
    pub address: String,
    pub pool: String,
}
