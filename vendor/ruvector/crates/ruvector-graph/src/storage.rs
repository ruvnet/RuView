//! Persistent storage layer with redb and memory-mapped vectors
//!
//! Provides ACID-compliant storage for graph nodes, edges, and hyperedges

#[cfg(feature = "storage")]
use crate::edge::Edge;
#[cfg(feature = "storage")]
use crate::hyperedge::{Hyperedge, HyperedgeId};
#[cfg(feature = "storage")]
use crate::node::Node;
#[cfg(feature = "storage")]
use crate::types::{EdgeId, NodeId};
#[cfg(feature = "storage")]
use anyhow::Result;
#[cfg(feature = "storage")]
use bincode::config;
#[cfg(feature = "storage")]
use once_cell::sync::Lazy;
#[cfg(feature = "storage")]
use parking_lot::Mutex;
#[cfg(feature = "storage")]
use redb::{Database, ReadableTable, TableDefinition};
#[cfg(feature = "storage")]
use std::collections::HashMap;
#[cfg(feature = "storage")]
use std::path::{Path, PathBuf};
#[cfg(feature = "storage")]
use std::sync::Arc;

#[cfg(feature = "storage")]
// Table definitions
const NODES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("nodes");
#[cfg(feature = "storage")]
const EDGES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("edges");
#[cfg(feature = "storage")]
const HYPEREDGES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("hyperedges");
#[cfg(feature = "storage")]
const METADATA_TABLE: TableDefinition<&str, &str> = TableDefinition::new("metadata");

#[cfg(feature = "storage")]
// Global database connection pool to allow multiple GraphStorage instances
// to share the same underlying database file
static DB_POOL: Lazy<Mutex<HashMap<PathBuf, Arc<Database>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[cfg(feature = "storage")]
/// Storage backend for graph database
pub struct GraphStorage {
    db: Arc<Database>,
}

#[cfg(feature = "storage")]
impl GraphStorage {
    /// Create or open a graph storage at the given path
    ///
    /// Uses a global connection pool to allow multiple GraphStorage
    /// instances to share the same underlying database file
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path_ref.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Convert to absolute path
        let path_buf = if path_ref.is_absolute() {
            path_ref.to_path_buf()
        } else {
            std::env::current_dir()?.join(path_ref)
        };

        // SECURITY: Check for path traversal attempts
        let path_str = path_ref.to_string_lossy();
        if path_str.contains("..") && !path_ref.is_absolute() {
            if let Ok(cwd) = std::env::current_dir() {
                let mut normalized = cwd.clone();
                for component in path_ref.components() {
                    match component {
                        std::path::Component::ParentDir => {
                            if !normalized.pop() || !normalized.starts_with(&cwd) {
                                anyhow::bail!("Path traversal attempt detected");
                            }
                        }
                        std::path::Component::Normal(c) => normalized.push(c),
                        _ => {}
                    }
                }
            }
        }

        // Check if we already have a Database instance for this path
        let db = {
            let mut pool = DB_POOL.lock();

            if let Some(existing_db) = pool.get(&path_buf) {
                // Reuse existing database connection
                Arc::clone(existing_db)
            } else {
                // Create new database and add to pool
                let new_db = Arc::new(Database::create(&path_buf)?);

                // Initialize tables
                let write_txn = new_db.begin_write()?;
                {
                    let _ = write_txn.open_table(NODES_TABLE)?;
                    let _ = write_txn.open_table(EDGES_TABLE)?;
                    let _ = write_txn.open_table(HYPEREDGES_TABLE)?;
                    let _ = write_txn.open_table(METADATA_TABLE)?;
                }
                write_txn.commit()?;

                pool.insert(path_buf, Arc::clone(&new_db));
                new_db
            }
        };

