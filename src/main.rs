mod cli;
mod config;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use config::config::Config;
use openssh::{KnownHosts, Session};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let config = std::fs::read_to_string(args.config_file)?;
    let config = serde_json::from_str::<Config>(&config)?;

    for server in &config.servers {

        let session = Session::connect(
            format!("{}@{}", server.user, server.address),
            KnownHosts::Strict,
        )
        .await?;

        let ls = session.command("zfs").arg("list").output().await?;
        eprintln!(
            "{}",
            String::from_utf8(ls.stdout).expect("server output was not valid UTF-8")
        );
    }

    Ok(())
}
