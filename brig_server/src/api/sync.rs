use std::{sync::Arc, time::Duration};

use tokio::sync::RwLock;

use crate::{config::{dataset, server::Server}, sync_state::SyncState, ConfigRef, SyncStateRef, SyncStates};

async fn sync_dataset(
    state: SyncStateRef,
    states: SyncStates,
    src: Server,
    dst: Server,
    dataset: dataset::Dataset,
) {
    /*
    let session = Session::connect(
        format!("{}@{}", &src.user, &src.address),
        KnownHosts::Strict,
    )
    .await
    .unwrap();
    */
    println!("{} sending {} to {}", src.name, dataset.name, dst.name);
    std::thread::sleep(Duration::from_secs(10));

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
            state.write().await.total = "Total".to_owned();
            state.write().await.sent = "Sent".to_owned();
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
