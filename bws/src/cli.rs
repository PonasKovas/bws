use clap::Parser;
use once_cell::sync::Lazy;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(long, env)]
    /// Save logs to the filesystem
    pub save_logs: bool,
    #[arg(long, env, default_value = "5000")]
    /// Number of milliseconds to wait before forcefully shutting down (and possibly losing data) (neg. to disable)
    pub shutdown_timeout: i32,
    #[arg(short, long, env, default_value = "4")]
    /// Number of tokio worker threads for networking
    pub net_workers: usize,
    #[arg(short, long, env, default_value = "25565")]
    /// Port on which to start the server
    pub port: u16,
}

pub static OPT: Lazy<Cli> = Lazy::new(|| Cli::parse());
