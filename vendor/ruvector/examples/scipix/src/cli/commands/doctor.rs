//! Doctor command for environment analysis and configuration optimization
//!
//! Analyzes the system environment and provides recommendations for optimal
//! SciPix configuration based on available hardware and software capabilities.

use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Arguments for the doctor command
#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {
    /// Run in fix mode to automatically apply recommendations
    #[arg(long, help = "Automatically apply safe fixes")]
    pub fix: bool,

    /// Output detailed diagnostic information
    #[arg(long, short, help = "Show detailed diagnostic information")]
    pub verbose: bool,

    /// Output results as JSON
    #[arg(long, help = "Output results as JSON")]
    pub json: bool,

    /// Check only specific category (cpu, memory, config, deps, all)
    #[arg(long, default_value = "all", help = "Category to check")]
    pub check: CheckCategory,

    /// Path to configuration file to validate
    #[arg(long, help = "Path to configuration file to validate")]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum CheckCategory {
    #[default]
    All,
    Cpu,
    Memory,
    Config,
    Deps,
    Network,
}

/// Status of a diagnostic check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warning,
    Fail,
    Info,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "✓"),
            CheckStatus::Warning => write!(f, "⚠"),
            CheckStatus::Fail => write!(f, "✗"),
            CheckStatus::Info => write!(f, "ℹ"),
        }
    }
}

/// A single diagnostic check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub category: String,
    pub status: CheckStatus,
    pub message: String,
    pub recommendation: Option<String>,
    pub auto_fixable: bool,
}

/// Complete diagnostic report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub timestamp: String,
    pub system_info: SystemInfo,
    pub checks: Vec<DiagnosticCheck>,
    pub recommendations: Vec<String>,
    pub optimal_config: OptimalConfig,
}

/// System information gathered during diagnosis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu_count: usize,
    pub cpu_brand: String,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub simd_features: SimdFeatures,
}

/// SIMD feature detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdFeatures {
    pub sse2: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub neon: bool,
    pub best_available: String,
}

/// Optimal configuration recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimalConfig {
    pub batch_size: usize,
    pub worker_threads: usize,
    pub simd_backend: String,
    pub memory_limit_mb: u64,
    pub preprocessing_mode: String,
    pub cache_enabled: bool,
    pub cache_size_mb: u64,
}

/// Execute the doctor command
pub async fn execute(args: DoctorArgs) -> Result<()> {
    if !args.json {
        println!("🩺 SciPix Doctor - Environment Analysis\n");
        println!("═══════════════════════════════════════════════════════════\n");
    }

    let mut checks = Vec::new();

    // Gather system information
    let system_info = gather_system_info();

    // Run checks based on category
    match args.check {
        CheckCategory::All => {
            checks.extend(check_cpu(&system_info, args.verbose));
            checks.extend(check_memory(&system_info, args.verbose));
            checks.extend(check_dependencies(args.verbose));
            checks.extend(check_config(&args.config_path, args.verbose));
            checks.extend(check_network(args.verbose).await);
        }
        CheckCategory::Cpu => {
            checks.extend(check_cpu(&system_info, args.verbose));
        }
        CheckCategory::Memory => {
            checks.extend(check_memory(&system_info, args.verbose));
        }
        CheckCategory::Config => {
            checks.extend(check_config(&args.config_path, args.verbose));
        }
        CheckCategory::Deps => {
            checks.extend(check_dependencies(args.verbose));
        }
        CheckCategory::Network => {
            checks.extend(check_network(args.verbose).await);
        }
    }

    // Generate optimal configuration
    let optimal_config = generate_optimal_config(&system_info);

    // Collect recommendations
    let recommendations: Vec<String> = checks
        .iter()
        .filter_map(|c| c.recommendation.clone())
        .collect();

    // Create report
    let report = DiagnosticReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        system_info: system_info.clone(),
        checks: checks.clone(),
        recommendations: recommendations.clone(),
        optimal_config: optimal_config.clone(),
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // Print system info
    print_system_info(&system_info);

    // Print check results
    print_check_results(&checks);

    // Print recommendations
    if !recommendations.is_empty() {
        println!("\n📋 Recommendations:");
        println!("───────────────────────────────────────────────────────────");
        for (i, rec) in recommendations.iter().enumerate() {
            println!("  {}. {}", i + 1, rec);
        }
    }

    // Print optimal configuration
    print_optimal_config(&optimal_config);

    // Apply fixes if requested
    if args.fix {
        apply_fixes(&checks).await?;
    }

    // Print summary
    print_summary(&checks);

    Ok(())
}

