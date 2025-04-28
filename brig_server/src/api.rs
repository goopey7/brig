use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{
    ConfigRef, SyncStateRef, SyncStates,
    config::{dataset, server::Server},
    sync_state::SyncState,
};
use brig_common::api::{Dataset, Datasets};
use openssh::{KnownHosts, Session};

async fn update_sessions(config: ConfigRef) -> Vec<Datasets> {
    let config = config.read().await;
    let mut response = vec![];

    for server in &config.servers {
        let session = Session::connect(
            format!("{}@{}", server.user, server.address),
            KnownHosts::Strict,
        )
        .await
        .unwrap();

        let ls = session
            .command("zfs")
            .arg("list")
            .arg("-t")
            .arg("snapshot")
            .arg("-o")
            .arg("name")
            .output()
            .await
            .unwrap();

        let output = String::from_utf8(ls.stdout).expect("server output was not valid UTF-8");

        let datasets: Vec<&str> = output
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();

        let mut ds = Datasets {
            server: server.address.clone(),
            datasets: vec![],
        };
        println!("Datasets on {}:", &ds.server);
        for dataset in &datasets {
            ds.datasets.push(Dataset {
                pool: dataset.split_once('/').unwrap().0.to_string(),
                dataset: dataset
                    .split_once('/')
                    .unwrap()
                    .1
                    .split_once('@')
                    .unwrap()
                    .0
                    .to_string(),
                snapshot: dataset.split_once('@').unwrap().1.to_string(),
            });
            println!("  {}", dataset);
        }
        response.push(ds);
    }
    response
}

pub async fn status(config: ConfigRef) -> warp::reply::Json {
    let res = update_sessions(config).await;
    warp::reply::json(&res)
}

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
