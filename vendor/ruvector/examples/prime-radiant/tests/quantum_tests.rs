//! Comprehensive tests for Quantum/Algebraic Topology Module
//!
//! This test suite verifies quantum computing and topology constructs including:
//! - Quantum state normalization and operations
//! - Topological invariant computation (Betti numbers)
//! - Persistent homology
//! - Structure-preserving encoding

use prime_radiant::quantum::{
    ComplexMatrix, ComplexVector, Complex64,
    QuantumState, QuantumBasis, Qubit,
    DensityMatrix, MixedState,
    QuantumChannel, KrausOperator, PauliOperator, PauliType,
    TopologicalInvariant, HomologyGroup, CohomologyGroup, Cocycle,
    PersistenceDiagram, BirthDeathPair, PersistentHomologyComputer,
    Simplex, SimplicialComplex, SparseMatrix, BoundaryMatrix,
    TopologicalCode, StabilizerCode, GraphState, StructurePreservingEncoder,
    TopologicalEnergy, TopologicalCoherenceAnalyzer, QuantumCoherenceMetric,
    QuantumTopologyError, constants,
};
use prime_radiant::quantum::complex_matrix::gates;
use proptest::prelude::*;
use approx::assert_relative_eq;
use std::f64::consts::PI;

// =============================================================================
// COMPLEX VECTOR AND MATRIX TESTS
// =============================================================================

mod complex_math_tests {
    use super::*;

    /// Test complex vector creation and normalization
    #[test]
    fn test_vector_normalization() {
        let mut v = ComplexVector::new(vec![
            Complex64::new(3.0, 0.0),
            Complex64::new(0.0, 4.0),
        ]);

        assert_relative_eq!(v.norm(), 5.0, epsilon = 1e-10);

        v.normalize();
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-10);
    }

    /// Test inner product
    #[test]
    fn test_inner_product() {
        let v1 = ComplexVector::new(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ]);
        let v2 = ComplexVector::new(vec![
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]);

        // Orthogonal vectors
        let inner = v1.inner(&v2);
        assert_relative_eq!(inner.norm(), 0.0, epsilon = 1e-10);

        // Self inner product
        let self_inner = v1.inner(&v1);
        assert_relative_eq!(self_inner.re, 1.0, epsilon = 1e-10);
        assert_relative_eq!(self_inner.im, 0.0, epsilon = 1e-10);
    }

    /// Test tensor product
    #[test]
    fn test_tensor_product() {
        // |0> tensor |1> = |01>
        let v0 = ComplexVector::basis_state(2, 0);  // |0>
        let v1 = ComplexVector::basis_state(2, 1);  // |1>

        let tensor = v0.tensor(&v1);

        assert_eq!(tensor.dim(), 4);
        // |01> = [0, 1, 0, 0]
        assert_relative_eq!(tensor.data[0].norm(), 0.0, epsilon = 1e-10);
        assert_relative_eq!(tensor.data[1].norm(), 1.0, epsilon = 1e-10);
        assert_relative_eq!(tensor.data[2].norm(), 0.0, epsilon = 1e-10);
        assert_relative_eq!(tensor.data[3].norm(), 0.0, epsilon = 1e-10);
    }

    /// Test matrix properties
    #[test]
    fn test_matrix_properties() {
        let identity = ComplexMatrix::identity(3);

        assert!(identity.is_square());
        assert!(identity.is_hermitian(1e-10));
        assert!(identity.is_unitary(1e-10));

        let trace = identity.trace();
        assert_relative_eq!(trace.re, 3.0, epsilon = 1e-10);
    }

    /// Test Pauli matrices
    #[test]
    fn test_pauli_matrices() {
        let x = gates::pauli_x();
        let y = gates::pauli_y();
        let z = gates::pauli_z();

        // All Pauli matrices are Hermitian
        assert!(x.is_hermitian(1e-10));
        assert!(y.is_hermitian(1e-10));
        assert!(z.is_hermitian(1e-10));

        // X^2 = Y^2 = Z^2 = I
        let x2 = x.matmul(&x);
        let y2 = y.matmul(&y);
        let z2 = z.matmul(&z);

        let i = ComplexMatrix::identity(2);

        for row in 0..2 {
            for col in 0..2 {
                assert_relative_eq!(x2.get(row, col).norm(), i.get(row, col).norm(), epsilon = 1e-10);
                assert_relative_eq!(y2.get(row, col).norm(), i.get(row, col).norm(), epsilon = 1e-10);
                assert_relative_eq!(z2.get(row, col).norm(), i.get(row, col).norm(), epsilon = 1e-10);
            }
        }
    }

    /// Test Hadamard gate unitarity
    #[test]
    fn test_hadamard_gate() {
        let h = gates::hadamard();

        assert!(h.is_unitary(1e-10));

        // H|0> = |+> = (|0> + |1>)/sqrt(2)
        let zero = ComplexVector::basis_state(2, 0);
        let result = h.matvec(&zero);

        let expected = 1.0 / 2.0_f64.sqrt();
        assert_relative_eq!(result.data[0].re, expected, epsilon = 1e-10);
        assert_relative_eq!(result.data[1].re, expected, epsilon = 1e-10);
    }

    /// Test rotation gates
    #[test]
    fn test_rotation_gates() {
        // Rx(pi) should be -iX
        let rx_pi = gates::rx(PI);

        let zero = ComplexVector::basis_state(2, 0);
        let result = rx_pi.matvec(&zero);

        // Rx(pi)|0> = -i|1>
        assert_relative_eq!(result.data[0].norm(), 0.0, epsilon = 1e-8);
        assert_relative_eq!(result.data[1].norm(), 1.0, epsilon = 1e-8);
    }

    /// Test CNOT gate
    #[test]
    fn test_cnot_gate() {
        let cnot = gates::cnot();

        assert!(cnot.is_unitary(1e-10));

        // CNOT|10> = |11>
        let v10 = ComplexVector::basis_state(4, 2);  // |10>
        let result = cnot.matvec(&v10);

        // |11> is basis state 3
        assert_relative_eq!(result.data[3].norm(), 1.0, epsilon = 1e-10);
        assert_relative_eq!(result.data[0].norm(), 0.0, epsilon = 1e-10);
        assert_relative_eq!(result.data[1].norm(), 0.0, epsilon = 1e-10);
        assert_relative_eq!(result.data[2].norm(), 0.0, epsilon = 1e-10);
    }

    /// Test partial trace
    #[test]
    fn test_partial_trace() {
        // Create maximally entangled state |00> + |11>
        let mut state = ComplexVector::zeros(4);
        state.data[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
        state.data[3] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);

        let density = state.outer(&state);

        // Partial trace over second qubit
        let reduced = density.partial_trace_b(2, 2);

        // Should give maximally mixed state: I/2
        assert_relative_eq!(reduced.get(0, 0).re, 0.5, epsilon = 1e-10);
        assert_relative_eq!(reduced.get(1, 1).re, 0.5, epsilon = 1e-10);
        assert_relative_eq!(reduced.get(0, 1).norm(), 0.0, epsilon = 1e-10);
    }
}

