//! Property-based tests using proptest

use proptest::prelude::*;
use ruvector_sparse_inference::*;

proptest! {
    #[test]
    fn sparse_output_finite(input in prop::collection::vec(-10.0f32..10.0, 512)) {
        let ffn = sparse::SparseFfn::new(512, 2048, sparse::ActivationType::Silu);
        let active: Vec<usize> = (0..1024).collect();

        let output = ffn.forward_sparse(&input, &active);

        prop_assert!(output.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn predictor_returns_valid_indices(
        input in prop::collection::vec(-1.0f32..1.0, 512)
    ) {
        let predictor = predictor::LowRankPredictor::new(512, 4096, 128, 0.1);
        let active = predictor.predict(&input);

        prop_assert!(active.iter().all(|&i| i < 4096));
        prop_assert!(active.len() <= 4096);
    }

    #[test]
    fn sparse_matches_dense_with_all_neurons(
        input in prop::collection::vec(-5.0f32..5.0, 512)
    ) {
        let ffn = sparse::SparseFfn::new(512, 2048, sparse::ActivationType::Silu);
        let all_neurons: Vec<usize> = (0..2048).collect();

        let dense = ffn.forward_dense(&input);
        let sparse = ffn.forward_sparse(&input, &all_neurons);

        // Allow small numerical differences
        for (d, s) in dense.iter().zip(sparse.iter()) {
            prop_assert!((d - s).abs() < 1e-4);
        }
    }

    #[test]
    fn quantization_preserves_order(
        mut values in prop::collection::vec(-100.0f32..100.0, 1..1000)
    ) {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let quantized = memory::quantization::QuantizedWeights::quantize_int8(&values);
        let dequantized = quantized.dequantize_row(0);

        // Dequantized values should maintain relative ordering (mostly)
        for i in 1..dequantized.len() {
            // Allow for some quantization error
            prop_assert!(
                dequantized[i] >= dequantized[i-1] - 0.5,
                "Order not preserved at index {}: {} vs {}",
                i, dequantized[i-1], dequantized[i]
            );
        }
    }

    #[test]
    fn predictor_top_k_returns_k_neurons(
        input in prop::collection::vec(-1.0f32..1.0, 512),
        k in 1usize..=2048
    ) {
        let mut predictor = predictor::LowRankPredictor::new(512, 4096, 128, 0.0);
        predictor.set_top_k(Some(k));

        let active = predictor.predict(&input);

        prop_assert_eq!(active.len(), k);
        prop_assert!(active.iter().all(|&i| i < 4096));
    }

    #[test]
    fn sparse_output_dimension_correct(
        input in prop::collection::vec(-10.0f32..10.0, 256..=1024),
        hidden_dim in 512usize..=4096
    ) {
        let input_dim = input.len();
        let ffn = sparse::SparseFfn::new(input_dim, hidden_dim, sparse::ActivationType::Relu);
        let active: Vec<usize> = (0..hidden_dim.min(100)).collect();

        let output = ffn.forward_sparse(&input, &active);

        prop_assert_eq!(output.len(), input_dim);
    }

    #[test]
    fn quantization_int4_roundtrip(
        values in prop::collection::vec(-50.0f32..50.0, 64..=512),
        group_size in prop::sample::select(vec![16, 32, 64, 128])
    ) {
        let quantized = memory::quantization::QuantizedWeights::quantize_int4(&values, group_size);
        let dequantized = quantized.dequantize_row(0);

        prop_assert_eq!(values.len(), dequantized.len());

        // Check approximate equality (int4 has lower precision)
        for (orig, deq) in values.iter().zip(dequantized.iter()) {
            prop_assert!(
                (orig - deq).abs() < 5.0,
                "Too much error: {} vs {}",
                orig, deq
            );
        }
    }

    #[test]
    fn sparse_inference_output_dimension(
        input in prop::collection::vec(-5.0f32..5.0, 512)
    ) {
        let model = model::LlamaModel::new(512, 2048, 4, 32000);
        let engine = SparseInferenceEngine::new_sparse(model, 0.3);

        let output = engine.infer(&input).unwrap();

        prop_assert_eq!(output.len(), 512);
        prop_assert!(output.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn swiglu_output_finite(
        input in prop::collection::vec(-10.0f32..10.0, 512)
    ) {
        let ffn = sparse::SwiGLUFfn::new(512, 2048);
        let active: Vec<usize> = (0..500).map(|i| i * 2).collect();

        let output = ffn.forward_sparse(&input, &active);

        prop_assert!(output.iter().all(|x| x.is_finite()));
        prop_assert_eq!(output.len(), 512);
    }

    #[test]
    fn calibration_handles_any_samples(
        num_samples in 1usize..=100
    ) {
        let mut predictor = predictor::LowRankPredictor::new(512, 4096, 128, 0.1);

        let samples: Vec<Vec<f32>> = (0..num_samples)
            .map(|_| vec![0.1; 512])
            .collect();

        let activations: Vec<Vec<usize>> = (0..num_samples)
            .map(|_| (0..100).collect())
            .collect();

        predictor.calibrate(&samples, &activations);

        // Should complete without panicking
        prop_assert!(true);
    }
}
