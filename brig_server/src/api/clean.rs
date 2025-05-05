use chrono::{Duration, Local};
use openssh::{KnownHosts, Session};
use regex::Regex;

use crate::ConfigRef;

pub async fn clean(config: ConfigRef) -> warp::reply::Json {
    let config = &config.read().await;
    for dataset in &config.datasets {
        let snapshot_expiration_duration = match &dataset.snapshot_lifetime.chars().last().unwrap()
        {
            'M' => Some(Duration::days(
                30 as i64
                    * (&dataset
                        .snapshot_lifetime
                        .trim_end_matches('M')
                        .parse()
                        .unwrap()),
            )),
            'w' => Some(Duration::days(
                7 as i64
                    * (&dataset
                        .snapshot_lifetime
                        .trim_end_matches('w')
                        .parse()
                        .unwrap()),
            )),
            'd' => Some(Duration::days(
                dataset
                    .snapshot_lifetime
                    .trim_end_matches('d')
                    .parse()
                    .unwrap(),
            )),
            _ => None,
        };

        let snapshot_expiration = Local::now() - snapshot_expiration_duration.unwrap();
        let snapshot_expiration = snapshot_expiration.format("%Y%m%d%H%M%S").to_string();

        for server in &config.servers {
            let session = Session::connect(
                format!("{}@{}", &server.user, &server.address),
                KnownHosts::Strict,
            )
            .await
            .unwrap();

            let output = session
                .command("zfs")
                .arg("list")
                .arg("-t")
                .arg("snapshot")
                .arg("-o")
                .arg("name")
                .arg("-s")
                .arg("creation")
                .arg(format!("{}/{}", &server.pool, &dataset.name))
                .output()
                .await
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);

            let brig_pattern = Regex::new(r"@brig-(\d{14})$").unwrap();
            for line in stdout.lines() {
                if line.starts_with(&format!("{}/{}@brig-", &server.pool, &dataset.name)) {
                    if let Some(caps) = brig_pattern.captures(line) {
                        let timestamp = &caps[1];
                        if *timestamp < *snapshot_expiration {
                            session
                                .command("zfs")
                                .arg("destroy")
                                .arg(line)
                                .status()
                                .await
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
    warp::reply::json(&())
}
