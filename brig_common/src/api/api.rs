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
    ZfsCommandError {
        msg: String,
    },
    ConfigIsInvalidJson,
    ErrorWritingConfigFile {
        path: PathBuf,
    },
    DatasetNotSynced {
        dataset: String,
    },
    NoCommonSnapshot {
        dataset: String,
    },
    FailedToTakeStdout {
        to: String,
        from: String,
    },
    FailedToTakeStdin {
        to: String,
        from: String,
    },
    FailedToReadSendOutputToBuffer,
    FailedToWriteBufferToRecvInput,
    FailedToShutdownOutputStream,
    FailedToWaitForZfsSend,
    FailedToWaitForZfsRecv,
    NoSnapshotsFound {
        pool: String,
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
