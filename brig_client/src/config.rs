use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config
{
    pub server_url: String,
}
