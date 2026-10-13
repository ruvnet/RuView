//! WiFi-DensePose CLI
//!
//! Command-line interface for WiFi-DensePose system, including the
//! Mass Casualty Assessment Tool (MAT) for disaster response.
//!
//! # Features
//!
//! - **mat**: Disaster survivor detection and triage management
//! - **version**: Display version information
//!
//! # Usage
//!
//! ```bash
//! # Start scanning for survivors
//! wifi-densepose mat scan --zone "Building A"
//!
//! # View current scan status
//! wifi-densepose mat status
//!
//! # List detected survivors
//! wifi-densepose mat survivors --sort-by triage
//!
//! # View and manage alerts
//! wifi-densepose mat alerts
//! ```

use clap::{Parser, Subcommand};

pub mod auth;
pub mod calibrate;
pub mod calibrate_api;
pub mod occupancy_readiness;
#[cfg(feature = "mat")]
pub mod mat;
pub mod room;
pub mod spaces;

/// WiFi-DensePose Command Line Interface
#[derive(Parser, Debug)]
#[command(name = "wifi-densepose")]
#[command(
    author,
    version,
    about = "WiFi-based pose estimation and disaster response"
)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Sign in to Cognitum (ADR-271). Stores a token this machine can present
    /// to a RuView sensing server instead of sharing one static API token.
    Login(auth::LoginArgs),

    /// Forget the locally stored Cognitum credentials.
    Logout(auth::LogoutArgs),

    /// Show the stored Cognitum session: account, scope, and whether it is live.
    Whoami(auth::WhoamiArgs),

    /// Read tenant-scoped semantic state from Cognitum Spaces (ADR-325).
    Spaces(spaces::SpacesArgs),

    /// Empty-room baseline calibration (ADR-135).
    /// Captures CSI frames via UDP and saves a per-subcarrier statistical
    /// baseline used for real-time motion z-scoring and CIR reference.
    Calibrate(calibrate::CalibrateArgs),

    /// Run the calibration HTTP API (ADR-135/151) for a UI to drive.
    /// Receives ESP32 CSI over UDP and exposes start/status/stop/result
    /// endpoints at `/api/v1/calibration/*` (CORS-enabled).
    CalibrateServe(calibrate_api::CalibrateServeArgs),

    /// Guided per-room enrollment (ADR-151 Stage 2) — walk the anchor sequence
    /// against a baseline, writing labelled features.
    Enroll(room::EnrollArgs),

    /// Train the per-room specialist bank from an enrollment (ADR-151 Stage 4).
    TrainRoom(room::TrainRoomArgs),

    /// Show a trained specialist bank's summary.
    RoomStatus(room::RoomStatusArgs),

    /// Live mixture-of-specialists readout from the CSI stream (ADR-151 Stage 5).
    RoomWatch(room::RoomWatchArgs),

    /// Mass Casualty Assessment Tool commands
    #[cfg(feature = "mat")]
    #[command(subcommand)]
    Mat(mat::MatCommand),

    /// Display version information
    Version,
}
