use crate::ConfigRef;
use openssh::{KnownHosts, Session};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Datasets
{
    pub server: String,
    pub datasets: Vec<String>,
}

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
            .arg("-o")
            .arg("name")
            .output()
            .await
            .unwrap();

        let output = String::from_utf8(ls.stdout).expect("server output was not valid UTF-8");

        let datasets: Vec<&str> = output
            .lines()
            .skip(2)
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();

        let mut ds = Datasets {server: server.address.clone(), datasets: vec![]};
        println!("Datasets on {}:", &ds.server);
        for dataset in &datasets {
            ds.datasets.push(dataset.to_string());
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
