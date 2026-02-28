//! RuVector Discovery CLI
//!
//! Command-line tool for running dataset discoveries with the RuVector framework.
//!
//! ## Usage Examples
//!
//! ```bash
//! # Run discovery with default settings
//! cargo run --bin discover -- discover --data synthetic
//!
//! # Benchmark performance
//! cargo run --bin discover -- benchmark --vectors 1000
//!
//! # Analyze specific domain
//! cargo run --bin discover -- analyze --domain climate --threshold 0.5
//!
//! # Export patterns to JSON
//! cargo run --bin discover -- export --output patterns.json
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

use chrono::{Duration as ChronoDuration, Utc};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::Serialize;
use serde_json;

use ruvector_data_framework::optimized::{
    OptimizedConfig, OptimizedDiscoveryEngine, SignificantPattern,
};
use ruvector_data_framework::ruvector_native::{Domain, PatternType, SemanticVector};

/// ANSI color codes for terminal output
mod color {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
}

/// CLI command
#[derive(Debug)]
enum Command {
    Discover {
        data: DataSource,
        threshold: f64,
        domain: Option<Domain>,
        output: OutputFormat,
        verbose: bool,
    },
    Benchmark {
        vectors: usize,
        iterations: usize,
        parallel: bool,
    },
    Analyze {
        domain: Option<Domain>,
        threshold: f64,
        output: OutputFormat,
        data: DataSource,
    },
    Export {
        output: PathBuf,
        data: DataSource,
        pretty: bool,
    },
}

/// Data source type
#[derive(Debug, Clone)]
enum DataSource {
    Synthetic,
    Climate,
    Finance,
    Research,
    CrossDomain,
}

impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Output format
#[derive(Debug, Clone)]
enum OutputFormat {
    Human,
    Json,
    JsonPretty,
}

/// Serializable discovery result
#[derive(Debug, Serialize)]
struct DiscoveryResult {
    timestamp: String,
    total_patterns: usize,
    significant_patterns: usize,
    domains: Vec<String>,
    patterns: Vec<PatternSummary>,
    statistics: Statistics,
}

#[derive(Debug, Serialize)]
struct PatternSummary {
    pattern_type: String,
    description: String,
    confidence: f64,
    p_value: f64,
    effect_size: f64,
    domains: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Statistics {
    total_nodes: usize,
    total_edges: usize,
    cross_domain_edges: usize,
    processing_time_ms: u128,
}

fn main() {
    let result = run();
    if let Err(e) = result {
        eprintln!("{}Error:{} {}", color::RED, color::RESET, e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    let command = parse_args(&args)?;
    execute_command(command)?;

    Ok(())
}

fn parse_args(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let cmd = args.get(1).ok_or("No command specified")?;

    match cmd.as_str() {
        "discover" => parse_discover(&args[2..]),
        "benchmark" => parse_benchmark(&args[2..]),
        "analyze" => parse_analyze(&args[2..]),
        "export" => parse_export(&args[2..]),
        "help" | "--help" | "-h" => {
            print_help();
            std::process::exit(0);
        }
        _ => Err(format!("Unknown command: {}", cmd).into()),
    }
}

fn parse_discover(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let mut data = DataSource::Synthetic;
    let mut threshold = 0.5;
    let mut domain = None;
    let mut output = OutputFormat::Human;
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--data" => {
                i += 1;
                data = parse_data_source(args.get(i).ok_or("Missing data source")?)?;
            }
            "--threshold" => {
                i += 1;
                threshold = args
                    .get(i)
                    .ok_or("Missing threshold")?
                    .parse()
                    .map_err(|_| "Invalid threshold value")?;
            }
            "--domain" => {
                i += 1;
                domain = Some(parse_domain(args.get(i).ok_or("Missing domain")?)?);
            }
            "--output" => {
                i += 1;
                output = parse_output_format(args.get(i).ok_or("Missing output format")?)?;
            }
            "--verbose" | "-v" => verbose = true,
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 1;
    }

    Ok(Command::Discover {
        data,
        threshold,
        domain,
        output,
        verbose,
    })
}

fn parse_benchmark(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let mut vectors = 1000;
    let mut iterations = 10;
    let mut parallel = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--vectors" => {
                i += 1;
                vectors = args
                    .get(i)
                    .ok_or("Missing vectors count")?
                    .parse()
                    .map_err(|_| "Invalid vectors count")?;
            }
            "--iterations" => {
                i += 1;
                iterations = args
                    .get(i)
                    .ok_or("Missing iterations count")?
                    .parse()
                    .map_err(|_| "Invalid iterations count")?;
            }
            "--no-parallel" => parallel = false,
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 1;
    }

    Ok(Command::Benchmark {
        vectors,
        iterations,
        parallel,
    })
}

