# wifi-densepose-config

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-config.svg)](https://crates.io/crates/wifi-densepose-config)
[![Documentation](https://docs.rs/wifi-densepose-config/badge.svg)](https://docs.rs/wifi-densepose-config)
[![License](https://img.shields.io/crates/l/wifi-densepose-config.svg)](LICENSE)

Configuration management for the WiFi-DensePose pose estimation system.

## Overview

`wifi-densepose-config` provides a unified configuration layer that merges values from environment
variables, TOML/YAML files, and CLI overrides into strongly-typed Rust structs. Built on the
[config](https://docs.rs/config), [dotenvy](https://docs.rs/dotenvy), and
[envy](https://docs.rs/envy) ecosystem from the workspace.

> **Status:** This crate is currently a stub. The intended API surface is documented below.

## Planned Features

- **Multi-source loading** -- Merge configuration from `.env`, TOML files, YAML files, and
  environment variables with well-defined precedence.
- **Typed configuration** -- Strongly-typed structs for server, signal processing, neural network,
  hardware, and database settings.
- **Validation** -- Schema validation with human-readable error messages on startup.
- **Hot reload** -- Watch configuration files for changes and notify dependent services.
- **Profile support** -- Named profiles (`development`, `production`, `testing`) with per-profile
  overrides.
- **Secret filtering** -- Redact sensitive values (API keys, database passwords) in logs and debug
  output.

## Quick Start

```rust
// Intended usage (not yet implemented)
use wifi_densepose_config::AppConfig;

fn main() -> anyhow::Result<()> {
    // Loads from env, config.toml, and CLI overrides
    let config = AppConfig::load()?;

    println!("Server bind: {}", config.server.bind_address);
    println!("CSI sample rate: {} Hz", config.signal.sample_rate);
    println!("Model path: {}", config.nn.model_path.display());

    Ok(())
}
```

## Planned Configuration Structure

```toml
# config.toml

[server]
bind_address = "0.0.0.0:3000"
websocket_path = "/ws/poses"

[signal]
sample_rate = 100
subcarrier_count = 56
hampel_window = 5

[nn]
model_path = "./models/densepose.rvf"
backend = "ort"        # ort | candle | tch
batch_size = 8

[hardware]
esp32_udp_port = 5005
serial_baud = 921600

[database]
url = "sqlite://data/wifi-densepose.db"
max_connections = 5
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Shared types and traits |
| [`wifi-densepose-api`](../wifi-densepose-api) | REST API (consumer) |
| [`wifi-densepose-db`](../wifi-densepose-db) | Database layer (consumer) |
| [`wifi-densepose-cli`](../wifi-densepose-cli) | CLI (consumer) |
| [`wifi-densepose-sensing-server`](../wifi-densepose-sensing-server) | Sensing server (consumer) |

## License

MIT OR Apache-2.0
