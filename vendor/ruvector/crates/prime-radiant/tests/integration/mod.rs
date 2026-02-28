//! Integration tests for Prime-Radiant Coherence Engine
//!
//! This module contains integration tests organized by bounded context:
//!
//! - `graph_tests`: SheafGraph CRUD operations and dimension validation
//! - `coherence_tests`: Energy computation and incremental updates
//! - `governance_tests`: Policy bundles and witness chain integrity
//! - `gate_tests`: Compute ladder escalation and persistence detection

mod coherence_tests;
mod gate_tests;
mod governance_tests;
mod graph_tests;