fn parse_analyze(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let mut domain = None;
    let mut threshold = 0.5;
    let mut output = OutputFormat::Human;
    let mut data = DataSource::Synthetic;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--domain" => {
                i += 1;
                domain = Some(parse_domain(args.get(i).ok_or("Missing domain")?)?);
            }
            "--threshold" => {
                i += 1;
                threshold = args
                    .get(i)
                    .ok_or("Missing threshold")?
                    .parse()
                    .map_err(|_| "Invalid threshold value")?;
            }
            "--output" => {
                i += 1;
                output = parse_output_format(args.get(i).ok_or("Missing output format")?)?;
            }
            "--data" => {
                i += 1;
                data = parse_data_source(args.get(i).ok_or("Missing data source")?)?;
            }
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 1;
    }

    Ok(Command::Analyze {
        domain,
        threshold,
        output,
        data,
    })
}

fn parse_export(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let mut output = PathBuf::from("patterns.json");
    let mut data = DataSource::Synthetic;
    let mut pretty = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                output = PathBuf::from(args.get(i).ok_or("Missing output path")?);
            }
            "--data" => {
                i += 1;
                data = parse_data_source(args.get(i).ok_or("Missing data source")?)?;
            }
            "--pretty" => pretty = true,
            _ => return Err(format!("Unknown option: {}", args[i]).into()),
        }
        i += 1;
    }

    Ok(Command::Export {
        output,
        data,
        pretty,
    })
}

fn parse_data_source(s: &str) -> Result<DataSource, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "synthetic" => Ok(DataSource::Synthetic),
        "climate" => Ok(DataSource::Climate),
        "finance" => Ok(DataSource::Finance),
        "research" => Ok(DataSource::Research),
        "cross-domain" | "crossdomain" => Ok(DataSource::CrossDomain),
        _ => Err(format!("Unknown data source: {}", s).into()),
    }
}

fn parse_domain(s: &str) -> Result<Domain, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "climate" => Ok(Domain::Climate),
        "finance" => Ok(Domain::Finance),
        "research" => Ok(Domain::Research),
        "crossdomain" | "cross-domain" => Ok(Domain::CrossDomain),
        _ => Err(format!("Unknown domain: {}", s).into()),
    }
}

fn parse_output_format(s: &str) -> Result<OutputFormat, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "human" | "text" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        "json-pretty" | "pretty" => Ok(OutputFormat::JsonPretty),
        _ => Err(format!("Unknown output format: {}", s).into()),
    }
}

fn execute_command(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Discover {
            data,
            threshold,
            domain,
            output,
            verbose,
        } => cmd_discover(data, threshold, domain, output, verbose),
        Command::Benchmark {
            vectors,
            iterations,
            parallel,
        } => cmd_benchmark(vectors, iterations, parallel),
        Command::Analyze {
            domain,
            threshold,
            output,
            data,
        } => cmd_analyze(domain, threshold, output, data),
        Command::Export {
            output,
            data,
            pretty,
        } => cmd_export(output, data, pretty),
    }
}

