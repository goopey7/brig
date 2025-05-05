use std::{path::PathBuf, sync::Arc};

use brig_common::api::{api::ErrorCode, switch::SwitchRequest};

use crate::{
    ConfigRef,
    config::{dataset::Dataset, server::Server},
    utils::{create_ssh_session, set_readonly},
};

async fn switch_dataset(
    config: ConfigRef,
    dataset: String,
    old_server: Server,
    new_server: Server,
) -> Result<(), ErrorCode> {
    let old_session = create_ssh_session(&old_server.user, &old_server.address).await?;
    let new_session = create_ssh_session(&new_server.user, &new_server.address).await?;

    set_readonly(&old_session, &old_server, &dataset, true).await?;
    set_readonly(&new_session, &new_server, &dataset, false).await?;

    let mut config = config.write().await;
    let dataset = config
        .datasets
        .iter_mut()
        .find(|ds: &&mut Dataset| ds.name == dataset)
        .ok_or(ErrorCode::DatasetNotFoundInConfig { dataset })?;
    dataset.server = new_server.name;
    Ok(())
}

async fn is_synced(dataset: &Dataset, config: ConfigRef) -> Result<bool, ErrorCode> {
    // ensure all servers have the same latest snapshot
    let mut latest_snapshots = vec![];
    for server in &config.read().await.servers {
        let session = create_ssh_session(&server.user, &server.address).await?;
        let output = session
            .command("zfs")
            .arg("list")
            .args(["-t", "snapshot"])
            .args(["-o", "name"])
            .args(["-S", "creation"])
            .arg(format!(
                "{pool}/{dataset}",
                pool = server.pool,
                dataset = dataset.name
            ))
            .output()
            .await
            .map_err(|_| ErrorCode::ZfsNotFound {
                user: server.user.clone(),
                ip: server.address.clone(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let latest = stdout.lines().nth(1);
        match latest {
            Some(latest) => {
                latest_snapshots.push(latest.to_owned());
            }
            None => {
                return Ok(false);
            }
        }
    }

    if let Some(first) = latest_snapshots.first() {
        if !latest_snapshots.iter().all(|s| s == first) {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }

    // if they all have the same latest, make sure owning server doesn't have a zfs diff
    let config = { config.read().await.clone() };
    let server = config
        .servers
        .iter()
        .find(|s: &&Server| s.name == dataset.server)
        .ok_or(ErrorCode::ServerNotFoundFromDataset {
            dataset: dataset.name.clone(),
            server_name: dataset.server.clone(),
        })?;

    let session = create_ssh_session(&server.user, &server.address).await?;

    if let Some(latest_snapshot) = latest_snapshots.first() {
        let output = session
            .command("zfs")
            .arg("diff")
            .arg(format!("{}", latest_snapshot))
            .output()
            .await
            .map_err(|_| ErrorCode::ZfsNotFound {
                user: server.user.clone(),
                ip: server.address.clone(),
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            return Ok(false);
        }
        return Ok(true);
    } else {
        return Ok(false);
    }
}

pub async fn switch(
    req: SwitchRequest,
    config_path: Arc<PathBuf>,
    config_arc: ConfigRef,
) -> warp::reply::Json {
    let config = { config_arc.read().await.clone() };
    let dataset = config
        .datasets
        .iter()
        .find(|ds: &&Dataset| ds.name == req.dataset)
        .take();
    if dataset.is_none() {
        return warp::reply::json(&ErrorCode::DatasetNotFoundInConfig {
            dataset: req.dataset.clone(),
        });
    }
    let dataset = dataset.unwrap();

    match is_synced(dataset, config_arc.clone()).await {
        Ok(is_synced) => {
            if !is_synced {
                return warp::reply::json(&ErrorCode::DatasetNotSynced {
                    dataset: dataset.name.clone(),
                });
            }
        }
        Err(e) => {
            return warp::reply::json(&e);
        }
    }

    let old_server = config
        .servers
        .iter()
        .find(|server: &&Server| server.name == dataset.server);
    if old_server.is_none() {
        return warp::reply::json(&ErrorCode::ServerNotFoundFromDataset {
            dataset: dataset.name.clone(),
            server_name: dataset.server.clone(),
        });
    }
    let old_server = old_server.unwrap();

    let new_server = config
        .servers
        .iter()
        .find(|server: &&Server| server.name == req.new_server);
    if new_server.is_none() {
        return warp::reply::json(&ErrorCode::ServerNotFoundFromRequest {
            server_name: req.new_server.clone(),
        });
    }
    let new_server = new_server.unwrap();

    if let Err(e) = switch_dataset(
        config_arc.clone(),
        req.dataset,
        old_server.clone(),
        new_server.clone(),
    )
    .await
    {
        return warp::reply::json(&e);
    }

    let json_str = serde_json::to_string_pretty(&*config_arc.read().await)
        .map_err(|_| ErrorCode::ConfigIsInvalidJson);
    if let Err(e) = json_str {
        return warp::reply::json(&e);
    }
    let json_str = json_str.unwrap();
    if let Err(e) =
        std::fs::write(&*config_path, &json_str).map_err(|_| ErrorCode::ErrorWritingConfigFile {
            path: (&*config_path).clone(),
        })
    {
        return warp::reply::json(&e);
    }

    warp::reply::json(&())
}
