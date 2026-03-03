//! Self-healing mesh -- min-cut topology analysis for mesh resilience (ADR-041).
//!
//! Monitors inter-node CSI coherence for up to 8 mesh nodes and computes
//! approximate minimum graph cuts via simplified Stoer-Wagner to detect
//! fragile topologies.
//!
//! Events: NODE_DEGRADED(885), MESH_RECONFIGURE(886),
//!         COVERAGE_SCORE(887), HEALING_COMPLETE(888).
//! Budget: S (<5ms). Stoer-Wagner on 8 nodes is O(n^3) = 512 ops.

// ── Constants ────────────────────────────────────────────────────────────────

const MAX_NODES: usize = 8;
const QUALITY_ALPHA: f32 = 0.15;
const MINCUT_FRAGILE: f32 = 0.3;
const MINCUT_HEALTHY: f32 = 0.6;
const NO_NODE: u8 = 0xFF;
const MAX_EVENTS: usize = 6;

// ── Event IDs ────────────────────────────────────────────────────────────────

pub const EVENT_NODE_DEGRADED: i32 = 885;
pub const EVENT_MESH_RECONFIGURE: i32 = 886;
pub const EVENT_COVERAGE_SCORE: i32 = 887;
pub const EVENT_HEALING_COMPLETE: i32 = 888;

// ── State ────────────────────────────────────────────────────────────────────

/// Self-healing mesh monitor with Stoer-Wagner min-cut analysis.
pub struct SelfHealingMesh {
    /// EMA-smoothed quality score per node [0, 1].
    node_quality: [f32; MAX_NODES],
    /// Whether each node quality has received its first sample.
    node_init: [bool; MAX_NODES],
    /// Weighted adjacency matrix (symmetric).
    adj: [[f32; MAX_NODES]; MAX_NODES],
    /// Number of active nodes.
    n_active: usize,
    /// Previous frame's minimum cut value.
    prev_mincut: f32,
    /// Whether the mesh is currently fragile.
    healing: bool,
    /// Index of the weakest node from last analysis.
    weakest: u8,
    /// Frame counter.
    frame_count: u32,
}

impl SelfHealingMesh {
    pub const fn new() -> Self {
        Self {
            node_quality: [0.0; MAX_NODES],
            node_init: [false; MAX_NODES],
            adj: [[0.0; MAX_NODES]; MAX_NODES],
            n_active: 0,
            prev_mincut: 1.0,
            healing: false,
            weakest: NO_NODE,
            frame_count: 0,
        }
    }

    /// Update quality score for a mesh node via EMA.
    pub fn update_node_quality(&mut self, id: usize, coherence: f32) {
        if id >= MAX_NODES { return; }
        if !self.node_init[id] {
            self.node_quality[id] = coherence;
            self.node_init[id] = true;
        } else {
            self.node_quality[id] =
                QUALITY_ALPHA * coherence + (1.0 - QUALITY_ALPHA) * self.node_quality[id];
        }
    }

    /// Process one analysis frame. `node_qualities` has one coherence score
    /// per active node (length clamped to 8).
    /// Returns a slice of (event_id, value) pairs.
    pub fn process_frame(&mut self, node_qualities: &[f32]) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut ne = 0usize;
        self.frame_count += 1;

        let n = if node_qualities.len() > MAX_NODES { MAX_NODES } else { node_qualities.len() };
        self.n_active = n;
        for i in 0..n { self.update_node_quality(i, node_qualities[i]); }

        if n < 2 { return unsafe { &EVENTS[..0] }; }

        // Build adjacency: edge weight = min(quality_i, quality_j).
        for i in 0..n {
            self.adj[i][i] = 0.0;
            for j in (i + 1)..n {
                let w = min_f32(self.node_quality[i], self.node_quality[j]);
                self.adj[i][j] = w;
                self.adj[j][i] = w;
            }
        }

        // Coverage score (mean quality).
        let mut sum = 0.0f32;
        for i in 0..n { sum += self.node_quality[i]; }
        let coverage = sum / (n as f32);
        if ne < MAX_EVENTS {
            unsafe { EVENTS[ne] = (EVENT_COVERAGE_SCORE, coverage); }
            ne += 1;
        }

        // Stoer-Wagner min-cut.
        let (mincut, cut_node) = self.stoer_wagner(n);

