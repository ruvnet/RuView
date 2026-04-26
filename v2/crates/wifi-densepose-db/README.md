# wifi-densepose-db

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-db.svg)](https://crates.io/crates/wifi-densepose-db)
[![Documentation](https://docs.rs/wifi-densepose-db/badge.svg)](https://docs.rs/wifi-densepose-db)
[![License](https://img.shields.io/crates/l/wifi-densepose-db.svg)](LICENSE)

Database persistence layer for the WiFi-DensePose pose estimation system.

## Overview

`wifi-densepose-db` implements the `DataStore` trait defined in `wifi-densepose-core`, providing
persistent storage for CSI frames, pose estimates, scan sessions, and alert history. The intended
backends are [SQLx](https://docs.rs/sqlx) for relational storage (PostgreSQL and SQLite) and
[Redis](https://docs.rs/redis) for real-time caching and pub/sub.

> **Status:** This crate is currently a stub. The intended API surface is documented below.

## Planned Features

- **Dual backend** -- PostgreSQL for production deployments, SQLite for single-node and embedded
  use. Selectable at compile time via feature flags.
- **Redis caching** -- Connection-pooled Redis for low-latency pose estimate lookups, session
  state, and pub/sub event distribution.
- **Migrations** -- Embedded SQL migrations managed by SQLx, applied automatically on startup.
- **Repository pattern** -- Typed repository structs (`PoseRepository`, `SessionRepository`,
  `AlertRepository`) implementing the core `DataStore` trait.
- **Connection pooling** -- Configurable pool sizes via `sqlx::PgPool` / `sqlx::SqlitePool`.
- **Transaction support** -- Scoped transactions for multi-table writes (e.g., survivor detection
  plus alert creation).
- **Time-series optimisation** -- Partitioned tables and retention policies for high-frequency CSI
  frame storage.

### Planned feature flags

| Flag       | Default | Description |
|------------|---------|-------------|
| `postgres` | no      | Enable PostgreSQL backend |
| `sqlite`   | yes     | Enable SQLite backend |
| `redis`    | no      | Enable Redis caching layer |

## Quick Start

```rust
// Intended usage (not yet implemented)
use wifi_densepose_db::{Database, PoseRepository};
use wifi_densepose_core::PoseEstimate;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Database::connect("sqlite://data/wifi-densepose.db").await?;
    db.run_migrations().await?;

    let repo = PoseRepository::new(db.pool());

    // Store a pose estimate
    repo.insert(&pose_estimate).await?;

    // Query recent poses
    let recent = repo.find_recent(10).await?;
    println!("Last 10 poses: {:?}", recent);

    Ok(())
}
```

## Planned Schema

```sql
-- Core tables
CREATE TABLE csi_frames (
    id          UUID PRIMARY KEY,
    session_id  UUID NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL,
    subcarriers BYTEA NOT NULL,
    antenna_id  INTEGER NOT NULL
);

CREATE TABLE pose_estimates (
    id          UUID PRIMARY KEY,
    frame_id    UUID REFERENCES csi_frames(id),
    timestamp   TIMESTAMPTZ NOT NULL,
    keypoints   JSONB NOT NULL,
    confidence  REAL NOT NULL
);

CREATE TABLE scan_sessions (
    id          UUID PRIMARY KEY,
    started_at  TIMESTAMPTZ NOT NULL,
    ended_at    TIMESTAMPTZ,
    config      JSONB NOT NULL
);
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | `DataStore` trait definition |
| [`wifi-densepose-config`](../wifi-densepose-config) | Database connection configuration |
| [`wifi-densepose-api`](../wifi-densepose-api) | REST API (consumer) |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Disaster detection (consumer) |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | CSI signal processing |

## License

MIT OR Apache-2.0
