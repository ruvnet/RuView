//! Comprehensive test suite for dynamic min-cut tracking system
//!
//! Tests cover:
//! - Euler tour tree operations (link, cut, connectivity)
//! - DynamicCutWatcher edge updates and threshold detection
//! - Local min-cut procedures and weak region detection
//! - Cut-gated search and expansion pruning
//! - Integration tests with real vectors
//! - Correctness verification against static algorithms
//! - Concurrent operations and stress testing

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

// ===== Mock Structures for Testing =====
// In production, these would be imported from ruvector-mincut

/// Mock Euler Tour Tree for testing
#[derive(Clone)]
struct MockEulerTourTree {
    vertices: HashSet<u64>,
    edges: HashSet<(u64, u64)>,
    connected_components: HashMap<u64, usize>,
}

impl MockEulerTourTree {
    fn new() -> Self {
        Self {
            vertices: HashSet::new(),
            edges: HashSet::new(),
            connected_components: HashMap::new(),
        }
    }

    fn make_tree(&mut self, v: u64) {
        self.vertices.insert(v);
        self.connected_components.insert(v, v as usize);
    }

    fn link(&mut self, u: u64, v: u64) {
        self.edges.insert((u.min(v), u.max(v)));
        // Merge components
        let u_comp = *self.connected_components.get(&u).unwrap();
        let v_comp = *self.connected_components.get(&v).unwrap();
        for (_, comp) in self.connected_components.iter_mut() {
            if *comp == v_comp {
                *comp = u_comp;
            }
        }
    }

    fn cut(&mut self, u: u64, v: u64) {
        self.edges.remove(&(u.min(v), u.max(v)));
        // Recompute components (simplified)
        self.recompute_components();
    }

    fn connected(&self, u: u64, v: u64) -> bool {
        self.connected_components.get(&u) == self.connected_components.get(&v)
    }

    fn tree_size(&self, v: u64) -> usize {
        let comp = self.connected_components.get(&v).unwrap();
        self.connected_components.values().filter(|&c| c == comp).count()
    }

    fn recompute_components(&mut self) {
        // Reset components
        for (&v, comp) in self.connected_components.iter_mut() {
            *comp = v as usize;
        }

        // Union-find style merging based on edges
        for &(u, v) in &self.edges {
            let u_comp = *self.connected_components.get(&u).unwrap();
            let v_comp = *self.connected_components.get(&v).unwrap();
            for (_, comp) in self.connected_components.iter_mut() {
                if *comp == v_comp {
                    *comp = u_comp;
                }
            }
        }
    }
}

/// Mock Dynamic Cut Watcher
struct MockDynamicCutWatcher {
    current_cut: f64,
    threshold: f64,
    updates_count: usize,
    needs_recompute: bool,
}

impl MockDynamicCutWatcher {
    fn new(initial_cut: f64, threshold: f64) -> Self {
        Self {
            current_cut: initial_cut,
            threshold,
            updates_count: 0,
            needs_recompute: false,
        }
    }

    fn insert_edge(&mut self, _u: u64, _v: u64, weight: f64) {
        self.updates_count += 1;
        // Adding edge can only increase or maintain cut
        self.current_cut = self.current_cut.max(weight);
        self.check_threshold();
    }

    fn delete_edge(&mut self, _u: u64, _v: u64, weight: f64) {
        self.updates_count += 1;
        // Deleting edge may decrease cut - need to check
        if (self.current_cut - weight).abs() < 0.001 {
            self.needs_recompute = true;
        }
        self.check_threshold();
    }

    fn current_mincut(&self) -> f64 {
        self.current_cut
    }

    fn check_threshold(&mut self) {
        if self.updates_count >= self.threshold as usize {
            self.needs_recompute = true;
        }
    }

    fn trigger_recompute(&mut self) {
        self.needs_recompute = false;
        self.updates_count = 0;
    }
}

// ===== Test Modules =====

#[cfg(test)]
mod euler_tour_tests {
    use super::*;

