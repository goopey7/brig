use chrono::Local;
use openssh::{KnownHosts, Session};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    ConfigRef, SyncStateRef, SyncStates,
    config::{dataset, server::Server},
    sync_state::SyncState,
};

async fn sync_dataset(
    state: SyncStateRef,
    states: SyncStates,
    src: Server,
    dst: Server,
    dataset: dataset::Dataset,
) {
    let session = Session::connect(
        format!("{}@{}", &src.user, &src.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    let timestamp = Local::now().format("%m-%d-%Y-%H-%M-%S").to_string();

    let from_snapshot = format!(
        "{pool}/{dataset}@{snapshot}",
        pool = &src.pool,
        dataset = &dataset.name,
        snapshot = &dataset.snapshot
    );
    let to_snapshot = format!(
        "{pool}/{dataset}@{snapshot}",
        pool = &src.pool,
        dataset = &dataset.name,
        snapshot = &timestamp
    );

    let snapshot_result = session
        .command("zfs")
        .arg("snapshot")
        .arg(&to_snapshot)
        .status()
        .await;

    let output = session
        .command("zfs")
        .arg("send")
        .arg("-n")
        .arg("-P")
        .arg("-i")
        .arg(from_snapshot)
        .arg(to_snapshot)
        .output()
        .await
        .unwrap();

    let stdout_str = String::from_utf8_lossy(&output.stdout);

    let size_line = stdout_str
        .lines()
        .find(|line| line.starts_with("size"))
        .expect("Couldn't find size line");

    let total_bytes: u64 = size_line
        .split_whitespace()
        .nth(1)
        .expect("Invalid size line format")
        .parse()
        .expect("Size not a number");

    println!(
        "{} sending {} to {} ({} bytes)",
        src.name, dataset.name, dst.name, total_bytes
    );

    let mut states = states.write().await;

    let mut pos = None;
    for (i, other) in states.iter().enumerate() {
        if other.read().await.dataset == dataset.name {
            pos = Some(i);
            break;
        }
    }
    if let Some(pos) = pos {
        states.remove(pos);
    }
}

pub async fn sync(config: ConfigRef, states: SyncStates) -> warp::reply::Json {
    let mut states_in_progress = vec![];
    let config = config.read().await;
    for dataset in &config.datasets {
        let mut is_in_progress = false;

        for state in &*states.read().await {
            if state.read().await.dataset == dataset.name {
                states_in_progress.push(state.read().await.clone());
                is_in_progress = true;
                break;
            }
        }

        if is_in_progress {
            continue;
        }

        let src_server = config
            .servers
            .iter()
            .find(|server: &&Server| server.name == dataset.server)
            .unwrap();

        for dst_server in &config.servers {
            if src_server.name == dst_server.name {
                continue;
            }
            let state = Arc::new(RwLock::new(SyncState::default()));
            state.write().await.dataset = dataset.name.clone();
            state.write().await.src = src_server.name.clone();
            state.write().await.dst = dst_server.name.clone();
            state.write().await.total_bytes = 0;
            state.write().await.sent_bytes = 0;
            tokio::spawn(sync_dataset(
                state.clone(),
                states.clone(),
                src_server.clone(),
                dst_server.clone(),
                dataset.clone(),
            ));
            states_in_progress.push(state.read().await.clone());
            states.write().await.push(state);
        }
    }
    warp::reply::json(&states_in_progress)
}