fn gather_system_info() -> SystemInfo {
    let cpu_count = num_cpus::get();

    // Get CPU brand string
    let cpu_brand = get_cpu_brand();

    // Get memory info
    let (total_memory_mb, available_memory_mb) = get_memory_info();

    // Detect SIMD features
    let simd_features = detect_simd_features();

    SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_count,
        cpu_brand,
        total_memory_mb,
        available_memory_mb,
        simd_features,
    }
}

fn get_cpu_brand() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        if let Some(brand) = get_x86_cpu_brand() {
            return brand;
        }
    }

    // Fallback
    format!("{} processor", std::env::consts::ARCH)
}

#[cfg(target_arch = "x86_64")]
fn get_x86_cpu_brand() -> Option<String> {
    // Try to read from /proc/cpuinfo on Linux
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                if let Some(brand) = line.split(':').nth(1) {
                    return Some(brand.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(not(target_arch = "x86_64"))]
fn get_x86_cpu_brand() -> Option<String> {
    None
}

fn get_memory_info() -> (u64, u64) {
    // Try to read from /proc/meminfo on Linux
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64;
        let mut available = 0u64;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = parse_meminfo_value(line) {
                    total = kb / 1024; // Convert to MB
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(kb) = parse_meminfo_value(line) {
                    available = kb / 1024; // Convert to MB
                }
            }
        }

        if total > 0 {
            return (total, available);
        }
    }

    // Fallback values
    (8192, 4096)
}

fn parse_meminfo_value(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1).and_then(|s| s.parse().ok())
}

fn detect_simd_features() -> SimdFeatures {
    let mut features = SimdFeatures {
        sse2: false,
        sse4_1: false,
        sse4_2: false,
        avx: false,
        avx2: false,
        avx512f: false,
        neon: false,
        best_available: "scalar".to_string(),
    };

    #[cfg(target_arch = "x86_64")]
    {
        features.sse2 = is_x86_feature_detected!("sse2");
        features.sse4_1 = is_x86_feature_detected!("sse4.1");
        features.sse4_2 = is_x86_feature_detected!("sse4.2");
        features.avx = is_x86_feature_detected!("avx");
        features.avx2 = is_x86_feature_detected!("avx2");
        features.avx512f = is_x86_feature_detected!("avx512f");

        if features.avx512f {
            features.best_available = "AVX-512".to_string();
        } else if features.avx2 {
            features.best_available = "AVX2".to_string();
        } else if features.avx {
            features.best_available = "AVX".to_string();
        } else if features.sse4_2 {
            features.best_available = "SSE4.2".to_string();
        } else if features.sse2 {
            features.best_available = "SSE2".to_string();
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        features.neon = true; // NEON is always available on AArch64
        features.best_available = "NEON".to_string();
    }

    features
}

fn check_cpu(system_info: &SystemInfo, verbose: bool) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // CPU count check
    let cpu_status = if system_info.cpu_count >= 8 {
        CheckStatus::Pass
    } else if system_info.cpu_count >= 4 {
        CheckStatus::Warning
    } else {
        CheckStatus::Fail
    };

    checks.push(DiagnosticCheck {
        name: "CPU Cores".to_string(),
        category: "CPU".to_string(),
        status: cpu_status,
        message: format!("{} cores detected", system_info.cpu_count),
        recommendation: if system_info.cpu_count < 4 {
            Some(
                "Consider running on a machine with more CPU cores for better batch processing"
                    .to_string(),
            )
        } else {
            None
        },
        auto_fixable: false,
    });

    // SIMD check
    let simd_status = match system_info.simd_features.best_available.as_str() {
        "AVX-512" | "AVX2" => CheckStatus::Pass,
        "AVX" | "SSE4.2" | "NEON" => CheckStatus::Warning,
        _ => CheckStatus::Fail,
    };

    checks.push(DiagnosticCheck {
        name: "SIMD Support".to_string(),
        category: "CPU".to_string(),
        status: simd_status,
        message: format!(
            "Best SIMD: {} (SSE2: {}, AVX: {}, AVX2: {}, AVX-512: {})",
            system_info.simd_features.best_available,
            if system_info.simd_features.sse2 {
                "✓"
            } else {
                "✗"
            },
            if system_info.simd_features.avx {
                "✓"
            } else {
                "✗"
            },
            if system_info.simd_features.avx2 {
                "✓"
            } else {
                "✗"
            },
            if system_info.simd_features.avx512f {
                "✓"
            } else {
                "✗"
            },
        ),
        recommendation: if simd_status == CheckStatus::Fail {
            Some("Upgrade to a CPU with AVX2 support for 4x faster preprocessing".to_string())
        } else {
            None
        },
        auto_fixable: false,
    });

    if verbose {
        checks.push(DiagnosticCheck {
            name: "CPU Brand".to_string(),
            category: "CPU".to_string(),
            status: CheckStatus::Info,
            message: system_info.cpu_brand.clone(),
            recommendation: None,
            auto_fixable: false,
        });
    }

    checks
}

