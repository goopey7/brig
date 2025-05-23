use serde::{Deserialize, Serialize};

use super::{dataset::Dataset, server::Server};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub servers: Vec<Server>,
    pub datasets: Vec<Dataset>,
}
