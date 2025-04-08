mod api;
mod cli;
mod config;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use config::config::Config;
use openssh::Session;
use tokio::sync::{Mutex,RwLock};

use warp::Filter;

pub type SshSessions = Arc<Mutex<Vec<Session>>>;
pub type ConfigRef = Arc<RwLock<Config>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let config = std::fs::read_to_string(args.config_file)?;
    let config = serde_json::from_str::<Config>(&config)?;
    let config_ref = Arc::new(RwLock::new(config));
    let config_filter = warp::any().map({
        let config = Arc::clone(&config_ref);
        move || config.clone()
    });

    let status = warp::get()
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(config_filter)
        .then(api::status);


    warp::serve(status).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}
