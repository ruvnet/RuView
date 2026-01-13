//! MAT (Mass Casualty Assessment Tool) CLI Subcommands
//!
//! This module provides CLI commands for disaster response operations including:
//! - Survivor scanning and detection
//! - Triage status management
//! - Alert handling
//! - Zone configuration
//! - Data export

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand, ValueEnum};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tabled::{settings::Style, Table, Tabled};

use wifi_densepose_mat::{
    DisasterConfig, DisasterType, Priority, ScanZone, TriageStatus, ZoneBounds,
    ZoneStatus, domain::alert::AlertStatus,
};

/// MAT subcommand
#[derive(Subcommand, Debug)]
pub enum MatCommand {
    /// Start scanning for survivors in disaster zones
    Scan(ScanArgs),

    /// Show current scan status
    Status(StatusArgs),

    /// Manage scan zones
    Zones(ZonesArgs),

    /// List detected survivors with triage status
    Survivors(SurvivorsArgs),

    /// View and manage alerts
    Alerts(AlertsArgs),

    /// Export scan data to JSON or CSV
    Export(ExportArgs),
}

/// Arguments for the scan command
#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Zone name or ID to scan (scans all active zones if not specified)
    #[arg(short, long)]
    pub zone: Option<String>,

    /// Disaster type for optimized detection
    #[arg(short, long, value_enum, default_value = "earthquake")]
    pub disaster_type: DisasterTypeArg,

    /// Detection sensitivity (0.0-1.0)
    #[arg(short, long, default_value = "0.8")]
    pub sensitivity: f64,

    /// Maximum scan depth in meters
    #[arg(short = 'd', long, default_value = "5.0")]
    pub max_depth: f64,

    /// Enable continuous monitoring
    #[arg(short, long)]
    pub continuous: bool,

    /// Scan interval in milliseconds (for continuous mode)
    #[arg(short, long, default_value = "500")]
    pub interval: u64,

    /// Run in simulation mode (for testing)
    #[arg(long)]
    pub simulate: bool,
}

/// Disaster type argument enum for CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum DisasterTypeArg {
    Earthquake,
    BuildingCollapse,
    Avalanche,
    Flood,
    MineCollapse,
    Unknown,
}

impl From<DisasterTypeArg> for DisasterType {
    fn from(val: DisasterTypeArg) -> Self {
        match val {
            DisasterTypeArg::Earthquake => DisasterType::Earthquake,
            DisasterTypeArg::BuildingCollapse => DisasterType::BuildingCollapse,
            DisasterTypeArg::Avalanche => DisasterType::Avalanche,
            DisasterTypeArg::Flood => DisasterType::Flood,
            DisasterTypeArg::MineCollapse => DisasterType::MineCollapse,
            DisasterTypeArg::Unknown => DisasterType::Unknown,
        }
    }
}

/// Arguments for the status command
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show detailed status including all zones
    #[arg(short, long)]
    pub verbose: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "table")]
    pub format: OutputFormat,

    /// Watch mode - continuously update status
    #[arg(short, long)]
    pub watch: bool,
}

/// Arguments for the zones command
#[derive(Args, Debug)]
pub struct ZonesArgs {
    /// Zones subcommand
    #[command(subcommand)]
    pub command: ZonesCommand,
}

/// Zone management subcommands
#[derive(Subcommand, Debug)]
pub enum ZonesCommand {
    /// List all scan zones
    List {
        /// Show only active zones
        #[arg(short, long)]
        active: bool,
    },

    /// Add a new scan zone
    Add {
        /// Zone name
        #[arg(short, long)]
        name: String,

        /// Zone type (rectangle or circle)
        #[arg(short = 't', long, value_enum, default_value = "rectangle")]
        zone_type: ZoneType,

        /// Bounds: min_x,min_y,max_x,max_y for rectangle; center_x,center_y,radius for circle
        #[arg(short, long)]
        bounds: String,

        /// Detection sensitivity override
        #[arg(short, long)]
        sensitivity: Option<f64>,
    },

    /// Remove a scan zone
    Remove {
        /// Zone ID or name
        zone: String,

        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Pause a scan zone
    Pause {
        /// Zone ID or name
        zone: String,
    },

    /// Resume a paused scan zone
    Resume {
        /// Zone ID or name
        zone: String,
    },
}

/// Zone type for CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum ZoneType {
    Rectangle,
    Circle,
}