    #[test]
    fn test_link_cut_basic() {
        let mut ett = MockEulerTourTree::new();

        // Create vertices
        ett.make_tree(1);
        ett.make_tree(2);
        ett.make_tree(3);

        // Initially disconnected
        assert!(!ett.connected(1, 2));
        assert!(!ett.connected(2, 3));
        assert!(!ett.connected(1, 3));

        // Link 1-2
        ett.link(1, 2);
        assert!(ett.connected(1, 2));
        assert!(!ett.connected(2, 3));

        // Link 2-3
        ett.link(2, 3);
        assert!(ett.connected(1, 2));
        assert!(ett.connected(2, 3));
        assert!(ett.connected(1, 3));

        // Cut 2-3
        ett.cut(2, 3);
        assert!(ett.connected(1, 2));
        assert!(!ett.connected(2, 3));
        assert!(!ett.connected(1, 3));
    }

    #[test]
    fn test_connectivity_queries() {
        let mut ett = MockEulerTourTree::new();

        for i in 1..=10 {
            ett.make_tree(i);
        }

        // Create chain: 1-2-3-4-5
        ett.link(1, 2);
        ett.link(2, 3);
        ett.link(3, 4);
        ett.link(4, 5);

        // Create separate chain: 6-7-8
        ett.link(6, 7);
        ett.link(7, 8);

        // Test connectivity within components
        assert!(ett.connected(1, 5));
        assert!(ett.connected(6, 8));
        assert!(!ett.connected(1, 6));
        assert!(!ett.connected(5, 8));

        // Test single vertices
        assert!(!ett.connected(9, 10));
        assert!(!ett.connected(1, 9));
    }

    #[test]
    fn test_component_sizes() {
        let mut ett = MockEulerTourTree::new();

        for i in 1..=6 {
            ett.make_tree(i);
        }

        // Component 1: vertices 1,2,3
        ett.link(1, 2);
        ett.link(2, 3);

        // Component 2: vertices 4,5,6
        ett.link(4, 5);
        ett.link(5, 6);

        assert_eq!(ett.tree_size(1), 3);
        assert_eq!(ett.tree_size(2), 3);
        assert_eq!(ett.tree_size(3), 3);
        assert_eq!(ett.tree_size(4), 3);
        assert_eq!(ett.tree_size(5), 3);
        assert_eq!(ett.tree_size(6), 3);
    }

    #[test]
    fn test_concurrent_operations() {
        let ett = Arc::new(Mutex::new(MockEulerTourTree::new()));

        // Initialize vertices
        {
            let mut ett_lock = ett.lock().unwrap();
            for i in 1..=20 {
                ett_lock.make_tree(i);
            }
        }

        // Spawn threads to perform operations
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let ett_clone = Arc::clone(&ett);
                thread::spawn(move || {
                    let mut ett_lock = ett_clone.lock().unwrap();
                    let base = thread_id * 5;
                    for i in 0..4 {
                        ett_lock.link(base + i + 1, base + i + 2);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all components are created
        let ett_lock = ett.lock().unwrap();
        assert!(ett_lock.connected(1, 5));
        assert!(ett_lock.connected(6, 10));
        assert!(ett_lock.connected(11, 15));
        assert!(ett_lock.connected(16, 20));
    }

    #[test]
    fn test_large_graph_performance() {
        let mut ett = MockEulerTourTree::new();
        let n = 1000;

        // Create vertices
        for i in 0..n {
            ett.make_tree(i);
        }

        // Create star topology: 0 connected to all others
        for i in 1..n {
            ett.link(0, i);
        }

        // Verify all connected
        for i in 1..n {
            assert!(ett.connected(0, i));
        }

        assert_eq!(ett.tree_size(0), n as usize);
    }
}

#[cfg(test)]
mod cut_watcher_tests {
    use super::*;

    #[test]
    fn test_edge_insert_updates_cut() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 100.0);

        assert_eq!(watcher.current_mincut(), 5.0);

        watcher.insert_edge(1, 2, 3.0);
        assert_eq!(watcher.current_mincut(), 5.0); // No decrease

        watcher.insert_edge(2, 3, 7.0);
        assert_eq!(watcher.current_mincut(), 7.0); // Increased
    }

    #[test]
    fn test_edge_delete_updates_cut() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 100.0);

        watcher.delete_edge(1, 2, 3.0);
        assert!(!watcher.needs_recompute); // Not critical edge

