use brig_common::api::api::ErrorCode;
use chrono::Local;
use openssh::{KnownHosts, Session, Stdio};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    SyncStateRef, config::{dataset::Dataset, server::Server},
};

pub async fn create_ssh_session(user: &str, address: &str) -> Result<Session, ErrorCode> {
    Session::connect(format!("{}@{}", user, address), KnownHosts::Strict)
        .await
        .map_err(|_| ErrorCode::SshSessionFail {
            user: user.to_owned(),
            ip: address.to_owned(),
        })
}

pub async fn set_readonly(
    session: &Session,
    server: &Server,
    dataset: &str,
    is_on: bool,
) -> Result<(), ErrorCode> {
    let err = ErrorCode::ReadOnlyFail {
        user: server.user.clone(),
        ip: server.address.clone(),
    };
    session
        .command("sudo")
        .arg("zfs")
        .arg("set")
        .arg(if is_on { "readonly=on" } else { "readonly=off" })
        .arg(format!(
            "{pool}/{dataset}",
            pool = &server.pool,
            dataset = dataset
        ))
        .status()
        .await
        .map_err(|_| err)?;
    Ok(())
}

pub async fn list_snapshots(
    session: &Session,
    pool: &str,
    dataset: &str,
) -> Result<Vec<String>, ErrorCode> {
    let output = session
        .command("zfs")
        .args(["list", "-t", "snapshot", "-o", "name", "-S", "creation"])
        .arg(format!("{}/{}", pool, dataset))
        .output()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("failed to list snapshots: {}/{}", pool, dataset),
        })?;

    if !&output.stderr.is_empty() {
        Err(ErrorCode::ZfsCommandError {
            msg: format!("{}", String::from_utf8_lossy(&output.stderr)),
        })
    } else {
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1)
            .map(|s| s.to_string())
            .collect())
    }
}

pub async fn find_latest_common_snapshot(
    dataset: &str,
    src_snapshots: &Vec<String>,
    dst_snapshots: &Vec<String>,
) -> Result<String, ErrorCode> {
    let mut common_snapshot = None;
    for src_snapshot in src_snapshots {
        if dst_snapshots.contains(&src_snapshot.split_once('@').unwrap().1.to_string()) {
            common_snapshot = Some(src_snapshot.to_owned());
            break;
        }
    }
    common_snapshot.ok_or(ErrorCode::NoCommonSnapshot {
        dataset: dataset.to_owned(),
    })
}

pub async fn create_snapshot(
    session: &Session,
    pool: &str,
    dataset: &str,
) -> Result<String, ErrorCode> {
    let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    let snapshot = format!(
        "{pool}/{dataset}@brig-{snapshot}",
        pool = &pool,
        dataset = &dataset,
        snapshot = &timestamp
    );
    session
        .command("zfs")
        .arg("snapshot")
        .arg(&snapshot)
        .status()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("failed to take snapshot {}", &snapshot),
        })?;

    Ok(snapshot)
}

pub async fn estimate_send_size(session: &Session, from: &str, to: &str) -> Result<u64, ErrorCode> {
    let output = session
        .command("zfs")
        .arg("send")
        .arg("-n")
        .arg("-P")
        .arg("-i")
        .arg(&from)
        .arg(&to)
        .output()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("failed to estimate size from {} to {}", &from, &to),
        })?;

    if !&output.stderr.is_empty() {
        return Err(ErrorCode::ZfsCommandError {
            msg: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let size_line_error = ErrorCode::ZfsCommandError {
        msg: format!("couldn't find size for: {}", &stdout),
    };
    let size_line = stdout
        .lines()
        .find(|line| line.starts_with("size"))
        .ok_or(size_line_error.clone())?;

    let total_bytes_str = size_line.split_whitespace().nth(1).ok_or(size_line_error)?;

    total_bytes_str
        .parse()
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!(
                "size is not a number!\ntried to parse {}\nfrom: {}",
                &total_bytes_str, &stdout
            ),
        })
}

pub async fn send_bytes(
    src_session: &Session,
    dst_session: &Session,
    from: &str,
    to: &str,
    dst: &Server,
    dataset: &Dataset,
    state: &SyncStateRef,
) -> Result<(), ErrorCode> {
    let mut zfs_send = src_session
        .command("zfs")
        .arg("send")
        .arg("-i")
        .arg(&from)
        .arg(&to)
        .stdout(Stdio::piped())
        .spawn()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("failed to spawn zfs send! from {} to {}", &from, &to),
        })?;

    let mut zfs_recv = dst_session
        .command("zfs")
        .arg("recv")
        .arg("-F")
        .arg(format!("{}/{}", &dst.pool, &dataset.name))
        .stdin(Stdio::piped())
        .spawn()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("failed to spawn zfs recv! from {} to {}", &from, &to),
        })?;

    let mut send_output = zfs_send
        .stdout()
        .take()
        .ok_or(ErrorCode::FailedToTakeStdout {
            to: to.to_string(),
            from: from.to_string(),
        })?;
    let mut recv_input = zfs_recv
        .stdin()
        .take()
        .ok_or(ErrorCode::FailedToTakeStdin {
            to: to.to_string(),
            from: from.to_string(),
        })?;

    let mut total_bytes_sent: u64 = 0;
    let mut buffer = [0u8; 65536]; // 64 KiB buffer
    loop {
        let n = send_output
            .read(&mut buffer)
            .await
            .map_err(|_| ErrorCode::FailedToReadSendOutputToBuffer)?;
        if n == 0 {
            break;
        }
        recv_input
            .write_all(&buffer[..n])
            .await
            .map_err(|_| ErrorCode::FailedToWriteBufferToRecvInput)?;
        total_bytes_sent += n as u64;
        {
            let mut state = state.write().await;
            state.sent_bytes = total_bytes_sent;
        }
    }
    recv_input
        .shutdown()
        .await
        .map_err(|_| ErrorCode::FailedToShutdownOutputStream)?;

    zfs_send
        .wait()
        .await
        .map_err(|_| ErrorCode::FailedToWaitForZfsSend)?;
    zfs_recv
        .wait()
        .await
        .map_err(|_| ErrorCode::FailedToWaitForZfsRecv)?;
    Ok(())
}

pub async fn get_latest_snapshot(
    session: &Session,
    pool: &str,
    dataset: &str,
) -> Result<String, ErrorCode> {
    let output = session
        .command("zfs")
        .arg("list")
        .args(["-t", "snapshot"])
        .args(["-o", "name"])
        .args(["-S", "creation"])
        .arg(format!("{pool}/{dataset}", pool = pool, dataset = dataset))
        .output()
        .await
        .map_err(|_| ErrorCode::ZfsCommandError {
            msg: format!("unable to list snapshots for {}/{}", pool, dataset),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let latest = stdout
        .lines()
        .nth(1)
        .ok_or(ErrorCode::NoSnapshotsFound {
            pool: pool.to_owned(),
            dataset: dataset.to_owned(),
        })?
        .to_owned();
    Ok(latest)
}