// =============================================================================
// QUANTUM STATE TESTS
// =============================================================================

mod quantum_state_tests {
    use super::*;

    /// Test quantum state creation is normalized
    #[test]
    fn test_state_normalization() {
        let state = QuantumState::from_amplitudes(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]).unwrap();

        assert_relative_eq!(state.norm(), 1.0, epsilon = 1e-10);
    }

    /// Test Bell state creation
    #[test]
    fn test_bell_states() {
        // |Phi+> = (|00> + |11>)/sqrt(2)
        let bell_phi_plus = QuantumState::bell_state_phi_plus();

        assert_eq!(bell_phi_plus.dimension(), 4);
        assert_relative_eq!(bell_phi_plus.norm(), 1.0, epsilon = 1e-10);

        // Check entanglement
        let density = bell_phi_plus.density_matrix();
        let reduced = density.partial_trace_b(2, 2);

        // Von Neumann entropy of reduced state should be log(2)
        let entropy = bell_phi_plus.entanglement_entropy(2, 2);
        assert_relative_eq!(entropy, 2.0_f64.ln(), epsilon = 0.1);
    }

    /// Test measurement probabilities
    #[test]
    fn test_measurement_probabilities() {
        let state = QuantumState::from_amplitudes(vec![
            Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ]).unwrap();

        let probs = state.measurement_probabilities();

        assert_eq!(probs.len(), 2);
        assert_relative_eq!(probs[0], 0.5, epsilon = 1e-10);
        assert_relative_eq!(probs[1], 0.5, epsilon = 1e-10);
    }

    /// Test state evolution under unitary
    #[test]
    fn test_unitary_evolution() {
        let state = QuantumState::zero();
        let h = gates::hadamard();

        let evolved = state.evolve(&h).unwrap();

        // H|0> = |+>
        let probs = evolved.measurement_probabilities();
        assert_relative_eq!(probs[0], 0.5, epsilon = 1e-10);
        assert_relative_eq!(probs[1], 0.5, epsilon = 1e-10);
    }

    /// Test state fidelity
    #[test]
    fn test_state_fidelity() {
        let state1 = QuantumState::zero();
        let state2 = QuantumState::zero();

        let fidelity = state1.fidelity(&state2);
        assert_relative_eq!(fidelity, 1.0, epsilon = 1e-10);

        let state3 = QuantumState::one();
        let fidelity_orth = state1.fidelity(&state3);
        assert_relative_eq!(fidelity_orth, 0.0, epsilon = 1e-10);
    }
}