/// Arguments for the survivors command
#[derive(Args, Debug)]
pub struct SurvivorsArgs {
    /// Filter by triage status
    #[arg(short, long, value_enum)]
    pub triage: Option<TriageFilter>,

    /// Filter by zone
    #[arg(short, long)]
    pub zone: Option<String>,

    /// Sort order
    #[arg(short, long, value_enum, default_value = "triage")]
    pub sort_by: SortOrder,

    /// Output format
    #[arg(short, long, value_enum, default_value = "table")]
    pub format: OutputFormat,

    /// Show only active survivors
    #[arg(short, long)]
    pub active: bool,

    /// Maximum number of results
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
}

/// Triage status filter for CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum TriageFilter {
    Immediate,
    Delayed,
    Minor,
    Deceased,
    Unknown,
}

impl From<TriageFilter> for TriageStatus {
    fn from(val: TriageFilter) -> Self {
        match val {
            TriageFilter::Immediate => TriageStatus::Immediate,
            TriageFilter::Delayed => TriageStatus::Delayed,
            TriageFilter::Minor => TriageStatus::Minor,
            TriageFilter::Deceased => TriageStatus::Deceased,
            TriageFilter::Unknown => TriageStatus::Unknown,
        }
    }
}

/// Sort order for survivors list
#[derive(ValueEnum, Clone, Debug)]
pub enum SortOrder {
    /// Sort by triage priority (most critical first)
    Triage,
    /// Sort by detection time (newest first)
    Time,
    /// Sort by zone
    Zone,
    /// Sort by confidence score
    Confidence,
}

/// Output format
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    /// Pretty table output
    #[default]
    Table,
    /// JSON output
    Json,
    /// Compact single-line output
    Compact,
}

/// Arguments for the alerts command
#[derive(Args, Debug)]
pub struct AlertsArgs {
    /// Alerts subcommand
    #[command(subcommand)]
    pub command: Option<AlertsCommand>,

    /// Filter by priority
    #[arg(short, long, value_enum)]
    pub priority: Option<PriorityFilter>,

    /// Show only pending alerts
    #[arg(long)]
    pub pending: bool,

    /// Maximum number of alerts to show
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
}

/// Alert management subcommands
#[derive(Subcommand, Debug)]
pub enum AlertsCommand {
    /// List all alerts
    List,

    /// Acknowledge an alert
    Ack {
        /// Alert ID
        alert_id: String,

        /// Acknowledging team or person
        #[arg(short, long)]
        by: String,
    },

    /// Resolve an alert
    Resolve {
        /// Alert ID
        alert_id: String,

        /// Resolution type
        #[arg(short, long, value_enum)]
        resolution: ResolutionType,

        /// Resolution notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Escalate an alert priority
    Escalate {
        /// Alert ID
        alert_id: String,
    },
}

/// Priority filter for CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum PriorityFilter {
    Critical,
    High,
    Medium,
    Low,
}

/// Resolution type for CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum ResolutionType {
    Rescued,
    FalsePositive,
    Deceased,
    Other,
}

/// Arguments for the export command
#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Output file path
    #[arg(short, long)]
    pub output: PathBuf,

    /// Export format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: ExportFormat,

    /// Include full history
    #[arg(long)]
    pub include_history: bool,

    /// Export only survivors matching triage status
    #[arg(short, long, value_enum)]
    pub triage: Option<TriageFilter>,

    /// Export data from specific zone
    #[arg(short = 'z', long)]
    pub zone: Option<String>,
}

/// Export format
#[derive(ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Json,
    Csv,
}

// ============================================================================
// Display Structs for Tables
// ============================================================================

/// Survivor display row for tables
#[derive(Tabled, Serialize, Deserialize)]
struct SurvivorRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Zone")]
    zone: String,
    #[tabled(rename = "Triage")]
    triage: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Confidence")]
    confidence: String,
    #[tabled(rename = "Location")]
    location: String,
    #[tabled(rename = "Last Update")]
    last_update: String,
}

/// Zone display row for tables
#[derive(Tabled, Serialize, Deserialize)]
struct ZoneRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Area (m2)")]
    area: String,
    #[tabled(rename = "Scans")]
    scan_count: u32,
    #[tabled(rename = "Detections")]
    detections: u32,
    #[tabled(rename = "Last Scan")]
    last_scan: String,
}

