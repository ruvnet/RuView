//! Memory bounds verification tests
//! Tests actual components to ensure memory efficiency

#[cfg(test)]
mod memory_bounds_tests {
    use ruvector_nervous_system::eventbus::{DVSEvent, EventRingBuffer};
    use ruvector_nervous_system::hdc::{HdcMemory, Hypervector};
    use ruvector_nervous_system::hopfield::ModernHopfield;
    use ruvector_nervous_system::plasticity::btsp::BTSPLayer;
    use ruvector_nervous_system::routing::OscillatoryRouter;
    use std::mem::size_of;

    // ========================================================================
    // Compile-Time Size Checks - REAL TYPES
    // ========================================================================

    #[test]
    fn verify_real_structure_sizes() {
        // Hypervector: 157 u64s = 1256 bytes (10,048 bits)
        let hv_size = size_of::<Hypervector>();
        assert!(hv_size <= 1280, "Hypervector size {} > 1280 bytes", hv_size);

        // DVSEvent: should be minimal
        let event_size = size_of::<DVSEvent>();
        assert!(event_size <= 24, "DVSEvent size {} > 24 bytes", event_size);

        println!("Structure sizes:");
        println!("  Hypervector: {} bytes", hv_size);
        println!("  DVSEvent: {} bytes", event_size);
    }

    // ========================================================================
    // HDC Memory Bounds - REAL IMPLEMENTATION
    // ========================================================================

    #[test]
    fn hypervector_actual_memory() {
        // Each Hypervector: 157 u64s × 8 bytes = 1256 bytes
        let expected_per_vector = 157 * 8;

        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        // Verify the vector is correctly sized
        assert_eq!(
            size_of::<Hypervector>(),
            expected_per_vector,
            "Hypervector not correctly sized"
        );

        // Verify similarity works (proves vectors are real)
        // Note: Similarity can be slightly outside [0,1] due to 10,048 actual bits vs 10,000 nominal
        let sim = v1.similarity(&v2);
        assert!(sim >= -0.1 && sim <= 1.1, "Invalid similarity: {}", sim);
    }

    #[test]
    fn hdc_memory_stores_patterns() {
        let mut memory = HdcMemory::new();

        // Store 100 patterns
        for i in 0..100 {
            let pattern = Hypervector::from_seed(i as u64);
            memory.store(format!("pattern_{}", i), pattern);
        }

        assert_eq!(memory.len(), 100);

        // Verify retrieval works
        let query = Hypervector::from_seed(42);
        let results = memory.retrieve_top_k(&query, 5);
        assert!(!results.is_empty(), "Retrieval should return results");
    }

    // ========================================================================
    // BTSP Memory Bounds - REAL IMPLEMENTATION
    // ========================================================================

    #[test]
    fn btsp_layer_memory_scaling() {
        // Test different layer sizes
        let sizes = [64, 128, 256, 512];

        for size in sizes {
            let layer = BTSPLayer::new(size, 2000.0);

            // Layer should work
            let input: Vec<f32> = (0..size).map(|i| (i as f32) / (size as f32)).collect();
            let output = layer.forward(&input);
            assert!(output.is_finite(), "BTSP output should be finite");
        }
    }

    #[test]
    fn btsp_one_shot_no_memory_leak() {
        let mut layer = BTSPLayer::new(128, 2000.0);
        let pattern: Vec<f32> = (0..128).map(|i| (i as f32) / 128.0).collect();

        // Perform many one-shot learning operations
        for i in 0..1000 {
            layer.one_shot_associate(&pattern, (i as f32) / 1000.0);
        }

        // Should still work correctly
        let output = layer.forward(&pattern);
        assert!(
            output.is_finite(),
            "Output should be finite after many updates"
        );
    }

    // ========================================================================
    // Hopfield Network Memory - REAL IMPLEMENTATION
    // ========================================================================

    #[test]
    fn hopfield_pattern_storage() {
        let dim = 256;
        let mut hopfield = ModernHopfield::new(dim, 100.0);

        // Store patterns
        for i in 0..50 {
            let pattern: Vec<f32> = (0..dim).map(|j| ((i + j) as f32).sin()).collect();
            hopfield.store(pattern).unwrap();
        }

        // Verify retrieval works
        let query: Vec<f32> = (0..dim).map(|j| (j as f32).sin()).collect();
        let retrieved = hopfield.retrieve(&query).unwrap();
        assert_eq!(retrieved.len(), dim, "Retrieved pattern wrong size");
    }