fn check_memory(system_info: &SystemInfo, verbose: bool) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // Total memory check
    let mem_status = if system_info.total_memory_mb >= 16384 {
        CheckStatus::Pass
    } else if system_info.total_memory_mb >= 8192 {
        CheckStatus::Warning
    } else {
        CheckStatus::Fail
    };

    checks.push(DiagnosticCheck {
        name: "Total Memory".to_string(),
        category: "Memory".to_string(),
        status: mem_status,
        message: format!("{} MB total", system_info.total_memory_mb),
        recommendation: if system_info.total_memory_mb < 8192 {
            Some("Consider upgrading to at least 8GB RAM for optimal batch processing".to_string())
        } else {
            None
        },
        auto_fixable: false,
    });

    // Available memory check
    let avail_ratio = system_info.available_memory_mb as f64 / system_info.total_memory_mb as f64;
    let avail_status = if avail_ratio >= 0.5 {
        CheckStatus::Pass
    } else if avail_ratio >= 0.25 {
        CheckStatus::Warning
    } else {
        CheckStatus::Fail
    };

    checks.push(DiagnosticCheck {
        name: "Available Memory".to_string(),
        category: "Memory".to_string(),
        status: avail_status,
        message: format!(
            "{} MB available ({:.1}%)",
            system_info.available_memory_mb,
            avail_ratio * 100.0
        ),
        recommendation: if avail_status == CheckStatus::Fail {
            Some("Close some applications to free up memory before batch processing".to_string())
        } else {
            None
        },
        auto_fixable: false,
    });

    if verbose {
        // Memory per core
        let mem_per_core = system_info.total_memory_mb / system_info.cpu_count as u64;
        checks.push(DiagnosticCheck {
            name: "Memory per Core".to_string(),
            category: "Memory".to_string(),
            status: CheckStatus::Info,
            message: format!("{} MB/core", mem_per_core),
            recommendation: None,
            auto_fixable: false,
        });
    }

    checks
}