/// Alert display row for tables
#[derive(Tabled, Serialize, Deserialize)]
struct AlertRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Survivor")]
    survivor_id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Age")]
    age: String,
}

/// Status display for system overview
#[derive(Serialize, Deserialize)]
struct SystemStatus {
    scanning: bool,
    active_zones: usize,
    total_zones: usize,
    survivors_detected: usize,
    critical_survivors: usize,
    pending_alerts: usize,
    disaster_type: String,
    uptime: String,
}

// ============================================================================
// Command Execution
// ============================================================================

/// Execute a MAT command
pub async fn execute(command: MatCommand) -> Result<()> {
    match command {
        MatCommand::Scan(args) => execute_scan(args).await,
        MatCommand::Status(args) => execute_status(args).await,
        MatCommand::Zones(args) => execute_zones(args).await,
        MatCommand::Survivors(args) => execute_survivors(args).await,
        MatCommand::Alerts(args) => execute_alerts(args).await,
        MatCommand::Export(args) => execute_export(args).await,
    }
}

/// Execute the scan command
async fn execute_scan(args: ScanArgs) -> Result<()> {
    println!(
        "{} Starting survivor scan...",
        "[MAT]".bright_cyan().bold()
    );
    println!();

    // Display configuration
    println!("{}", "Configuration:".bold());
    println!(
        "  {} {:?}",
        "Disaster Type:".dimmed(),
        args.disaster_type
    );
    println!(
        "  {} {:.1}",
        "Sensitivity:".dimmed(),
        args.sensitivity
    );
    println!(
        "  {} {:.1}m",
        "Max Depth:".dimmed(),
        args.max_depth
    );
    println!(
        "  {} {}",
        "Continuous:".dimmed(),
        if args.continuous { "Yes" } else { "No" }
    );
    if args.continuous {
        println!(
            "  {} {}ms",
            "Interval:".dimmed(),
            args.interval
        );
    }
    if let Some(ref zone) = args.zone {
        println!("  {} {}", "Zone:".dimmed(), zone);
    }
    println!();

    if args.simulate {
        println!(
            "{} Running in simulation mode",
            "[SIMULATION]".yellow().bold()
        );
        println!();

        // Simulate some detections
        simulate_scan_output().await?;
    } else {
        // Build configuration
        let config = DisasterConfig::builder()
            .disaster_type(args.disaster_type.into())
            .sensitivity(args.sensitivity)
            .max_depth(args.max_depth)
            .continuous_monitoring(args.continuous)
            .scan_interval_ms(args.interval)
            .build();

        println!(
            "{} Initializing detection pipeline with config: {:?}",
            "[INFO]".blue(),
            config.disaster_type
        );
        println!(
            "{} Waiting for hardware connection...",
            "[INFO]".blue()
        );
        println!();
        println!(
            "{} No hardware detected. Use --simulate for demo mode.",
            "[WARN]".yellow()
        );
    }

    Ok(())
}

/// Simulate scan output for demonstration
async fn simulate_scan_output() -> Result<()> {
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;

    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
            .progress_chars("#>-"),
    );

    for i in 0..100 {
        pb.set_position(i);
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Simulate detection events
        if i == 25 {
            pb.suspend(|| {
                println!();
                print_detection(
                    "SURV-001",
                    "Zone A",
                    TriageStatus::Immediate,
                    0.92,
                    Some((12.5, 8.3, -2.1)),
                );
            });
        }
        if i == 55 {
            pb.suspend(|| {
                print_detection(
                    "SURV-002",
                    "Zone A",
                    TriageStatus::Delayed,
                    0.78,
                    Some((15.2, 10.1, -1.5)),
                );
            });
        }
        if i == 80 {
            pb.suspend(|| {
                print_detection(
                    "SURV-003",
                    "Zone B",
                    TriageStatus::Minor,
                    0.85,
                    Some((8.7, 22.4, -0.8)),
                );
            });
        }
    }

    pb.finish_with_message("Scan complete");
    println!();
    println!(
        "{} Scan complete. Detected {} survivors.",
        "[MAT]".bright_cyan().bold(),
        "3".green().bold()
    );
    println!(
        "  {} {}  {} {}  {} {}",
        "IMMEDIATE:".red().bold(),
        "1",
        "DELAYED:".yellow().bold(),
        "1",
        "MINOR:".green().bold(),
        "1"
    );

    Ok(())
}

