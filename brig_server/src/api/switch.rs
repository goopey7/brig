use std::{path::PathBuf, sync::Arc};

use openssh::{KnownHosts, Session};
use serde::{Deserialize, Serialize};

use crate::{
    ConfigRef, SyncStates,
    config::{dataset::Dataset, server::Server},
};

#[derive(Serialize, Deserialize)]
pub struct SwitchJson {
    dataset: String,
    new_server: String,
}

async fn switch_dataset(
    config: ConfigRef,
    dataset: String,
    old_server: Server,
    new_server: Server,
) {
    let old_session = Session::connect(
        format!("{}@{}", &old_server.user, &old_server.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    old_session
        .command("sudo")
        .arg("zfs")
        .arg("set")
        .arg("readonly=on")
        .arg(format!(
            "{pool}/{dataset}",
            pool = old_server.pool,
            dataset = dataset
        ))
        .status()
        .await
        .unwrap();

    let new_session = Session::connect(
        format!("{}@{}", &new_server.user, &new_server.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    new_session
        .command("sudo")
        .arg("zfs")
        .arg("set")
        .arg("readonly=off")
        .arg(format!(
            "{pool}/{dataset}",
            pool = new_server.pool,
            dataset = dataset
        ))
        .status()
        .await
        .unwrap();

    {
        let mut config = config.write().await;
        let dataset = config
            .datasets
            .iter_mut()
            .find(|ds: &&mut Dataset| ds.name == dataset)
            .unwrap();
        dataset.server = new_server.name;
    }
}

async fn is_synced(dataset: &Dataset, config: ConfigRef) -> bool {
    // ensure all servers have the same latest snapshot
    let mut latest_snapshots = vec![];
    for server in &config.read().await.servers {
        let session = Session::connect(
            format!("{}@{}", &server.user, &server.address),
            KnownHosts::Strict,
        )
        .await
        .unwrap();

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
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let latest = stdout.lines().nth(1).unwrap().to_owned();
        latest_snapshots.push(latest);
    }

    let first = latest_snapshots.first().unwrap();
    if !latest_snapshots.iter().all(|s| {
        println!("{}", s);
        s == first
    }) {
        return false;
    }

    // if they all have the same latest, make sure owning server doesn't have a zfs diff
    let config = { config.read().await.clone() };
    let server = config
        .servers
        .iter()
        .find(|s: &&Server| s.name == dataset.server)
        .unwrap();

    let session = Session::connect(
        format!("{}@{}", &server.user, &server.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    let output = session
        .command("zfs")
        .arg("diff")
        .arg(format!("{}", latest_snapshots.first().unwrap(),))
        .output()
        .await
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("stdout: {}", stdout.to_string());
    println!("stderr: {}", stderr.to_string());
    if !stdout.is_empty() {
        return false;
    }
    true
}

pub async fn switch(
    req: SwitchJson,
    config_path: Arc<PathBuf>,
    config_arc: ConfigRef,
    states: SyncStates,
) -> warp::reply::Json {
    let config = { config_arc.read().await.clone() };
    let dataset = config
        .datasets
        .iter()
        .find(|ds: &&Dataset| ds.name == req.dataset)
        .take()
        .unwrap();

    if !is_synced(dataset, config_arc.clone()).await {
        return warp::reply::json(&());
    }

    let old_server = config
        .servers
        .iter()
        .find(|server: &&Server| server.name == dataset.server)
        .unwrap();

    let new_server = config
        .servers
        .iter()
        .find(|server: &&Server| server.name == req.new_server)
        .unwrap();

    switch_dataset(
        config_arc.clone(),
        req.dataset,
        old_server.clone(),
        new_server.clone(),
    )
    .await;

    std::fs::write(
        &*config_path,
        serde_json::to_string_pretty(&*config_arc.read().await).unwrap(),
    )
    .unwrap();

    warp::reply::json(&())
}
