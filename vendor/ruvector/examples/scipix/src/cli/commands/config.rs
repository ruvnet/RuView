use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use std::path::PathBuf;
use tracing::info;

use super::OcrConfig;
use crate::cli::Cli;

/// Manage configuration
#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommand {
    /// Generate default configuration file
    Init {
        /// Output path for config file
        #[arg(short, long, default_value = "scipix.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Validate configuration file
    Validate {
        /// Path to config file to validate
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Show current configuration
    Show {
        /// Path to config file (default: from --config or scipix.toml)
        #[arg(value_name = "FILE")]
        file: Option<PathBuf>,
    },

    /// Edit configuration interactively
    Edit {
        /// Path to config file to edit
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Get configuration directory path
    Path,
}

pub async fn execute(args: ConfigArgs, cli: &Cli) -> Result<()> {
    match args.command {
        ConfigCommand::Init { output, force } => {
            init_config(&output, force)?;
        }
        ConfigCommand::Validate { file } => {
            validate_config(&file)?;
        }
        ConfigCommand::Show { file } => {
            show_config(file.or(cli.config.clone()))?;
        }
        ConfigCommand::Edit { file } => {
            edit_config(&file)?;
        }
        ConfigCommand::Path => {
            show_config_path()?;
        }
    }

    Ok(())
}

fn init_config(output: &PathBuf, force: bool) -> Result<()> {
    if output.exists() && !force {
        anyhow::bail!(
            "Config file already exists: {} (use --force to overwrite)",
            output.display()
        );
    }

    let config = OcrConfig::default();
    let toml = toml::to_string_pretty(&config).context("Failed to serialize config")?;

    std::fs::write(output, toml).context("Failed to write config file")?;

    info!("Configuration file created: {}", output.display());
    println!("✓ Created configuration file: {}", output.display());
    println!("\nTo use this config, run:");
    println!("  scipix-cli --config {} <command>", output.display());
    println!("\nOr set environment variable:");
    println!("  export MATHPIX_CONFIG={}", output.display());

    Ok(())
}

fn validate_config(file: &PathBuf) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("Config file not found: {}", file.display());
    }

    let content = std::fs::read_to_string(file).context("Failed to read config file")?;

    let config: OcrConfig = toml::from_str(&content).context("Failed to parse config file")?;

    // Validate configuration values
    if config.min_confidence < 0.0 || config.min_confidence > 1.0 {
        anyhow::bail!("min_confidence must be between 0.0 and 1.0");
    }

    if config.max_image_size == 0 {
        anyhow::bail!("max_image_size must be greater than 0");
    }

    if config.supported_extensions.is_empty() {
        anyhow::bail!("supported_extensions cannot be empty");
    }

    println!("✓ Configuration is valid");
    println!("\nSettings:");
    println!("  Min confidence: {}", config.min_confidence);
    println!("  Max image size: {} bytes", config.max_image_size);
    println!(
        "  Supported extensions: {}",
        config.supported_extensions.join(", ")
    );

    if let Some(endpoint) = &config.api_endpoint {
        println!("  API endpoint: {}", endpoint);
    }

    Ok(())
}

fn show_config(file: Option<PathBuf>) -> Result<()> {
    let config_path = file.unwrap_or_else(|| PathBuf::from("scipix.toml"));

    if !config_path.exists() {
        println!("No configuration file found.");
        println!("\nCreate one with:");
        println!("  scipix-cli config init");
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path).context("Failed to read config file")?;

    println!("Configuration from: {}\n", config_path.display());
    println!("{}", content);

    Ok(())
}

fn edit_config(file: &PathBuf) -> Result<()> {
    if !file.exists() {
        anyhow::bail!(
            "Config file not found: {} (use 'config init' to create)",
            file.display()
        );
    }

    let content = std::fs::read_to_string(file).context("Failed to read config file")?;

    let mut config: OcrConfig = toml::from_str(&content).context("Failed to parse config file")?;

    let theme = ColorfulTheme::default();

    println!("Interactive Configuration Editor\n");

    // Edit min_confidence
    config.min_confidence = Input::with_theme(&theme)
        .with_prompt("Minimum confidence threshold (0.0-1.0)")
        .default(config.min_confidence)
        .validate_with(|v: &f64| {
            if *v >= 0.0 && *v <= 1.0 {
                Ok(())
            } else {
                Err("Value must be between 0.0 and 1.0")
            }
        })
        .interact_text()
        .context("Failed to read input")?;

    // Edit max_image_size
    let max_size_mb = config.max_image_size / (1024 * 1024);
    let new_size_mb: usize = Input::with_theme(&theme)
        .with_prompt("Maximum image size (MB)")
        .default(max_size_mb)
        .interact_text()
        .context("Failed to read input")?;
    config.max_image_size = new_size_mb * 1024 * 1024;

    // Edit API endpoint
    if config.api_endpoint.is_some() {
        let edit_endpoint = Confirm::with_theme(&theme)
            .with_prompt("Edit API endpoint?")
            .default(false)
            .interact()
            .context("Failed to read input")?;

        if edit_endpoint {
            let endpoint: String = Input::with_theme(&theme)
                .with_prompt("API endpoint URL")
                .allow_empty(true)
                .interact_text()
                .context("Failed to read input")?;

            config.api_endpoint = if endpoint.is_empty() {
                None
            } else {
                Some(endpoint)
            };
        }
    } else {
        let add_endpoint = Confirm::with_theme(&theme)
            .with_prompt("Add API endpoint?")
            .default(false)
            .interact()
            .context("Failed to read input")?;

        if add_endpoint {
            let endpoint: String = Input::with_theme(&theme)
                .with_prompt("API endpoint URL")
                .interact_text()
                .context("Failed to read input")?;

            config.api_endpoint = Some(endpoint);
        }
    }

    // Save configuration
    let save = Confirm::with_theme(&theme)
        .with_prompt("Save changes?")
        .default(true)
        .interact()
        .context("Failed to read input")?;

    if save {
        let toml = toml::to_string_pretty(&config).context("Failed to serialize config")?;

        std::fs::write(file, toml).context("Failed to write config file")?;

        println!("\n✓ Configuration saved to: {}", file.display());
    } else {
        println!("\nChanges discarded.");
    }

    Ok(())
}

fn show_config_path() -> Result<()> {
    if let Some(config_dir) = dirs::config_dir() {
        let app_config = config_dir.join("scipix");
        println!("Default config directory: {}", app_config.display());

        if !app_config.exists() {
            println!("\nDirectory does not exist. Create it with:");
            println!("  mkdir -p {}", app_config.display());
        }
    } else {
        println!("Could not determine config directory");
    }

    println!("\nYou can also use a custom config file:");
    println!("  scipix-cli --config /path/to/config.toml <command>");
    println!("\nOr set environment variable:");
    println!("  export MATHPIX_CONFIG=/path/to/config.toml");

    Ok(())
}
