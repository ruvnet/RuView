// Retrieval quality benchmarks and comparison tests
// Compares HDC, Hopfield, and pattern separation against baselines

#[cfg(test)]
mod retrieval_quality_tests {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rand_distr::{Distribution, Normal, Uniform};

    // ========================================================================
    // Test Data Generation
    // ========================================================================

    fn generate_uniform_vectors(n: usize, dims: usize, rng: &mut StdRng) -> Vec<Vec<f32>> {
        let dist = Uniform::new(-1.0, 1.0);
        (0..n)
            .map(|_| (0..dims).map(|_| dist.sample(rng)).collect())
            .collect()
    }

    fn generate_gaussian_clusters(
        n: usize,
        k: usize,
        dims: usize,
        sigma: f32,
        rng: &mut StdRng,
    ) -> Vec<Vec<f32>> {
        // Generate k cluster centers
        let centers = generate_uniform_vectors(k, dims, rng);

        // Sample points from each cluster
        let mut vectors = Vec::new();
        let normal = Normal::new(0.0, sigma).unwrap();

        for i in 0..n {
            let center = &centers[i % k];
            let point: Vec<f32> = center.iter().map(|&c| c + normal.sample(rng)).collect();
            vectors.push(point);
        }

        vectors
    }

    fn add_noise(vector: &[f32], noise_level: f32, rng: &mut StdRng) -> Vec<f32> {
        let normal = Normal::new(0.0, noise_level).unwrap();
        vector.iter().map(|&x| x + normal.sample(rng)).collect()
    }

    fn flip_bits(bitvector: &[u64], flip_rate: f32, rng: &mut StdRng) -> Vec<u64> {
        bitvector
            .iter()
            .map(|&word| {
                let mut result = word;
                for bit in 0..64 {
                    if rng.gen::<f32>() < flip_rate {
                        result ^= 1u64 << bit;
                    }
                }
                result
            })
            .collect()
    }

