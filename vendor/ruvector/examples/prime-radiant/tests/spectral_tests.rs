//! Integration tests for the Spectral Invariants module

use prime_radiant::spectral::{
    Graph, SparseMatrix, SpectralAnalyzer, SpectralGap, Vector,
    CheegerAnalyzer, CheegerBounds, cheeger_inequality,
    SpectralClusterer, ClusterAssignment, ClusterConfig,
    CollapsePredictor, CollapsePrediction, Warning, WarningLevel,
    spectral_coherence_energy, SpectralEnergy, EnergyMinimizer,
    LanczosAlgorithm, PowerIteration,
    NodeId, EPS,
};

// ============================================================================
// Graph Construction Helpers
// ============================================================================

fn create_path_graph(n: usize) -> Graph {
    let edges: Vec<(usize, usize, f64)> = (0..n - 1)
        .map(|i| (i, i + 1, 1.0))
        .collect();
    Graph::from_edges(n, &edges)
}

fn create_cycle_graph(n: usize) -> Graph {
    let mut edges: Vec<(usize, usize, f64)> = (0..n - 1)
        .map(|i| (i, i + 1, 1.0))
        .collect();
    edges.push((n - 1, 0, 1.0));
    Graph::from_edges(n, &edges)
}

fn create_complete_graph(n: usize) -> Graph {
    let mut edges = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            edges.push((i, j, 1.0));
        }
    }
    Graph::from_edges(n, &edges)
}

fn create_barbell_graph(clique_size: usize) -> Graph {
    let n = 2 * clique_size;
    let mut g = Graph::new(n);

    // First clique
    for i in 0..clique_size {
        for j in i + 1..clique_size {
            g.add_edge(i, j, 1.0);
        }
    }

    // Second clique
    for i in clique_size..n {
        for j in i + 1..n {
            g.add_edge(i, j, 1.0);
        }
    }

    // Bridge
    g.add_edge(clique_size - 1, clique_size, 1.0);

    g
}

fn create_star_graph(n: usize) -> Graph {
    let edges: Vec<(usize, usize, f64)> = (1..n)
        .map(|i| (0, i, 1.0))
        .collect();
    Graph::from_edges(n, &edges)
}

// ============================================================================
// Graph and SparseMatrix Tests
// ============================================================================

#[test]
fn test_graph_construction() {
    let g = create_complete_graph(5);

    assert_eq!(g.n, 5);
    assert_eq!(g.num_edges(), 10);
    assert!(g.is_connected());
    assert_eq!(g.num_components(), 1);
}

#[test]
fn test_graph_degrees() {
    let g = create_complete_graph(5);
    let degrees = g.degrees();

    for &d in &degrees {
        assert!((d - 4.0).abs() < EPS);
    }
}

#[test]
fn test_disconnected_graph() {
    let g = Graph::from_edges(6, &[
        (0, 1, 1.0), (1, 2, 1.0), (2, 0, 1.0),
        (3, 4, 1.0), (4, 5, 1.0), (5, 3, 1.0),
    ]);

    assert!(!g.is_connected());
    assert_eq!(g.num_components(), 2);
}

#[test]
fn test_laplacian_properties() {
    let g = create_complete_graph(4);
    let l = g.laplacian();

    for i in 0..4 {
        let row_sum: f64 = (0..4).map(|j| l.get(i, j)).sum();
        assert!(row_sum.abs() < EPS, "Row sum should be zero");
    }
}

// ============================================================================
// Spectral Analyzer Tests
// ============================================================================

#[test]
fn test_spectral_analyzer_basic() {
    let g = create_cycle_graph(6);
    let mut analyzer = SpectralAnalyzer::new(g);
    analyzer.compute_laplacian_spectrum();

    assert!(!analyzer.eigenvalues.is_empty());
    assert!(analyzer.eigenvalues[0].abs() < 0.01);
}

#[test]
fn test_algebraic_connectivity() {
    let complete = create_complete_graph(10);
    let path = create_path_graph(10);

    let mut analyzer_complete = SpectralAnalyzer::new(complete);
    let mut analyzer_path = SpectralAnalyzer::new(path);

    analyzer_complete.compute_laplacian_spectrum();
    analyzer_path.compute_laplacian_spectrum();

    let ac_complete = analyzer_complete.algebraic_connectivity();
    let ac_path = analyzer_path.algebraic_connectivity();

    assert!(ac_complete > ac_path);
    assert!(ac_complete > 0.0);
    assert!(ac_path > 0.0);
}