fn check_dependencies(verbose: bool) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // Check for ONNX Runtime
    let onnx_status = check_onnx_runtime();
    checks.push(DiagnosticCheck {
        name: "ONNX Runtime".to_string(),
        category: "Dependencies".to_string(),
        status: if onnx_status.0 {
            CheckStatus::Pass
        } else {
            CheckStatus::Warning
        },
        message: onnx_status.1.clone(),
        recommendation: if !onnx_status.0 {
            Some(
                "Install ONNX Runtime for neural network acceleration: https://onnxruntime.ai/"
                    .to_string(),
            )
        } else {
            None
        },
        auto_fixable: false,
    });

    // Check for image processing libraries
    checks.push(DiagnosticCheck {
        name: "Image Processing".to_string(),
        category: "Dependencies".to_string(),
        status: CheckStatus::Pass,
        message: "image crate available (built-in)".to_string(),
        recommendation: None,
        auto_fixable: false,
    });

    // Check for OpenSSL (for HTTPS)
    let openssl_available = std::process::Command::new("openssl")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    checks.push(DiagnosticCheck {
        name: "OpenSSL".to_string(),
        category: "Dependencies".to_string(),
        status: if openssl_available {
            CheckStatus::Pass
        } else {
            CheckStatus::Warning
        },
        message: if openssl_available {
            "OpenSSL available for HTTPS".to_string()
        } else {
            "OpenSSL not found".to_string()
        },
        recommendation: if !openssl_available {
            Some("Install OpenSSL for secure API communication".to_string())
        } else {
            None
        },
        auto_fixable: false,
    });

    if verbose {
        // Check Rust version
        if let Ok(output) = std::process::Command::new("rustc")
            .arg("--version")
            .output()
        {
            let version = String::from_utf8_lossy(&output.stdout);
            checks.push(DiagnosticCheck {
                name: "Rust Compiler".to_string(),
                category: "Dependencies".to_string(),
                status: CheckStatus::Info,
                message: version.trim().to_string(),
                recommendation: None,
                auto_fixable: false,
            });
        }
    }

    checks
}

fn check_onnx_runtime() -> (bool, String) {
    // Check for ONNX runtime shared library
    let lib_paths = [
        "/usr/lib/libonnxruntime.so",
        "/usr/local/lib/libonnxruntime.so",
        "/opt/onnxruntime/lib/libonnxruntime.so",
    ];

    for path in &lib_paths {
        if std::path::Path::new(path).exists() {
            return (true, format!("Found at {}", path));
        }
    }

    // Check via environment variable
    if std::env::var("ORT_DYLIB_PATH").is_ok() {
        return (true, "Configured via ORT_DYLIB_PATH".to_string());
    }

    (
        false,
        "Not found (optional for ONNX acceleration)".to_string(),
    )
}

fn check_config(config_path: &Option<PathBuf>, verbose: bool) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // Check for config file
    let config_locations = [
        config_path.clone(),
        Some(PathBuf::from("scipix.toml")),
        Some(PathBuf::from("config/scipix.toml")),
        dirs::config_dir().map(|p| p.join("scipix/config.toml")),
    ];

    let mut found_config = false;
    for loc in config_locations.iter().flatten() {
        if loc.exists() {
            checks.push(DiagnosticCheck {
                name: "Configuration File".to_string(),
                category: "Config".to_string(),
                status: CheckStatus::Pass,
                message: format!("Found at {}", loc.display()),
                recommendation: None,
                auto_fixable: false,
            });
            found_config = true;

            // Validate config content
            if let Ok(content) = std::fs::read_to_string(loc) {
                if content.contains("[api]") || content.contains("[processing]") {
                    checks.push(DiagnosticCheck {
                        name: "Config Validity".to_string(),
                        category: "Config".to_string(),
                        status: CheckStatus::Pass,
                        message: "Configuration file is valid".to_string(),
                        recommendation: None,
                        auto_fixable: false,
                    });
                }
            }
            break;
        }
    }

    if !found_config {
        checks.push(DiagnosticCheck {
            name: "Configuration File".to_string(),
            category: "Config".to_string(),
            status: CheckStatus::Info,
            message: "No configuration file found (using defaults)".to_string(),
            recommendation: Some("Create a scipix.toml for custom settings".to_string()),
            auto_fixable: true,
        });
    }

    // Check environment variables
    let env_vars = [
        ("SCIPIX_API_KEY", "API authentication"),
        ("SCIPIX_MODEL_PATH", "Custom model path"),
        ("SCIPIX_CACHE_DIR", "Cache directory"),
    ];

    for (var, desc) in &env_vars {
        let status = if std::env::var(var).is_ok() {
            CheckStatus::Pass
        } else {
            CheckStatus::Info
        };

        if verbose || status == CheckStatus::Pass {
            checks.push(DiagnosticCheck {
                name: format!("Env: {}", var),
                category: "Config".to_string(),
                status,
                message: if status == CheckStatus::Pass {
                    format!("{} configured", desc)
                } else {
                    format!("{} not set (optional)", desc)
                },
                recommendation: None,
                auto_fixable: false,
            });
        }
    }

    checks
}

