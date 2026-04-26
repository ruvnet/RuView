//! ruview-pointcloud — real-time dense point cloud from camera + WiFi CSI
//!
//! Pipeline: Camera → Depth → Backproject → Fuse with WiFi occupancy → Stream
//!
//! Usage:
//!   ruview-pointcloud serve               # HTTP + Three.js viewer
//!   ruview-pointcloud capture --frames 1  # capture to PLY
//!   ruview-pointcloud demo                # synthetic demo
//!   ruview-pointcloud train               # calibration training
//!   ruview-pointcloud csi-test            # send test CSI frames (ADR-018 binary)

mod brain_bridge;
mod camera;
mod csi_pipeline;
mod depth;
mod fusion;
mod parser;
mod pointcloud;
mod stream;
mod training;

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
    /// Start real-time point cloud server.
    ///
    /// By default the HTTP server binds to `127.0.0.1:9880` — exposing it on
    /// `0.0.0.0` leaks live camera/CSI/vitals data to the network and must
    /// be an explicit opt-in via `--bind 0.0.0.0:9880`.
    Serve {
        /// Bind address for the HTTP/viewer server. Default
        /// `127.0.0.1:9880` (loopback only — safe by default).
        #[arg(long, default_value = "127.0.0.1:9880")]
        bind: String,
        /// Brain URL for storing observations
        #[arg(long)]
        brain: Option<String>,
    },
    /// Capture frames to PLY file
    Capture {
        #[arg(long, default_value = "1")]
        frames: usize,
        #[arg(long, default_value = "output.ply")]
        output: String,
    },
    /// Generate demo point cloud
    Demo,
    /// List available cameras
    Cameras,
    /// Training and calibration
    Train {
        #[arg(long, default_value = "~/.local/share/ruview/training")]
        data_dir: String,
        /// Brain URL for submitting results
        #[arg(long)]
        brain: Option<String>,
    },
    /// Send synthetic ADR-018 binary CSI frames (for local testing without ESP32).
    CsiTest {
        #[arg(long, default_value = "127.0.0.1:3333")]
        target: String,
        #[arg(long, default_value = "100")]
        count: usize,
    },
    /// Record a CSI fingerprint for the current location.
    ///
    /// Listens on UDP 3333 for `--seconds` seconds, accumulates CSI frames,
    /// and stores a named fingerprint that future sessions can match
    /// against to identify the room.
    Fingerprint {
        /// Human-readable name for the fingerprint (e.g. "office", "lab").
        name: String,
        /// How long to listen before recording (default 5 s).
        #[arg(long, default_value = "5")]
        seconds: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { bind, brain } => {
            stream::serve(&bind, brain.as_deref()).await?;
        }
        Commands::Capture { frames: _, output } => {
            if camera::camera_available() {
                let config = camera::CameraConfig::default();
                let frame = camera::capture_frame(&config)?;
                let depth = depth::estimate_depth(&frame.rgb, frame.width, frame.height)?;
                let intrinsics = depth::CameraIntrinsics::default();
                let cloud = depth::backproject_depth(&depth, &intrinsics, Some(&frame.rgb), 2);
                pointcloud::write_ply(&cloud, &output)?;
                println!("Captured {} points to {output}", cloud.points.len());
            } else {
                let cloud = depth::demo_depth_cloud();
                pointcloud::write_ply(&cloud, &output)?;
                println!("No camera — wrote {} demo points to {output}", cloud.points.len());
            }
        }
        Commands::Demo => {
            demo().await?;
        }
        Commands::Cameras => {
            let cams = camera::list_cameras();
            if cams.is_empty() {
                println!("No cameras found");
            } else {
                println!("Available cameras:");
                for (i, c) in cams.iter().enumerate() {
                    println!("  [{i}] {c}");
                }
            }
        }
        Commands::Train { data_dir, brain } => {
            train(&data_dir, brain.as_deref()).await?;
        }
        Commands::CsiTest { target, count } => {
            println!("Sending {count} synthetic ADR-018 CSI frames to {target}...");
            csi_pipeline::send_test_frames(&target, count)?;
            println!("Done");
        }
        Commands::Fingerprint { name, seconds } => {
            println!("Recording CSI fingerprint '{name}' for {seconds} s on UDP 3333...");
            let state = csi_pipeline::start_pipeline("0.0.0.0:3333");
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            // record_fingerprint takes a brief lock on the shared state to
            // read the last N frames from every node's history.
            {
                let mut st = state.lock().expect("pipeline state lock poisoned");
                st.record_fingerprint(&name);
                println!(
                    "  Stored: {} fingerprint(s) total, {} total CSI frames received",
                    st.fingerprints.len(),
                    st.total_frames
                );
            }
        }
    }

    Ok(())
}