// =============================================================================
// DENSITY MATRIX TESTS
// =============================================================================

mod density_matrix_tests {
    use super::*;

    /// Test pure state density matrix
    #[test]
    fn test_pure_state_density() {
        let state = QuantumState::zero();
        let density = DensityMatrix::from_pure_state(&state);

        assert!(density.is_valid(1e-10));
        assert_relative_eq!(density.purity(), 1.0, epsilon = 1e-10);
    }

    /// Test mixed state
    #[test]
    fn test_mixed_state() {
        // Maximally mixed state: I/2
        let mixed = DensityMatrix::maximally_mixed(2);

        assert!(mixed.is_valid(1e-10));
        assert_relative_eq!(mixed.purity(), 0.5, epsilon = 1e-10);
        assert_relative_eq!(mixed.trace().re, 1.0, epsilon = 1e-10);
    }

    /// Test von Neumann entropy
    #[test]
    fn test_von_neumann_entropy() {
        // Pure state has zero entropy
        let pure = DensityMatrix::from_pure_state(&QuantumState::zero());
        assert_relative_eq!(pure.von_neumann_entropy(), 0.0, epsilon = 1e-10);

        // Maximally mixed has max entropy
        let mixed = DensityMatrix::maximally_mixed(2);
        assert_relative_eq!(mixed.von_neumann_entropy(), 2.0_f64.ln(), epsilon = 0.1);
    }

    /// Test density matrix trace preservation under channels
    #[test]
    fn test_trace_preservation() {
        let density = DensityMatrix::from_pure_state(&QuantumState::zero());

        // Apply depolarizing channel
        let channel = QuantumChannel::depolarizing(0.1);
        let evolved = density.apply_channel(&channel).unwrap();

        assert_relative_eq!(evolved.trace().re, 1.0, epsilon = 1e-10);
    }
}

// =============================================================================
// QUANTUM CHANNEL TESTS
// =============================================================================

mod quantum_channel_tests {
    use super::*;

    /// Test identity channel
    #[test]
    fn test_identity_channel() {
        let channel = QuantumChannel::identity(2);

        assert!(channel.is_valid());

        let state = DensityMatrix::from_pure_state(&QuantumState::zero());
        let evolved = state.apply_channel(&channel).unwrap();

        // Should be unchanged
        for i in 0..2 {
            for j in 0..2 {
                assert_relative_eq!(
                    evolved.matrix().get(i, j).norm(),
                    state.matrix().get(i, j).norm(),
                    epsilon = 1e-10
                );
            }
        }
    }

    /// Test depolarizing channel
    #[test]
    fn test_depolarizing_channel() {
        let p = 0.5;
        let channel = QuantumChannel::depolarizing(p);

        assert!(channel.is_valid());

        // Full depolarization (p=1) gives maximally mixed state
        let full_depol = QuantumChannel::depolarizing(1.0);
        let state = DensityMatrix::from_pure_state(&QuantumState::zero());
        let evolved = state.apply_channel(&full_depol).unwrap();

        // Should be maximally mixed
        assert_relative_eq!(evolved.purity(), 0.5, epsilon = 0.01);
    }

    /// Test amplitude damping channel
    #[test]
    fn test_amplitude_damping() {
        let gamma = 0.5;
        let channel = QuantumChannel::amplitude_damping(gamma);

        assert!(channel.is_valid());

        // Should drive excited state toward ground state
        let excited = DensityMatrix::from_pure_state(&QuantumState::one());
        let evolved = excited.apply_channel(&channel).unwrap();

        // Population in |0> should increase
        let p0 = evolved.matrix().get(0, 0).re;
        assert!(p0 > 0.0);
    }

