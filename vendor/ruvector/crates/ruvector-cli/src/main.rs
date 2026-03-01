//! Ruvector CLI - High-performance vector database command-line interface

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

mod cli;
mod config;

use crate::cli::commands::*;
use crate::config::Config;

#[derive(Parser)]
#[command(name = "ruvector")]
#[command(about = "High-performance Rust vector database CLI", long_about = None)]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable debug mode
    #[arg(short, long, global = true)]
    debug: bool,

    /// Disable colors
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new vector database
    Create {
        /// Database file path
        #[arg(short, long, default_value = "./ruvector.db")]
        path: String,

        /// Vector dimensions
        #[arg(short = 'D', long)]
        dimensions: usize,
    },

    /// Insert vectors from a file
    Insert {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,

        /// Input file path
        #[arg(short, long)]
        input: String,

        /// Input format (json, csv, npy)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Hide progress bar
        #[arg(long)]
        no_progress: bool,
    },

    /// Search for similar vectors
    Search {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,

        /// Query vector (comma-separated floats or JSON array)
        #[arg(short, long)]
        query: String,

        /// Number of results
        #[arg(short = 'k', long, default_value = "10")]
        top_k: usize,

        /// Show full vectors in results
        #[arg(long)]
        show_vectors: bool,
    },

    /// Show database information
    Info {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,
    },

    /// Run a quick performance benchmark
    Benchmark {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,

        /// Number of queries to run
        #[arg(short = 'n', long, default_value = "1000")]
        queries: usize,
    },

    /// Export database to file
    Export {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,

        /// Output file path
        #[arg(short, long)]
        output: String,

        /// Output format (json, csv)
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Import from other vector databases
    Import {
        /// Database file path
        #[arg(short = 'b', long, default_value = "./ruvector.db")]
        db: String,

        /// Source database type (faiss, pinecone, weaviate)
        #[arg(short, long)]
        source: String,

        /// Source file or connection path
        #[arg(short = 'p', long)]
        source_path: String,
    },

    /// Graph database operations (Neo4j-compatible)
    Graph {
        #[command(subcommand)]
        action: cli::graph::GraphCommands,
    },

    /// Self-learning intelligence hooks for Claude Code
    Hooks {
        #[command(subcommand)]
        action: cli::hooks::HooksCommands,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.debug {
        tracing_subscriber::fmt()
            .with_env_filter("ruvector=debug")
            .init();
    }

    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }

    // Load configuration
    let config = Config::load(cli.config)?;

    // Execute command
    let result = match cli.command {
        Commands::Create { path, dimensions } => create_database(&path, dimensions, &config),
        Commands::Insert {
            db,
            input,
            format,
            no_progress,
        } => insert_vectors(&db, &input, &format, &config, !no_progress),
        Commands::Search {
            db,
            query,
            top_k,
            show_vectors,
        } => {
            let query_vec = parse_query_vector(&query)?;
            search_vectors(&db, query_vec, top_k, &config, show_vectors)
        }
        Commands::Info { db } => show_info(&db, &config),
        Commands::Benchmark { db, queries } => run_benchmark(&db, &config, queries),
        Commands::Export { db, output, format } => export_database(&db, &output, &format, &config),
        Commands::Import {
            db,
            source,
            source_path,
        } => import_from_external(&db, &source, &source_path, &config),
        Commands::Graph { action } => {
            use cli::graph::GraphCommands;
            match action {
                GraphCommands::Create {
                    path,
                    name,
                    indexed,
                } => cli::graph::create_graph(&path, &name, indexed, &config),
                GraphCommands::Query {
                    db,
                    cypher,
                    format,
                    explain,
                } => cli::graph::execute_query(&db, &cypher, &format, explain, &config),
                GraphCommands::Shell { db, multiline } => {
                    cli::graph::run_shell(&db, multiline, &config)
                }
                GraphCommands::Import {
                    db,
                    input,
                    format,
                    graph,
                    skip_errors,
                } => cli::graph::import_graph(&db, &input, &format, &graph, skip_errors, &config),
                GraphCommands::Export {
                    db,
                    output,
                    format,
                    graph,
                } => cli::graph::export_graph(&db, &output, &format, &graph, &config),
                GraphCommands::Info { db, detailed } => {
                    cli::graph::show_graph_info(&db, detailed, &config)
                }
                GraphCommands::Benchmark {
                    db,
                    queries,
                    bench_type,
                } => cli::graph::run_graph_benchmark(&db, queries, &bench_type, &config),
                GraphCommands::Serve {
                    db,
                    host,
                    http_port,
                    grpc_port,
                    graphql,
                } => cli::graph::serve_graph(&db, &host, http_port, grpc_port, graphql, &config),
            }
        }
        Commands::Hooks { action } => {
            use cli::hooks::HooksCommands;
            match action {
                HooksCommands::Init { force, postgres } => {
                    cli::hooks::init_hooks(force, postgres, &config)
                }
                HooksCommands::Install { settings_dir } => {
                    cli::hooks::install_hooks(&settings_dir, &config)
                }
                HooksCommands::Stats => cli::hooks::show_stats(&config),
                HooksCommands::Remember {
                    memory_type,
                    content,
                } => cli::hooks::remember_content(&memory_type, &content.join(" "), &config),
                HooksCommands::Recall { query, top_k } => {
                    cli::hooks::recall_content(&query.join(" "), top_k, &config)
                }
                HooksCommands::Learn {
                    state,
                    action,
                    reward,
                } => cli::hooks::learn_trajectory(&state, &action, reward, &config),
                HooksCommands::Suggest { state, actions } => {
                    cli::hooks::suggest_action(&state, &actions, &config)
                }
                HooksCommands::Route {
                    task,
                    file,
                    crate_name,
                    operation,
                } => cli::hooks::route_task(
                    &task.join(" "),
                    file.as_deref(),
                    crate_name.as_deref(),
                    &operation,
                    &config,
                ),
                HooksCommands::PreEdit { file } => cli::hooks::pre_edit_hook(&file, &config),
                HooksCommands::PostEdit { file, success } => {
                    cli::hooks::post_edit_hook(&file, success, &config)
                }
                HooksCommands::PreCommand { command } => {
                    cli::hooks::pre_command_hook(&command.join(" "), &config)
                }
                HooksCommands::PostCommand {
                    command,
                    success,
                    stderr,
                } => cli::hooks::post_command_hook(
                    &command.join(" "),
                    success,
                    stderr.as_deref(),
                    &config,
                ),
                HooksCommands::SessionStart { session_id, resume } => {
                    cli::hooks::session_start_hook(session_id.as_deref(), resume, &config)
                }
                HooksCommands::SessionEnd { export_metrics } => {
                    cli::hooks::session_end_hook(export_metrics, &config)
                }
                HooksCommands::PreCompact { length, auto } => {
                    cli::hooks::pre_compact_hook(length, auto, &config)
                }
                HooksCommands::SuggestContext => cli::hooks::suggest_context_cmd(&config),
                HooksCommands::TrackNotification { notification_type } => {
                    cli::hooks::track_notification_cmd(notification_type.as_deref(), &config)
                }
                // Claude Code v2.0.55+ features
                HooksCommands::LspDiagnostic {
                    file,
                    severity,
                    message,
                } => cli::hooks::lsp_diagnostic_cmd(
                    file.as_deref(),
                    severity.as_deref(),
                    message.as_deref(),
                    &config,
                ),
                HooksCommands::SuggestUltrathink { task, file } => {
                    cli::hooks::suggest_ultrathink_cmd(&task.join(" "), file.as_deref(), &config)
                }
                HooksCommands::AsyncAgent {
                    action,
                    agent_id,
                    task,
                } => cli::hooks::async_agent_cmd(
                    &action,
                    agent_id.as_deref(),
                    task.as_deref(),
                    &config,
                ),
                HooksCommands::RecordError { command, stderr } => {
                    cli::hooks::record_error_cmd(&command, &stderr, &config)
                }
                HooksCommands::SuggestFix { error_code } => {
                    cli::hooks::suggest_fix_cmd(&error_code, &config)
                }
                HooksCommands::SuggestNext { file, count } => {
                    cli::hooks::suggest_next_cmd(&file, count, &config)
                }
                HooksCommands::ShouldTest { file } => cli::hooks::should_test_cmd(&file, &config),
                HooksCommands::SwarmRegister {
                    agent_id,
                    agent_type,
                    capabilities,
                } => cli::hooks::swarm_register_cmd(
                    &agent_id,
                    &agent_type,
                    capabilities.as_deref(),
                    &config,
                ),
                HooksCommands::SwarmCoordinate {
                    source,
                    target,
                    weight,
                } => cli::hooks::swarm_coordinate_cmd(&source, &target, weight, &config),
                HooksCommands::SwarmOptimize { tasks } => {
                    cli::hooks::swarm_optimize_cmd(&tasks, &config)
                }
                HooksCommands::SwarmRecommend { task_type } => {
                    cli::hooks::swarm_recommend_cmd(&task_type, &config)
                }
                HooksCommands::SwarmHeal { agent_id } => {
                    cli::hooks::swarm_heal_cmd(&agent_id, &config)
                }
                HooksCommands::SwarmStats => cli::hooks::swarm_stats_cmd(&config),
                HooksCommands::Completions { shell } => cli::hooks::generate_completions(shell),
                HooksCommands::Compress => cli::hooks::compress_storage(&config),
                HooksCommands::CacheStats => cli::hooks::cache_stats(&config),
            }
        }
    };

    // Handle errors
    if let Err(e) = result {
        eprintln!("{}", cli::format::format_error(&e.to_string()));
        if cli.debug {
            eprintln!("\n{:#?}", e);
        } else {
            eprintln!("\n{}", "Run with --debug for more details".dimmed());
        }
        std::process::exit(1);
    }

    Ok(())
}

/// Parse query vector from string
fn parse_query_vector(s: &str) -> Result<Vec<f32>> {
    // Try JSON first
    if s.trim().starts_with('[') {
        return serde_json::from_str(s)
            .map_err(|e| anyhow::anyhow!("Failed to parse query vector as JSON: {}", e));
    }

    // Try comma-separated
    s.split(',')
        .map(|s| s.trim().parse::<f32>())
        .collect::<std::result::Result<Vec<f32>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to parse query vector: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_vector_json() {
        let vec = parse_query_vector("[1.0, 2.0, 3.0]").unwrap();
        assert_eq!(vec, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_query_vector_csv() {
        let vec = parse_query_vector("1.0, 2.0, 3.0").unwrap();
        assert_eq!(vec, vec![1.0, 2.0, 3.0]);
    }
}