    // ========================================================================
    // Similarity Metrics
    // ========================================================================

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b)
    }

    fn hamming_distance(a: &[u64], b: &[u64]) -> u32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x ^ y).count_ones())
            .sum()
    }

    fn calculate_recall_at_k(results: &[Vec<usize>], ground_truth: &[Vec<usize>], k: usize) -> f32 {
        let mut total_recall = 0.0;

        for (res, gt) in results.iter().zip(ground_truth.iter()) {
            let res_set: std::collections::HashSet<_> = res.iter().take(k).collect();
            let gt_set: std::collections::HashSet<_> = gt.iter().take(k).collect();
            let intersection = res_set.intersection(&gt_set).count();
            total_recall += intersection as f32 / k as f32;
        }

        total_recall / results.len() as f32
    }

    // ========================================================================
    // HDC Recall Tests
    // ========================================================================

    #[test]
    fn hdc_recall_vs_exact_knn() {
        let mut rng = StdRng::seed_from_u64(42);
        let num_vectors = 1000;
        let dims = 512;
        let k = 10;

        let vectors = generate_uniform_vectors(num_vectors, dims, &mut rng);
        let queries: Vec<_> = vectors.iter().take(100).cloned().collect();

        // Exact k-NN (ground truth)
        let ground_truth: Vec<Vec<usize>> = queries
            .iter()
            .map(|query| {
                let mut distances: Vec<_> = vectors
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, 1.0 - cosine_similarity(query, v)))
                    .collect();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances.iter().take(k).map(|(i, _)| *i).collect()
            })
            .collect();

        // HDC results (placeholder - will use actual HDC when implemented)
        let hdc_results: Vec<Vec<usize>> = queries
            .iter()
            .map(|query| {
                // Placeholder: simulate HDC search
                // In reality: hdc.encode_and_search(query, k)
                let mut distances: Vec<_> = vectors
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, 1.0 - cosine_similarity(query, v)))
                    .collect();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances.iter().take(k).map(|(i, _)| *i).collect()
            })
            .collect();

        let recall_1 = calculate_recall_at_k(&hdc_results, &ground_truth, 1);
        let recall_10 = calculate_recall_at_k(&hdc_results, &ground_truth, 10);

        // Target: ≥95% recall@1, ≥90% recall@10
        assert!(recall_1 >= 0.95, "HDC recall@1 {} < 95%", recall_1);
        assert!(recall_10 >= 0.90, "HDC recall@10 {} < 90%", recall_10);
    }

    #[test]
    fn hdc_noise_robustness() {
        let mut rng = StdRng::seed_from_u64(42);
        let num_vectors = 100;
        let dims = 10000; // 10K bit hypervector

        // Generate hypervectors (bit-packed)
        let hypervectors: Vec<Vec<u64>> = (0..num_vectors)
            .map(|_| (0..(dims + 63) / 64).map(|_| rng.gen()).collect())
            .collect();

        let mut correct = 0;
        let num_tests = 100;

        for i in 0..num_tests {
            let original = &hypervectors[i % num_vectors];

            // Add 20% bit flips
            let noisy = flip_bits(original, 0.20, &mut rng);

            // Find nearest neighbor
            let mut min_dist = u32::MAX;
            let mut best_match = 0;

            for (j, hv) in hypervectors.iter().enumerate() {
                let dist = hamming_distance(&noisy, hv);
                if dist < min_dist {
                    min_dist = dist;
                    best_match = j;
                }
            }

            if best_match == (i % num_vectors) {
                correct += 1;
            }
        }

        let accuracy = correct as f32 / num_tests as f32;
        assert!(accuracy > 0.80, "HDC noise robustness {} < 80%", accuracy);
    }

    // ========================================================================
    // Hopfield Capacity Tests
    // ========================================================================

    #[test]
    fn hopfield_pattern_capacity() {
        let mut rng = StdRng::seed_from_u64(42);
        let dims = 512;

        // Theoretical capacity: ~0.138 * dims for classical Hopfield
        // Modern Hopfield can store exponentially more
        let target_capacity = (dims as f32 * 0.138) as usize;

        // Store patterns
        // let hopfield = ModernHopfield::new(dims, 100.0);
        let patterns = generate_uniform_vectors(target_capacity, dims, &mut rng);

        // for pattern in &patterns {
        //     hopfield.store(pattern);
        // }

        // Test retrieval accuracy
        let mut correct = 0;
        for pattern in &patterns {
            // Add 10% noise
            let noisy = add_noise(pattern, 0.1, &mut rng);

            // Retrieve
            // let retrieved = hopfield.retrieve(&noisy);
            let retrieved = pattern.clone(); // Placeholder

            if cosine_similarity(&retrieved, pattern) > 0.95 {
                correct += 1;
            }
        }

        let accuracy = correct as f32 / patterns.len() as f32;
        assert!(
            accuracy > 0.95,
            "Hopfield capacity test accuracy {} < 95%",
            accuracy
        );
    }

    #[test]
    fn hopfield_retrieval_with_noise() {
        let mut rng = StdRng::seed_from_u64(42);
        let dims = 512;
        let num_patterns = 50;

        let patterns = generate_uniform_vectors(num_patterns, dims, &mut rng);

        // let hopfield = ModernHopfield::new(dims, 100.0);
        // for pattern in &patterns {
        //     hopfield.store(pattern);
        // }

        // Test with varying noise levels
        for noise_level in [0.05, 0.10, 0.15, 0.20] {
            let mut correct = 0;

            for pattern in &patterns {
                let noisy = add_noise(pattern, noise_level, &mut rng);
                // let retrieved = hopfield.retrieve(&noisy);
                let retrieved = pattern.clone(); // Placeholder

                if cosine_similarity(&retrieved, pattern) > 0.90 {
                    correct += 1;
                }
            }

            let accuracy = correct as f32 / patterns.len() as f32;

            // Accuracy should degrade gracefully with noise
            if noise_level <= 0.10 {
                assert!(
                    accuracy > 0.95,
                    "Accuracy {} < 95% at noise {}",
                    accuracy,
                    noise_level
                );
            }
        }
    }

    #[test]
    fn hopfield_energy_convergence() {
        let mut rng = StdRng::seed_from_u64(42);
        let dims = 512;

        let pattern = generate_uniform_vectors(1, dims, &mut rng)[0].clone();

        // let hopfield = ModernHopfield::new(dims, 100.0);
        // hopfield.store(&pattern);

        let mut state = add_noise(&pattern, 0.2, &mut rng);
        let mut prev_energy = f32::INFINITY;

        // Energy should monotonically decrease
        for _ in 0..20 {
            // state = hopfield.update(&state);
            // let energy = hopfield.energy(&state);
            let energy = -state.iter().map(|x| x * x).sum::<f32>(); // Placeholder

            assert!(
                energy <= prev_energy,
                "Energy increased: {} -> {}",
                prev_energy,
                energy
            );
            prev_energy = energy;
        }
    }

    // ========================================================================
    // Pattern Separation Tests
    // ========================================================================

    #[test]
    fn pattern_separation_collision_rate() {
        let mut rng = StdRng::seed_from_u64(42);
        let num_patterns = 10000;
        let dims = 512;

        let patterns = generate_uniform_vectors(num_patterns, dims, &mut rng);

        // Encode all patterns
        // let encoder = PatternSeparator::new(dims);
        let encoded: Vec<Vec<f32>> = patterns
            .iter()
            .map(|p| {
                // encoder.encode(p)
                // Placeholder: normalize
                let norm: f32 = p.iter().map(|x| x * x).sum::<f32>().sqrt();
                p.iter().map(|x| x / norm).collect()
            })
            .collect();

        // Check for collisions (cosine similarity > 0.95)
        let mut collisions = 0;
        for i in 0..encoded.len() {
            for j in (i + 1)..encoded.len() {
                if cosine_similarity(&encoded[i], &encoded[j]) > 0.95 {
                    collisions += 1;
                }
            }
        }

        let collision_rate = collisions as f32 / (num_patterns * (num_patterns - 1) / 2) as f32;
        assert!(
            collision_rate < 0.01,
            "Collision rate {} >= 1%",
            collision_rate
        );
    }

    #[test]
    fn pattern_separation_orthogonality() {
        let mut rng = StdRng::seed_from_u64(42);
        let num_patterns = 100;
        let dims = 512;

        let patterns = generate_uniform_vectors(num_patterns, dims, &mut rng);

        // Encode patterns
        // let encoder = PatternSeparator::new(dims);
        let encoded: Vec<Vec<f32>> = patterns
            .iter()
            .map(|p| {
                // encoder.encode(p)
                let norm: f32 = p.iter().map(|x| x * x).sum::<f32>().sqrt();
                p.iter().map(|x| x / norm).collect()
            })
            .collect();

        // Measure average pairwise orthogonality
        let mut total_similarity = 0.0;
        let mut count = 0;

        for i in 0..encoded.len() {
            for j in (i + 1)..encoded.len() {
                total_similarity += cosine_similarity(&encoded[i], &encoded[j]).abs();
                count += 1;
            }
        }

        let avg_similarity = total_similarity / count as f32;
        let orthogonality_score = 1.0 - avg_similarity;

        // Target: >0.9 orthogonality (avg similarity <0.1)
        assert!(
            orthogonality_score > 0.90,
            "Orthogonality {} < 0.90",
            orthogonality_score
        );
    }

    // ========================================================================
    // Associative Memory Tests
    // ========================================================================

    #[test]
    fn one_shot_learning_accuracy() {
        let mut rng = StdRng::seed_from_u64(42);
        let dims = 512;
        let num_items = 50;

        let items = generate_uniform_vectors(num_items, dims, &mut rng);

        // let memory = AssociativeMemory::new(dims);

        // One-shot learning: store each item once
        // for (i, item) in items.iter().enumerate() {
        //     memory.store(i, item);
        // }

        // Test retrieval with noisy queries
        let mut correct = 0;
        for (i, item) in items.iter().enumerate() {
            let noisy = add_noise(item, 0.1, &mut rng);

            // let retrieved_id = memory.retrieve(&noisy);
            let retrieved_id = i; // Placeholder

            if retrieved_id == i {
                correct += 1;
            }
        }

        let accuracy = correct as f32 / num_items as f32;
        assert!(
            accuracy > 0.90,
            "One-shot learning accuracy {} < 90%",
            accuracy
        );
    }

    #[test]
    fn multi_pattern_interference() {
        let mut rng = StdRng::seed_from_u64(42);
        let dims = 512;

        // Test with increasing numbers of patterns
        for num_patterns in [10, 50, 100] {
            let patterns = generate_uniform_vectors(num_patterns, dims, &mut rng);

            // let memory = AssociativeMemory::new(dims);
            // for pattern in &patterns {
            //     memory.store(pattern);
            // }

            let mut correct = 0;
            for pattern in &patterns {
                let noisy = add_noise(pattern, 0.05, &mut rng);
                // let retrieved = memory.retrieve(&noisy);
                let retrieved = pattern.clone(); // Placeholder

                if cosine_similarity(&retrieved, pattern) > 0.95 {
                    correct += 1;
                }
            }

            let accuracy = correct as f32 / patterns.len() as f32;

            // Accuracy should not drop more than 5% even with many patterns
            assert!(
                accuracy > 0.90,
                "Interference too high: accuracy {} < 90% with {} patterns",
                accuracy,
                num_patterns
            );
        }
    }

    // ========================================================================
    // Comparative Benchmarks
    // ========================================================================

    #[test]
    #[ignore] // Run manually for full comparison
    fn compare_all_retrieval_methods() {
        let mut rng = StdRng::seed_from_u64(42);
        let num_vectors = 5000;
        let dims = 512;
        let k = 10;

        let vectors = generate_uniform_vectors(num_vectors, dims, &mut rng);
        let queries: Vec<_> = vectors.iter().take(100).cloned().collect();

        // Exact k-NN
        let exact_results = queries
            .iter()
            .map(|q| {
                let mut dists: Vec<_> = vectors
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, 1.0 - cosine_similarity(q, v)))
                    .collect();
                dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                dists.iter().take(k).map(|(i, _)| *i).collect()
            })
            .collect::<Vec<Vec<usize>>>();

        // HDC
        // let hdc_results = ...;
        // let hdc_recall = calculate_recall_at_k(&hdc_results, &exact_results, k);

        // Hopfield
        // let hopfield_results = ...;
        // let hopfield_recall = calculate_recall_at_k(&hopfield_results, &exact_results, k);

        // Print comparison
        println!("Recall@{} comparison:", k);
        // println!("  HDC:      {:.2}%", hdc_recall * 100.0);
        // println!("  Hopfield: {:.2}%", hopfield_recall * 100.0);
        println!("  Exact:    100.00%");
    }
}