/// Print a detection event
fn print_detection(
    id: &str,
    zone: &str,
    triage: TriageStatus,
    confidence: f64,
    location: Option<(f64, f64, f64)>,
) {
    let triage_str = format_triage(&triage);
    let location_str = location
        .map(|(x, y, z)| format!("({:.1}, {:.1}, {:.1})", x, y, z))
        .unwrap_or_else(|| "Unknown".to_string());

    println!(
        "{} {} detected in {} - {} | Confidence: {:.0}% | Location: {}",
        format!("[{}]", triage_str).bold(),
        id.cyan(),
        zone,
        triage_str,
        confidence * 100.0,
        location_str.dimmed()
    );
}

/// Execute the status command
async fn execute_status(args: StatusArgs) -> Result<()> {
    // In a real implementation, this would connect to a running daemon
    let status = SystemStatus {
        scanning: false,
        active_zones: 0,
        total_zones: 0,
        survivors_detected: 0,
        critical_survivors: 0,
        pending_alerts: 0,
        disaster_type: "Not configured".to_string(),
        uptime: "N/A".to_string(),
    };

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        OutputFormat::Compact => {
            println!(
                "scanning={} zones={}/{} survivors={} critical={} alerts={}",
                status.scanning,
                status.active_zones,
                status.total_zones,
                status.survivors_detected,
                status.critical_survivors,
                status.pending_alerts
            );
        }
        OutputFormat::Table => {
            println!("{}", "MAT System Status".bold().cyan());
            println!("{}", "=".repeat(50));
            println!(
                "  {} {}",
                "Scanning:".dimmed(),
                if status.scanning {
                    "Active".green()
                } else {
                    "Inactive".red()
                }
            );
            println!(
                "  {} {}/{}",
                "Zones:".dimmed(),
                status.active_zones,
                status.total_zones
            );
            println!(
                "  {} {}",
                "Disaster Type:".dimmed(),
                status.disaster_type
            );
            println!(
                "  {} {}",
                "Survivors Detected:".dimmed(),
                status.survivors_detected
            );
            println!(
                "  {} {}",
                "Critical (Immediate):".dimmed(),
                if status.critical_survivors > 0 {
                    status.critical_survivors.to_string().red().bold()
                } else {
                    status.critical_survivors.to_string().normal()
                }
            );
            println!(
                "  {} {}",
                "Pending Alerts:".dimmed(),
                if status.pending_alerts > 0 {
                    status.pending_alerts.to_string().yellow().bold()
                } else {
                    status.pending_alerts.to_string().normal()
                }
            );
            println!("  {} {}", "Uptime:".dimmed(), status.uptime);
            println!();

            if !status.scanning {
                println!(
                    "{} No active scan. Run '{}' to start.",
                    "[INFO]".blue(),
                    "wifi-densepose mat scan".green()
                );
            }
        }
    }

    Ok(())
}