async fn demo() -> Result<()> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  RuView Dense Point Cloud — Demo             ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    let occupancy = fusion::demo_occupancy();
    let wifi_cloud = fusion::occupancy_to_pointcloud(&occupancy);
    println!("WiFi occupancy: {}x{}x{} voxels → {} points",
        occupancy.nx, occupancy.ny, occupancy.nz, wifi_cloud.points.len());

    let depth_cloud = depth::demo_depth_cloud();
    println!("Camera depth: {} points", depth_cloud.points.len());

    let fused = fusion::fuse_clouds(&[&wifi_cloud, &depth_cloud], 0.05);
    println!("Fused: {} points (voxel size=0.05m)", fused.points.len());

    pointcloud::write_ply(&fused, "demo_pointcloud.ply")?;
    println!("\nWrote: demo_pointcloud.ply");

    let splats = pointcloud::to_gaussian_splats(&fused);
    let json = serde_json::to_string_pretty(&splats)?;
    std::fs::write("demo_splats.json", &json)?;
    println!("Wrote: demo_splats.json ({} splats)", splats.len());

    Ok(())
}

async fn train(data_dir: &str, brain_url: Option<&str>) -> Result<()> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  RuView Point Cloud — Training               ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    let expanded = data_dir.replace('~', &dirs::home_dir().unwrap_or_default().to_string_lossy());
    // Defence-in-depth: reject path-traversal in the CLI argument before we
    // hand it to TrainingSession (which also checks). This catches malicious
    // CLI input early, before any I/O.
    let _sanitised = training::sanitize_data_path(&expanded)?;
    let mut session = training::TrainingSession::new(&expanded)?;
    session.load_samples()?;

    // Capture training samples
    println!("==> Capturing training samples...");

    // Camera samples
    if camera::camera_available() {
        println!("  Camera detected — capturing depth frames...");
        let config = camera::CameraConfig::default();
        for i in 0..5 {
            if let Ok(frame) = camera::capture_frame(&config) {
                let depth = depth::estimate_depth(&frame.rgb, frame.width, frame.height)?;
                // Score based on depth variance (good frames have varied depth)
                let mean: f32 = depth.iter().sum::<f32>() / depth.len() as f32;
                let variance: f32 = depth.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / depth.len() as f32;
                let quality = (variance / 2.0).min(1.0);

                session.add_sample(
                    Some(depth), frame.width, frame.height,
                    None, None, quality,
                );
                println!("  Frame {}: quality={:.2}", i, quality);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    } else {
        println!("  No camera — using synthetic samples for calibration demo");
        for i in 0..10 {
            let w = 160u32;
            let h = 120u32;
            let depth: Vec<f32> = (0..w * h).map(|j| 1.0 + (j as f32 / (w * h) as f32) * 4.0 + (i as f32 * 0.1)).collect();
            let quality = if i < 7 { 0.8 } else { 0.2 };
            let gt = if i % 3 == 0 {
                Some(training::GroundTruth {
                    reference_distances: vec![
                        training::ReferencePoint { name: "wall".into(), x_pixel: 80, y_pixel: 60, true_distance_m: 3.0 },
                    ],
                    occupancy_label: Some(if i < 5 { "occupied" } else { "empty" }.into()),
                })
            } else { None };
            session.add_sample(Some(depth), w, h, None, gt, quality);
        }
    }

    session.save_samples()?;

    // Calibrate depth
    println!("\n==> Calibrating depth estimation...");
    let cal = session.calibrate_depth()?;
    println!("  Result: scale={:.2} offset={:.2} gamma={:.2} RMSE={:.4}m",
        cal.scale, cal.offset, cal.gamma, cal.rmse);

    // Train occupancy
    println!("\n==> Training occupancy model...");
    let occ_cal = session.train_occupancy()?;
    println!("  Result: threshold={:.2} accuracy={:.1}%",
        occ_cal.density_threshold, occ_cal.accuracy * 100.0);

    // Export preference pairs
    println!("\n==> Exporting preference pairs...");
    let pairs = session.export_preference_pairs()?;
    println!("  Exported: {} pairs", pairs.len());

    // Submit to brain if available
    if let Some(url) = brain_url {
        println!("\n==> Submitting to brain at {url}...");
        let stored = session.submit_to_brain(url).await?;
        println!("  Stored: {} observations", stored);
    }

    println!("\n==> Training complete!");
    println!("  Data dir: {expanded}");
    println!("  Samples: {}", session.samples.len());
    println!("  Calibration: {expanded}/calibration.json");

    Ok(())
}
