//! RVF adapter for SONA (Self-Optimizing Neural Architecture).
//!
//! This crate bridges SONA's learning trajectory tracking, pattern
//! recognition, and experience replay with the RuVector Format (RVF)
//! segment store per ADR-029. All three data types share a single
//! underlying RVF file, distinguished by a type marker in metadata
//! field 4.
//!
//! # Architecture
//!
//! - **`TrajectoryStore`**: Records and queries sequences of state
//!   embeddings that form a learning trajectory.
//! - **`ExperienceReplayBuffer`**: Circular buffer of (state, action,
//!   reward, next_state) tuples for off-policy training.
//! - **`NeuralPatternStore`**: Stores recognized neural patterns with
//!   confidence scores, searchable by category or embedding similarity.
//! - **`SonaConfig`**: Configuration for data directory, dimension,
//!   replay capacity, and trajectory window size.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rvf_adapter_sona::{SonaConfig, TrajectoryStore, ExperienceReplayBuffer, NeuralPatternStore};
//!
//! let config = SonaConfig::new("/tmp/sona-data", 256);
//! let mut trajectory = TrajectoryStore::create(config.clone()).unwrap();
//!
//! let embedding = vec![0.1f32; 256];
//! trajectory.record_step(1, &embedding, "explore", 0.5).unwrap();
//!
//! let recent = trajectory.get_recent(10);
//! let similar = trajectory.search_similar_states(&embedding, 5).unwrap();
//! trajectory.close().unwrap();
//! ```

pub mod config;
pub mod experience;
pub mod pattern;
pub mod trajectory;

pub use config::{ConfigError, SonaConfig};
pub use experience::{Experience, ExperienceReplayBuffer};
pub use pattern::{NeuralPattern, NeuralPatternStore};
pub use trajectory::{TrajectoryStep, TrajectoryStore};