/// Execute the zones command
async fn execute_zones(args: ZonesArgs) -> Result<()> {
    match args.command {
        ZonesCommand::List { active } => {
            println!("{}", "Scan Zones".bold().cyan());
            println!("{}", "=".repeat(80));

            // Demo data
            let zones = vec![
                ZoneRow {
                    id: "zone-001".to_string(),
                    name: "Building A - North Wing".to_string(),
                    status: format_zone_status(&ZoneStatus::Active),
                    area: "1500.0".to_string(),
                    scan_count: 42,
                    detections: 3,
                    last_scan: "2 min ago".to_string(),
                },
                ZoneRow {
                    id: "zone-002".to_string(),
                    name: "Building A - South Wing".to_string(),
                    status: format_zone_status(&ZoneStatus::Paused),
                    area: "1200.0".to_string(),
                    scan_count: 28,
                    detections: 1,
                    last_scan: "15 min ago".to_string(),
                },
            ];

            let filtered: Vec<_> = if active {
                zones
                    .into_iter()
                    .filter(|z| z.status.contains("Active"))
                    .collect()
            } else {
                zones
            };

            if filtered.is_empty() {
                println!("No zones configured. Use 'wifi-densepose mat zones add' to create one.");
            } else {
                let table = Table::new(filtered).with(Style::rounded()).to_string();
                println!("{}", table);
            }
        }
        ZonesCommand::Add {
            name,
            zone_type,
            bounds,
            sensitivity,
        } => {
            // Parse bounds
            let bounds_parsed: Result<ZoneBounds, _> = parse_bounds(&zone_type, &bounds);
            match bounds_parsed {
                Ok(zone_bounds) => {
                    let zone = if let Some(sens) = sensitivity {
                        let mut params = wifi_densepose_mat::ScanParameters::default();
                        params.sensitivity = sens;
                        ScanZone::with_parameters(&name, zone_bounds, params)
                    } else {
                        ScanZone::new(&name, zone_bounds)
                    };

                    println!(
                        "{} Zone '{}' created with ID: {}",
                        "[OK]".green().bold(),
                        name.cyan(),
                        zone.id()
                    );
                    println!("  Area: {:.1} m2", zone.area());
                }
                Err(e) => {
                    eprintln!("{} Failed to parse bounds: {}", "[ERROR]".red().bold(), e);
                    eprintln!("  Expected format for rectangle: min_x,min_y,max_x,max_y");
                    eprintln!("  Expected format for circle: center_x,center_y,radius");
                    return Err(e);
                }
            }
        }
        ZonesCommand::Remove { zone, force } => {
            if !force {
                println!(
                    "{} This will remove zone '{}' and stop any active scans.",
                    "[WARN]".yellow().bold(),
                    zone
                );
                println!("Use --force to confirm.");
            } else {
                println!(
                    "{} Zone '{}' removed.",
                    "[OK]".green().bold(),
                    zone.cyan()
                );
            }
        }
        ZonesCommand::Pause { zone } => {
            println!(
                "{} Zone '{}' paused.",
                "[OK]".green().bold(),
                zone.cyan()
            );
        }
        ZonesCommand::Resume { zone } => {
            println!(
                "{} Zone '{}' resumed.",
                "[OK]".green().bold(),
                zone.cyan()
            );
        }
    }

    Ok(())
}

/// Parse bounds string into ZoneBounds
fn parse_bounds(zone_type: &ZoneType, bounds: &str) -> Result<ZoneBounds> {
    let parts: Vec<f64> = bounds
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to parse bounds values as numbers")?;

    match zone_type {
        ZoneType::Rectangle => {
            if parts.len() != 4 {
                anyhow::bail!(
                    "Rectangle requires 4 values: min_x,min_y,max_x,max_y (got {})",
                    parts.len()
                );
            }
            Ok(ZoneBounds::rectangle(parts[0], parts[1], parts[2], parts[3]))
        }
        ZoneType::Circle => {
            if parts.len() != 3 {
                anyhow::bail!(
                    "Circle requires 3 values: center_x,center_y,radius (got {})",
                    parts.len()
                );
            }
            Ok(ZoneBounds::circle(parts[0], parts[1], parts[2]))
        }
    }
}

/// Execute the survivors command
async fn execute_survivors(args: SurvivorsArgs) -> Result<()> {
    // Demo data
    let survivors = vec![
        SurvivorRow {
            id: "SURV-001".to_string(),
            zone: "Zone A".to_string(),
            triage: format_triage(&TriageStatus::Immediate),
            status: "Active".green().to_string(),
            confidence: "92%".to_string(),
            location: "(12.5, 8.3, -2.1)".to_string(),
            last_update: "30s ago".to_string(),
        },
        SurvivorRow {
            id: "SURV-002".to_string(),
            zone: "Zone A".to_string(),
            triage: format_triage(&TriageStatus::Delayed),
            status: "Active".green().to_string(),
            confidence: "78%".to_string(),
            location: "(15.2, 10.1, -1.5)".to_string(),
            last_update: "1m ago".to_string(),
        },
        SurvivorRow {
            id: "SURV-003".to_string(),
            zone: "Zone B".to_string(),
            triage: format_triage(&TriageStatus::Minor),
            status: "Active".green().to_string(),
            confidence: "85%".to_string(),
            location: "(8.7, 22.4, -0.8)".to_string(),
            last_update: "2m ago".to_string(),
        },
    ];

    // Apply filters
    let mut filtered = survivors;

    if let Some(ref triage_filter) = args.triage {
        let status: TriageStatus = triage_filter.clone().into();
        let status_str = format_triage(&status);
        filtered.retain(|s| s.triage == status_str);
    }

    if let Some(ref zone) = args.zone {
        filtered.retain(|s| s.zone.contains(zone));
    }

    if let Some(limit) = args.limit {
        filtered.truncate(limit);
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        }
        OutputFormat::Compact => {
            for s in &filtered {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    s.id, s.zone, s.triage, s.confidence, s.location
                );
            }
        }
        OutputFormat::Table => {
            println!("{}", "Detected Survivors".bold().cyan());
            println!("{}", "=".repeat(100));

            if filtered.is_empty() {
                println!("No survivors detected matching criteria.");
            } else {
                // Print summary
                let immediate = filtered
                    .iter()
                    .filter(|s| s.triage.contains("IMMEDIATE"))
                    .count();
                let delayed = filtered
                    .iter()
                    .filter(|s| s.triage.contains("DELAYED"))
                    .count();
                let minor = filtered
                    .iter()
                    .filter(|s| s.triage.contains("MINOR"))
                    .count();

                println!(
                    "Total: {} | {} {} | {} {} | {} {}",
                    filtered.len().to_string().bold(),
                    "IMMEDIATE:".red().bold(),
                    immediate,
                    "DELAYED:".yellow().bold(),
                    delayed,
                    "MINOR:".green().bold(),
                    minor
                );
                println!();

                let table = Table::new(filtered).with(Style::rounded()).to_string();
                println!("{}", table);
            }
        }
    }

    Ok(())
}