fn cmd_discover(
    data_source: DataSource,
    threshold: f64,
    domain_filter: Option<Domain>,
    output: OutputFormat,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    print_header("RuVector Discovery Engine");

    let start_time = Instant::now();

    if verbose {
        println!(
            "{}Configuration:{}",
            color::BOLD,
            color::RESET
        );
        println!("  Data source: {:?}", data_source);
        println!("  Threshold: {}", threshold);
        if let Some(d) = domain_filter {
            println!("  Domain filter: {:?}", d);
        }
        println!();
    }

    // Configure engine
    let config = OptimizedConfig {
        similarity_threshold: threshold,
        mincut_sensitivity: 0.1,
        cross_domain: true,
        use_simd: true,
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Load data
    print_status("Loading data", &data_source);
    let vectors = generate_data(&data_source);
    println!("  Loaded {} vectors", vectors.len());

    // Add vectors
    print_status("Building graph", &"");
    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(vectors);
    #[cfg(not(feature = "parallel"))]
    for v in vectors {
        engine.add_vector(v);
    }

    // Detect patterns
    print_status("Detecting patterns", &"");
    let patterns = engine.detect_patterns_with_significance();

    // Filter by domain if requested
    let filtered_patterns: Vec<_> = if let Some(domain) = domain_filter {
        patterns
            .into_iter()
            .filter(|p| {
                p.pattern
                    .cross_domain_links
                    .iter()
                    .any(|l| l.source_domain == domain || l.target_domain == domain)
            })
            .collect()
    } else {
        patterns
    };

    let stats = engine.stats();
    let elapsed = start_time.elapsed();

    // Output results
    match output {
        OutputFormat::Human => {
            print_human_results(&filtered_patterns, &stats, elapsed, verbose);
        }
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let result = build_result(&filtered_patterns, &stats, elapsed);
            let json = if matches!(output, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&result)?
            } else {
                serde_json::to_string(&result)?
            };
            println!("{}", json);
        }
    }

    Ok(())
}

fn cmd_benchmark(
    num_vectors: usize,
    iterations: usize,
    parallel: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    print_header("RuVector Performance Benchmark");

    println!("Configuration:");
    println!("  Vectors: {}", num_vectors);
    println!("  Iterations: {}", iterations);
    println!("  Parallel: {}", parallel);
    println!();

    let config = OptimizedConfig {
        similarity_threshold: 0.65,
        use_simd: true,
        ..Default::default()
    };

    let mut times = Vec::with_capacity(iterations);

    for i in 0..iterations {
        print!("{}Iteration {}/{}...{} ", color::CYAN, i + 1, iterations, color::RESET);
        io::stdout().flush()?;

        let start = Instant::now();
        let mut engine = OptimizedDiscoveryEngine::new(config.clone());

        // Generate random data
        let vectors = generate_synthetic_data(num_vectors);

        #[cfg(feature = "parallel")]
        if parallel {
            engine.add_vectors_batch(vectors);
        } else {
            for v in vectors {
                engine.add_vector(v);
            }
        }

        #[cfg(not(feature = "parallel"))]
        for v in vectors {
            engine.add_vector(v);
        }

        let patterns = engine.detect_patterns_with_significance();
        let elapsed = start.elapsed();
        times.push(elapsed.as_micros() as f64 / 1000.0);

        println!(
            "{}✓{} {:.2}ms ({} patterns)",
            color::GREEN,
            color::RESET,
            times[i],
            patterns.len()
        );
    }

    println!();
    println!("{}Results:{}",  color::BOLD, color::RESET);

    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let variance = times.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / times.len() as f64;
    let stddev = variance.sqrt();
    let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    println!("  Mean:   {:.2} ms", mean);
    println!("  Stddev: {:.2} ms", stddev);
    println!("  Min:    {:.2} ms", min);
    println!("  Max:    {:.2} ms", max);
    println!();
    println!(
        "  Throughput: {:.0} vectors/sec",
        (num_vectors as f64 / mean) * 1000.0
    );

    Ok(())
}

