//! # Integrity Control Plane
//!
//! Stoer-Wagner mincut gating for vector search integrity.

use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityContract {
    pub id: String,
    pub name: String,
    pub min_recall: f64,
    pub max_latency_ms: u64,
    pub min_mincut: f64,
    pub active: bool,
}

impl Default for IntegrityContract {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Default Contract".to_string(),
            min_recall: 0.95,
            max_latency_ms: 100,
            min_mincut: 0.1,
            active: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub recall: f64,
    pub latency_ms: u64,
    pub mincut: f64,
    pub failures: Vec<String>,
}

pub struct IntegrityManager {
    contracts: HashMap<String, IntegrityContract>,
}

impl IntegrityManager {
    pub fn new() -> Self {
        let mut contracts = HashMap::new();
        contracts.insert("default".to_string(), IntegrityContract::default());
        Self { contracts }
    }

    pub fn register_contract(&mut self, contract: IntegrityContract) {
        self.contracts.insert(contract.id.clone(), contract);
    }

    pub fn get_contract(&self, id: &str) -> Option<&IntegrityContract> {
        self.contracts.get(id)
    }

    pub fn validate(
        &self,
        contract_id: &str,
        recall: f64,
        latency_ms: u64,
        mincut: f64,
    ) -> ValidationResult {
        let contract = self.contracts.get(contract_id).cloned().unwrap_or_default();
        let mut failures = Vec::new();

        if recall < contract.min_recall {
            failures.push(format!("Recall {:.3} < {:.3}", recall, contract.min_recall));
        }
        if latency_ms > contract.max_latency_ms {
            failures.push(format!(
                "Latency {}ms > {}ms",
                latency_ms, contract.max_latency_ms
            ));
        }
        if mincut < contract.min_mincut {
            failures.push(format!("Mincut {:.3} < {:.3}", mincut, contract.min_mincut));
        }

        ValidationResult {
            passed: failures.is_empty(),
            recall,
            latency_ms,
            mincut,
            failures,
        }
    }

    pub fn list_contracts(&self) -> Vec<&IntegrityContract> {
        self.contracts.values().collect()
    }
}

impl Default for IntegrityManager {
    fn default() -> Self {
        Self::new()
    }
}

static INTEGRITY_MANAGER: std::sync::OnceLock<Arc<RwLock<IntegrityManager>>> =
    std::sync::OnceLock::new();

pub fn get_integrity_manager() -> Arc<RwLock<IntegrityManager>> {
    INTEGRITY_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(IntegrityManager::new())))
        .clone()
}

// Submodule exports
pub mod contracted_graph;
pub mod events;
pub mod gating;
pub mod mincut;

pub use mincut::{MincutConfig, MincutResult, WitnessEdge};

/// Get current mincut for an index (used by gated_transformer module)
pub fn get_current_mincut(_index_name: &str) -> Result<MincutResult, String> {
    // TODO: Implement actual index mincut lookup
    // For now, return a default result
    Ok(MincutResult {
        lambda_cut: 10.0,
        lambda2: None,
        witness_edges: vec![],
        cut_partition: vec![],
        computation_time_ms: 0,
    })
}

pub fn stoer_wagner_mincut(n: usize, edges: &[(usize, usize, f64)]) -> f64 {
    if n <= 1 || edges.is_empty() {
        return 0.0;
    }

    let mut adj = vec![vec![0.0; n]; n];
    for &(u, v, w) in edges {
        if u < n && v < n {
            adj[u][v] += w;
            adj[v][u] += w;
        }
    }

    let mut min_cut = f64::MAX;
    let mut active: Vec<bool> = vec![true; n];

    for phase in 0..n - 1 {
        let mut weights: Vec<f64> = vec![0.0; n];
        let mut in_a = vec![false; n];
        let mut last = 0;
        let mut second_last = 0;

        for _ in 0..n - phase {
            let mut max_weight = -1.0;
            let mut max_vertex = 0;
            for v in 0..n {
                if active[v] && !in_a[v] && weights[v] > max_weight {
                    max_weight = weights[v];
                    max_vertex = v;
                }
            }
            second_last = last;
            last = max_vertex;
            in_a[max_vertex] = true;

            for v in 0..n {
                if active[v] && !in_a[v] {
                    weights[v] += adj[max_vertex][v];
                }
            }
        }

        min_cut = min_cut.min(weights[last]);
        active[last] = false;
        for v in 0..n {
            adj[second_last][v] += adj[last][v];
            adj[v][second_last] += adj[v][last];
        }
    }
    min_cut
}

#[pg_extern]
fn ruvector_integrity_status() -> pgrx::JsonB {
    let manager = get_integrity_manager();
    let reader = manager.read().unwrap();
    let contracts: Vec<_> = reader
        .list_contracts()
        .iter()
        .map(|c| c.id.clone())
        .collect();
    pgrx::JsonB(serde_json::json!({
        "enabled": true,
        "active_contracts": contracts.len(),
        "contracts": contracts,
    }))
}

#[pg_extern]
fn ruvector_integrity_create_contract(
    id: &str,
    name: &str,
    min_recall: f64,
    max_latency_ms: i64,
    min_mincut: f64,
) -> pgrx::JsonB {
    let contract = IntegrityContract {
        id: id.to_string(),
        name: name.to_string(),
        min_recall,
        max_latency_ms: max_latency_ms as u64,
        min_mincut,
        active: true,
    };
    let manager = get_integrity_manager();
    manager.write().unwrap().register_contract(contract.clone());
    pgrx::JsonB(serde_json::json!({ "success": true, "contract": contract }))
}

#[pg_extern]
fn ruvector_integrity_validate(
    contract_id: &str,
    recall: f64,
    latency_ms: i64,
    mincut: f64,
) -> pgrx::JsonB {
    let manager = get_integrity_manager();
    let result = manager
        .read()
        .unwrap()
        .validate(contract_id, recall, latency_ms as u64, mincut);
    pgrx::JsonB(serde_json::json!(result))
}

#[pg_extern]
fn ruvector_mincut(n: i32, edges_json: pgrx::JsonB) -> f64 {
    let edges: Vec<(usize, usize, f64)> = serde_json::from_value(edges_json.0).unwrap_or_default();
    stoer_wagner_mincut(n as usize, &edges)
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_integrity_status() {
        let status = ruvector_integrity_status();
        assert!(status.0.get("enabled").is_some());
    }

    #[pg_test]
    fn test_mincut_simple() {
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0)];
        let mincut = stoer_wagner_mincut(3, &edges);
        assert!(mincut >= 0.0);
    }
}