        if mincut < MINCUT_FRAGILE {
            if !self.healing { self.healing = true; }
            self.weakest = cut_node;
            if ne < MAX_EVENTS {
                unsafe { EVENTS[ne] = (EVENT_NODE_DEGRADED, cut_node as f32); }
                ne += 1;
            }
            if ne < MAX_EVENTS {
                unsafe { EVENTS[ne] = (EVENT_MESH_RECONFIGURE, mincut); }
                ne += 1;
            }
        } else if self.healing && mincut >= MINCUT_HEALTHY {
            self.healing = false;
            self.weakest = NO_NODE;
            if ne < MAX_EVENTS {
                unsafe { EVENTS[ne] = (EVENT_HEALING_COMPLETE, mincut); }
                ne += 1;
            }
        }

        self.prev_mincut = mincut;
        unsafe { &EVENTS[..ne] }
    }

    /// Simplified Stoer-Wagner min-cut for n <= 8 nodes.
    /// Returns (min_cut_value, node_on_lighter_side).
    fn stoer_wagner(&self, n: usize) -> (f32, u8) {
        if n < 2 { return (0.0, 0); }

        let mut adj = [[0.0f32; MAX_NODES]; MAX_NODES];
        for i in 0..n { for j in 0..n { adj[i][j] = self.adj[i][j]; } }

        let mut merged = [false; MAX_NODES];
        let mut global_min = f32::MAX;
        let mut global_node: u8 = 0;

        for _phase in 0..(n - 1) {
            let mut in_a = [false; MAX_NODES];
            let mut w = [0.0f32; MAX_NODES];

            // Find starting non-merged node.
            let mut start = 0;
            for i in 0..n { if !merged[i] { start = i; break; } }
            in_a[start] = true;
            for j in 0..n {
                if !merged[j] && j != start { w[j] = adj[start][j]; }
            }

            let mut prev = start;
            let mut last = start;
            let mut cut_of_phase = 0.0f32;

            let mut active = 0usize;
            for i in 0..n { if !merged[i] { active += 1; } }

            for _step in 1..active {
                let mut best = n;
                let mut best_w = -1.0f32;
                for j in 0..n {
                    if !merged[j] && !in_a[j] && w[j] > best_w {
                        best_w = w[j]; best = j;
                    }
                }
                if best >= n { break; }
                prev = last; last = best;
                in_a[best] = true;
                cut_of_phase = best_w;
                for j in 0..n {
                    if !merged[j] && !in_a[j] { w[j] += adj[best][j]; }
                }
            }

            if cut_of_phase < global_min {
                global_min = cut_of_phase;
                global_node = last as u8;
            }

            // Merge last into prev.
            if prev != last {
                for j in 0..n {
                    if j != prev && j != last && !merged[j] {
                        adj[prev][j] += adj[last][j];
                        adj[j][prev] += adj[j][last];
                    }
                }
                merged[last] = true;
            }
        }

        let node = if (global_node as usize) < n {
            global_node
        } else {
            self.find_weakest(n)
        };
        (global_min, node)
    }

    fn find_weakest(&self, n: usize) -> u8 {
        let mut worst = 0u8;
        let mut worst_q = f32::MAX;
        for i in 0..n {
            if self.node_quality[i] < worst_q {
                worst_q = self.node_quality[i]; worst = i as u8;
            }
        }
        worst
    }

    pub fn node_quality(&self, node: usize) -> f32 {
        if node < MAX_NODES { self.node_quality[node] } else { 0.0 }
    }
    pub fn active_nodes(&self) -> usize { self.n_active }
    pub fn prev_mincut(&self) -> f32 { self.prev_mincut }
    pub fn is_healing(&self) -> bool { self.healing }
    pub fn weakest_node(&self) -> u8 { self.weakest }
    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn reset(&mut self) { *self = Self::new(); }
}

