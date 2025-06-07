import pytest
import torch
import numpy as np
from unittest.mock import Mock, patch, MagicMock
from src.core.csi_processor import CSIProcessor
from src.core.phase_sanitizer import PhaseSanitizer
from src.models.modality_translation import ModalityTranslationNetwork
from src.models.densepose_head import DensePoseHead


class TestInferencePipeline:
    """Integration tests for inference pipeline following London School TDD principles"""
    
    @pytest.fixture
    def mock_csi_processor_config(self):
        """Configuration for CSI processor"""
        return {
            'window_size': 100,
            'overlap': 0.5,
            'filter_type': 'butterworth',
            'filter_order': 4,
            'cutoff_frequency': 50,
            'normalization': 'minmax',
            'outlier_threshold': 3.0
        }
    
    @pytest.fixture
    def mock_sanitizer_config(self):
        """Configuration for phase sanitizer"""
        return {
            'unwrap_method': 'numpy',
            'smoothing_window': 5,
            'outlier_threshold': 2.0,
            'interpolation_method': 'linear',
            'phase_correction': True
        }
    
    @pytest.fixture
    def mock_translation_config(self):
        """Configuration for modality translation network"""
        return {
            'input_channels': 6,
            'output_channels': 256,
            'hidden_channels': [64, 128, 256],
            'kernel_sizes': [7, 5, 3],
            'strides': [2, 2, 1],
            'dropout_rate': 0.1,
            'use_attention': True,
            'attention_heads': 8,
            'use_residual': True,
            'activation': 'relu',
            'normalization': 'batch'
        }
    
    @pytest.fixture
    def mock_densepose_config(self):
        """Configuration for DensePose head"""
        return {
            'input_channels': 256,
            'num_body_parts': 24,
            'num_uv_coordinates': 2,
            'hidden_channels': [128, 64],
            'kernel_size': 3,
            'padding': 1,
            'dropout_rate': 0.1,
            'use_deformable_conv': False,
            'use_fpn': True,
            'fpn_levels': [2, 3, 4, 5],
            'output_stride': 4
        }
    
    @pytest.fixture
    def inference_pipeline_components(self, mock_csi_processor_config, mock_sanitizer_config,
                                    mock_translation_config, mock_densepose_config):
        """Create inference pipeline components for testing"""
        csi_processor = CSIProcessor(mock_csi_processor_config)
        phase_sanitizer = PhaseSanitizer(mock_sanitizer_config)
        translation_network = ModalityTranslationNetwork(mock_translation_config)
        densepose_head = DensePoseHead(mock_densepose_config)
        
        return {
            'csi_processor': csi_processor,
            'phase_sanitizer': phase_sanitizer,
            'translation_network': translation_network,
            'densepose_head': densepose_head
        }
    
    @pytest.fixture
    def mock_raw_csi_input(self):
        """Generate mock raw CSI input data"""
        batch_size = 4
        antennas = 3
        subcarriers = 56
        time_samples = 100
        
        # Generate complex CSI data
        real_part = np.random.randn(batch_size, antennas, subcarriers, time_samples)
        imag_part = np.random.randn(batch_size, antennas, subcarriers, time_samples)
        
        return real_part + 1j * imag_part
    
    @pytest.fixture
    def mock_ground_truth_densepose(self):
        """Generate mock ground truth DensePose annotations"""
        batch_size = 4
        height = 224
        width = 224
        num_parts = 24
        
        # Segmentation masks
        seg_masks = torch.randint(0, num_parts + 1, (batch_size, height, width))
        
        # UV coordinates
        uv_coords = torch.randn(batch_size, 2, height, width)
        
        return {
            'segmentation': seg_masks,
            'uv_coordinates': uv_coords
        }
    
    def test_end_to_end_inference_pipeline_produces_valid_output(self, inference_pipeline_components, mock_raw_csi_input):
        """Test that end-to-end inference pipeline produces valid DensePose output"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Set models to evaluation mode
        translation_network.eval()
        densepose_head.eval()
        
        # Act - Run the complete inference pipeline
        with torch.no_grad():
            # 1. Process CSI data
            processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
            
            # 2. Sanitize phase information
            sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
            
            # 3. Translate CSI to visual features
            visual_features = translation_network(sanitized_csi)
            
            # 4. Generate DensePose predictions
            densepose_output = densepose_head(visual_features)
        
        # Assert
        assert densepose_output is not None
        assert isinstance(densepose_output, dict)
        assert 'segmentation' in densepose_output
        assert 'uv_coordinates' in densepose_output
        
        seg_output = densepose_output['segmentation']
        uv_output = densepose_output['uv_coordinates']
        
        # Check output shapes
        assert seg_output.shape[0] == mock_raw_csi_input.shape[0]  # Batch size preserved
        assert seg_output.shape[1] == 25  # 24 body parts + 1 background
        assert uv_output.shape[0] == mock_raw_csi_input.shape[0]   # Batch size preserved
        assert uv_output.shape[1] == 2    # U and V coordinates
        
        # Check output ranges
        assert torch.all(uv_output >= 0) and torch.all(uv_output <= 1)  # UV in [0, 1]
    
    def test_inference_pipeline_handles_different_batch_sizes(self, inference_pipeline_components):
        """Test that inference pipeline handles different batch sizes"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Different batch sizes
        small_batch = np.random.randn(1, 3, 56, 100) + 1j * np.random.randn(1, 3, 56, 100)
        large_batch = np.random.randn(8, 3, 56, 100) + 1j * np.random.randn(8, 3, 56, 100)
        
        # Set models to evaluation mode
        translation_network.eval()
        densepose_head.eval()
        
        # Act
        with torch.no_grad():
            # Small batch
            small_processed = csi_processor.process_csi_batch(small_batch)
            small_sanitized = phase_sanitizer.sanitize_phase_batch(small_processed)
            small_features = translation_network(small_sanitized)
            small_output = densepose_head(small_features)
            
            # Large batch
            large_processed = csi_processor.process_csi_batch(large_batch)
            large_sanitized = phase_sanitizer.sanitize_phase_batch(large_processed)
            large_features = translation_network(large_sanitized)
            large_output = densepose_head(large_features)
        
        # Assert
        assert small_output['segmentation'].shape[0] == 1
        assert large_output['segmentation'].shape[0] == 8
        assert small_output['uv_coordinates'].shape[0] == 1
        assert large_output['uv_coordinates'].shape[0] == 8
    
    def test_inference_pipeline_maintains_gradient_flow_during_training(self, inference_pipeline_components, 
                                                                       mock_raw_csi_input, mock_ground_truth_densepose):
        """Test that inference pipeline maintains gradient flow during training"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Set models to training mode
        translation_network.train()
        densepose_head.train()
        
        # Create optimizer
        optimizer = torch.optim.Adam(
            list(translation_network.parameters()) + list(densepose_head.parameters()),
            lr=0.001
        )
        
        # Act
        optimizer.zero_grad()
        
        # Forward pass
        processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
        sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
        visual_features = translation_network(sanitized_csi)
        densepose_output = densepose_head(visual_features)
        
        # Resize ground truth to match output
        seg_target = torch.nn.functional.interpolate(
            mock_ground_truth_densepose['segmentation'].float().unsqueeze(1),
            size=densepose_output['segmentation'].shape[2:],
            mode='nearest'
        ).squeeze(1).long()
        
        uv_target = torch.nn.functional.interpolate(
            mock_ground_truth_densepose['uv_coordinates'],
            size=densepose_output['uv_coordinates'].shape[2:],
            mode='bilinear',
            align_corners=False
        )
        
        # Compute loss
        loss = densepose_head.compute_total_loss(densepose_output, seg_target, uv_target)
        
        # Backward pass
        loss.backward()
        
        # Assert - Check that gradients are computed
        for param in translation_network.parameters():
            if param.requires_grad:
                assert param.grad is not None
                assert not torch.allclose(param.grad, torch.zeros_like(param.grad))
        
        for param in densepose_head.parameters():
            if param.requires_grad:
                assert param.grad is not None
                assert not torch.allclose(param.grad, torch.zeros_like(param.grad))
    
    def test_inference_pipeline_performance_benchmarking(self, inference_pipeline_components, mock_raw_csi_input):
        """Test inference pipeline performance for real-time requirements"""
        import time
        
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Set models to evaluation mode for inference
        translation_network.eval()
        densepose_head.eval()
        
        # Warm up (first inference is often slower)
        with torch.no_grad():
            processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
            visual_features = translation_network(sanitized_csi)
            _ = densepose_head(visual_features)
        
        # Act - Measure inference time
        start_time = time.time()
        
        with torch.no_grad():
            processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
            visual_features = translation_network(sanitized_csi)
            densepose_output = densepose_head(visual_features)
        
        end_time = time.time()
        inference_time = end_time - start_time
        
        # Assert - Should meet real-time requirements (< 50ms for batch of 4)
        assert inference_time < 0.05, f"Inference took {inference_time:.3f}s, expected < 0.05s"
    
    def test_inference_pipeline_handles_edge_cases(self, inference_pipeline_components):
        """Test that inference pipeline handles edge cases gracefully"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Edge cases
        zero_input = np.zeros((1, 3, 56, 100), dtype=complex)
        noisy_input = np.random.randn(1, 3, 56, 100) * 100 + 1j * np.random.randn(1, 3, 56, 100) * 100
        
        translation_network.eval()
        densepose_head.eval()
        
        # Act & Assert
        with torch.no_grad():
            # Zero input
            zero_processed = csi_processor.process_csi_batch(zero_input)
            zero_sanitized = phase_sanitizer.sanitize_phase_batch(zero_processed)
            zero_features = translation_network(zero_sanitized)
            zero_output = densepose_head(zero_features)
            
            assert not torch.isnan(zero_output['segmentation']).any()
            assert not torch.isnan(zero_output['uv_coordinates']).any()
            
            # Noisy input
            noisy_processed = csi_processor.process_csi_batch(noisy_input)
            noisy_sanitized = phase_sanitizer.sanitize_phase_batch(noisy_processed)
            noisy_features = translation_network(noisy_sanitized)
            noisy_output = densepose_head(noisy_features)
            
            assert not torch.isnan(noisy_output['segmentation']).any()
            assert not torch.isnan(noisy_output['uv_coordinates']).any()
    
    def test_inference_pipeline_memory_efficiency(self, inference_pipeline_components):
        """Test that inference pipeline is memory efficient"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Large batch to test memory usage
        large_input = np.random.randn(16, 3, 56, 100) + 1j * np.random.randn(16, 3, 56, 100)
        
        translation_network.eval()
        densepose_head.eval()
        
        # Act - Process in chunks to manage memory
        chunk_size = 4
        outputs = []
        
        with torch.no_grad():
            for i in range(0, large_input.shape[0], chunk_size):
                chunk = large_input[i:i+chunk_size]
                
                processed_chunk = csi_processor.process_csi_batch(chunk)
                sanitized_chunk = phase_sanitizer.sanitize_phase_batch(processed_chunk)
                feature_chunk = translation_network(sanitized_chunk)
                output_chunk = densepose_head(feature_chunk)
                
                outputs.append(output_chunk)
                
                # Clear intermediate tensors to free memory
                del processed_chunk, sanitized_chunk, feature_chunk
        
        # Assert
        assert len(outputs) == 4  # 16 samples / 4 chunk_size
        for output in outputs:
            assert output['segmentation'].shape[0] <= chunk_size
    
    def test_inference_pipeline_deterministic_output(self, inference_pipeline_components, mock_raw_csi_input):
        """Test that inference pipeline produces deterministic output in eval mode"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        # Set models to evaluation mode
        translation_network.eval()
        densepose_head.eval()
        
        # Act - Run inference twice
        with torch.no_grad():
            # First run
            processed_csi_1 = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi_1 = phase_sanitizer.sanitize_phase_batch(processed_csi_1)
            visual_features_1 = translation_network(sanitized_csi_1)
            output_1 = densepose_head(visual_features_1)
            
            # Second run
            processed_csi_2 = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi_2 = phase_sanitizer.sanitize_phase_batch(processed_csi_2)
            visual_features_2 = translation_network(sanitized_csi_2)
            output_2 = densepose_head(visual_features_2)
        
        # Assert - Outputs should be identical in eval mode
        assert torch.allclose(output_1['segmentation'], output_2['segmentation'], atol=1e-6)
        assert torch.allclose(output_1['uv_coordinates'], output_2['uv_coordinates'], atol=1e-6)
    
    def test_inference_pipeline_confidence_estimation(self, inference_pipeline_components, mock_raw_csi_input):
        """Test that inference pipeline provides confidence estimates"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        translation_network.eval()
        densepose_head.eval()
        
        # Act
        with torch.no_grad():
            processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
            visual_features = translation_network(sanitized_csi)
            densepose_output = densepose_head(visual_features)
            
            # Get confidence estimates
            confidence = densepose_head.get_prediction_confidence(densepose_output)
        
        # Assert
        assert 'segmentation_confidence' in confidence
        assert 'uv_confidence' in confidence
        
        seg_conf = confidence['segmentation_confidence']
        uv_conf = confidence['uv_confidence']
        
        assert seg_conf.shape[0] == mock_raw_csi_input.shape[0]
        assert uv_conf.shape[0] == mock_raw_csi_input.shape[0]
        assert torch.all(seg_conf >= 0) and torch.all(seg_conf <= 1)
        assert torch.all(uv_conf >= 0)
    
    def test_inference_pipeline_post_processing(self, inference_pipeline_components, mock_raw_csi_input):
        """Test that inference pipeline post-processes predictions correctly"""
        # Arrange
        csi_processor = inference_pipeline_components['csi_processor']
        phase_sanitizer = inference_pipeline_components['phase_sanitizer']
        translation_network = inference_pipeline_components['translation_network']
        densepose_head = inference_pipeline_components['densepose_head']
        
        translation_network.eval()
        densepose_head.eval()
        
        # Act
        with torch.no_grad():
            processed_csi = csi_processor.process_csi_batch(mock_raw_csi_input)
            sanitized_csi = phase_sanitizer.sanitize_phase_batch(processed_csi)
            visual_features = translation_network(sanitized_csi)
            raw_output = densepose_head(visual_features)
            
            # Post-process predictions
            processed_output = densepose_head.post_process_predictions(raw_output)
        
        # Assert
        assert 'body_parts' in processed_output
        assert 'uv_coordinates' in processed_output
        assert 'confidence_scores' in processed_output
        
        body_parts = processed_output['body_parts']
        assert body_parts.dtype == torch.long  # Class indices
        assert torch.all(body_parts >= 0) and torch.all(body_parts <= 24)  # Valid class range