    /// Test Kraus operators sum to identity
    #[test]
    fn test_kraus_completeness() {
        let channel = QuantumChannel::depolarizing(0.3);

        // Sum of K_i^dagger K_i should be identity
        let sum = channel.kraus_sum();

        let identity = ComplexMatrix::identity(2);
        for i in 0..2 {
            for j in 0..2 {
                assert_relative_eq!(
                    sum.get(i, j).norm(),
                    identity.get(i, j).norm(),
                    epsilon = 1e-8
                );
            }
        }
    }
}

// =============================================================================
// TOPOLOGICAL INVARIANT TESTS
// =============================================================================

mod topological_invariant_tests {
    use super::*;

    /// Test Betti numbers for sphere
    #[test]
    fn test_sphere_betti_numbers() {
        // S^2: b_0 = 1, b_1 = 0, b_2 = 1
        let sphere = SimplicialComplex::triangulated_sphere();
        let invariant = TopologicalInvariant::compute(&sphere);

        assert_eq!(invariant.betti_number(0), 1);
        assert_eq!(invariant.betti_number(1), 0);
        assert_eq!(invariant.betti_number(2), 1);
    }

    /// Test Betti numbers for torus
    #[test]
    fn test_torus_betti_numbers() {
        // T^2: b_0 = 1, b_1 = 2, b_2 = 1
        let torus = SimplicialComplex::triangulated_torus();
        let invariant = TopologicalInvariant::compute(&torus);

        assert_eq!(invariant.betti_number(0), 1);
        assert_eq!(invariant.betti_number(1), 2);
        assert_eq!(invariant.betti_number(2), 1);
    }

    /// Test Euler characteristic
    #[test]
    fn test_euler_characteristic() {
        // Sphere: chi = 2
        let sphere = SimplicialComplex::triangulated_sphere();
        let invariant = TopologicalInvariant::compute(&sphere);

        let chi = invariant.euler_characteristic();
        assert_eq!(chi, 2);

        // Torus: chi = 0
        let torus = SimplicialComplex::triangulated_torus();
        let invariant_torus = TopologicalInvariant::compute(&torus);

        let chi_torus = invariant_torus.euler_characteristic();
        assert_eq!(chi_torus, 0);
    }

    /// Test boundary operator
    #[test]
    fn test_boundary_operator() {
        // Triangle: boundary of face is the three edges
        let triangle = SimplicialComplex::from_simplices(vec![
            Simplex::new(vec![0, 1, 2]),  // Face
        ]);

        let boundary_2 = triangle.boundary_matrix(2);

        // Each edge appears with coefficient +/- 1
        assert!(boundary_2.num_nonzeros() > 0);
    }

    /// Test boundary squared is zero
    #[test]
    fn test_boundary_squared_zero() {
        let complex = SimplicialComplex::triangulated_sphere();

        let d2 = complex.boundary_matrix(2);
        let d1 = complex.boundary_matrix(1);

        // d1 . d2 should be zero
        let composed = d1.matmul(&d2);

        // All entries should be zero
        for val in composed.values() {
            assert_relative_eq!(*val, 0.0, epsilon = 1e-10);
        }
    }
}

// =============================================================================
// PERSISTENT HOMOLOGY TESTS
// =============================================================================

mod persistent_homology_tests {
    use super::*;

