//! ruview-pointcloud — real-time dense point cloud from camera + WiFi CSI
//!
//! Pipeline: Camera → Depth (MiDaS ONNX) → Backproject → Fuse with WiFi occupancy → Stream
//!
//! Usage:
//!   ruview-pointcloud serve              # start HTTP + WebSocket server
//!   ruview-pointcloud capture --frames 1 # capture single frame to PLY
//!   ruview-pointcloud demo               # generate demo point cloud

mod depth;
mod pointcloud;
mod fusion;
mod stream;

use anyhow::Result;
use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "ruview-pointcloud", version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start real-time point cloud server (HTTP + WebSocket)
    Serve {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value = "9880")]
        port: u16,
        /// WiFi occupancy source URL (e.g., http://ruvultra:9876)
        #[arg(long)]
        wifi_source: Option<String>,
    },
    /// Capture frames to PLY file
    Capture {
        #[arg(long, default_value = "1")]
        frames: usize,
        #[arg(long, default_value = "output.ply")]
        output: String,
    },
    /// Generate demo point cloud (no camera needed)
    Demo,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port, wifi_source } => {
            stream::serve(&host, port, wifi_source.as_deref()).await?;
        }
        Commands::Capture { frames, output } => {
            let cloud = depth::capture_depth_cloud(frames).await?;
            pointcloud::write_ply(&cloud, &output)?;
            println!("Wrote {} points to {output}", cloud.points.len());
        }
        Commands::Demo => {
            demo().await?;
        }
    }

    Ok(())
}

async fn demo() -> Result<()> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  RuView Dense Point Cloud — Demo             ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    // Generate a demo occupancy volume (simulated WiFi tomography)
    let occupancy = fusion::demo_occupancy();
    let wifi_cloud = fusion::occupancy_to_pointcloud(&occupancy);
    println!("WiFi occupancy: {}x{}x{} voxels → {} points",
        occupancy.nx, occupancy.ny, occupancy.nz, wifi_cloud.points.len());

    // Generate a demo depth cloud (simulated camera)
    let depth_cloud = depth::demo_depth_cloud();
    println!("Camera depth: {} points", depth_cloud.points.len());

    // Fuse
    let fused = fusion::fuse_clouds(&[&wifi_cloud, &depth_cloud], 0.05);
    println!("Fused: {} points (voxel size=0.05m)", fused.points.len());

    // Write PLY
    pointcloud::write_ply(&fused, "demo_pointcloud.ply")?;
    println!("\nWrote: demo_pointcloud.ply");

    // Write Gaussian splats
    let splats = pointcloud::to_gaussian_splats(&fused);
    let json = serde_json::to_string_pretty(&splats)?;
    std::fs::write("demo_splats.json", &json)?;
    println!("Wrote: demo_splats.json ({} splats)", splats.len());

    Ok(())
}
