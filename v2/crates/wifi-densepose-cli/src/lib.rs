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

pub mod mat;

/// WiFi-DensePose Command Line Interface
#[derive(Parser, Debug)]
#[command(name = "wifi-densepose")]
#[command(author, version, about = "WiFi-based pose estimation and disaster response")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Mass Casualty Assessment Tool commands
    #[command(subcommand)]
    Mat(mat::MatCommand),

    /// Display version information
    Version,
}