/// Execute the alerts command
async fn execute_alerts(args: AlertsArgs) -> Result<()> {
    match args.command {
        Some(AlertsCommand::Ack { alert_id, by }) => {
            println!(
                "{} Alert {} acknowledged by {}",
                "[OK]".green().bold(),
                alert_id.cyan(),
                by
            );
        }
        Some(AlertsCommand::Resolve {
            alert_id,
            resolution,
            notes,
        }) => {
            println!(
                "{} Alert {} resolved as {:?}",
                "[OK]".green().bold(),
                alert_id.cyan(),
                resolution
            );
            if let Some(notes) = notes {
                println!("  Notes: {}", notes);
            }
        }
        Some(AlertsCommand::Escalate { alert_id }) => {
            println!(
                "{} Alert {} escalated to higher priority",
                "[OK]".green().bold(),
                alert_id.cyan()
            );
        }
        Some(AlertsCommand::List) | None => {
            // Demo data
            let alerts = vec![
                AlertRow {
                    id: "ALRT-001".to_string(),
                    priority: format_priority(Priority::Critical),
                    status: format_alert_status(&AlertStatus::Pending),
                    survivor_id: "SURV-001".to_string(),
                    title: "Immediate: Survivor detected".to_string(),
                    age: "5m".to_string(),
                },
                AlertRow {
                    id: "ALRT-002".to_string(),
                    priority: format_priority(Priority::High),
                    status: format_alert_status(&AlertStatus::Acknowledged),
                    survivor_id: "SURV-002".to_string(),
                    title: "Delayed: Survivor needs attention".to_string(),
                    age: "12m".to_string(),
                },
            ];

            let mut filtered = alerts;

            if args.pending {
                filtered.retain(|a| a.status.contains("Pending"));
            }

            if let Some(limit) = args.limit {
                filtered.truncate(limit);
            }

            println!("{}", "Alerts".bold().cyan());
            println!("{}", "=".repeat(100));

            if filtered.is_empty() {
                println!("No alerts.");
            } else {
                let pending = filtered.iter().filter(|a| a.status.contains("Pending")).count();
                if pending > 0 {
                    println!(
                        "{} {} pending alert(s) require attention!",
                        "[ALERT]".red().bold(),
                        pending
                    );
                    println!();
                }

                let table = Table::new(filtered).with(Style::rounded()).to_string();
                println!("{}", table);
            }
        }
    }

    Ok(())
}