async fn check_network(verbose: bool) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // Check localhost binding
    let localhost_available = tokio::net::TcpListener::bind("127.0.0.1:0").await.is_ok();

    checks.push(DiagnosticCheck {
        name: "Localhost Binding".to_string(),
        category: "Network".to_string(),
        status: if localhost_available {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        message: if localhost_available {
            "Can bind to localhost".to_string()
        } else {
            "Cannot bind to localhost".to_string()
        },
        recommendation: if !localhost_available {
            Some("Check firewall settings and port availability".to_string())
        } else {
            None
        },
        auto_fixable: false,
    });

    // Check common ports
    let ports_to_check = [(8080, "API server"), (3000, "Alternative API")];

    for (port, desc) in &ports_to_check {
        let available = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .is_ok();

        if verbose || !available {
            checks.push(DiagnosticCheck {
                name: format!("Port {}", port),
                category: "Network".to_string(),
                status: if available {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Warning
                },
                message: if available {
                    format!("Port {} ({}) available", port, desc)
                } else {
                    format!("Port {} ({}) in use", port, desc)
                },
                recommendation: if !available {
                    Some(format!(
                        "Free port {} or use --port to specify alternative",
                        port
                    ))
                } else {
                    None
                },
                auto_fixable: false,
            });
        }
    }

    checks
}

fn generate_optimal_config(system_info: &SystemInfo) -> OptimalConfig {
    // Calculate optimal batch size based on memory
    let batch_size = if system_info.available_memory_mb >= 8192 {
        32
    } else if system_info.available_memory_mb >= 4096 {
        16
    } else if system_info.available_memory_mb >= 2048 {
        8
    } else {
        4
    };

    // Calculate worker threads (leave some headroom)
    let worker_threads = (system_info.cpu_count as f64 * 0.75).ceil() as usize;
    let worker_threads = worker_threads.max(2);

    // Determine SIMD backend
    let simd_backend = system_info.simd_features.best_available.clone();

    // Memory limit (use 60% of available)
    let memory_limit_mb = (system_info.available_memory_mb as f64 * 0.6) as u64;

    // Preprocessing mode based on SIMD
    let preprocessing_mode = if system_info.simd_features.avx2 || system_info.simd_features.neon {
        "simd_optimized".to_string()
    } else if system_info.simd_features.sse4_2 {
        "simd_basic".to_string()
    } else {
        "scalar".to_string()
    };

    // Cache settings
    let cache_enabled = system_info.available_memory_mb >= 2048;
    let cache_size_mb = if cache_enabled {
        (system_info.available_memory_mb as f64 * 0.1) as u64
    } else {
        0
    };

    OptimalConfig {
        batch_size,
        worker_threads,
        simd_backend,
        memory_limit_mb,
        preprocessing_mode,
        cache_enabled,
        cache_size_mb,
    }
}

fn print_system_info(info: &SystemInfo) {
    println!("📊 System Information:");
    println!("───────────────────────────────────────────────────────────");
    println!("  OS:           {} ({})", info.os, info.arch);
    println!("  CPU:          {}", info.cpu_brand);
    println!("  Cores:        {}", info.cpu_count);
    println!(
        "  Memory:       {} MB total, {} MB available",
        info.total_memory_mb, info.available_memory_mb
    );
    println!("  Best SIMD:    {}", info.simd_features.best_available);
    println!();
}

