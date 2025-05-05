use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ErrorCode {
    Unauthorized,
    SshSessionFail {
        user: String,
        ip: String,
    },
    ReadOnlyFail {
        user: String,
        ip: String,
    },
    DatasetNotFoundInConfig {
        dataset: String,
    },
    ServerNotFoundFromDataset {
        dataset: String,
        server_name: String,
    },
    ServerNotFoundFromRequest {
        server_name: String,
    },
    ZfsNotFound {
        user: String,
        ip: String,
    },
    ConfigIsInvalidJson,
    ErrorWritingConfigFile {
        path: PathBuf,
    },
    DatasetNotSynced {
        dataset: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Dataset {
    pub pool: String,
    pub dataset: String,
    pub snapshot: String,
}

#[derive(Serialize, Deserialize)]
pub struct Datasets {
    pub server: String,
    pub datasets: Vec<Dataset>,
}
