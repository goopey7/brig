use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Dataset
{
    pub pool: String,
    pub dataset: String,
    pub snapshot: String,
}

#[derive(Serialize, Deserialize)]
pub struct Datasets {
    pub server: String,
    pub datasets: Vec<Dataset>,
}

