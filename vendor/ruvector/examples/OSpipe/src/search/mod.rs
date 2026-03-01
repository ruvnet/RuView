//! Query routing and hybrid search.
//!
//! Provides intelligent query routing that selects the optimal search
//! backend (semantic, keyword, temporal, graph, or hybrid) based on
//! query characteristics.

pub mod enhanced;
pub mod hybrid;
pub mod mmr;
pub mod reranker;
pub mod router;

pub use enhanced::EnhancedSearch;
pub use hybrid::HybridSearch;
pub use mmr::MmrReranker;
pub use reranker::AttentionReranker;
pub use router::{QueryRoute, QueryRouter};