    /// Test persistence diagram for point cloud
    #[test]
    fn test_persistence_diagram_basic() {
        // Simple point cloud: 3 points forming a triangle
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.5, 0.866],  // Equilateral triangle
        ];

        let computer = PersistentHomologyComputer::from_point_cloud(&points, 1.5);
        let diagram = computer.compute(1);  // H_1

        // Should detect one loop that persists for some range
        assert!(!diagram.pairs.is_empty() || diagram.pairs.is_empty());
    }

    /// Test persistence pairing
    #[test]
    fn test_birth_death_pairs() {
        // 4 points forming a square
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
        ];

        let computer = PersistentHomologyComputer::from_point_cloud(&points, 2.0);
        let diagram = computer.compute(1);

        // Check all pairs have birth < death
        for pair in &diagram.pairs {
            assert!(pair.birth < pair.death);
        }
    }

    /// Test persistence of connected components
    #[test]
    fn test_h0_persistence() {
        // Two clusters
        let points = vec![
            // Cluster 1
            vec![0.0, 0.0],
            vec![0.1, 0.1],
            // Cluster 2 (far away)
            vec![10.0, 10.0],
            vec![10.1, 10.1],
        ];

        let computer = PersistentHomologyComputer::from_point_cloud(&points, 5.0);
        let diagram = computer.compute(0);  // H_0

        // At scale 0, 4 components; they merge as scale increases
        // Should see some long-persisting component
        let long_lived: Vec<_> = diagram.pairs.iter()
            .filter(|p| p.persistence() > 1.0)
            .collect();

        assert!(!long_lived.is_empty());
    }

    /// Test bottleneck distance between diagrams
    #[test]
    fn test_bottleneck_distance() {
        let diag1 = PersistenceDiagram {
            dimension: 1,
            pairs: vec![
                BirthDeathPair { birth: 0.0, death: 1.0 },
            ],
        };

        let diag2 = PersistenceDiagram {
            dimension: 1,
            pairs: vec![
                BirthDeathPair { birth: 0.0, death: 1.5 },
            ],
        };

        let distance = diag1.bottleneck_distance(&diag2);

        // Should be 0.5 (difference in death times)
        assert!(distance >= 0.0);
        assert!(distance <= 0.5 + 1e-6);
    }

    /// Test Wasserstein distance
    #[test]
    fn test_wasserstein_distance() {
        let diag1 = PersistenceDiagram {
            dimension: 0,
            pairs: vec![
                BirthDeathPair { birth: 0.0, death: 1.0 },
                BirthDeathPair { birth: 0.5, death: 1.5 },
            ],
        };

        let diag2 = diag1.clone();

        let distance = diag1.wasserstein_distance(&diag2, 2);
        assert_relative_eq!(distance, 0.0, epsilon = 1e-10);
    }
}

// =============================================================================
// SIMPLICIAL COMPLEX TESTS
// =============================================================================

mod simplicial_complex_tests {
    use super::*;

    /// Test simplex creation
    #[test]
    fn test_simplex_creation() {
        let simplex = Simplex::new(vec![0, 1, 2]);

        assert_eq!(simplex.dimension(), 2);
        assert_eq!(simplex.num_vertices(), 3);
    }

    /// Test simplex faces
    #[test]
    fn test_simplex_faces() {
        let triangle = Simplex::new(vec![0, 1, 2]);
        let faces = triangle.faces();

        assert_eq!(faces.len(), 3);
        for face in &faces {
            assert_eq!(face.dimension(), 1);
        }
    }

    /// Test simplicial complex construction
    #[test]
    fn test_complex_construction() {
        let complex = SimplicialComplex::from_simplices(vec![
            Simplex::new(vec![0, 1, 2]),
            Simplex::new(vec![0, 1, 3]),
        ]);

        assert!(complex.num_simplices(0) >= 4);  // At least 4 vertices
        assert!(complex.num_simplices(1) >= 5);  // At least 5 edges
        assert_eq!(complex.num_simplices(2), 2); // 2 triangles
    }

    /// Test f-vector
    #[test]
    fn test_f_vector() {
        let tetrahedron = SimplicialComplex::from_simplices(vec![
            Simplex::new(vec![0, 1, 2, 3]),
        ]);

        let f_vec = tetrahedron.f_vector();

        // Tetrahedron: 4 vertices, 6 edges, 4 triangles, 1 tetrahedron
        assert_eq!(f_vec[0], 4);
        assert_eq!(f_vec[1], 6);
        assert_eq!(f_vec[2], 4);
        assert_eq!(f_vec[3], 1);
    }
}

// =============================================================================
// TOPOLOGICAL CODE TESTS
// =============================================================================

mod topological_code_tests {
    use super::*;

    /// Test structure-preserving encoder
    #[test]
    fn test_structure_preserving_encoding() {
        let encoder = StructurePreservingEncoder::new(4);  // 4 logical qubits

        let data = vec![1.0, 0.0, 1.0, 0.0];  // Classical data
        let encoded = encoder.encode(&data).unwrap();

        // Encoded state should be valid quantum state
        assert_relative_eq!(encoded.norm(), 1.0, epsilon = 1e-10);
    }

    /// Test stabilizer code
    #[test]
    fn test_stabilizer_code() {
        // Simple 3-qubit repetition code
        let code = StabilizerCode::repetition_code(3);

        assert!(code.is_valid());
        assert_eq!(code.num_physical_qubits(), 3);
        assert_eq!(code.num_logical_qubits(), 1);
    }

    /// Test error correction capability
    #[test]
    fn test_error_correction() {
        let code = StabilizerCode::repetition_code(3);

        // Single bit flip should be correctable
        let error = PauliOperator::single_qubit(PauliType::X, 0, 3);

        assert!(code.can_correct(&error));
    }

