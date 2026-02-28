//! # ruqu-exotic — Exotic Quantum-Classical Hybrid Algorithms
//!
//! Novel algorithms that emerge from embedding a quantum simulation engine
//! inside a vector database stack. These were structurally impossible before
//! because the required primitives (amplitude space, interference, decoherence,
//! syndrome extraction) did not coexist with vector search infrastructure.
//!
//! ## Modules
//!
//! | Module | Concept | What it replaces |
//! |--------|---------|-----------------|
//! | [`quantum_decay`] | Embeddings decohere instead of being deleted | TTL-based eviction |
//! | [`interference_search`] | Concepts interfere during retrieval | Cosine reranking |
//! | [`quantum_collapse`] | Search collapses from superposition | Deterministic top-k |
//! | [`reasoning_qec`] | Surface-code correction on reasoning traces | Semantic checks |
//! | [`swarm_interference`] | Agents interfere instead of voting | Consensus protocols |
//! | [`syndrome_diagnosis`] | QEC syndrome extraction for system diagnosis | Log-based monitoring |
//! | [`reversible_memory`] | Time-reversible state for counterfactual debugging | Forward-only ML |
//! | [`reality_check`] | Browser-native quantum verification circuits | Trust-based claims |

pub mod interference_search;
pub mod quantum_collapse;
pub mod quantum_decay;
pub mod reality_check;
pub mod reasoning_qec;
pub mod reversible_memory;
pub mod swarm_interference;
pub mod syndrome_diagnosis;