fn cmd_analyze(
    domain: Option<Domain>,
    threshold: f64,
    output: OutputFormat,
    data_source: DataSource,
) -> Result<(), Box<dyn std::error::Error>> {
    print_header("RuVector Domain Analysis");

    let config = OptimizedConfig {
        similarity_threshold: threshold,
        cross_domain: domain.is_none(),
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Load data
    let vectors = generate_data(&data_source);
    println!("Analyzing {} vectors", vectors.len());
    if let Some(d) = domain {
        println!("Domain focus: {:?}", d);
    }
    println!();

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(vectors);
    #[cfg(not(feature = "parallel"))]
    for v in vectors {
        engine.add_vector(v);
    }

    let patterns = engine.detect_patterns_with_significance();
    let stats = engine.stats();

    // Domain-specific analysis
    if let Some(d) = domain {
        let domain_coherence = engine.domain_coherence(d);
        println!("{}Domain Coherence:{}", color::BOLD, color::RESET);
        if let Some(coh) = domain_coherence {
            println!("  {:?}: {:.3} {}", d, coh, interpret_coherence(coh));
        } else {
            println!("  No data for domain {:?}", d);
        }
        println!();

        // Filter patterns for this domain
        let domain_patterns: Vec<_> = patterns
            .iter()
            .filter(|p| {
                p.pattern
                    .cross_domain_links
                    .iter()
                    .any(|l| l.source_domain == d || l.target_domain == d)
            })
            .collect();

        println!(
            "{}Patterns involving {:?}:{} {}",
            color::BOLD,
            d,
            color::RESET,
            domain_patterns.len()
        );
        for (i, pattern) in domain_patterns.iter().take(10).enumerate() {
            println!(
                "  {}. {} (p={:.4})",
                i + 1,
                pattern.pattern.description,
                pattern.p_value
            );
        }
    } else {
        // Cross-domain analysis
        println!("{}Cross-Domain Analysis:{}",  color::BOLD, color::RESET);
        println!("  Total edges: {}", stats.total_edges);
        println!("  Cross-domain edges: {}", stats.cross_domain_edges);
        let coupling = stats.cross_domain_edges as f64 / stats.total_edges.max(1) as f64;
        println!("  Coupling ratio: {:.1}%", coupling * 100.0);
        println!();

        // Patterns by type
        let mut by_type: HashMap<PatternType, usize> = HashMap::new();
        for p in &patterns {
            *by_type.entry(p.pattern.pattern_type).or_insert(0) += 1;
        }

        println!("{}Patterns by Type:{}",  color::BOLD, color::RESET);
        for (pt, count) in by_type {
            println!("  {:?}: {}", pt, count);
        }
    }

    Ok(())
}

fn cmd_export(
    output_path: PathBuf,
    data_source: DataSource,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    print_header("RuVector Export");

    let config = OptimizedConfig::default();
    let mut engine = OptimizedDiscoveryEngine::new(config);

    print_status("Loading data", &data_source);
    let vectors = generate_data(&data_source);

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(vectors);
    #[cfg(not(feature = "parallel"))]
    for v in vectors {
        engine.add_vector(v);
    }

    print_status("Detecting patterns", &"");
    let patterns = engine.detect_patterns_with_significance();

    let stats = engine.stats();
    let result = build_result(&patterns, &stats, std::time::Duration::from_secs(0));

    print_status("Writing to", &output_path.display());
    let json = if pretty {
        serde_json::to_string_pretty(&result)?
    } else {
        serde_json::to_string(&result)?
    };

    fs::write(&output_path, json)?;

    println!(
        "{}✓{} Exported {} patterns to {}",
        color::GREEN,
        color::RESET,
        patterns.len(),
        output_path.display()
    );

    Ok(())
}

// Helper functions

fn print_header(title: &str) {
    println!();
    println!("{}", "═".repeat(70));
    println!("{}  {}  {}", color::BOLD, title, color::RESET);
    println!("{}", "═".repeat(70));
    println!();
}

fn print_status(action: &str, detail: &dyn std::fmt::Display) {
    let detail_str = detail.to_string();
    println!(
        "{}▸{} {}{}",
        color::CYAN,
        color::RESET,
        action,
        if detail_str.is_empty() {
            String::new()
        } else {
            format!(": {}", detail_str)
        }
    );
}

fn print_human_results(
    patterns: &[SignificantPattern],
    stats: &ruvector_data_framework::optimized::OptimizedStats,
    elapsed: std::time::Duration,
    verbose: bool,
) {
    println!();
    println!("{}Discovery Results{}",  color::BOLD, color::RESET);
    println!("{}", "─".repeat(70));
    println!();

    println!("{}Statistics:{}",  color::BOLD, color::RESET);
    println!("  Total nodes: {}", stats.total_nodes);
    println!("  Total edges: {}", stats.total_edges);
    println!("  Cross-domain edges: {}", stats.cross_domain_edges);
    println!("  Processing time: {:.2}ms", elapsed.as_millis());
    println!();

    let significant: Vec<_> = patterns
        .iter()
        .filter(|p| p.p_value < 0.05)
        .collect();

    println!(
        "{}Patterns Found:{} {} total, {} significant (p < 0.05)",
        color::BOLD,
        color::RESET,
        patterns.len(),
        significant.len()
    );
    println!();

    if verbose {
        // Group by type
        let mut by_type: HashMap<PatternType, Vec<&SignificantPattern>> = HashMap::new();
        for p in patterns {
            by_type.entry(p.pattern.pattern_type).or_default().push(p);
        }

        for (pattern_type, group) in by_type {
            println!("{}  {:?}:{} {}", color::YELLOW, pattern_type, color::RESET, group.len());
            for (i, p) in group.iter().take(5).enumerate() {
                println!(
                    "    {}. {} (conf: {:.2}, p: {:.4})",
                    i + 1,
                    p.pattern.description,
                    p.pattern.confidence,
                    p.p_value
                );
            }
            if group.len() > 5 {
                println!("    ... and {} more", group.len() - 5);
            }
            println!();
        }
    } else {
        // Show top 10 most significant
        let mut sorted = patterns.to_vec();
        sorted.sort_by(|a, b| a.p_value.partial_cmp(&b.p_value).unwrap());

        for (i, p) in sorted.iter().take(10).enumerate() {
            let marker = if p.p_value < 0.05 {
                format!("{}*{}", color::GREEN, color::RESET)
            } else {
                " ".to_string()
            };
            println!(
                "  {}{:2}. {:?}: {} (p={:.4})",
                marker, i + 1, p.pattern.pattern_type, p.pattern.description, p.p_value
            );
        }

        if patterns.len() > 10 {
            println!("\n  ... and {} more patterns", patterns.len() - 10);
        }
    }

    println!();
    println!("{}* = statistically significant (p < 0.05){}", color::GREEN, color::RESET);
}

fn build_result(
    patterns: &[SignificantPattern],
    stats: &ruvector_data_framework::optimized::OptimizedStats,
    elapsed: std::time::Duration,
) -> DiscoveryResult {
    let significant = patterns.iter().filter(|p| p.p_value < 0.05).count();

    let mut domains = std::collections::HashSet::new();
    for p in patterns {
        for link in &p.pattern.cross_domain_links {
            domains.insert(format!("{:?}", link.source_domain));
            domains.insert(format!("{:?}", link.target_domain));
        }
    }

    let pattern_summaries: Vec<_> = patterns
        .iter()
        .map(|p| PatternSummary {
            pattern_type: format!("{:?}", p.pattern.pattern_type),
            description: p.pattern.description.clone(),
            confidence: p.pattern.confidence,
            p_value: p.p_value,
            effect_size: p.effect_size,
            domains: p
                .pattern
                .cross_domain_links
                .iter()
                .flat_map(|l| {
                    vec![
                        format!("{:?}", l.source_domain),
                        format!("{:?}", l.target_domain),
                    ]
                })
                .collect(),
        })
        .collect();

    DiscoveryResult {
        timestamp: Utc::now().to_rfc3339(),
        total_patterns: patterns.len(),
        significant_patterns: significant,
        domains: domains.into_iter().collect(),
        patterns: pattern_summaries,
        statistics: Statistics {
            total_nodes: stats.total_nodes,
            total_edges: stats.total_edges,
            cross_domain_edges: stats.cross_domain_edges,
            processing_time_ms: elapsed.as_millis(),
        },
    }
}

fn interpret_coherence(value: f64) -> &'static str {
    if value > 0.9 {
        "(highly coherent)"
    } else if value > 0.7 {
        "(coherent)"
    } else if value > 0.5 {
        "(moderate)"
    } else {
        "(fragmented)"
    }
}