fn print_check_results(checks: &[DiagnosticCheck]) {
    println!("🔍 Diagnostic Checks:");
    println!("───────────────────────────────────────────────────────────");

    let mut current_category = String::new();
    for check in checks {
        if check.category != current_category {
            if !current_category.is_empty() {
                println!();
            }
            println!("  [{}]", check.category);
            current_category = check.category.clone();
        }

        let status_color = match check.status {
            CheckStatus::Pass => "\x1b[32m",    // Green
            CheckStatus::Warning => "\x1b[33m", // Yellow
            CheckStatus::Fail => "\x1b[31m",    // Red
            CheckStatus::Info => "\x1b[36m",    // Cyan
        };

        println!(
            "    {}{}\x1b[0m {} - {}",
            status_color, check.status, check.name, check.message
        );
    }
    println!();
}

fn print_optimal_config(config: &OptimalConfig) {
    println!("\n⚙️  Optimal Configuration:");
    println!("───────────────────────────────────────────────────────────");
    println!("  batch_size:        {}", config.batch_size);
    println!("  worker_threads:    {}", config.worker_threads);
    println!("  simd_backend:      {}", config.simd_backend);
    println!("  memory_limit:      {} MB", config.memory_limit_mb);
    println!("  preprocessing:     {}", config.preprocessing_mode);
    println!("  cache_enabled:     {}", config.cache_enabled);
    if config.cache_enabled {
        println!("  cache_size:        {} MB", config.cache_size_mb);
    }

    println!("\n  📝 Example configuration (scipix.toml):");
    println!("  ─────────────────────────────────────────");
    println!("  [processing]");
    println!("  batch_size = {}", config.batch_size);
    println!("  worker_threads = {}", config.worker_threads);
    println!("  simd_backend = \"{}\"", config.simd_backend);
    println!("  memory_limit_mb = {}", config.memory_limit_mb);
    println!();
    println!("  [cache]");
    println!("  enabled = {}", config.cache_enabled);
    println!("  size_mb = {}", config.cache_size_mb);
}

fn print_summary(checks: &[DiagnosticCheck]) {
    let pass_count = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Pass)
        .count();
    let warn_count = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Warning)
        .count();
    let fail_count = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();

    println!("\n═══════════════════════════════════════════════════════════");
    println!(
        "📋 Summary: {} passed, {} warnings, {} failed",
        pass_count, warn_count, fail_count
    );

    if fail_count > 0 {
        println!("\n⚠️  Some checks failed. Review recommendations above.");
    } else if warn_count > 0 {
        println!("\n✓ System is functional with some areas for improvement.");
    } else {
        println!("\n✅ System is optimally configured for SciPix!");
    }
}

async fn apply_fixes(checks: &[DiagnosticCheck]) -> Result<()> {
    println!("\n🔧 Applying automatic fixes...");
    println!("───────────────────────────────────────────────────────────");

    let fixable: Vec<_> = checks.iter().filter(|c| c.auto_fixable).collect();

    if fixable.is_empty() {
        println!("  No automatic fixes available.");
        return Ok(());
    }

    for check in fixable {
        println!("  Fixing: {}", check.name);

        if check.name == "Configuration File" {
            // Create default config file
            let config_content = r#"# SciPix Configuration
# Generated by scipix doctor --fix

[processing]
batch_size = 16
worker_threads = 4
simd_backend = "auto"
memory_limit_mb = 4096

[cache]
enabled = true
size_mb = 256

[api]
host = "127.0.0.1"
port = 8080
timeout_seconds = 30

[logging]
level = "info"
format = "pretty"
"#;

            // Create config directory if needed
            let config_path = PathBuf::from("config");
            if !config_path.exists() {
                std::fs::create_dir_all(&config_path)?;
            }

            let config_file = config_path.join("scipix.toml");
            std::fs::write(&config_file, config_content)?;
            println!("    ✓ Created {}", config_file.display());
        }
    }

    Ok(())
}
