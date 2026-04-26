# wifi-densepose-api

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-api.svg)](https://crates.io/crates/wifi-densepose-api)
[![Documentation](https://docs.rs/wifi-densepose-api/badge.svg)](https://docs.rs/wifi-densepose-api)
[![License](https://img.shields.io/crates/l/wifi-densepose-api.svg)](LICENSE)

REST and WebSocket API layer for the WiFi-DensePose pose estimation system.

## Overview

`wifi-densepose-api` provides the HTTP service boundary for WiFi-DensePose. Built on
[axum](https://github.com/tokio-rs/axum), it exposes REST endpoints for pose queries, CSI frame
ingestion, and model management, plus a WebSocket feed for real-time pose streaming to frontend
clients.

> **Status:** This crate is currently a stub. The intended API surface is documented below.

## Planned Features

- **REST endpoints** -- CRUD for scan zones, pose queries, model configuration, and health checks.
- **WebSocket streaming** -- Real-time pose estimate broadcasts with per-client subscription filters.
- **Authentication** -- Token-based auth middleware via `tower` layers.
- **Rate limiting** -- Configurable per-route limits to protect hardware-constrained deployments.
- **OpenAPI spec** -- Auto-generated documentation via `utoipa`.
- **CORS** -- Configurable cross-origin support for browser-based dashboards.
- **Graceful shutdown** -- Clean connection draining on SIGTERM.

## Quick Start

```rust
// Intended usage (not yet implemented)
use wifi_densepose_api::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Server::builder()
        .bind("0.0.0.0:3000")
        .with_websocket("/ws/poses")
        .build()
        .await?;

    server.run().await
}
```

## Planned Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/health` | Liveness and readiness probes |
| `GET` | `/api/v1/poses` | Latest pose estimates |
| `POST` | `/api/v1/csi` | Ingest raw CSI frames |
| `GET` | `/api/v1/zones` | List scan zones |
| `POST` | `/api/v1/zones` | Create a scan zone |
| `WS` | `/ws/poses` | Real-time pose stream |
| `WS` | `/ws/vitals` | Real-time vital sign stream |

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Shared types and traits |
| [`wifi-densepose-config`](../wifi-densepose-config) | Configuration loading |
| [`wifi-densepose-db`](../wifi-densepose-db) | Database persistence |
| [`wifi-densepose-nn`](../wifi-densepose-nn) | Neural network inference |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | CSI signal processing |
| [`wifi-densepose-sensing-server`](../wifi-densepose-sensing-server) | Lightweight sensing UI server |

## License

MIT OR Apache-2.0