fn generate_data(source: &DataSource) -> Vec<SemanticVector> {
    match source {
        DataSource::Synthetic => generate_synthetic_data(500),
        DataSource::Climate => generate_climate_data(),
        DataSource::Finance => generate_finance_data(),
        DataSource::Research => generate_research_data(),
        DataSource::CrossDomain => {
            let mut all = Vec::new();
            all.extend(generate_climate_data());
            all.extend(generate_finance_data());
            all.extend(generate_research_data());
            all
        }
    }
}

fn generate_synthetic_data(count: usize) -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(count);

    let domains = [
        Domain::Climate,
        Domain::Finance,
        Domain::Research,
        Domain::CrossDomain,
    ];

    for i in 0..count {
        let mut embedding = vec![0.0_f32; 128];
        for val in &mut embedding {
            *val = rng.gen::<f32>();
        }
        normalize(&mut embedding);

        vectors.push(SemanticVector {
            id: format!("syn_{}", i),
            embedding,
            domain: domains[i % domains.len()],
            timestamp: Utc::now() - ChronoDuration::days((count - i) as i64),
            metadata: HashMap::new(),
        });
    }

    vectors
}

fn generate_climate_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2024);
    let mut vectors = Vec::new();

    let regions = ["arctic", "tropical", "temperate"];
    let events = ["heatwave", "drought", "flooding"];

    for region in &regions {
        for event in &events {
            for year in 2020..2025 {
                let mut embedding = vec![0.0_f32; 128];

                // Climate signature
                for i in 0..30 {
                    embedding[i] = 0.3 + rng.gen::<f32>() * 0.3;
                }

                // Region encoding
                let region_idx = regions.iter().position(|r| r == region).unwrap();
                for i in 0..10 {
                    embedding[40 + region_idx * 10 + i] = 0.5 + rng.gen::<f32>() * 0.3;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("climate_{}_{}_{}", region, event, year),
                    embedding,
                    domain: Domain::Climate,
                    timestamp: Utc::now() - ChronoDuration::days((2024 - year) as i64 * 365),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("region".to_string(), region.to_string());
                        m.insert("event".to_string(), event.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn generate_finance_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2025);
    let mut vectors = Vec::new();

    let sectors = ["energy", "utilities", "insurance"];
    let indicators = ["volatility", "credit_spread"];

    for sector in &sectors {
        for indicator in &indicators {
            for quarter in 0..16 {
                let mut embedding = vec![0.0_f32; 128];

                // Finance signature
                for i in 80..120 {
                    embedding[i] = 0.35 + rng.gen::<f32>() * 0.25;
                }

                // Sector encoding
                let sector_idx = sectors.iter().position(|s| s == sector).unwrap();
                for i in 0..15 {
                    embedding[20 + sector_idx * 15 + i] = 0.4 + rng.gen::<f32>() * 0.3;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("finance_{}_{}_Q{}", sector, indicator, quarter),
                    embedding,
                    domain: Domain::Finance,
                    timestamp: Utc::now() - ChronoDuration::days((16 - quarter) as i64 * 90),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("sector".to_string(), sector.to_string());
                        m.insert("indicator".to_string(), indicator.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn generate_research_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2026);
    let mut vectors = Vec::new();

    let topics = ["climate_risk", "stranded_assets", "green_bonds"];

    for topic in &topics {
        for year in 2020..2025 {
            for paper in 0..3 {
                let mut embedding = vec![0.0_f32; 128];

                // Research bridges climate and finance
                for i in 0..15 {
                    embedding[i] = 0.2 + rng.gen::<f32>() * 0.2;
                }
                for i in 80..95 {
                    embedding[i] = 0.2 + rng.gen::<f32>() * 0.2;
                }

                // Topic signature
                let topic_idx = topics.iter().position(|t| t == topic).unwrap();
                for i in 0..20 {
                    embedding[30 + topic_idx * 15 + i % 15] = 0.5 + rng.gen::<f32>() * 0.3;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("research_{}_{}_{}", topic, year, paper),
                    embedding,
                    domain: Domain::Research,
                    timestamp: Utc::now() - ChronoDuration::days((2024 - year) as i64 * 365),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("topic".to_string(), topic.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn normalize(embedding: &mut [f32]) {
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    }
}

fn print_help() {
    println!(
        r#"
{}RuVector Discovery CLI{}

{}USAGE:{}
    discover <COMMAND> [OPTIONS]

{}COMMANDS:{}
    discover        Run discovery analysis on data
    benchmark       Benchmark performance with different configurations
    analyze         Analyze specific domains or patterns
    export          Export patterns to JSON file
    help            Show this help message

{}DISCOVER OPTIONS:{}
    --data <SOURCE>         Data source: synthetic, climate, finance, research, cross-domain
                            (default: synthetic)
    --threshold <FLOAT>     Similarity threshold (0.0-1.0, default: 0.5)
    --domain <DOMAIN>       Filter by domain: climate, finance, research, crossdomain
    --output <FORMAT>       Output format: human, json, json-pretty (default: human)
    --verbose, -v           Show detailed output

{}BENCHMARK OPTIONS:{}
    --vectors <NUM>         Number of vectors to test (default: 1000)
    --iterations <NUM>      Number of benchmark iterations (default: 10)
    --no-parallel           Disable parallel processing

{}ANALYZE OPTIONS:{}
    --domain <DOMAIN>       Focus on specific domain (optional)
    --threshold <FLOAT>     Similarity threshold (default: 0.5)
    --output <FORMAT>       Output format: human, json, json-pretty (default: human)
    --data <SOURCE>         Data source (default: synthetic)

{}EXPORT OPTIONS:{}
    --output, -o <PATH>     Output file path (default: patterns.json)
    --data <SOURCE>         Data source (default: synthetic)
    --pretty                Pretty-print JSON output

{}EXAMPLES:{}
    # Run discovery with default settings
    cargo run --bin discover -- discover --data synthetic

    # Discover climate patterns with high threshold
    cargo run --bin discover -- discover --data climate --threshold 0.7 --verbose

    # Benchmark with 5000 vectors
    cargo run --bin discover -- benchmark --vectors 5000 --iterations 20

    # Analyze cross-domain relationships
    cargo run --bin discover -- analyze --data cross-domain --output json-pretty

    # Export finance patterns to JSON
    cargo run --bin discover -- export --data finance --output finance_patterns.json --pretty

{}FEATURES:{}
    • SIMD-accelerated vector operations (4-8x speedup)
    • Parallel processing with rayon (linear scaling)
    • Statistical significance testing (p-values)
    • Cross-domain pattern detection
    • Temporal causality analysis
    • Multiple output formats (human-readable, JSON)

{}MORE INFO:{}
    Repository: https://github.com/ruvnet/ruvector
    Documentation: See examples/data/framework/README.md
"#,
        color::BOLD,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
        color::CYAN,
        color::RESET,
    );
}
