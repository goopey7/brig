use chrono::Local;
use openssh::{KnownHosts, Session, Stdio};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    sync::{Barrier, RwLock},
};

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
    http_return_barrier: Arc<Barrier>,
    http_cleanup_barrier: Arc<Barrier>,
) {
    let src_session = Session::connect(
        format!("{}@{}", &src.user, &src.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    let dst_session = Session::connect(
        format!("{}@{}", &dst.user, &dst.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();

    let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();

    let from_snapshot = format!(
        "{pool}/{dataset}@{snapshot}",
        pool = &src.pool,
        dataset = &dataset.name,
        snapshot = &dataset.snapshot
    );
    let to_snapshot = format!(
        "{pool}/{dataset}@brig-{snapshot}",
        pool = &src.pool,
        dataset = &dataset.name,
        snapshot = &timestamp
    );

    let snapshot_result = src_session
        .command("zfs")
        .arg("snapshot")
        .arg(&to_snapshot)
        .status()
        .await;

    let output = src_session
        .command("zfs")
        .arg("send")
        .arg("-n")
        .arg("-P")
        .arg("-i")
        .arg(&from_snapshot)
        .arg(&to_snapshot)
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

    {
        let mut state = state.write().await;
        state.total_bytes = total_bytes;
    }
    http_return_barrier.wait().await;

    let mut zfs_send = src_session
        .command("zfs")
        .arg("send")
        .arg("-i")
        .arg(&from_snapshot)
        .arg(&to_snapshot)
        .stdout(Stdio::piped())
        .spawn()
        .await
        .unwrap();
    let mut zfs_recv = dst_session
        .command("zfs")
        .arg("recv")
        .arg("-F")
        .arg(format!("{}/{}", &dst.pool, &dataset.name))
        .stdin(Stdio::piped())
        .spawn()
        .await
        .unwrap();

    let mut send_output = zfs_send.stdout().take().unwrap();
    let mut recv_input = zfs_recv.stdin().take().unwrap();

    tokio::io::copy(&mut send_output, &mut recv_input)
        .await
        .unwrap();
    recv_input.shutdown().await.unwrap();

    let send_status = zfs_send.wait().await.unwrap();
    let recv_status = zfs_recv.wait().await.unwrap();

    println!("Send exited with: {}", send_status);
    println!("Receive exited with: {}", recv_status);

    http_cleanup_barrier.wait().await;
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
    let mut http_return_barriers = vec![];
    let mut http_cleanup_barriers = vec![];
    for dataset in &config.datasets {
        let mut is_in_progress = false;

        for state in &*states.read().await {
            if state.read().await.dataset == dataset.name {
                states_in_progress.push(state.clone());
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

            let http_return_barrier = Arc::new(Barrier::new(2));
            let http_cleanup_barrier = Arc::new(Barrier::new(2));
            http_return_barriers.push(http_return_barrier.clone());
            http_cleanup_barriers.push(http_cleanup_barrier.clone());
            tokio::spawn(sync_dataset(
                state.clone(),
                states.clone(),
                src_server.clone(),
                dst_server.clone(),
                dataset.clone(),
                http_return_barrier,
                http_cleanup_barrier,
            ));
            states_in_progress.push(state.clone());
            states.write().await.push(state);
        }
    }

    let mut handles = vec![];

    for barrier in http_return_barriers {
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move { barrier.wait().await }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let mut states_to_return = vec![];
    for state in states.read().await.iter() {
        let state = state.read().await.clone();
        states_to_return.push(state);
    }

    for barrier in http_cleanup_barriers {
        barrier.wait().await;
    }
    warp::reply::json(&states_to_return)
}
