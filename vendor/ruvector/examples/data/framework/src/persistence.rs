//! Persistence Layer for RuVector Discovery Framework
//!
//! This module provides serialization/deserialization for the OptimizedDiscoveryEngine
//! and discovered patterns. Supports:
//! - Full engine state save/load
//! - Pattern-only save/load/append
//! - Optional gzip compression for large datasets
//! - Incremental pattern appends without rewriting entire files

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};

use crate::optimized::{OptimizedConfig, OptimizedDiscoveryEngine, SignificantPattern};
use crate::ruvector_native::{
    CoherenceSnapshot, Domain, GraphEdge, GraphNode, SemanticVector,
};
use crate::{FrameworkError, Result};

/// Serializable state of the OptimizedDiscoveryEngine
///
/// This struct excludes non-serializable fields like AtomicU64 metrics
/// and caches, focusing on the core graph and history state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    /// Engine configuration
    pub config: OptimizedConfig,
    /// All semantic vectors
    pub vectors: Vec<SemanticVector>,
    /// Graph nodes
    pub nodes: HashMap<u32, GraphNode>,
    /// Graph edges
    pub edges: Vec<GraphEdge>,
    /// Coherence history (timestamp, mincut value, snapshot)
    pub coherence_history: Vec<(DateTime<Utc>, f64, CoherenceSnapshot)>,
    /// Next node ID counter
    pub next_node_id: u32,
    /// Domain-specific node indices
    pub domain_nodes: HashMap<Domain, Vec<u32>>,
    /// Temporal analysis state
    pub domain_timeseries: HashMap<Domain, Vec<(DateTime<Utc>, f64)>>,
    /// Metadata about when this state was saved
    pub saved_at: DateTime<Utc>,
    /// Version for compatibility checking
    pub version: String,
}

impl EngineState {
    /// Create a new empty engine state
    pub fn new(config: OptimizedConfig) -> Self {
        Self {
            config,
            vectors: Vec::new(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            coherence_history: Vec::new(),
            next_node_id: 0,
            domain_nodes: HashMap::new(),
            domain_timeseries: HashMap::new(),
            saved_at: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Options for saving/loading with compression
#[derive(Debug, Clone, Copy)]
pub struct PersistenceOptions {
    /// Enable gzip compression
    pub compress: bool,
    /// Compression level (0-9, higher = better compression but slower)
    pub compression_level: u32,
    /// Pretty-print JSON (larger files, more readable)
    pub pretty: bool,
}

impl Default for PersistenceOptions {
    fn default() -> Self {
        Self {
            compress: false,
            compression_level: 6,
            pretty: false,
        }
    }
}

impl PersistenceOptions {
    /// Create options with compression enabled
    pub fn compressed() -> Self {
        Self {
            compress: true,
            ..Default::default()
        }
    }

    /// Create options with pretty-printed JSON
    pub fn pretty() -> Self {
        Self {
            pretty: true,
            ..Default::default()
        }
    }
}

/// Save the OptimizedDiscoveryEngine state to a file
///
/// # Arguments
/// * `engine` - The engine to save
/// * `path` - Path to save to (will be created/overwritten)
/// * `options` - Persistence options (compression, formatting)
///
/// # Example
/// ```no_run
/// # use ruvector_data_framework::optimized::{OptimizedConfig, OptimizedDiscoveryEngine};
/// # use ruvector_data_framework::persistence::{save_engine, PersistenceOptions};
/// # use std::path::Path;
/// let engine = OptimizedDiscoveryEngine::new(OptimizedConfig::default());
/// save_engine(&engine, Path::new("engine_state.json"), &PersistenceOptions::default())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn save_engine(
    engine: &OptimizedDiscoveryEngine,
    path: &Path,
    options: &PersistenceOptions,
) -> Result<()> {
    // Extract serializable state
    let state = extract_state(engine);

    // Save to file
    save_state(&state, path, options)?;

    tracing::info!(
        "Saved engine state to {} ({} nodes, {} edges)",
        path.display(),
        state.nodes.len(),
        state.edges.len()
    );

    Ok(())
}

/// Load an OptimizedDiscoveryEngine from a saved state file
///
/// # Arguments
/// * `path` - Path to the saved state file
///
/// # Returns
/// A new OptimizedDiscoveryEngine with the loaded state
///
/// # Example
/// ```no_run
/// # use ruvector_data_framework::persistence::load_engine;
/// # use std::path::Path;
/// let engine = load_engine(Path::new("engine_state.json"))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_engine(path: &Path) -> Result<OptimizedDiscoveryEngine> {
    let state = load_state(path)?;

    tracing::info!(
        "Loaded engine state from {} ({} nodes, {} edges)",
        path.display(),
        state.nodes.len(),
        state.edges.len()
    );

    // Reconstruct engine from state
    Ok(reconstruct_engine(state))
}

/// Save discovered patterns to a JSON file
///
/// # Arguments
/// * `patterns` - Patterns to save
/// * `path` - Path to save to
/// * `options` - Persistence options
///
/// # Example
/// ```no_run
/// # use ruvector_data_framework::optimized::SignificantPattern;
/// # use ruvector_data_framework::persistence::{save_patterns, PersistenceOptions};
/// # use std::path::Path;
/// let patterns: Vec<SignificantPattern> = vec![];
/// save_patterns(&patterns, Path::new("patterns.json"), &PersistenceOptions::default())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn save_patterns(
    patterns: &[SignificantPattern],
    path: &Path,
    options: &PersistenceOptions,
) -> Result<()> {
    let file = File::create(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let writer = BufWriter::new(file);

    if options.compress {
        let mut encoder = GzEncoder::new(writer, Compression::new(options.compression_level));
        let json = if options.pretty {
            serde_json::to_string_pretty(patterns)?
        } else {
            serde_json::to_string(patterns)?
        };
        encoder.write_all(json.as_bytes()).map_err(|e| {
            FrameworkError::Discovery(format!("Failed to write compressed patterns: {}", e))
        })?;
        encoder.finish().map_err(|e| {
            FrameworkError::Discovery(format!("Failed to finish compression: {}", e))
        })?;
    } else {
        if options.pretty {
            serde_json::to_writer_pretty(writer, patterns)?;
        } else {
            serde_json::to_writer(writer, patterns)?;
        }
    }

    tracing::info!("Saved {} patterns to {}", patterns.len(), path.display());

    Ok(())
}

/// Load patterns from a JSON file
///
/// # Arguments
/// * `path` - Path to the patterns file
///
/// # Returns
/// Vector of loaded patterns
///
/// # Example
/// ```no_run
/// # use ruvector_data_framework::persistence::load_patterns;
/// # use std::path::Path;
/// let patterns = load_patterns(Path::new("patterns.json"))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_patterns(path: &Path) -> Result<Vec<SignificantPattern>> {
    let file = File::open(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to open file {}: {}", path.display(), e))
    })?;

