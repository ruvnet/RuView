//! Sona self-learning module — Micro-LoRA trajectories and EWC++ for PostgreSQL.

pub mod operators;

use dashmap::DashMap;
use ruvector_sona::{SonaConfig, SonaEngine};
use std::sync::Arc;

/// Global Sona engine state per table.
static SONA_ENGINES: once_cell::sync::Lazy<DashMap<String, Arc<SonaEngine>>> =
    once_cell::sync::Lazy::new(DashMap::new);

/// Get or create a SonaEngine for a given table.
pub fn get_or_create_engine(table_name: &str) -> Arc<SonaEngine> {
    SONA_ENGINES
        .entry(table_name.to_string())
        .or_insert_with(|| {
            Arc::new(SonaEngine::with_config(SonaConfig {
                hidden_dim: 256,
                embedding_dim: 256,
                ..Default::default()
            }))
        })
        .value()
        .clone()
}
