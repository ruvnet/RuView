//! rUv Neural Graph -- Brain connectivity graph construction from neural signals.
//!
//! This crate builds brain connectivity graphs from multi-channel neural time series
//! data, provides graph-theoretic metrics, spectral analysis, and temporal dynamics
//! tracking for brain topology research.
//!
//! # Modules
//!
//! - [`atlas`] -- Brain atlas definitions (Desikan-Killiany 68 regions)
//! - [`constructor`] -- Graph construction from connectivity matrices and time series
//! - [`petgraph_bridge`] -- Convert between `BrainGraph` and petgraph types
//! - [`metrics`] -- Graph-theoretic metrics (efficiency, clustering, centrality)
//! - [`spectral`] -- Spectral graph properties (Laplacian, Fiedler value)
//! - [`dynamics`] -- Temporal graph dynamics and topology tracking

pub mod atlas;
pub mod constructor;
pub mod dynamics;
pub mod metrics;
pub mod petgraph_bridge;
pub mod spectral;

pub use atlas::{load_atlas, AtlasType};
pub use constructor::BrainGraphConstructor;
pub use dynamics::TopologyTracker;
pub use metrics::{
    betweenness_centrality, clustering_coefficient, degree_distribution, global_efficiency,
    graph_density, local_efficiency, modularity, node_degree, small_world_index,
};
pub use petgraph_bridge::{from_petgraph, to_petgraph};
pub use spectral::{fiedler_value, graph_laplacian, normalized_laplacian, spectral_gap};
