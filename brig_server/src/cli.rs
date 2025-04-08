/// Snap Conductor Server
#[derive(clap::Parser)]
#[command(version, about)]
pub struct Cli {
    /// config file
    #[arg(name = "config", short = 'c', long, default_value = "./config.json")]
    pub config_file: std::path::PathBuf,
}
