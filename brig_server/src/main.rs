mod api;
mod cli;
mod config;
mod sync_state;
mod utils;

use std::sync::Arc;

use anyhow::Result;
use brig_common::api::switch::SwitchRequest;
use clap::Parser;
use cli::Cli;
use config::config::Config;
use openssh::Session;
use sync_state::SyncState;
use tokio::sync::{Mutex, RwLock};

use warp::Filter;

pub type SshSessions = Arc<Mutex<Vec<Session>>>;
pub type ConfigRef = Arc<RwLock<Config>>;
pub type SyncStateRef = Arc<RwLock<SyncState>>;
pub type SyncStates = Arc<RwLock<Vec<SyncStateRef>>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let config_path = Arc::new(args.config_file);
    let config_path_filter = warp::any().map({
        let config = Arc::clone(&config_path);
        move || config.clone()
    });

    let config = std::fs::read_to_string(&*config_path)?;
    let config = serde_json::from_str::<Config>(&config)?;
    let config_ref = Arc::new(RwLock::new(config));
    let config_filter = warp::any().map({
        let config = Arc::clone(&config_ref);
        move || config.clone()
    });

    let states: Arc<RwLock<Vec<SyncStateRef>>> = Arc::new(RwLock::new(Vec::new()));
    let states_filter = warp::any().map({
        let states = Arc::clone(&states);
        move || states.clone()
    });

    let status = warp::get()
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(config_filter.clone())
        .then(api::status);

    let sync = warp::get()
        .and(warp::path("sync"))
        .and(warp::path::end())
        .and(config_filter.clone())
        .and(states_filter.clone())
        .then(api::sync);

    let clean = warp::get()
        .and(warp::path("clean"))
        .and(warp::path::end())
        .and(config_filter.clone())
        .then(api::clean);

    let switch = warp::post()
        .and(warp::path("switch"))
        .and(warp::path::end())
        .and(warp::body::json::<SwitchRequest>())
        .and(config_path_filter)
        .and(config_filter)
        .then(api::switch);

    let routes = status.or(sync).or(clean).or(switch);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;

    Ok(())
}
