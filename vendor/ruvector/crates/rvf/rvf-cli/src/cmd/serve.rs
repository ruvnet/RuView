//! `rvf serve` -- Start HTTP/TCP server for an RVF store.

use clap::Args;

#[derive(Args)]
pub struct ServeArgs {
    /// Path to the RVF store
    pub path: String,
    /// HTTP server port
    #[arg(short, long, default_value = "8080")]
    pub port: u16,
    /// TCP streaming port (defaults to HTTP port + 1000)
    #[arg(long)]
    pub tcp_port: Option<u16>,
}

pub fn run(args: ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "serve")]
    {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let config = rvf_server::ServerConfig {
                http_port: args.port,
                tcp_port: args.tcp_port.unwrap_or(args.port + 1000),
                data_path: std::path::PathBuf::from(&args.path),
                dimension: 0, // auto-detect from file
            };
            rvf_server::run(config).await
        })
    }
    #[cfg(not(feature = "serve"))]
    {
        let _ = args;
        eprintln!(
            "The 'serve' feature is not enabled. Rebuild with: cargo build -p rvf-cli --features serve"
        );
        Ok(())
    }
}