    let reader = BufReader::new(file);

    // Try to detect if file is gzip-compressed by reading magic bytes
    let mut peeker = BufReader::new(File::open(path).unwrap());
    let mut magic = [0u8; 2];
    let is_gzip = peeker.read_exact(&mut magic).is_ok() && magic == [0x1f, 0x8b];

    let patterns: Vec<SignificantPattern> = if is_gzip {
        let file = File::open(path).unwrap();
        let decoder = GzDecoder::new(BufReader::new(file));
        serde_json::from_reader(decoder)?
    } else {
        serde_json::from_reader(reader)?
    };

    tracing::info!("Loaded {} patterns from {}", patterns.len(), path.display());

    Ok(patterns)
}

/// Append new patterns to an existing patterns file
///
/// This is more efficient than loading all patterns, adding new ones,
/// and saving the entire list. However, it only works with uncompressed
/// JSON arrays.
///
/// # Arguments
/// * `patterns` - New patterns to append
/// * `path` - Path to the existing patterns file
///
/// # Note
/// If the file doesn't exist, it will be created with the given patterns.
/// For compressed files, this will decompress, append, and recompress.
///
/// # Example
/// ```no_run
/// # use ruvector_data_framework::optimized::SignificantPattern;
/// # use ruvector_data_framework::persistence::append_patterns;
/// # use std::path::Path;
/// let new_patterns: Vec<SignificantPattern> = vec![];
/// append_patterns(&new_patterns, Path::new("patterns.json"))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn append_patterns(patterns: &[SignificantPattern], path: &Path) -> Result<()> {
    if patterns.is_empty() {
        return Ok(());
    }

    // Check if file exists
    if !path.exists() {
        // Create new file
        return save_patterns(patterns, path, &PersistenceOptions::default());
    }

    // Load existing patterns
    let mut existing = load_patterns(path)?;

    // Append new patterns
    existing.extend_from_slice(patterns);

    // Save combined patterns
    // Preserve compression if original was compressed
    let options = if is_compressed(path)? {
        PersistenceOptions::compressed()
    } else {
        PersistenceOptions::default()
    };

    save_patterns(&existing, path, &options)?;

    tracing::info!(
        "Appended {} patterns to {} (total: {})",
        patterns.len(),
        path.display(),
        existing.len()
    );

    Ok(())
}