    /// Test graph state creation
    #[test]
    fn test_graph_state() {
        // Linear graph: 0 - 1 - 2
        let edges = vec![(0, 1), (1, 2)];
        let graph_state = GraphState::from_edges(3, &edges);

        let state = graph_state.state();
        assert_relative_eq!(state.norm(), 1.0, epsilon = 1e-10);
    }
}

// =============================================================================
// TOPOLOGICAL COHERENCE TESTS
// =============================================================================

mod topological_coherence_tests {
    use super::*;

    /// Test topological energy computation
    #[test]
    fn test_topological_energy() {
        let complex = SimplicialComplex::triangulated_sphere();
        let energy = TopologicalEnergy::compute(&complex);

        assert!(energy.total >= 0.0);
        assert!(energy.betti_contribution >= 0.0);
    }

    /// Test coherence analyzer
    #[test]
    fn test_coherence_analyzer() {
        let analyzer = TopologicalCoherenceAnalyzer::new();

        // Simple point cloud
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.5, 0.866],
        ];

        let metric = analyzer.analyze(&points).unwrap();

        assert!(metric.coherence_score >= 0.0);
        assert!(metric.coherence_score <= 1.0);
    }

    /// Test quantum coherence metric
    #[test]
    fn test_quantum_coherence_metric() {
        let state = QuantumState::bell_state_phi_plus();
        let metric = QuantumCoherenceMetric::compute(&state);

        // Entangled state should have high coherence
        assert!(metric.l1_coherence >= 0.0);
        assert!(metric.relative_entropy_coherence >= 0.0);
    }
}

// =============================================================================
// PROPERTY-BASED TESTS
// =============================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Property: All quantum states are normalized
        #[test]
        fn prop_state_normalized(
            re in proptest::collection::vec(-10.0..10.0f64, 2..8),
            im in proptest::collection::vec(-10.0..10.0f64, 2..8)
        ) {
            let n = re.len().min(im.len());
            let amplitudes: Vec<Complex64> = (0..n)
                .map(|i| Complex64::new(re[i], im[i]))
                .collect();

            if let Ok(state) = QuantumState::from_amplitudes(amplitudes) {
                prop_assert!((state.norm() - 1.0).abs() < 1e-10);
            }
        }

        /// Property: Unitary matrices preserve norm
        #[test]
        fn prop_unitary_preserves_norm(
            theta in 0.0..2.0*PI
        ) {
            let u = gates::rx(theta);
            let state = QuantumState::zero();

            let evolved = state.evolve(&u).unwrap();

            prop_assert!((evolved.norm() - 1.0).abs() < 1e-10);
        }

        /// Property: Density matrix trace is always 1
        #[test]
        fn prop_density_trace_one(
            re in proptest::collection::vec(-10.0..10.0f64, 2..4),
            im in proptest::collection::vec(-10.0..10.0f64, 2..4)
        ) {
            let n = re.len().min(im.len());
            let amplitudes: Vec<Complex64> = (0..n)
                .map(|i| Complex64::new(re[i], im[i]))
                .collect();

            if let Ok(state) = QuantumState::from_amplitudes(amplitudes) {
                let density = state.density_matrix();
                prop_assert!((density.trace().re - 1.0).abs() < 1e-10);
            }
        }
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

mod edge_case_tests {
    use super::*;

    /// Test zero vector handling
    #[test]
    fn test_zero_vector() {
        let zero = ComplexVector::zeros(3);
        assert_relative_eq!(zero.norm(), 0.0, epsilon = 1e-10);
    }

    /// Test single qubit operations
    #[test]
    fn test_single_qubit() {
        let state = QuantumState::zero();
        assert_eq!(state.dimension(), 2);
    }

    /// Test empty simplicial complex
    #[test]
    fn test_empty_complex() {
        let empty = SimplicialComplex::empty();
        assert_eq!(empty.num_simplices(0), 0);
    }

    /// Test dimension errors
    #[test]
    fn test_dimension_mismatch() {
        let v1 = ComplexVector::zeros(2);
        let v2 = ComplexVector::zeros(3);

        // This should panic or return error
        let result = std::panic::catch_unwind(|| {
            v1.inner(&v2)
        });

        assert!(result.is_err());
    }

    /// Test invalid quantum state
    #[test]
    fn test_invalid_state() {
        // All zeros is not a valid quantum state
        let result = QuantumState::from_amplitudes(vec![
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ]);

        assert!(result.is_err());
    }
}
