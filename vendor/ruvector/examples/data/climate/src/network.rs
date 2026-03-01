//! Sensor network graph construction and analysis

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ClimateObservation, WeatherVariable, BoundingBox};

/// A sensor node in the network graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorNode {
    /// Station/sensor ID
    pub id: String,

    /// Station name
    pub name: String,

    /// Location (lat, lon)
    pub location: (f64, f64),

    /// Elevation (meters)
    pub elevation: Option<f64>,

    /// Variables measured
    pub variables: Vec<WeatherVariable>,

    /// Observation count
    pub observation_count: u64,

    /// Quality score (0-1)
    pub quality_score: f64,

    /// First observation
    pub first_observation: Option<DateTime<Utc>>,

    /// Last observation
    pub last_observation: Option<DateTime<Utc>>,
}

/// An edge between sensors in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorEdge {
    /// Source sensor ID
    pub source: String,

    /// Target sensor ID
    pub target: String,

    /// Correlation coefficient
    pub correlation: f64,

    /// Distance (km)
    pub distance_km: f64,

    /// Edge weight (for min-cut)
    pub weight: f64,

    /// Variables used for correlation
    pub variables: Vec<WeatherVariable>,

    /// Observation overlap count
    pub overlap_count: usize,
}

/// A sensor network graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorNetwork {
    /// Network identifier
    pub id: String,

    /// Nodes (sensors)
    pub nodes: HashMap<String, SensorNode>,

    /// Edges (correlations)
    pub edges: Vec<SensorEdge>,

    /// Bounding box
    pub bounding_box: Option<BoundingBox>,

    /// Creation time
    pub created_at: DateTime<Utc>,

    /// Network statistics
    pub stats: NetworkStats,
}

/// Network statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Number of nodes
    pub node_count: usize,

    /// Number of edges
    pub edge_count: usize,

    /// Average correlation
    pub avg_correlation: f64,

    /// Network density
    pub density: f64,

    /// Average degree
    pub avg_degree: f64,

    /// Clustering coefficient
    pub clustering_coefficient: f64,

    /// Min-cut value
    pub min_cut_value: Option<f64>,
}