        Ok(Self { db })
    }

    // Node operations

    /// Insert a node
    pub fn insert_node(&self, node: &Node) -> Result<NodeId> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(NODES_TABLE)?;

            // Serialize node data
            let node_data = bincode::encode_to_vec(node, config::standard())?;
            table.insert(node.id.as_str(), node_data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(node.id.clone())
    }

    /// Insert multiple nodes in a batch
    pub fn insert_nodes_batch(&self, nodes: &[Node]) -> Result<Vec<NodeId>> {
        let write_txn = self.db.begin_write()?;
        let mut ids = Vec::with_capacity(nodes.len());

        {
            let mut table = write_txn.open_table(NODES_TABLE)?;

            for node in nodes {
                let node_data = bincode::encode_to_vec(node, config::standard())?;
                table.insert(node.id.as_str(), node_data.as_slice())?;
                ids.push(node.id.clone());
            }
        }

        write_txn.commit()?;
        Ok(ids)
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Result<Option<Node>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;

        let Some(node_data) = table.get(id)? else {
            return Ok(None);
        };

        let (node, _): (Node, usize) =
            bincode::decode_from_slice(node_data.value(), config::standard())?;
        Ok(Some(node))
    }

    /// Delete a node by ID
    pub fn delete_node(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let deleted;
        {
            let mut table = write_txn.open_table(NODES_TABLE)?;
            let result = table.remove(id)?;
            deleted = result.is_some();
        }
        write_txn.commit()?;
        Ok(deleted)
    }

    /// Get all node IDs
    pub fn all_node_ids(&self) -> Result<Vec<NodeId>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;

        let mut ids = Vec::new();
        let iter = table.iter()?;
        for item in iter {
            let (key, _) = item?;
            ids.push(key.value().to_string());
        }

        Ok(ids)
    }

    // Edge operations

    /// Insert an edge
    pub fn insert_edge(&self, edge: &Edge) -> Result<EdgeId> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(EDGES_TABLE)?;

            // Serialize edge data
            let edge_data = bincode::encode_to_vec(edge, config::standard())?;
            table.insert(edge.id.as_str(), edge_data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(edge.id.clone())
    }

    /// Insert multiple edges in a batch
    pub fn insert_edges_batch(&self, edges: &[Edge]) -> Result<Vec<EdgeId>> {
        let write_txn = self.db.begin_write()?;
        let mut ids = Vec::with_capacity(edges.len());

        {
            let mut table = write_txn.open_table(EDGES_TABLE)?;

            for edge in edges {
                let edge_data = bincode::encode_to_vec(edge, config::standard())?;
                table.insert(edge.id.as_str(), edge_data.as_slice())?;
                ids.push(edge.id.clone());
            }
        }

        write_txn.commit()?;
        Ok(ids)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: &str) -> Result<Option<Edge>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(EDGES_TABLE)?;

        let Some(edge_data) = table.get(id)? else {
            return Ok(None);
        };

        let (edge, _): (Edge, usize) =
            bincode::decode_from_slice(edge_data.value(), config::standard())?;
        Ok(Some(edge))
    }

    /// Delete an edge by ID
    pub fn delete_edge(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let deleted;
        {
            let mut table = write_txn.open_table(EDGES_TABLE)?;
            let result = table.remove(id)?;
            deleted = result.is_some();
        }
        write_txn.commit()?;
        Ok(deleted)
    }

    /// Get all edge IDs
    pub fn all_edge_ids(&self) -> Result<Vec<EdgeId>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(EDGES_TABLE)?;

        let mut ids = Vec::new();
        let iter = table.iter()?;
        for item in iter {
            let (key, _) = item?;
            ids.push(key.value().to_string());
        }

        Ok(ids)
    }

    // Hyperedge operations

    /// Insert a hyperedge
    pub fn insert_hyperedge(&self, hyperedge: &Hyperedge) -> Result<HyperedgeId> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(HYPEREDGES_TABLE)?;

            // Serialize hyperedge data
            let hyperedge_data = bincode::encode_to_vec(hyperedge, config::standard())?;
            table.insert(hyperedge.id.as_str(), hyperedge_data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(hyperedge.id.clone())
    }

    /// Insert multiple hyperedges in a batch
    pub fn insert_hyperedges_batch(&self, hyperedges: &[Hyperedge]) -> Result<Vec<HyperedgeId>> {
        let write_txn = self.db.begin_write()?;
        let mut ids = Vec::with_capacity(hyperedges.len());

        {
            let mut table = write_txn.open_table(HYPEREDGES_TABLE)?;

            for hyperedge in hyperedges {
                let hyperedge_data = bincode::encode_to_vec(hyperedge, config::standard())?;
                table.insert(hyperedge.id.as_str(), hyperedge_data.as_slice())?;
                ids.push(hyperedge.id.clone());
            }
        }

        write_txn.commit()?;
        Ok(ids)
    }

    /// Get a hyperedge by ID
    pub fn get_hyperedge(&self, id: &str) -> Result<Option<Hyperedge>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(HYPEREDGES_TABLE)?;

        let Some(hyperedge_data) = table.get(id)? else {
            return Ok(None);
        };

        let (hyperedge, _): (Hyperedge, usize) =
            bincode::decode_from_slice(hyperedge_data.value(), config::standard())?;
        Ok(Some(hyperedge))
    }

    /// Delete a hyperedge by ID
    pub fn delete_hyperedge(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let deleted;
        {
            let mut table = write_txn.open_table(HYPEREDGES_TABLE)?;
            let result = table.remove(id)?;
            deleted = result.is_some();
        }
        write_txn.commit()?;
        Ok(deleted)
    }

    /// Get all hyperedge IDs
    pub fn all_hyperedge_ids(&self) -> Result<Vec<HyperedgeId>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(HYPEREDGES_TABLE)?;

        let mut ids = Vec::new();
        let iter = table.iter()?;
        for item in iter {
            let (key, _) = item?;
            ids.push(key.value().to_string());
        }

        Ok(ids)
    }

    // Metadata operations

    /// Set metadata
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(METADATA_TABLE)?;

        let value = table.get(key)?.map(|v| v.value().to_string());
        Ok(value)
    }

    // Statistics

    /// Get the number of nodes
    pub fn node_count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;
        Ok(table.iter()?.count())
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(EDGES_TABLE)?;
        Ok(table.iter()?.count())
    }

    /// Get the number of hyperedges
    pub fn hyperedge_count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(HYPEREDGES_TABLE)?;
        Ok(table.iter()?.count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::EdgeBuilder;
    use crate::hyperedge::HyperedgeBuilder;
    use crate::node::NodeBuilder;
    use tempfile::tempdir;

    #[test]
    fn test_node_storage() -> Result<()> {
        let dir = tempdir()?;
        let storage = GraphStorage::new(dir.path().join("test.db"))?;

        let node = NodeBuilder::new()
            .label("Person")
            .property("name", "Alice")
            .build();

        let id = storage.insert_node(&node)?;
        assert_eq!(id, node.id);

        let retrieved = storage.get_node(&id)?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, node.id);
        assert!(retrieved.has_label("Person"));

        Ok(())
    }

    #[test]
    fn test_edge_storage() -> Result<()> {
        let dir = tempdir()?;
        let storage = GraphStorage::new(dir.path().join("test.db"))?;

        let edge = EdgeBuilder::new("n1".to_string(), "n2".to_string(), "KNOWS")
            .property("since", 2020i64)
            .build();

        let id = storage.insert_edge(&edge)?;
        assert_eq!(id, edge.id);

        let retrieved = storage.get_edge(&id)?;
        assert!(retrieved.is_some());

        Ok(())
    }

    #[test]
    fn test_batch_insert() -> Result<()> {
        let dir = tempdir()?;
        let storage = GraphStorage::new(dir.path().join("test.db"))?;

        let nodes = vec![
            NodeBuilder::new().label("Person").build(),
            NodeBuilder::new().label("Person").build(),
        ];

        let ids = storage.insert_nodes_batch(&nodes)?;
        assert_eq!(ids.len(), 2);
        assert_eq!(storage.node_count()?, 2);

        Ok(())
    }

    #[test]
    fn test_hyperedge_storage() -> Result<()> {
        let dir = tempdir()?;
        let storage = GraphStorage::new(dir.path().join("test.db"))?;

        let hyperedge = HyperedgeBuilder::new(
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
            "MEETING",
        )
        .description("Team meeting")
        .build();

        let id = storage.insert_hyperedge(&hyperedge)?;
        assert_eq!(id, hyperedge.id);

        let retrieved = storage.get_hyperedge(&id)?;
        assert!(retrieved.is_some());

        Ok(())
    }
}
