use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    error: ErrorCode,
    msg: String,
}

impl ErrorResponse {
    pub fn new(error: ErrorCode) -> Self {
        let msg = match error {
            ErrorCode::Unauthorized => "Not authorized by oidc",
            ErrorCode::SshSessionFail { ref user, ref ip } => {
                &format!("failed to ssh into {}@{}", user, ip)
            }
            ErrorCode::ReadOnlyFail { ref user, ref ip } => {
                &format!("{}@{}: unable to zfs set readonly!", user, ip)
            }
            ErrorCode::DatasetNotFoundInConfig { ref dataset } => {
                &format!("dataset {} not found in config!", dataset)
            }
            ErrorCode::ZfsNotFound { ref user, ref ip } => {
                &format!("{}@{}: zfs not found!", user, ip)
            }
            ErrorCode::ServerNotFoundFromDataset {
                ref dataset,
                ref server_name,
            } => &format!("{} not found! referenced by {}", server_name, dataset),
            ErrorCode::ServerNotFoundFromRequest { ref server_name } => {
                &format!("server: {} not found!", server_name)
            }
            ErrorCode::ConfigIsInvalidJson => "Config file is not valid json!",
            ErrorCode::ErrorWritingConfigFile { ref path } => {
                &format!("unable to write config to {}", path.display())
            }
        };

        Self {
            error,
            msg: msg.to_owned(),
        }
    }
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