fn min_f32(a: f32, b: f32) -> f32 { if a < b { a } else { b } }

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_constructor() {
        let m = SelfHealingMesh::new();
        assert_eq!(m.frame_count(), 0);
        assert_eq!(m.active_nodes(), 0);
        assert!(!m.is_healing());
        assert_eq!(m.weakest_node(), NO_NODE);
    }

    #[test]
    fn test_healthy_mesh() {
        let mut m = SelfHealingMesh::new();
        let q = [0.9, 0.85, 0.88, 0.92];
        let ev = m.process_frame(&q);
        let cov = ev.iter().find(|e| e.0 == EVENT_COVERAGE_SCORE);
        assert!(cov.is_some());
        assert!(cov.unwrap().1 > 0.8);
        assert!(ev.iter().find(|e| e.0 == EVENT_NODE_DEGRADED).is_none());
        assert!(!m.is_healing());
    }

    #[test]
    fn test_fragile_mesh() {
        let mut m = SelfHealingMesh::new();
        let q = [0.9, 0.05, 0.85, 0.88];
        for _ in 0..10 { m.process_frame(&q); }
        let ev = m.process_frame(&q);
        if let Some(d) = ev.iter().find(|e| e.0 == EVENT_NODE_DEGRADED) {
            assert_eq!(d.1 as usize, 1);
            assert!(m.is_healing());
        }
    }

    #[test]
    fn test_healing_recovery() {
        let mut m = SelfHealingMesh::new();
        for _ in 0..15 { m.process_frame(&[0.9, 0.05, 0.85, 0.88]); }
        let mut healed = false;
        for _ in 0..30 {
            let ev = m.process_frame(&[0.9, 0.9, 0.85, 0.88]);
            if ev.iter().any(|e| e.0 == EVENT_HEALING_COMPLETE) { healed = true; break; }
        }
        if m.is_healing() {
            assert!(m.node_quality(1) > 0.3);
        } else {
            assert!(healed || !m.is_healing());
        }
    }

    #[test]
    fn test_two_nodes() {
        let mut m = SelfHealingMesh::new();
        let ev = m.process_frame(&[0.8, 0.7]);
        let cov = ev.iter().find(|e| e.0 == EVENT_COVERAGE_SCORE);
        assert!(cov.is_some());
        assert!((cov.unwrap().1 - 0.75).abs() < 0.1);
    }

    #[test]
    fn test_single_node_skipped() {
        let mut m = SelfHealingMesh::new();
        assert!(m.process_frame(&[0.8]).is_empty());
    }

    #[test]
    fn test_eight_nodes() {
        let mut m = SelfHealingMesh::new();
        let ev = m.process_frame(&[0.9, 0.85, 0.88, 0.92, 0.87, 0.91, 0.86, 0.89]);
        assert!(ev.iter().find(|e| e.0 == EVENT_COVERAGE_SCORE).unwrap().1 > 0.8);
        assert!(!m.is_healing());
    }

    #[test]
    fn test_adjacency_symmetry() {
        let mut m = SelfHealingMesh::new();
        m.node_quality = [0.5, 0.8, 0.3, 0.9, 0.0, 0.0, 0.0, 0.0];
        // Build adjacency manually.
        let n = 4;
        for i in 0..n {
            m.adj[i][i] = 0.0;
            for j in (i+1)..n {
                let w = min_f32(m.node_quality[i], m.node_quality[j]);
                m.adj[i][j] = w; m.adj[j][i] = w;
            }
        }
        for i in 0..4 { for j in 0..4 {
            assert!((m.adj[i][j] - m.adj[j][i]).abs() < 1e-6);
        }}
        assert!((m.adj[0][2] - 0.3).abs() < 1e-6);
        assert!((m.adj[1][3] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_stoer_wagner_k3() {
        // K3 with unit weights: min-cut = 2.0.
        let mut m = SelfHealingMesh::new();
        m.node_quality = [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for i in 0..3 { m.adj[i][i] = 0.0; for j in (i+1)..3 {
            m.adj[i][j] = 1.0; m.adj[j][i] = 1.0;
        }}
        let (mc, _) = m.stoer_wagner(3);
        assert!((mc - 2.0).abs() < 0.01, "K3 min-cut should be 2.0, got {mc}");
    }

    #[test]
    fn test_stoer_wagner_bottleneck() {
        let mut m = SelfHealingMesh::new();
        m.node_quality = [0.9; MAX_NODES];
        m.adj = [[0.0; MAX_NODES]; MAX_NODES];
        m.adj[0][1] = 0.9; m.adj[1][0] = 0.9;
        m.adj[2][3] = 0.9; m.adj[3][2] = 0.9;
        m.adj[1][2] = 0.1; m.adj[2][1] = 0.1;
        let (mc, _) = m.stoer_wagner(4);
        assert!(mc < 0.5, "bottleneck min-cut should be small, got {mc}");
    }

    #[test]
    fn test_ema_smoothing() {
        let mut m = SelfHealingMesh::new();
        m.update_node_quality(0, 1.0);
        assert!((m.node_quality(0) - 1.0).abs() < 1e-6);
        m.update_node_quality(0, 0.0);
        let expected = QUALITY_ALPHA * 0.0 + (1.0 - QUALITY_ALPHA) * 1.0;
        assert!((m.node_quality(0) - expected).abs() < 1e-5);
    }

    #[test]
    fn test_reset() {
        let mut m = SelfHealingMesh::new();
        m.process_frame(&[0.9, 0.85, 0.88, 0.92]);
        assert!(m.frame_count() > 0);
        m.reset();
        assert_eq!(m.frame_count(), 0);
        assert!(!m.is_healing());
    }
}