/// Execute the export command
async fn execute_export(args: ExportArgs) -> Result<()> {
    println!(
        "{} Exporting data to {}...",
        "[INFO]".blue(),
        args.output.display()
    );

    // Demo export data
    #[derive(Serialize)]
    struct ExportData {
        exported_at: DateTime<Utc>,
        survivors: Vec<SurvivorExport>,
        zones: Vec<ZoneExport>,
        alerts: Vec<AlertExport>,
    }

    #[derive(Serialize)]
    struct SurvivorExport {
        id: String,
        zone_id: String,
        triage_status: String,
        confidence: f64,
        location: Option<(f64, f64, f64)>,
        first_detected: DateTime<Utc>,
        last_updated: DateTime<Utc>,
    }

    #[derive(Serialize)]
    struct ZoneExport {
        id: String,
        name: String,
        status: String,
        area: f64,
        scan_count: u32,
    }

    #[derive(Serialize)]
    struct AlertExport {
        id: String,
        priority: String,
        status: String,
        survivor_id: String,
        created_at: DateTime<Utc>,
    }

    let data = ExportData {
        exported_at: Utc::now(),
        survivors: vec![SurvivorExport {
            id: "SURV-001".to_string(),
            zone_id: "zone-001".to_string(),
            triage_status: "Immediate".to_string(),
            confidence: 0.92,
            location: Some((12.5, 8.3, -2.1)),
            first_detected: Utc::now() - chrono::Duration::minutes(15),
            last_updated: Utc::now() - chrono::Duration::seconds(30),
        }],
        zones: vec![ZoneExport {
            id: "zone-001".to_string(),
            name: "Building A - North Wing".to_string(),
            status: "Active".to_string(),
            area: 1500.0,
            scan_count: 42,
        }],
        alerts: vec![AlertExport {
            id: "ALRT-001".to_string(),
            priority: "Critical".to_string(),
            status: "Pending".to_string(),
            survivor_id: "SURV-001".to_string(),
            created_at: Utc::now() - chrono::Duration::minutes(5),
        }],
    };

    match args.format {
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(&data)?;
            std::fs::write(&args.output, json)?;
        }
        ExportFormat::Csv => {
            let mut wtr = csv::Writer::from_path(&args.output)?;
            for survivor in &data.survivors {
                wtr.serialize(survivor)?;
            }
            wtr.flush()?;
        }
    }

    println!(
        "{} Export complete: {}",
        "[OK]".green().bold(),
        args.output.display()
    );

    Ok(())
}

// ============================================================================
// Formatting Helpers
// ============================================================================

/// Format triage status with color
fn format_triage(status: &TriageStatus) -> String {
    match status {
        TriageStatus::Immediate => "IMMEDIATE (Red)".red().bold().to_string(),
        TriageStatus::Delayed => "DELAYED (Yellow)".yellow().bold().to_string(),
        TriageStatus::Minor => "MINOR (Green)".green().bold().to_string(),
        TriageStatus::Deceased => "DECEASED (Black)".dimmed().to_string(),
        TriageStatus::Unknown => "UNKNOWN".dimmed().to_string(),
    }
}

/// Format zone status with color
fn format_zone_status(status: &ZoneStatus) -> String {
    match status {
        ZoneStatus::Active => "Active".green().to_string(),
        ZoneStatus::Paused => "Paused".yellow().to_string(),
        ZoneStatus::Complete => "Complete".blue().to_string(),
        ZoneStatus::Inaccessible => "Inaccessible".red().to_string(),
        ZoneStatus::Deactivated => "Deactivated".dimmed().to_string(),
    }
}

/// Format priority with color
fn format_priority(priority: Priority) -> String {
    match priority {
        Priority::Critical => "CRITICAL".red().bold().to_string(),
        Priority::High => "HIGH".bright_red().to_string(),
        Priority::Medium => "MEDIUM".yellow().to_string(),
        Priority::Low => "LOW".blue().to_string(),
    }
}

/// Format alert status with color
fn format_alert_status(status: &AlertStatus) -> String {
    match status {
        AlertStatus::Pending => "Pending".red().to_string(),
        AlertStatus::Acknowledged => "Acknowledged".yellow().to_string(),
        AlertStatus::InProgress => "In Progress".blue().to_string(),
        AlertStatus::Resolved => "Resolved".green().to_string(),
        AlertStatus::Cancelled => "Cancelled".dimmed().to_string(),
        AlertStatus::Expired => "Expired".dimmed().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rectangle_bounds() {
        let result = parse_bounds(&ZoneType::Rectangle, "0,0,10,20");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_circle_bounds() {
        let result = parse_bounds(&ZoneType::Circle, "5,5,10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_bounds() {
        let result = parse_bounds(&ZoneType::Rectangle, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_disaster_type_conversion() {
        let dt: DisasterType = DisasterTypeArg::Earthquake.into();
        assert!(matches!(dt, DisasterType::Earthquake));
    }

    #[test]
    fn test_triage_filter_conversion() {
        let ts: TriageStatus = TriageFilter::Immediate.into();
        assert!(matches!(ts, TriageStatus::Immediate));
    }
}
