//! Repository trait for sheaf graph persistence.

use super::{SheafGraph, SheafNode};
use crate::error::StorageResult;
use crate::types::{GraphId, NamespaceId, NodeId};

/// Repository trait for sheaf graph persistence.
///
/// This trait defines the interface for storing and retrieving sheaf graphs.
/// Implementations may use various backends (in-memory, PostgreSQL, ruvector, etc.)
#[allow(async_fn_in_trait)]
pub trait SheafGraphRepository: Send + Sync {
    /// Find a graph by its ID.
    async fn find_by_id(&self, id: GraphId) -> StorageResult<Option<SheafGraph>>;

    /// Save a graph (insert or update).
    async fn save(&self, graph: &SheafGraph) -> StorageResult<()>;

    /// Delete a graph.
    async fn delete(&self, id: GraphId) -> StorageResult<()>;

    /// Find all nodes in a namespace.
    async fn find_nodes_by_namespace(&self, namespace: &NamespaceId) -> StorageResult<Vec<SheafNode>>;

    /// Find nodes similar to a query state using vector search.
    async fn find_similar_nodes(
        &self,
        state: &[f32],
        k: usize,
    ) -> StorageResult<Vec<(NodeId, f32)>>;
}

/// In-memory repository implementation (for testing).
#[derive(Debug, Default)]
pub struct InMemoryGraphRepository {
    graphs: parking_lot::RwLock<std::collections::HashMap<GraphId, SheafGraph>>,
}

impl InMemoryGraphRepository {
    /// Create a new in-memory repository.
    pub fn new() -> Self {
        Self::default()
    }
}

// Note: Actual async implementation would go here if the `tokio` feature is enabled.
// For now, we provide a synchronous implementation.

impl InMemoryGraphRepository {
    /// Find a graph by ID (sync version).
    pub fn find_by_id_sync(&self, id: GraphId) -> Option<SheafGraph> {
        // Note: SheafGraph doesn't implement Clone due to DashMap,
        // so we can't easily clone it. In practice, you'd need a different
        // approach for in-memory storage.
        let _graphs = self.graphs.read();
        // This is a placeholder - real implementation would need redesign
        None
    }
}
