# Ruvector Snapshot

[![Crates.io](https://img.shields.io/crates/v/ruvector-snapshot.svg)](https://crates.io/crates/ruvector-snapshot)
[![Documentation](https://docs.rs/ruvector-snapshot/badge.svg)](https://docs.rs/ruvector-snapshot)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Point-in-time snapshots and backup for Ruvector vector databases.**

`ruvector-snapshot` provides efficient snapshot creation, storage, and restoration for Ruvector databases. Supports incremental snapshots, compression, and integrity verification. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Ruvector Snapshot?

- **Point-in-Time Recovery**: Restore to any snapshot
- **Incremental Snapshots**: Only store changed data
- **Compression**: GZIP compression for storage efficiency
- **Integrity Verification**: SHA-256 checksums
- **Async I/O**: Non-blocking snapshot operations

## Features

### Core Capabilities

- **Full Snapshots**: Complete database backup
- **Incremental Snapshots**: Delta-based backups
- **Compression**: GZIP compression support
- **Checksums**: SHA-256 integrity verification
- **Async Operations**: Tokio-based async I/O

### Advanced Features

- **Snapshot Scheduling**: Automated snapshot creation
- **Retention Policies**: Automatic cleanup of old snapshots
- **Remote Storage**: S3/GCS compatible storage (planned)
- **Streaming Restore**: Progressive restoration
- **Parallel Processing**: Multi-threaded snapshot creation

## Installation

Add `ruvector-snapshot` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-snapshot = "0.1.1"
```

## Quick Start

### Create Snapshot

```rust
use ruvector_snapshot::{SnapshotManager, SnapshotConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure snapshot manager
    let config = SnapshotConfig {
        snapshot_dir: "./snapshots".into(),
        compression: true,
        verify_checksum: true,
        ..Default::default()
    };

    let manager = SnapshotManager::new(config)?;

    // Create a full snapshot
    let snapshot = manager.create_snapshot(&db, "backup-2024-01").await?;
    println!("Created snapshot: {} ({} bytes)",
        snapshot.id,
        snapshot.size_bytes
    );

    Ok(())
}
```

### Restore from Snapshot

```rust
use ruvector_snapshot::SnapshotManager;

// List available snapshots
let snapshots = manager.list_snapshots().await?;
for snapshot in &snapshots {
    println!("{}: {} ({})",
        snapshot.id,
        snapshot.created_at,
        snapshot.size_bytes
    );
}

// Restore from snapshot
let restored_db = manager.restore_snapshot(&snapshots[0].id).await?;
println!("Restored {} vectors", restored_db.len()?);
```

### Incremental Snapshots

```rust
use ruvector_snapshot::{SnapshotManager, SnapshotType};

// Create base snapshot
let base = manager.create_snapshot(&db, "base").await?;

// ... database modifications ...

// Create incremental snapshot
let incremental = manager.create_incremental_snapshot(
    &db,
    "incremental-1",
    &base.id
).await?;

println!("Incremental snapshot: {} bytes (vs {} full)",
    incremental.size_bytes,
    base.size_bytes
);
```

## API Overview

### Core Types

```rust
// Snapshot configuration
pub struct SnapshotConfig {
    pub snapshot_dir: PathBuf,
    pub compression: bool,
    pub compression_level: u32,
    pub verify_checksum: bool,
    pub max_concurrent_io: usize,
}

// Snapshot metadata
pub struct Snapshot {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub checksum: String,
    pub snapshot_type: SnapshotType,
    pub vector_count: usize,
    pub metadata: serde_json::Value,
}

// Snapshot types
pub enum SnapshotType {
    Full,
    Incremental { base_id: String },
}
```

### Manager Operations

```rust
impl SnapshotManager {
    pub fn new(config: SnapshotConfig) -> Result<Self>;

    // Snapshot creation
    pub async fn create_snapshot(&self, db: &VectorDB, name: &str) -> Result<Snapshot>;
    pub async fn create_incremental_snapshot(
        &self,
        db: &VectorDB,
        name: &str,
        base_id: &str
    ) -> Result<Snapshot>;

    // Listing and info
    pub async fn list_snapshots(&self) -> Result<Vec<Snapshot>>;
    pub async fn get_snapshot(&self, id: &str) -> Result<Option<Snapshot>>;

    // Restoration
    pub async fn restore_snapshot(&self, id: &str) -> Result<VectorDB>;
    pub async fn verify_snapshot(&self, id: &str) -> Result<bool>;

    // Management
    pub async fn delete_snapshot(&self, id: &str) -> Result<()>;
    pub async fn cleanup_old_snapshots(&self, keep: usize) -> Result<usize>;
}
```

## Snapshot Format

```
snapshot-{id}/
├── metadata.json       # Snapshot metadata
├── vectors.bin.gz      # Compressed vector data
├── index.bin.gz        # HNSW index data
├── metadata.bin.gz     # Vector metadata
└── checksum.sha256     # Integrity checksum
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-replication](../ruvector-replication/)** - Data replication

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-snapshot)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-snapshot) | [Crates.io](https://crates.io/crates/ruvector-snapshot) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
