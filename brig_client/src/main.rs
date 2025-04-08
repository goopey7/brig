use clap::Parser;
use cli::{Cli, Commands};
use config::Config;

mod cli;
mod config;

fn main() {
    let cli = Cli::parse();
    let config = std::fs::read_to_string("config.json").unwrap();
    let config = serde_json::from_str::<Config>(&config).unwrap();

    match &cli.command
    {
        Commands::List => {
            let t = reqwest::blocking::get(format!("{}/status", &config.server_url)).unwrap().text().unwrap();
            println!("{}", t);
        },
    }
}