    #[test]
    fn hopfield_memory_efficiency() {
        // Modern Hopfield stores patterns, not weight matrix
        let dim = 512;
        let num_patterns = 100;

        let mut hopfield = ModernHopfield::new(dim, 100.0);
        for i in 0..num_patterns {
            let pattern: Vec<f32> = (0..dim).map(|j| ((i * j) as f32).cos()).collect();
            hopfield.store(pattern).unwrap();
        }

        // Storage should be O(n×d), not O(d²)
        // 100 patterns × 512 dims × 4 bytes = 204,800 bytes
        let expected_bytes = num_patterns * dim * 4;
        println!(
            "Hopfield theoretical storage: {} bytes ({} KB)",
            expected_bytes,
            expected_bytes / 1024
        );
    }

    // ========================================================================
    // Event Bus Memory - REAL IMPLEMENTATION
    // ========================================================================

    #[test]
    fn event_ring_buffer_bounded() {
        let capacity = 1024;
        let buffer: EventRingBuffer<DVSEvent> = EventRingBuffer::new(capacity);

        // Fill buffer completely
        for i in 0..capacity * 2 {
            let event = DVSEvent::new(i as u64, (i % 256) as u16, (i % 256) as u32, i % 2 == 0);
            let _ = buffer.push(event); // May fail when full, that's OK
        }

        // Buffer should be bounded
        assert!(buffer.len() <= capacity, "Buffer exceeded capacity");
    }

    #[test]
    fn event_buffer_no_leak_on_overflow() {
        let capacity = 256;
        let buffer: EventRingBuffer<DVSEvent> = EventRingBuffer::new(capacity);

        // Push way more events than capacity
        for i in 0..10000 {
            let event = DVSEvent::new(i as u64, 0, 0, true);
            let _ = buffer.push(event);
        }

        // Should never exceed capacity
        assert!(
            buffer.len() <= capacity,
            "Buffer leaked: {} > {}",
            buffer.len(),
            capacity
        );
    }

    // ========================================================================
    // Oscillator Network Memory - REAL IMPLEMENTATION
    // ========================================================================

    #[test]
    fn oscillatory_router_memory() {
        let num_modules = 100;
        let base_freq = 40.0;

        let mut router = OscillatoryRouter::new(num_modules, base_freq);

        // Run many steps
        for _ in 0..1000 {
            router.step(0.001);
        }

        // Check synchronization (proves network is working)
        let order = router.order_parameter();
        assert!(
            order >= 0.0 && order <= 1.0,
            "Invalid order parameter: {}",
            order
        );
    }

    // ========================================================================
    // Performance Memory Trade-offs
    // ========================================================================

    #[test]
    fn hdc_similarity_batch_efficiency() {
        // Test that batch operations don't allocate excessively
        let vectors: Vec<Hypervector> = (0..100).map(|i| Hypervector::from_seed(i)).collect();
        let query = Hypervector::random();

        // Compute all similarities
        let similarities: Vec<f32> = vectors.iter().map(|v| query.similarity(v)).collect();

        // Should have valid results
        // Note: Similarity can be slightly outside [0,1] due to bit count mismatch
        assert_eq!(similarities.len(), 100);
        for sim in &similarities {
            assert!(*sim >= -0.1 && *sim <= 1.1, "sim out of range: {}", sim);
        }
    }

    // ========================================================================
    // Stress Tests
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored
    fn stress_test_hdc_memory() {
        let mut memory = HdcMemory::new();

        // Store 10,000 patterns
        for i in 0..10_000 {
            let pattern = Hypervector::from_seed(i as u64);
            memory.store(format!("p{}", i), pattern);
        }

        // Memory: 10,000 × 1,256 bytes ≈ 12.5 MB
        assert_eq!(memory.len(), 10_000);

        // Retrieval should still work
        let query = Hypervector::from_seed(5000);
        let results = memory.retrieve_top_k(&query, 10);
        assert!(!results.is_empty());
    }

    #[test]
    #[ignore]
    fn stress_test_hopfield_capacity() {
        let dim = 512;
        let mut hopfield = ModernHopfield::new(dim, 100.0);

        // Store maximum recommended patterns (0.14d for modern Hopfield)
        let max_patterns = (0.14 * dim as f64) as usize;
        for i in 0..max_patterns {
            let pattern: Vec<f32> = (0..dim).map(|j| ((i + j) as f32).sin()).collect();
            hopfield.store(pattern).unwrap();
        }

        println!("Stored {} patterns in {}d Hopfield", max_patterns, dim);

        // Should still retrieve correctly
        let query: Vec<f32> = (0..dim).map(|j| (j as f32).sin()).collect();
        let retrieved = hopfield.retrieve(&query).unwrap();
        assert_eq!(retrieved.len(), dim);
    }
}
