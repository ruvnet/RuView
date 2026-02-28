//! Configuration for rvlite collections.
//!
//! Provides [`RvliteConfig`] with sensible defaults for lightweight,
//! resource-constrained environments.

use std::path::PathBuf;

use rvf_runtime::options::DistanceMetric;

/// Distance metric for rvlite similarity search.
///
/// Maps directly to the underlying `DistanceMetric` in rvf-runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RvliteMetric {
    /// Squared Euclidean distance.
    L2,
    /// Cosine distance (1 - cosine_similarity).
    #[default]
    Cosine,
    /// Inner (dot) product distance (negated).
    InnerProduct,
}

impl From<RvliteMetric> for DistanceMetric {
    fn from(m: RvliteMetric) -> Self {
        match m {
            RvliteMetric::L2 => DistanceMetric::L2,
            RvliteMetric::Cosine => DistanceMetric::Cosine,
            RvliteMetric::InnerProduct => DistanceMetric::InnerProduct,
        }
    }
}

/// Configuration for creating a new rvlite collection.
#[derive(Clone, Debug)]
pub struct RvliteConfig {
    /// File path for the RVF file.
    pub path: PathBuf,
    /// Vector dimensionality (required, must be > 0).
    pub dimension: u16,
    /// Distance metric for similarity search.
    pub metric: RvliteMetric,
    /// Optional capacity hint for pre-allocation.
    pub max_elements: Option<usize>,
}

impl RvliteConfig {
    /// Create a new config with the required fields and sensible defaults.
    ///
    /// The metric defaults to `Cosine` and `max_elements` is `None`.
    pub fn new(path: impl Into<PathBuf>, dimension: u16) -> Self {
        Self {
            path: path.into(),
            dimension,
            metric: RvliteMetric::default(),
            max_elements: None,
        }
    }

    /// Set the distance metric.
    pub fn with_metric(mut self, metric: RvliteMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Set the capacity hint.
    pub fn with_max_elements(mut self, max: usize) -> Self {
        self.max_elements = Some(max);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metric_is_cosine() {
        assert_eq!(RvliteMetric::default(), RvliteMetric::Cosine);
    }

    #[test]
    fn config_new_defaults() {
        let cfg = RvliteConfig::new("/tmp/test.rvf", 128);
        assert_eq!(cfg.dimension, 128);
        assert_eq!(cfg.metric, RvliteMetric::Cosine);
        assert!(cfg.max_elements.is_none());
    }

    #[test]
    fn config_builder_methods() {
        let cfg = RvliteConfig::new("/tmp/test.rvf", 64)
            .with_metric(RvliteMetric::L2)
            .with_max_elements(1000);
        assert_eq!(cfg.metric, RvliteMetric::L2);
        assert_eq!(cfg.max_elements, Some(1000));
    }

    #[test]
    fn metric_conversion() {
        assert_eq!(DistanceMetric::from(RvliteMetric::L2), DistanceMetric::L2);
        assert_eq!(
            DistanceMetric::from(RvliteMetric::Cosine),
            DistanceMetric::Cosine
        );
        assert_eq!(
            DistanceMetric::from(RvliteMetric::InnerProduct),
            DistanceMetric::InnerProduct
        );
    }
}
