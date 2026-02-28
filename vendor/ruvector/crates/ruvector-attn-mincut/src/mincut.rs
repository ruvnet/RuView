use crate::graph::AttentionGraph;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Result of a single s-t min-cut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutResult {
    pub cut_edges: Vec<(usize, usize)>,
    pub cut_cost: f32,
    pub keep_mask: Vec<bool>,
}

/// Aggregated gating decision from `dynamic_min_cut`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatingResult {
    pub keep_mask: Vec<bool>,
    pub cut_cost: f32,
    pub edges_kept: usize,
    pub edges_total: usize,
}

#[derive(Debug, Clone)]
struct FlowEdge {
    to: usize,
    rev: usize,
    cap: f32,
}

/// Dinic's max-flow solver for s-t min-cut on an attention graph.
pub struct DinicSolver {
    adj: Vec<Vec<FlowEdge>>,
    level: Vec<i32>,
    iter: Vec<usize>,
}

impl DinicSolver {
    fn new(n: usize) -> Self {
        Self {
            adj: vec![Vec::new(); n],
            level: vec![0; n],
            iter: vec![0; n],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize, cap: f32) {
        let (rf, rt) = (self.adj[to].len(), self.adj[from].len());
        self.adj[from].push(FlowEdge { to, rev: rf, cap });
        self.adj[to].push(FlowEdge {
            to: from,
            rev: rt,
            cap: 0.0,
        });
    }

    fn bfs(&mut self, s: usize) {
        self.level.fill(-1);
        self.level[s] = 0;
        let mut q = VecDeque::new();
        q.push_back(s);
        while let Some(v) = q.pop_front() {
            for e in &self.adj[v] {
                if e.cap > 0.0 && self.level[e.to] < 0 {
                    self.level[e.to] = self.level[v] + 1;
                    q.push_back(e.to);
                }
            }
        }
    }

    fn dfs(&mut self, v: usize, t: usize, f: f32) -> f32 {
        if v == t {
            return f;
        }
        while self.iter[v] < self.adj[v].len() {
            let i = self.iter[v];
            let (to, cap) = (self.adj[v][i].to, self.adj[v][i].cap);
            if cap > 0.0 && self.level[v] < self.level[to] {
                let d = self.dfs(to, t, f.min(cap));
                if d > 0.0 {
                    self.adj[v][i].cap -= d;
                    let rev = self.adj[v][i].rev;
                    self.adj[to][rev].cap += d;
                    return d;
                }
            }
            self.iter[v] += 1;
        }
        0.0
    }

    /// Compute s-t min-cut on the given attention graph.
    pub fn min_cut(&mut self, graph: &AttentionGraph, s: usize, t: usize) -> CutResult {
        assert!(s < graph.nodes && t < graph.nodes && s != t);
        *self = Self::new(graph.nodes);
        for edge in &graph.edges {
            self.add_edge(edge.src, edge.dst, edge.weight);
        }

        let inf = f32::MAX / 2.0;
        loop {
            self.bfs(s);
            if self.level[t] < 0 {
                break;
            }
            self.iter.fill(0);
            while self.dfs(s, t, inf) > 0.0 {}
        }

        // Final BFS to find S-side of the cut
        self.bfs(s);
        let mut cut_edges = Vec::new();
        let mut cut_cost = 0.0f32;
        let mut keep_mask = vec![true; graph.edges.len()];
        for (idx, e) in graph.edges.iter().enumerate() {
            if self.level[e.src] >= 0 && self.level[e.dst] < 0 {
                cut_edges.push((e.src, e.dst));
                cut_cost += e.weight;
                keep_mask[idx] = false;
            }
        }
        CutResult {
            cut_edges,
            cut_cost,
            keep_mask,
        }
    }
}

/// Compute dynamic min-cut gating over a flattened `seq_len x seq_len` logit matrix.
pub fn dynamic_min_cut(
    logits: &[f32],
    seq_len: usize,
    lambda: f32,
    _tau: usize,
    eps: f32,
) -> GatingResult {
    assert_eq!(logits.len(), seq_len * seq_len);
    let n = seq_len * seq_len;
    let clamped: Vec<f32> = logits
        .iter()
        .map(|&v| if v > eps { v } else { 0.0 })
        .collect();
    let graph = crate::graph::graph_from_logits(&clamped, seq_len);

    if graph.edges.is_empty() || seq_len < 2 {
        return GatingResult {
            keep_mask: vec![false; n],
            cut_cost: 0.0,
            edges_kept: 0,
            edges_total: n,
        };
    }

    let mean_w: f32 = graph.edges.iter().map(|e| e.weight).sum::<f32>() / graph.edges.len() as f32;
    let threshold = lambda * mean_w;
    let mut flat_keep = vec![true; n];
    let mut total_cut_cost = 0.0f32;

    let mut solver = DinicSolver::new(seq_len);
    let result = solver.min_cut(&graph, 0, seq_len - 1);
    if result.cut_cost <= threshold {
        total_cut_cost += result.cut_cost;
        for &(s, d) in &result.cut_edges {
            flat_keep[s * seq_len + d] = false;
        }
    }

    for i in 0..n {
        if clamped[i] <= 0.0 {
            flat_keep[i] = false;
        }
    }
    let edges_kept = flat_keep.iter().filter(|&&k| k).count();
    GatingResult {
        keep_mask: flat_keep,
        cut_cost: total_cut_cost,
        edges_kept,
        edges_total: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;

    #[test]
    fn test_dinic_simple_cut() {
        let graph = AttentionGraph {
            nodes: 4,
            edges: vec![
                Edge {
                    src: 0,
                    dst: 1,
                    weight: 5.0,
                },
                Edge {
                    src: 0,
                    dst: 2,
                    weight: 4.0,
                },
                Edge {
                    src: 1,
                    dst: 3,
                    weight: 3.0,
                },
                Edge {
                    src: 2,
                    dst: 3,
                    weight: 6.0,
                },
                Edge {
                    src: 1,
                    dst: 2,
                    weight: 2.0,
                },
            ],
        };
        let mut solver = DinicSolver::new(4);
        let r = solver.min_cut(&graph, 0, 3);
        assert!((r.cut_cost - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_dinic_two_node() {
        let graph = AttentionGraph {
            nodes: 2,
            edges: vec![Edge {
                src: 0,
                dst: 1,
                weight: 3.5,
            }],
        };
        let mut solver = DinicSolver::new(2);
        let r = solver.min_cut(&graph, 0, 1);
        assert!((r.cut_cost - 3.5).abs() < 0.01);
        assert!(!r.keep_mask[0]);
    }

    #[test]
    fn test_dynamic_basic() {
        let logits = vec![1.0, 0.5, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 1.0];
        let r = dynamic_min_cut(&logits, 3, 0.5, 2, 0.01);
        assert_eq!(r.edges_total, 9);
        assert!(r.edges_kept > 0);
    }

    #[test]
    fn test_dynamic_all_negative() {
        assert_eq!(dynamic_min_cut(&[-1.0; 4], 2, 0.5, 2, 0.01).edges_kept, 0);
    }

    #[test]
    fn test_dynamic_single_token() {
        assert_eq!(dynamic_min_cut(&[1.0], 1, 0.5, 2, 0.01).edges_total, 1);
    }
}