impl SensorNetwork {
    /// Create an empty network
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            bounding_box: None,
            created_at: Utc::now(),
            stats: NetworkStats::default(),
        }
    }

    /// Add a sensor node
    pub fn add_node(&mut self, node: SensorNode) {
        self.nodes.insert(node.id.clone(), node);
        self.update_stats();
    }

    /// Add an edge
    pub fn add_edge(&mut self, edge: SensorEdge) {
        self.edges.push(edge);
        self.update_stats();
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<&SensorNode> {
        self.nodes.get(id)
    }

    /// Get edges for a node
    pub fn get_edges_for_node(&self, id: &str) -> Vec<&SensorEdge> {
        self.edges
            .iter()
            .filter(|e| e.source == id || e.target == id)
            .collect()
    }

    /// Get neighbors of a node
    pub fn get_neighbors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter_map(|e| {
                if e.source == id {
                    Some(e.target.as_str())
                } else if e.target == id {
                    Some(e.source.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.node_count = self.nodes.len();
        self.stats.edge_count = self.edges.len();

        if !self.edges.is_empty() {
            self.stats.avg_correlation = self.edges.iter().map(|e| e.correlation).sum::<f64>()
                / self.edges.len() as f64;
        }

        let max_edges = if self.nodes.len() > 1 {
            self.nodes.len() * (self.nodes.len() - 1) / 2
        } else {
            1
        };
        self.stats.density = self.edges.len() as f64 / max_edges as f64;

        if !self.nodes.is_empty() {
            self.stats.avg_degree = (2 * self.edges.len()) as f64 / self.nodes.len() as f64;
        }
    }

    /// Convert to format suitable for RuVector min-cut
    pub fn to_mincut_edges(&self) -> Vec<(u64, u64, f64)> {
        let mut node_ids: HashMap<&str, u64> = HashMap::new();
        let mut next_id = 0u64;

        for id in self.nodes.keys() {
            node_ids.insert(id.as_str(), next_id);
            next_id += 1;
        }

        self.edges
            .iter()
            .filter_map(|e| {
                let src_id = node_ids.get(e.source.as_str())?;
                let tgt_id = node_ids.get(e.target.as_str())?;
                Some((*src_id, *tgt_id, e.weight))
            })
            .collect()
    }

    /// Get node ID mapping
    pub fn node_id_mapping(&self) -> HashMap<u64, String> {
        let mut mapping = HashMap::new();
        for (i, id) in self.nodes.keys().enumerate() {
            mapping.insert(i as u64, id.clone());
        }
        mapping
    }
}

/// Builder for sensor networks
pub struct SensorNetworkBuilder {
    id: String,
    observations: Vec<ClimateObservation>,
    correlation_threshold: f64,
    max_distance_km: f64,
    min_overlap: usize,
    variables: Vec<WeatherVariable>,
}

impl SensorNetworkBuilder {
    /// Create a new network builder
    pub fn new() -> Self {
        Self {
            id: format!("network_{}", Utc::now().timestamp()),
            observations: Vec::new(),
            correlation_threshold: 0.5,
            max_distance_km: 500.0,
            min_overlap: 30,
            variables: vec![WeatherVariable::Temperature],
        }
    }

    /// Set network ID
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Add observations
    pub fn add_observations(mut self, observations: Vec<ClimateObservation>) -> Self {
        self.observations.extend(observations);
        self
    }

    /// Set correlation threshold
    pub fn correlation_threshold(mut self, threshold: f64) -> Self {
        self.correlation_threshold = threshold;
        self
    }

    /// Set maximum distance
    pub fn max_distance_km(mut self, distance: f64) -> Self {
        self.max_distance_km = distance;
        self
    }

    /// Set minimum overlap
    pub fn min_overlap(mut self, min: usize) -> Self {
        self.min_overlap = min;
        self
    }

    /// Set variables to use
    pub fn variables(mut self, vars: Vec<WeatherVariable>) -> Self {
        self.variables = vars;
        self
    }

    /// Build the network
    pub fn build(self) -> SensorNetwork {
        let mut network = SensorNetwork::new(&self.id);

        // Group observations by station
        let mut station_obs: HashMap<String, Vec<&ClimateObservation>> = HashMap::new();
        for obs in &self.observations {
            station_obs.entry(obs.station_id.clone()).or_default().push(obs);
        }

        // Create nodes
        for (station_id, observations) in &station_obs {
            let first_obs = observations.iter().min_by_key(|o| o.timestamp);
            let last_obs = observations.iter().max_by_key(|o| o.timestamp);

            let location = first_obs.map(|o| o.location).unwrap_or((0.0, 0.0));
            let variables: Vec<_> = observations.iter().map(|o| o.variable).collect::<std::collections::HashSet<_>>().into_iter().collect();

            let node = SensorNode {
                id: station_id.clone(),
                name: station_id.clone(),
                location,
                elevation: None,
                variables,
                observation_count: observations.len() as u64,
                quality_score: self.compute_quality_score(observations),
                first_observation: first_obs.map(|o| o.timestamp),
                last_observation: last_obs.map(|o| o.timestamp),
            };

            network.add_node(node);
        }

        // Create edges based on correlation
        let station_ids: Vec<_> = station_obs.keys().cloned().collect();

        for i in 0..station_ids.len() {
            for j in (i + 1)..station_ids.len() {
                let id_i = &station_ids[i];
                let id_j = &station_ids[j];

                let obs_i = &station_obs[id_i];
                let obs_j = &station_obs[id_j];

                // Check distance
                let loc_i = obs_i.first().map(|o| o.location).unwrap_or((0.0, 0.0));
                let loc_j = obs_j.first().map(|o| o.location).unwrap_or((0.0, 0.0));
                let distance = haversine_distance(loc_i.0, loc_i.1, loc_j.0, loc_j.1);

                if distance > self.max_distance_km {
                    continue;
                }

                // Compute correlation
                let (correlation, overlap) = self.compute_correlation(obs_i, obs_j);

                if correlation.abs() >= self.correlation_threshold && overlap >= self.min_overlap {
                    let edge = SensorEdge {
                        source: id_i.clone(),
                        target: id_j.clone(),
                        correlation,
                        distance_km: distance,
                        weight: correlation.abs(), // Use abs correlation as weight
                        variables: self.variables.clone(),
                        overlap_count: overlap,
                    };

                    network.add_edge(edge);
                }
            }
        }

        network
    }

    /// Compute quality score for a station
    fn compute_quality_score(&self, observations: &[&ClimateObservation]) -> f64 {
        if observations.is_empty() {
            return 0.0;
        }

        let good_count = observations
            .iter()
            .filter(|o| o.quality == crate::QualityFlag::Good)
            .count();

        good_count as f64 / observations.len() as f64
    }

    /// Compute correlation between two stations
    fn compute_correlation(&self, obs_a: &[&ClimateObservation], obs_b: &[&ClimateObservation]) -> (f64, usize) {
        // Build time-aligned series
        let mut map_a: HashMap<i64, f64> = HashMap::new();
        let mut map_b: HashMap<i64, f64> = HashMap::new();

        for obs in obs_a {
            if self.variables.contains(&obs.variable) {
                // Round to daily
                let day = obs.timestamp.timestamp() / 86400;
                map_a.insert(day, obs.value);
            }
        }

        for obs in obs_b {
            if self.variables.contains(&obs.variable) {
                let day = obs.timestamp.timestamp() / 86400;
                map_b.insert(day, obs.value);
            }
        }

        // Find overlapping days
        let mut vals_a = Vec::new();
        let mut vals_b = Vec::new();

        for (day, val_a) in &map_a {
            if let Some(&val_b) = map_b.get(day) {
                vals_a.push(*val_a);
                vals_b.push(val_b);
            }
        }

        let overlap = vals_a.len();
        if overlap < 3 {
            return (0.0, overlap);
        }

        // Pearson correlation
        let mean_a = vals_a.iter().sum::<f64>() / overlap as f64;
        let mean_b = vals_b.iter().sum::<f64>() / overlap as f64;

        let mut cov = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;

        for i in 0..overlap {
            let da = vals_a[i] - mean_a;
            let db = vals_b[i] - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        let correlation = if var_a * var_b > 0.0 {
            cov / (var_a.sqrt() * var_b.sqrt())
        } else {
            0.0
        };

        (correlation, overlap)
    }
}

impl Default for SensorNetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Haversine distance between two points (km)
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0; // Earth radius in km

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    R * c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haversine_distance() {
        // NYC to LA approximately 3940 km
        let dist = haversine_distance(40.7128, -74.0060, 34.0522, -118.2437);
        assert!((dist - 3940.0).abs() < 100.0);
    }

    #[test]
    fn test_empty_network() {
        let network = SensorNetwork::new("test");
        assert_eq!(network.stats.node_count, 0);
        assert_eq!(network.stats.edge_count, 0);
    }

    #[test]
    fn test_network_builder() {
        let builder = SensorNetworkBuilder::new()
            .correlation_threshold(0.7)
            .max_distance_km(100.0);

        let network = builder.build();
        assert!(network.nodes.is_empty());
    }
}