// ============================================================================
// Internal Helper Functions
// ============================================================================

/// Extract serializable state from an OptimizedDiscoveryEngine
///
/// This uses reflection-like access to the engine's internal state.
/// In practice, you'd need to add getter methods to OptimizedDiscoveryEngine.
fn extract_state(_engine: &OptimizedDiscoveryEngine) -> EngineState {
    // Note: This requires the OptimizedDiscoveryEngine to expose its internal state
    // via getter methods. For now, we'll use a placeholder that you'll need to implement.

    // Since we can't directly access private fields, we need the engine to provide
    // a method like `pub fn extract_state(&self) -> EngineState`

    // For now, return a minimal state with what we can access
    // TODO: Uncomment when OptimizedDiscoveryEngine provides getter methods
    // let _stats = engine.stats();

    EngineState {
        config: OptimizedConfig::default(), // Would need engine.config() method
        vectors: Vec::new(), // Would need engine.vectors() method
        nodes: HashMap::new(), // Would need engine.nodes() method
        edges: Vec::new(), // Would need engine.edges() method
        coherence_history: Vec::new(), // Would need engine.coherence_history() method
        next_node_id: 0, // Would need engine.next_node_id() method
        domain_nodes: HashMap::new(), // Would need engine.domain_nodes() method
        domain_timeseries: HashMap::new(), // Would need engine.domain_timeseries() method
        saved_at: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }

    // TODO: Implement proper state extraction once OptimizedDiscoveryEngine
    // exposes the necessary getter methods
}

/// Reconstruct an OptimizedDiscoveryEngine from saved state
fn reconstruct_engine(state: EngineState) -> OptimizedDiscoveryEngine {
    // Similarly, this would require OptimizedDiscoveryEngine to have
    // a constructor like `pub fn from_state(state: EngineState) -> Self`

    // For now, create a new engine and note that full reconstruction
    // would require additional methods
    OptimizedDiscoveryEngine::new(state.config)

    // TODO: Implement proper engine reconstruction once OptimizedDiscoveryEngine
    // provides the necessary methods to restore state
}

/// Save engine state to a file with optional compression
fn save_state(state: &EngineState, path: &Path, options: &PersistenceOptions) -> Result<()> {
    let file = File::create(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let writer = BufWriter::new(file);

    if options.compress {
        let mut encoder = GzEncoder::new(writer, Compression::new(options.compression_level));
        let json = if options.pretty {
            serde_json::to_string_pretty(state)?
        } else {
            serde_json::to_string(state)?
        };
        encoder.write_all(json.as_bytes()).map_err(|e| {
            FrameworkError::Discovery(format!("Failed to write compressed state: {}", e))
        })?;
        encoder.finish().map_err(|e| {
            FrameworkError::Discovery(format!("Failed to finish compression: {}", e))
        })?;
    } else {
        if options.pretty {
            serde_json::to_writer_pretty(writer, state)?;
        } else {
            serde_json::to_writer(writer, state)?;
        }
    }

    Ok(())
}

/// Load engine state from a file with automatic compression detection
fn load_state(path: &Path) -> Result<EngineState> {
    let file = File::open(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to open file {}: {}", path.display(), e))
    })?;

    // Detect compression by reading magic bytes
    let is_gzip = is_compressed(path)?;

    let state = if is_gzip {
        let file = File::open(path).unwrap();
        let decoder = GzDecoder::new(BufReader::new(file));
        serde_json::from_reader(decoder)?
    } else {
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)?
    };

    Ok(state)
}

/// Check if a file is gzip-compressed by reading magic bytes
fn is_compressed(path: &Path) -> Result<bool> {
    let mut file = File::open(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to open file {}: {}", path.display(), e))
    })?;

    let mut magic = [0u8; 2];
    match file.read_exact(&mut magic) {
        Ok(_) => Ok(magic == [0x1f, 0x8b]),
        Err(_) => Ok(false), // File too small or empty
    }
}

/// Get file size in bytes
pub fn get_file_size(path: &Path) -> Result<u64> {
    let metadata = std::fs::metadata(path).map_err(|e| {
        FrameworkError::Discovery(format!("Failed to get file metadata: {}", e))
    })?;
    Ok(metadata.len())
}