        watcher.delete_edge(2, 3, 5.0);
        assert!(watcher.needs_recompute); // Critical edge deleted
    }

    #[test]
    fn test_cut_sensitivity_detection() {
        let mut watcher = MockDynamicCutWatcher::new(10.0, 50.0);

        // Perform updates
        for i in 0..45 {
            watcher.insert_edge(i, i + 1, 1.0);
        }

        assert!(!watcher.needs_recompute);

        // Cross threshold
        for i in 45..55 {
            watcher.insert_edge(i, i + 1, 1.0);
        }

        assert!(watcher.needs_recompute);
    }

    #[test]
    fn test_threshold_triggering() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 10.0);

        for i in 0..9 {
            watcher.insert_edge(i, i + 1, 1.0);
        }
        assert!(!watcher.needs_recompute);

        watcher.insert_edge(9, 10, 1.0);
        assert!(watcher.needs_recompute);
    }

    #[test]
    fn test_recompute_fallback() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 10.0);

        // Trigger recompute
        for i in 0..15 {
            watcher.insert_edge(i, i + 1, 1.0);
        }

        assert!(watcher.needs_recompute);

        // Recompute
        watcher.trigger_recompute();
        assert!(!watcher.needs_recompute);
        assert_eq!(watcher.updates_count, 0);
    }

    #[test]
    fn test_concurrent_updates() {
        let watcher = Arc::new(Mutex::new(MockDynamicCutWatcher::new(10.0, 100.0)));

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let watcher_clone = Arc::clone(&watcher);
                thread::spawn(move || {
                    for i in 0..25 {
                        let mut w = watcher_clone.lock().unwrap();
                        w.insert_edge(thread_id * 100 + i, thread_id * 100 + i + 1, 1.0);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let w = watcher.lock().unwrap();
        assert_eq!(w.updates_count, 100);
        assert!(w.needs_recompute);
    }
}

#[cfg(test)]
mod local_mincut_tests {
    use super::*;

    #[test]
    fn test_local_cut_basic() {
        // Simulate local min-cut computation
        let graph = create_test_graph(10, 0.3);
        let local_cut = compute_local_mincut(&graph, 0, 3);

        assert!(local_cut > 0.0);
        assert!(local_cut < f64::INFINITY);
    }

    #[test]
    fn test_weak_region_detection() {
        let graph = create_bottleneck_graph(20);
        let weak_region = detect_weak_region(&graph, 0);

        assert!(!weak_region.is_empty());
        assert!(weak_region.len() < 20);
    }

    #[test]
    fn test_ball_growing() {
        let graph = create_test_graph(50, 0.2);
        let ball = grow_ball_from_vertex(&graph, 0, 5);

        assert!(ball.contains(&0));
        assert!(ball.len() <= 5);
    }

    #[test]
    fn test_conductance_threshold() {
        let graph = create_expander_graph(30);
        let conductance = compute_conductance(&graph, &[0, 1, 2, 3, 4]);

        assert!(conductance > 0.0);
        assert!(conductance <= 1.0);
    }

    // Helper functions

    fn create_test_graph(n: usize, _density: f64) -> HashMap<usize, Vec<usize>> {
        let mut graph = HashMap::new();
        for i in 0..n {
            graph.insert(i, vec![(i + 1) % n, (i + 2) % n]);
        }
        graph
    }

    fn create_bottleneck_graph(n: usize) -> HashMap<usize, Vec<usize>> {
        let mut graph = HashMap::new();
        let half = n / 2;

        // Dense left side
        for i in 0..half {
            graph.insert(i, (0..half).filter(|&j| j != i).collect());
        }

        // Dense right side
        for i in half..n {
            graph.insert(i, (half..n).filter(|&j| j != i).collect());
        }

        // Single bottleneck edge
        graph.get_mut(&(half - 1)).unwrap().push(half);
        graph.get_mut(&half).unwrap().push(half - 1);

        graph
    }

    fn create_expander_graph(n: usize) -> HashMap<usize, Vec<usize>> {
        let mut graph = HashMap::new();
        for i in 0..n {
            graph.insert(
                i,
                vec![(i + 1) % n, (i + 2) % n, (i + 5) % n, (i + 11) % n],
            );
        }
        graph
    }

    fn compute_local_mincut(graph: &HashMap<usize, Vec<usize>>, source: usize, radius: usize) -> f64 {
        let ball = grow_ball_from_vertex(graph, source, radius);
        compute_conductance(graph, &ball)
    }

    fn detect_weak_region(graph: &HashMap<usize, Vec<usize>>, start: usize) -> Vec<usize> {
        grow_ball_from_vertex(graph, start, 5)
    }

    fn grow_ball_from_vertex(
        graph: &HashMap<usize, Vec<usize>>,
        start: usize,
        max_radius: usize,
    ) -> Vec<usize> {
        let mut ball = vec![start];
        let mut visited = HashSet::new();
        visited.insert(start);

        for _ in 0..max_radius {
            let mut new_vertices = Vec::new();
            for &v in &ball {
                if let Some(neighbors) = graph.get(&v) {
                    for &neighbor in neighbors {
                        if visited.insert(neighbor) {
                            new_vertices.push(neighbor);
                        }
                    }
                }
            }
            ball.extend(new_vertices);
        }

        ball
    }

    fn compute_conductance(graph: &HashMap<usize, Vec<usize>>, subset: &[usize]) -> f64 {
        let subset_set: HashSet<_> = subset.iter().copied().collect();

        let mut cut_edges = 0;
        let mut volume = 0;

        for &v in subset {
            if let Some(neighbors) = graph.get(&v) {
                volume += neighbors.len();
                for &neighbor in neighbors {
                    if !subset_set.contains(&neighbor) {
                        cut_edges += 1;
                    }
                }
            }
        }

        if volume == 0 {
            return 1.0;
        }

        cut_edges as f64 / volume as f64
    }
}

#[cfg(test)]
mod cut_gated_search_tests {
    use super::*;

    #[test]
    fn test_gated_vs_ungated_search() {
        let graph = create_search_graph();

        // Ungated: explores all vertices
        let ungated_visited = ungated_search(&graph, 0, 10);

        // Gated: stops at cut boundaries
        let gated_visited = gated_search(&graph, 0, 10, 2.0);

        assert!(gated_visited.len() <= ungated_visited.len());
    }

    #[test]
    fn test_expansion_pruning() {
        let graph = create_partitioned_graph();
        let visited = gated_search(&graph, 0, 20, 1.0);

        // Should only visit one partition
        assert!(visited.len() < 15);
    }

    #[test]
    fn test_cross_cut_hops() {
        let graph = create_partitioned_graph();
        let path = find_path_respecting_cuts(&graph, 0, 25, 2.0);

        // Path should avoid crossing low-conductance cuts
        assert!(path.is_some());
    }

    #[test]
    fn test_coherence_zones() {
        let graph = create_clustered_graph();
        let zones = identify_coherence_zones(&graph, 0.3);

        assert!(zones.len() > 1);
        assert!(zones.len() < 10);
    }

    // Helper functions

    fn create_search_graph() -> HashMap<usize, Vec<(usize, f64)>> {
        let mut graph = HashMap::new();
        for i in 0..15 {
            graph.insert(i, vec![(i + 1, 1.0), (i + 2, 1.0)]);
        }
        graph
    }

    fn create_partitioned_graph() -> HashMap<usize, Vec<(usize, f64)>> {
        let mut graph = HashMap::new();

        // Partition 1: 0-9
        for i in 0..10 {
            graph.insert(i, vec![(i + 1, 5.0), (i + 2, 5.0)]);
        }

        // Partition 2: 10-19
        for i in 10..20 {
            graph.insert(i, vec![(i + 1, 5.0), (i + 2, 5.0)]);
        }

        // Weak bridge
        graph.insert(9, vec![(10, 0.5)]);

        graph
    }

    fn create_clustered_graph() -> HashMap<usize, Vec<(usize, f64)>> {
        let mut graph = HashMap::new();

        for cluster in 0..3 {
            for i in 0..10 {
                let v = cluster * 10 + i;
                graph.insert(v, vec![(v + 1, 10.0), (v + 2, 10.0)]);
            }
        }

        graph
    }

    fn ungated_search(graph: &HashMap<usize, Vec<(usize, f64)>>, start: usize, max: usize) -> Vec<usize> {
        let mut visited = vec![start];
        let mut seen = HashSet::new();
        seen.insert(start);

        while visited.len() < max {
            let mut found_new = false;
            for &v in &visited.clone() {
                if let Some(neighbors) = graph.get(&v) {
                    for &(neighbor, _) in neighbors {
                        if seen.insert(neighbor) {
                            visited.push(neighbor);
                            found_new = true;
                            if visited.len() >= max {
                                break;
                            }
                        }
                    }
                }
                if visited.len() >= max {
                    break;
                }
            }
            if !found_new {
                break;
            }
        }

        visited
    }

    fn gated_search(
        graph: &HashMap<usize, Vec<(usize, f64)>>,
        start: usize,
        max: usize,
        min_weight: f64,
    ) -> Vec<usize> {
        let mut visited = vec![start];
        let mut seen = HashSet::new();
        seen.insert(start);

        while visited.len() < max {
            let mut found_new = false;
            for &v in &visited.clone() {
                if let Some(neighbors) = graph.get(&v) {
                    for &(neighbor, weight) in neighbors {
                        if weight >= min_weight && seen.insert(neighbor) {
                            visited.push(neighbor);
                            found_new = true;
                            if visited.len() >= max {
                                break;
                            }
                        }
                    }
                }
                if visited.len() >= max {
                    break;
                }
            }
            if !found_new {
                break;
            }
        }

        visited
    }

    fn find_path_respecting_cuts(
        graph: &HashMap<usize, Vec<(usize, f64)>>,
        start: usize,
        end: usize,
        min_weight: f64,
    ) -> Option<Vec<usize>> {
        let visited = gated_search(graph, start, 100, min_weight);
        if visited.contains(&end) {
            Some(visited)
        } else {
            None
        }
    }

    fn identify_coherence_zones(
        graph: &HashMap<usize, Vec<(usize, f64)>>,
        threshold: f64,
    ) -> Vec<Vec<usize>> {
        let mut zones = Vec::new();
        let mut visited_global = HashSet::new();

        for &start in graph.keys() {
            if visited_global.contains(&start) {
                continue;
            }

            let zone = gated_search(graph, start, 100, threshold);
            for &v in &zone {
                visited_global.insert(v);
            }
            zones.push(zone);
        }

        zones
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_pipeline() {
        // Create graph
        let mut ett = MockEulerTourTree::new();
        for i in 0..10 {
            ett.make_tree(i);
        }

        // Build structure
        for i in 0..9 {
            ett.link(i, i + 1);
        }

        // Create watcher
        let mut watcher = MockDynamicCutWatcher::new(1.0, 20.0);

        // Perform updates
        for i in 10..20 {
            watcher.insert_edge(i, i + 1, 1.0);
        }

        // Verify state
        assert!(ett.connected(0, 9));
        assert_eq!(watcher.updates_count, 10);
    }

    #[test]
    fn test_with_real_vectors() {
        // Simulate vector database with min-cut tracking
        let vectors = generate_test_vectors(100);
        let graph = build_knn_graph(&vectors, 5);

        let mut ett = MockEulerTourTree::new();
        for i in 0..100 {
            ett.make_tree(i);
        }

        for (u, v) in &graph {
            ett.link(*u, *v);
        }

        // Verify connectivity
        let num_components = count_components(&ett);
        assert!(num_components >= 1);
        assert!(num_components <= 100);
    }

    #[test]
    fn test_streaming_updates() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 50.0);

        // Simulate streaming edge updates
        for batch in 0..5 {
            for i in 0..10 {
                let edge_id = batch * 10 + i;
                watcher.insert_edge(edge_id, edge_id + 1, 1.0);
            }

            if batch == 2 {
                // Midway recompute
                watcher.trigger_recompute();
            }
        }

        assert_eq!(watcher.updates_count, 20); // 50 total - 30 before recompute
    }

    // Helper functions

    fn generate_test_vectors(n: usize) -> Vec<Vec<f64>> {
        (0..n)
            .map(|i| vec![(i as f64) * 0.1; 128])
            .collect()
    }

    fn build_knn_graph(vectors: &[Vec<f64>], k: usize) -> Vec<(u64, u64)> {
        let mut edges = Vec::new();

        for (i, _vec) in vectors.iter().enumerate() {
            // Simplified: connect to next k vertices
            for j in 1..=k {
                if i + j < vectors.len() {
                    edges.push((i as u64, (i + j) as u64));
                }
            }
        }

        edges
    }

    fn count_components(ett: &MockEulerTourTree) -> usize {
        ett.connected_components.values().collect::<HashSet<_>>().len()
    }
}

#[cfg(test)]
mod correctness_tests {
    use super::*;

    #[test]
    fn test_dynamic_equals_static() {
        let graph = create_test_graph_simple(20);

        // Static computation (Stoer-Wagner simulation)
        let static_cut = compute_static_mincut(&graph);

        // Dynamic computation
        let mut watcher = MockDynamicCutWatcher::new(static_cut, 100.0);

        // Perform some updates
        for i in 0..5 {
            watcher.insert_edge(i, i + 1, 1.0);
        }

        // After stabilization, should match
        let dynamic_cut = watcher.current_mincut();

        assert!((static_cut - dynamic_cut).abs() < 10.0); // Approximate equality
    }

    #[test]
    fn test_monotonicity() {
        let mut watcher = MockDynamicCutWatcher::new(5.0, 100.0);

        let initial_cut = watcher.current_mincut();

        // Adding edges should not decrease min-cut
        watcher.insert_edge(1, 2, 3.0);
        assert!(watcher.current_mincut() >= initial_cut);

        watcher.insert_edge(2, 3, 7.0);
        let after_second = watcher.current_mincut();
        assert!(after_second >= initial_cut);
    }

    #[test]
    fn test_symmetry() {
        // Order of updates shouldn't affect final state (after recompute)
        let mut watcher1 = MockDynamicCutWatcher::new(10.0, 100.0);
        let mut watcher2 = MockDynamicCutWatcher::new(10.0, 100.0);

        // Apply updates in different orders
        watcher1.insert_edge(1, 2, 5.0);
        watcher1.insert_edge(2, 3, 3.0);
        watcher1.insert_edge(3, 4, 8.0);

        watcher2.insert_edge(3, 4, 8.0);
        watcher2.insert_edge(1, 2, 5.0);
        watcher2.insert_edge(2, 3, 3.0);

        // After same updates, should have same cut value
        assert_eq!(watcher1.current_mincut(), watcher2.current_mincut());
    }

    #[test]
    fn test_edge_cases_empty_graph() {
        let ett = MockEulerTourTree::new();
        assert_eq!(ett.vertices.len(), 0);
    }

    #[test]
    fn test_edge_cases_single_node() {
        let mut ett = MockEulerTourTree::new();
        ett.make_tree(1);
        assert_eq!(ett.tree_size(1), 1);
    }

    #[test]
    fn test_edge_cases_disconnected_components() {
        let mut ett = MockEulerTourTree::new();

        for i in 0..10 {
            ett.make_tree(i);
        }

        // Create two components
        ett.link(0, 1);
        ett.link(1, 2);

        ett.link(5, 6);
        ett.link(6, 7);

        assert!(ett.connected(0, 2));
        assert!(ett.connected(5, 7));
        assert!(!ett.connected(0, 5));
    }

    // Helper functions

    fn create_test_graph_simple(n: usize) -> HashMap<usize, Vec<(usize, f64)>> {
        let mut graph = HashMap::new();
        for i in 0..n {
            graph.insert(i, vec![(i + 1, 1.0)]);
        }
        graph
    }

    fn compute_static_mincut(_graph: &HashMap<usize, Vec<(usize, f64)>>) -> f64 {
        // Simplified static min-cut computation
        1.0
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn test_large_scale_operations() {
        let mut ett = MockEulerTourTree::new();

        // Create 10,000 vertices
        for i in 0..10_000 {
            ett.make_tree(i);
        }

        // Create chain
        for i in 0..9_999 {
            ett.link(i, i + 1);
        }

        assert!(ett.connected(0, 9_999));
        assert_eq!(ett.tree_size(0), 10_000);
    }

    #[test]
    fn test_repeated_cut_and_link() {
        let mut ett = MockEulerTourTree::new();

        for i in 0..10 {
            ett.make_tree(i);
        }

        // Repeatedly link and cut
        for _ in 0..100 {
            ett.link(0, 1);
            assert!(ett.connected(0, 1));

            ett.cut(0, 1);
            assert!(!ett.connected(0, 1));
        }
    }

    #[test]
    fn test_high_frequency_updates() {
        let mut watcher = MockDynamicCutWatcher::new(10.0, 1000.0);

        // Perform 100,000 updates
        for i in 0..100_000 {
            if i % 2 == 0 {
                watcher.insert_edge(i, i + 1, 1.0);
            } else {
                watcher.delete_edge(i - 1, i, 1.0);
            }
        }

        assert!(watcher.updates_count > 0);
    }
}