#[test]
fn test_fiedler_vector() {
    let g = create_barbell_graph(4);
    let mut analyzer = SpectralAnalyzer::new(g);
    analyzer.compute_laplacian_spectrum();

    let fiedler = analyzer.fiedler_vector();
    assert!(fiedler.is_some());
    assert_eq!(fiedler.unwrap().len(), 8);
}

#[test]
fn test_bottleneck_detection() {
    let g = create_barbell_graph(5);
    let mut analyzer = SpectralAnalyzer::new(g);
    analyzer.compute_laplacian_spectrum();

    let bottlenecks = analyzer.detect_bottlenecks();
    assert!(!bottlenecks.is_empty());

    let has_bridge = bottlenecks.iter().any(|b| {
        b.crossing_edges.contains(&(4, 5))
    });
    assert!(has_bridge, "Bridge edge should be in bottleneck");
}

// ============================================================================
// Cheeger Analyzer Tests
// ============================================================================

#[test]
fn test_cheeger_bounds() {
    let g = create_complete_graph(10);
    let mut analyzer = CheegerAnalyzer::new(&g);
    let bounds = analyzer.compute_cheeger_bounds();

    assert!(bounds.lower_bound >= 0.0);
    assert!(bounds.lower_bound <= bounds.cheeger_constant);
    assert!(bounds.cheeger_constant <= bounds.upper_bound);
}

#[test]
fn test_cheeger_well_connected() {
    let g = create_complete_graph(10);
    let mut analyzer = CheegerAnalyzer::new(&g);
    let bounds = analyzer.compute_cheeger_bounds();

    assert!(bounds.is_well_connected());
}

// ============================================================================
// Spectral Clustering Tests
// ============================================================================

#[test]
fn test_spectral_clustering_two_clusters() {
    let g = create_barbell_graph(5);
    let clusterer = SpectralClusterer::new(2);
    let assignment = clusterer.cluster(&g);

    assert_eq!(assignment.k, 2);
    assert_eq!(assignment.labels.len(), 10);
    assert!(assignment.quality.modularity > 0.0);
}

// ============================================================================
// Collapse Predictor Tests
// ============================================================================

#[test]
fn test_collapse_predictor_stable() {
    let g = create_complete_graph(10);
    let predictor = CollapsePredictor::new();

    let prediction = predictor.predict_collapse(&g);

    assert!(prediction.risk_score < 0.5);
}

#[test]
fn test_warning_levels() {
    assert_eq!(WarningLevel::None.severity(), 0);
    assert_eq!(WarningLevel::Critical.severity(), 4);
    assert_eq!(WarningLevel::from_severity(2), WarningLevel::Medium);
}

// ============================================================================
// Spectral Energy Tests
// ============================================================================

#[test]
fn test_spectral_energy_basic() {
    let g = create_complete_graph(10);
    let energy = spectral_coherence_energy(&g);

    assert!(energy.laplacian_energy > 0.0);
    assert!(energy.coherence_energy > 0.0);
    assert!(energy.stability_score >= 0.0 && energy.stability_score <= 1.0);
}

#[test]
fn test_spectral_energy_comparison() {
    let complete = create_complete_graph(10);
    let path = create_path_graph(10);

    let energy_complete = spectral_coherence_energy(&complete);
    let energy_path = spectral_coherence_energy(&path);

    assert!(energy_complete.coherence_energy > energy_path.coherence_energy);
}

// ============================================================================
// Lanczos Algorithm Tests
// ============================================================================

#[test]
fn test_power_iteration() {
    let g = create_complete_graph(5);
    let l = g.laplacian();

    let power = PowerIteration::default();
    let (lambda, v) = power.largest_eigenvalue(&l);

    let av = l.mul_vec(&v);
    let error: f64 = av.iter()
        .zip(v.iter())
        .map(|(avi, vi)| (avi - lambda * vi).powi(2))
        .sum::<f64>()
        .sqrt();

    assert!(error < 0.1, "Eigenvalue error: {}", error);
}

#[test]
fn test_lanczos_algorithm() {
    let g = create_cycle_graph(8);
    let l = g.laplacian();

    let lanczos = LanczosAlgorithm::new(5);
    let (eigenvalues, eigenvectors) = lanczos.compute_smallest(&l);

    assert!(!eigenvalues.is_empty());
    assert!(eigenvalues[0].abs() < 0.01);
}
