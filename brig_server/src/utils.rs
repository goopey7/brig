use brig_common::api::api::ErrorCode;
use openssh::{KnownHosts, Session};

use crate::config::server::Server;

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
