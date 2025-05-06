use brig_common::api::{api::ErrorCode, sync::SyncRequest};
use std::sync::Arc;
use tokio::
    sync::{Barrier, RwLock}
;

use crate::{
    ConfigRef, SyncStateRef, SyncStates,
    config::{
        dataset::{self, Dataset},
        server::Server,
    },
    sync_state::SyncState,
    utils,
};

async fn sync_dataset(
    state: SyncStateRef,
    states: SyncStates,
    src: Server,
    dst: Server,
    dataset: dataset::Dataset,
    http_return_barrier: Arc<Barrier>,
) -> Result<(), ErrorCode> {
    let src_session = utils::create_ssh_session(&src.user, &src.address).await?;
    let dst_session = utils::create_ssh_session(&dst.user, &dst.address).await?;
    let src_snapshots = utils::list_snapshots(&src_session, &src.pool, &dataset.name).await?;
    let dst_snapshots = utils::list_snapshots(&dst_session, &dst.pool, &dataset.name).await?;
    let latest_common_snapshot =
        utils::find_latest_common_snapshot(&dataset.name, &src_snapshots, &dst_snapshots).await?;
    let new_snapshot = utils::create_snapshot(&src_session, &src.pool, &dataset.name).await?;
    let total_bytes =
        utils::estimate_send_size(&src_session, &latest_common_snapshot, &new_snapshot).await?;
    {
        let mut state = state.write().await;
        state.total_bytes = total_bytes;
    }

    http_return_barrier.wait().await;

    utils::send_bytes(
        &src_session,
        &dst_session,
        &latest_common_snapshot,
        &new_snapshot,
        &dst,
        &dataset,
        &state,
    )
    .await?;

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
    Ok(())
}

pub async fn sync_all(config: ConfigRef, states: SyncStates) -> warp::reply::Json {
    let mut states_in_progress = vec![];
    let config = config.read().await;
    let mut http_return_barriers = vec![];
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
            println!("dataset {} is already in progress", &dataset.name);
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
            http_return_barriers.push(http_return_barrier.clone());
            tokio::spawn(sync_dataset(
                state.clone(),
                states.clone(),
                src_server.clone(),
                dst_server.clone(),
                dataset.clone(),
                http_return_barrier,
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

    warp::reply::json(&states_to_return)
}

pub async fn sync(req: SyncRequest, config: ConfigRef, states: SyncStates) -> warp::reply::Json {
    let config = config.read().await;
    let mut http_return_barriers = vec![];
    let mut is_in_progress = false;

    for dataset in req.datasets {
        let dataset = config
            .datasets
            .iter()
            .find(|ds: &&Dataset| dataset == ds.name)
            .unwrap();

        for state in &*states.read().await {
            if state.read().await.dataset == dataset.name {
                is_in_progress = true;
                break;
            }
        }

        if is_in_progress {
            println!("dataset {} is already in progress", &dataset.name);
            return warp::reply::json(&());
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
            http_return_barriers.push(http_return_barrier.clone());
            tokio::spawn(sync_dataset(
                state.clone(),
                states.clone(),
                src_server.clone(),
                dst_server.clone(),
                dataset.clone(),
                http_return_barrier,
            ));
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

    warp::reply::json(&states_to_return)
}
