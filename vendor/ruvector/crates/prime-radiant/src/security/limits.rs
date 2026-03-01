//! Resource Limits Configuration
//!
//! Defines configurable limits to prevent resource exhaustion attacks.

use serde::{Deserialize, Serialize};

/// Default maximum number of nodes in a graph
pub const DEFAULT_MAX_NODES: usize = 1_000_000;

/// Default maximum number of edges in a graph
pub const DEFAULT_MAX_EDGES: usize = 10_000_000;

/// Default maximum state vector dimension
pub const DEFAULT_MAX_STATE_DIM: usize = 65536;

/// Default maximum matrix dimension (for restriction maps)
pub const DEFAULT_MAX_MATRIX_DIM: usize = 8192;

/// Default maximum payload size in bytes (10 MB)
pub const DEFAULT_MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Default maximum node ID length
pub const DEFAULT_MAX_NODE_ID_LEN: usize = 256;

/// Default maximum concurrent computations
pub const DEFAULT_MAX_CONCURRENT_OPS: usize = 100;

/// Graph size limits to prevent DoS through resource exhaustion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLimits {
    /// Maximum number of nodes allowed
    pub max_nodes: usize,
    /// Maximum number of edges allowed
    pub max_edges: usize,
    /// Maximum state vector dimension
    pub max_state_dim: usize,
    /// Maximum edges per node (degree limit)
    pub max_node_degree: usize,
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            max_nodes: DEFAULT_MAX_NODES,
            max_edges: DEFAULT_MAX_EDGES,
            max_state_dim: DEFAULT_MAX_STATE_DIM,
            max_node_degree: 10_000,
        }
    }
}

impl GraphLimits {
    /// Create limits for a small graph (testing/development)
    #[must_use]
    pub fn small() -> Self {
        Self {
            max_nodes: 10_000,
            max_edges: 100_000,
            max_state_dim: 1024,
            max_node_degree: 1000,
        }
    }

    /// Create limits for a large graph (production)
    #[must_use]
    pub fn large() -> Self {
        Self {
            max_nodes: 10_000_000,
            max_edges: 100_000_000,
            max_state_dim: 65536,
            max_node_degree: 100_000,
        }
    }

    /// Check if adding a node would exceed limits
    #[must_use]
    pub fn can_add_node(&self, current_count: usize) -> bool {
        current_count < self.max_nodes
    }

    /// Check if adding an edge would exceed limits
    #[must_use]
    pub fn can_add_edge(&self, current_count: usize) -> bool {
        current_count < self.max_edges
    }

    /// Check if a state dimension is within limits
    #[must_use]
    pub fn is_valid_state_dim(&self, dim: usize) -> bool {
        dim <= self.max_state_dim
    }
}

/// Resource limits for computation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum matrix dimension for restriction maps
    pub max_matrix_dim: usize,
    /// Maximum payload size in bytes
    pub max_payload_size: usize,
    /// Maximum concurrent operations
    pub max_concurrent_ops: usize,
    /// Maximum recursion depth for graph traversal
    pub max_recursion_depth: usize,
    /// Timeout for single operation in milliseconds
    pub operation_timeout_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_matrix_dim: DEFAULT_MAX_MATRIX_DIM,
            max_payload_size: DEFAULT_MAX_PAYLOAD_SIZE,
            max_concurrent_ops: DEFAULT_MAX_CONCURRENT_OPS,
            max_recursion_depth: 1000,
            operation_timeout_ms: 30_000,
        }
    }
}

impl ResourceLimits {
    /// Check if a matrix dimension is within limits
    #[must_use]
    pub fn is_valid_matrix_dim(&self, dim: usize) -> bool {
        dim <= self.max_matrix_dim
    }

    /// Check if payload size is within limits
    #[must_use]
    pub fn is_valid_payload_size(&self, size: usize) -> bool {
        size <= self.max_payload_size
    }

    /// Calculate maximum allowed matrix size (elements)
    #[must_use]
    pub fn max_matrix_elements(&self) -> usize {
        self.max_matrix_dim.saturating_mul(self.max_matrix_dim)
    }
}

/// Combined security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Graph size limits
    pub graph_limits: GraphLimits,
    /// Resource limits
    pub resource_limits: ResourceLimits,
    /// Maximum node ID length
    pub max_node_id_len: usize,
    /// Whether to enforce strict validation
    pub strict_mode: bool,
    /// Whether to log security events
    pub log_security_events: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            graph_limits: GraphLimits::default(),
            resource_limits: ResourceLimits::default(),
            max_node_id_len: DEFAULT_MAX_NODE_ID_LEN,
            strict_mode: true,
            log_security_events: true,
        }
    }
}

impl SecurityConfig {
    /// Create a permissive configuration (use with caution)
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            graph_limits: GraphLimits::large(),
            resource_limits: ResourceLimits::default(),
            max_node_id_len: 1024,
            strict_mode: false,
            log_security_events: false,
        }
    }

    /// Create a strict configuration (recommended for production)
    #[must_use]
    pub fn strict() -> Self {
        Self {
            graph_limits: GraphLimits::default(),
            resource_limits: ResourceLimits::default(),
            max_node_id_len: DEFAULT_MAX_NODE_ID_LEN,
            strict_mode: true,
            log_security_events: true,
        }
    }

    /// Create configuration for testing
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            graph_limits: GraphLimits::small(),
            resource_limits: ResourceLimits::default(),
            max_node_id_len: 256,
            strict_mode: true,
            log_security_events: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_limits_default() {
        let limits = GraphLimits::default();
        assert_eq!(limits.max_nodes, DEFAULT_MAX_NODES);
        assert_eq!(limits.max_edges, DEFAULT_MAX_EDGES);
    }

    #[test]
    fn test_can_add_node() {
        let limits = GraphLimits {
            max_nodes: 100,
            ..Default::default()
        };
        assert!(limits.can_add_node(50));
        assert!(limits.can_add_node(99));
        assert!(!limits.can_add_node(100));
        assert!(!limits.can_add_node(150));
    }

    #[test]
    fn test_valid_state_dim() {
        let limits = GraphLimits::default();
        assert!(limits.is_valid_state_dim(1024));
        assert!(limits.is_valid_state_dim(DEFAULT_MAX_STATE_DIM));
        assert!(!limits.is_valid_state_dim(DEFAULT_MAX_STATE_DIM + 1));
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits::default();
        assert!(limits.is_valid_matrix_dim(1024));
        assert!(!limits.is_valid_matrix_dim(1_000_000));
        assert!(limits.is_valid_payload_size(1024));
        assert!(!limits.is_valid_payload_size(100 * 1024 * 1024));
    }

    #[test]
    fn test_security_config_presets() {
        let strict = SecurityConfig::strict();
        assert!(strict.strict_mode);
        assert!(strict.log_security_events);

        let permissive = SecurityConfig::permissive();
        assert!(!permissive.strict_mode);
        assert!(!permissive.log_security_events);
    }
}
