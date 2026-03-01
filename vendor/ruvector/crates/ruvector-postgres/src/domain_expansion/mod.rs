//! Domain expansion module — cross-domain transfer learning for PostgreSQL.

pub mod operators;

use dashmap::DashMap;
use parking_lot::RwLock;
use ruvector_domain_expansion::DomainExpansionEngine;
use std::sync::Arc;

/// Global domain expansion engine state.
static DOMAIN_ENGINES: once_cell::sync::Lazy<DashMap<String, Arc<RwLock<DomainExpansionEngine>>>> =
    once_cell::sync::Lazy::new(DashMap::new);

/// Get or create a DomainExpansionEngine for a given context.
pub fn get_or_create_engine(context: &str) -> Arc<RwLock<DomainExpansionEngine>> {
    DOMAIN_ENGINES
        .entry(context.to_string())
        .or_insert_with(|| Arc::new(RwLock::new(DomainExpansionEngine::new())))
        .value()
        .clone()
}