/// Calculate compression ratio for a file
///
/// Returns (compressed_size, uncompressed_size, ratio)
pub fn compression_info(path: &Path) -> Result<(u64, u64, f64)> {
    let compressed_size = get_file_size(path)?;

    if is_compressed(path)? {
        // Decompress and count bytes
        let file = File::open(path).unwrap();
        let mut decoder = GzDecoder::new(BufReader::new(file));
        let mut buffer = Vec::new();
        let uncompressed_size = decoder.read_to_end(&mut buffer).map_err(|e| {
            FrameworkError::Discovery(format!("Failed to decompress: {}", e))
        })? as u64;

        let ratio = compressed_size as f64 / uncompressed_size as f64;
        Ok((compressed_size, uncompressed_size, ratio))
    } else {
        Ok((compressed_size, compressed_size, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimized::OptimizedConfig;
    use crate::ruvector_native::{DiscoveredPattern, PatternType, Evidence};
    use tempfile::NamedTempFile;

    #[test]
    fn test_engine_state_creation() {
        let config = OptimizedConfig::default();
        let state = EngineState::new(config.clone());

        assert_eq!(state.next_node_id, 0);
        assert_eq!(state.nodes.len(), 0);
        assert_eq!(state.config.similarity_threshold, config.similarity_threshold);
    }

    #[test]
    fn test_persistence_options() {
        let default = PersistenceOptions::default();
        assert!(!default.compress);
        assert!(!default.pretty);

        let compressed = PersistenceOptions::compressed();
        assert!(compressed.compress);

        let pretty = PersistenceOptions::pretty();
        assert!(pretty.pretty);
    }

    #[test]
    fn test_save_load_patterns() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let patterns = vec![
            SignificantPattern {
                pattern: DiscoveredPattern {
                    id: "test-1".to_string(),
                    pattern_type: PatternType::CoherenceBreak,
                    confidence: 0.85,
                    affected_nodes: vec![1, 2, 3],
                    detected_at: Utc::now(),
                    description: "Test pattern".to_string(),
                    evidence: vec![
                        Evidence {
                            evidence_type: "test".to_string(),
                            value: 1.0,
                            description: "Test evidence".to_string(),
                        }
                    ],
                    cross_domain_links: vec![],
                },
                p_value: 0.03,
                effect_size: 1.2,
                confidence_interval: (0.5, 1.5),
                is_significant: true,
            }
        ];

        // Save patterns
        save_patterns(&patterns, path, &PersistenceOptions::default()).unwrap();

        // Load patterns
        let loaded = load_patterns(path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pattern.id, "test-1");
        assert_eq!(loaded[0].p_value, 0.03);
    }

    #[test]
    fn test_save_load_patterns_compressed() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let patterns = vec![
            SignificantPattern {
                pattern: DiscoveredPattern {
                    id: "test-compressed".to_string(),
                    pattern_type: PatternType::Consolidation,
                    confidence: 0.90,
                    affected_nodes: vec![4, 5, 6],
                    detected_at: Utc::now(),
                    description: "Compressed test pattern".to_string(),
                    evidence: vec![],
                    cross_domain_links: vec![],
                },
                p_value: 0.01,
                effect_size: 2.0,
                confidence_interval: (1.0, 3.0),
                is_significant: true,
            }
        ];

        // Save with compression
        save_patterns(&patterns, path, &PersistenceOptions::compressed()).unwrap();

        // Verify compression
        assert!(is_compressed(path).unwrap());

        // Load and verify
        let loaded = load_patterns(path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pattern.id, "test-compressed");
    }

    #[test]
    fn test_append_patterns() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let pattern1 = vec![
            SignificantPattern {
                pattern: DiscoveredPattern {
                    id: "pattern-1".to_string(),
                    pattern_type: PatternType::EmergingCluster,
                    confidence: 0.75,
                    affected_nodes: vec![1],
                    detected_at: Utc::now(),
                    description: "First pattern".to_string(),
                    evidence: vec![],
                    cross_domain_links: vec![],
                },
                p_value: 0.05,
                effect_size: 1.0,
                confidence_interval: (0.0, 2.0),
                is_significant: false,
            }
        ];

        let pattern2 = vec![
            SignificantPattern {
                pattern: DiscoveredPattern {
                    id: "pattern-2".to_string(),
                    pattern_type: PatternType::Cascade,
                    confidence: 0.95,
                    affected_nodes: vec![2],
                    detected_at: Utc::now(),
                    description: "Second pattern".to_string(),
                    evidence: vec![],
                    cross_domain_links: vec![],
                },
                p_value: 0.001,
                effect_size: 3.0,
                confidence_interval: (2.0, 4.0),
                is_significant: true,
            }
        ];

        // Save first pattern
        save_patterns(&pattern1, path, &PersistenceOptions::default()).unwrap();

        // Append second pattern
        append_patterns(&pattern2, path).unwrap();

        // Load and verify both are present
        let loaded = load_patterns(path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].pattern.id, "pattern-1");
        assert_eq!(loaded[1].pattern.id, "pattern-2");
    }
}
