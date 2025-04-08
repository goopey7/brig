use crate::ConfigRef;
use openssh::{KnownHosts, Session};

async fn update_sessions(config: ConfigRef) {
    let config = config.read().await;

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

        println!("Datasets on {}:", server.address);
        for dataset in &datasets {
            println!("  {}", dataset);
        }
    }
}

pub async fn status(config: ConfigRef) -> warp::reply::Json {
    update_sessions(config).await;
    warp::reply::json(&String::new())
}
