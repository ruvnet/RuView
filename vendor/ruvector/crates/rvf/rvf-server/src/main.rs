//! Binary entrypoint for the RVF streaming server.

use clap::Parser;
use std::path::PathBuf;

use rvf_server::ServerConfig;

#[derive(Parser)]
#[command(
    name = "rvf-server",
    about = "RuVector Format TCP/HTTP streaming server"
)]
struct Cli {
    /// HTTP listen port
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// TCP streaming listen port
    #[arg(long, default_value_t = 9090)]
    tcp_port: u16,

    /// Path to the RVF data directory / file
    #[arg(long, default_value = "data.rvf")]
    data_dir: PathBuf,

    /// Vector dimension (used when creating a new store)
    #[arg(long, default_value_t = 128)]
    dimension: u16,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config = ServerConfig {
        http_port: cli.port,
        tcp_port: cli.tcp_port,
        data_path: cli.data_dir,
        dimension: cli.dimension,
    };

    if let Err(e) = rvf_server::run(config).await {
        eprintln!("fatal: {e}");
        std::process::exit(1);
    }
}
