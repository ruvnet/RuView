# sevensense-core

[![Crate](https://img.shields.io/badge/crates.io-sevensense--core-orange.svg)](https://crates.io/crates/sevensense-core)
[![Docs](https://img.shields.io/badge/docs-sevensense--core-blue.svg)](https://docs.rs/sevensense-core)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Shared domain primitives for the 7sense bioacoustic intelligence platform.

**sevensense-core** provides the foundational types, traits, and utilities used across all 7sense crates. It defines the core vocabulary of the domain—species identifiers, temporal boundaries, audio metadata, and error types—ensuring consistency throughout the platform.

## Features

- **Species Taxonomy**: Type-safe species identifiers with scientific/common name support
- **Temporal Primitives**: Time ranges, segments, and duration utilities for audio analysis
- **Domain Events**: Event-sourced primitives for audit trails and streaming
- **Error Handling**: Unified error types with rich context and error chains
- **Configuration**: Shared configuration primitives and validation

## Use Cases

| Use Case | Description | Example Types |
|----------|-------------|---------------|
| Species Management | Track and validate bird species | `SpeciesId`, `TaxonomicRank`, `SpeciesMetadata` |
| Time Handling | Represent audio segments and recordings | `TimeRange`, `SegmentBounds`, `Duration` |
| Audio Metadata | Describe recordings and their properties | `RecordingId`, `AudioMetadata`, `Location` |
| Error Propagation | Consistent error handling across crates | `CoreError`, `ErrorContext`, `Result<T>` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-core = "0.1"
```

## Quick Start

```rust
use sevensense_core::{SpeciesId, TimeRange, AudioMetadata};

// Create a species identifier
let species = SpeciesId::from_scientific("Turdus merula");
println!("Species: {}", species.scientific_name());

// Define a time range for analysis
let range = TimeRange::new(
    chrono::Utc::now() - chrono::Duration::hours(1),
    chrono::Utc::now()
);
println!("Duration: {:?}", range.duration());
```

---

<details>
<summary><b>Tutorial: Working with Species</b></summary>

### Creating Species Identifiers

```rust
use sevensense_core::{SpeciesId, TaxonomicRank};

// From scientific name
let blackbird = SpeciesId::from_scientific("Turdus merula");

// With common name
let blackbird = SpeciesId::new("Turdus merula", Some("Eurasian Blackbird"));

// Check taxonomic information
assert_eq!(blackbird.genus(), "Turdus");
assert_eq!(blackbird.species_epithet(), "merula");
```

### Species Collections

```rust
use sevensense_core::{SpeciesRegistry, SpeciesId};

let mut registry = SpeciesRegistry::new();
registry.register(SpeciesId::from_scientific("Turdus merula"));
registry.register(SpeciesId::from_scientific("Turdus philomelos"));

// Search by partial name
let thrushes = registry.search("Turdus");
println!("Found {} thrush species", thrushes.len());
```

</details>

<details>
<summary><b>Tutorial: Time Range Operations</b></summary>

### Basic Time Ranges

```rust
use sevensense_core::TimeRange;
use chrono::{Utc, Duration};

// Create a range for the last hour
let now = Utc::now();
let range = TimeRange::new(now - Duration::hours(1), now);

// Check duration
println!("Range spans: {:?}", range.duration());

// Check if a timestamp is within range
let test_time = now - Duration::minutes(30);
assert!(range.contains(test_time));
```

### Splitting Time Ranges

```rust
use sevensense_core::TimeRange;
use chrono::Duration;

let range = TimeRange::last_n_hours(24);

// Split into 1-hour windows
let windows = range.split_by_duration(Duration::hours(1));
println!("Created {} 1-hour windows", windows.len());

// Split into equal parts
let parts = range.split_into_n_parts(4);
assert_eq!(parts.len(), 4);
```

</details>

<details>
<summary><b>Tutorial: Error Handling</b></summary>

### Using Core Errors

```rust
use sevensense_core::{CoreError, CoreResult, ErrorContext};

fn process_audio(path: &str) -> CoreResult<()> {
    // Operations that might fail
    let file = std::fs::File::open(path)
        .map_err(|e| CoreError::io(e).with_context("opening audio file"))?;

    Ok(())
}

fn main() {
    match process_audio("missing.wav") {
        Ok(_) => println!("Success!"),
        Err(e) => {
            eprintln!("Error: {}", e);
            if let Some(ctx) = e.context() {
                eprintln!("Context: {}", ctx);
            }
        }
    }
}
```

### Error Chains

```rust
use sevensense_core::{CoreError, CoreResult};

fn outer_function() -> CoreResult<()> {
    inner_function()
        .map_err(|e| e.with_context("in outer_function"))?;
    Ok(())
}

fn inner_function() -> CoreResult<()> {
    Err(CoreError::validation("Invalid input"))
}
```

</details>

---

## API Overview

### Core Types

| Type | Description |
|------|-------------|
| `SpeciesId` | Unique identifier for a bird species |
| `RecordingId` | UUID-based recording identifier |
| `TimeRange` | Start/end time boundary |
| `Location` | Geographic coordinates (lat/lon) |
| `AudioMetadata` | Recording metadata (format, channels, etc.) |

### Traits

| Trait | Description |
|-------|-------------|
| `Identifiable` | Types with unique identifiers |
| `Timestamped` | Types with timestamp information |
| `Bounded` | Types with time boundaries |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-core](https://crates.io/crates/sevensense-core)
- **Documentation**: [docs.rs/sevensense-core](https://docs.rs/sevensense-core)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
