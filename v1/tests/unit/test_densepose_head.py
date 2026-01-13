import pytest
import torch
import torch.nn as nn
import numpy as np
from unittest.mock import Mock, patch
from src.models.densepose_head import DensePoseHead, DensePoseError


class TestDensePoseHead:
    """Test suite for DensePose Head following London School TDD principles"""
    
    @pytest.fixture
    def mock_config(self):
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
    def densepose_head(self, mock_config):
        """Create DensePose head instance for testing"""
        return DensePoseHead(mock_config)
    
    @pytest.fixture
    def mock_feature_input(self):
        """Generate mock feature input tensor"""
        batch_size = 2
        channels = 256
        height = 56
        width = 56
        return torch.randn(batch_size, channels, height, width)
    
    @pytest.fixture
    def mock_target_masks(self):
        """Generate mock target segmentation masks"""
        batch_size = 2
        num_parts = 24
        height = 224
        width = 224
        return torch.randint(0, num_parts + 1, (batch_size, height, width))
    
    @pytest.fixture
    def mock_target_uv(self):
        """Generate mock target UV coordinates"""
        batch_size = 2
        num_coords = 2
        height = 224
        width = 224
        return torch.randn(batch_size, num_coords, height, width)
    
    def test_head_initialization_creates_correct_architecture(self, mock_config):
        """Test that DensePose head initializes with correct architecture"""
        # Act
        head = DensePoseHead(mock_config)
        
        # Assert
        assert head is not None
        assert isinstance(head, nn.Module)
        assert head.input_channels == mock_config['input_channels']
        assert head.num_body_parts == mock_config['num_body_parts']
        assert head.num_uv_coordinates == mock_config['num_uv_coordinates']
        assert head.use_fpn == mock_config['use_fpn']
        assert hasattr(head, 'segmentation_head')
        assert hasattr(head, 'uv_regression_head')
        if mock_config['use_fpn']:
            assert hasattr(head, 'fpn')
    
    def test_forward_pass_produces_correct_output_format(self, densepose_head, mock_feature_input):
        """Test that forward pass produces correctly formatted output"""
        # Act
        output = densepose_head(mock_feature_input)
        
        # Assert
        assert output is not None
        assert isinstance(output, dict)
        assert 'segmentation' in output
        assert 'uv_coordinates' in output
        
        seg_output = output['segmentation']
        uv_output = output['uv_coordinates']
        
        assert isinstance(seg_output, torch.Tensor)
        assert isinstance(uv_output, torch.Tensor)
        assert seg_output.shape[0] == mock_feature_input.shape[0]  # Batch size preserved
        assert uv_output.shape[0] == mock_feature_input.shape[0]   # Batch size preserved
    
    def test_segmentation_head_produces_correct_shape(self, densepose_head, mock_feature_input):
        """Test that segmentation head produces correct output shape"""
        # Act
        output = densepose_head(mock_feature_input)
        seg_output = output['segmentation']
        
        # Assert
        expected_channels = densepose_head.num_body_parts + 1  # +1 for background
        assert seg_output.shape[1] == expected_channels
        assert seg_output.shape[2] >= mock_feature_input.shape[2]  # Height upsampled
        assert seg_output.shape[3] >= mock_feature_input.shape[3]  # Width upsampled
    
    def test_uv_regression_head_produces_correct_shape(self, densepose_head, mock_feature_input):
        """Test that UV regression head produces correct output shape"""
        # Act
        output = densepose_head(mock_feature_input)
        uv_output = output['uv_coordinates']
        
        # Assert
        assert uv_output.shape[1] == densepose_head.num_uv_coordinates
        assert uv_output.shape[2] >= mock_feature_input.shape[2]  # Height upsampled
        assert uv_output.shape[3] >= mock_feature_input.shape[3]  # Width upsampled
    
    def test_compute_segmentation_loss_measures_pixel_classification(self, densepose_head, mock_feature_input, mock_target_masks):
        """Test that compute_segmentation_loss measures pixel classification accuracy"""
        # Arrange
        output = densepose_head(mock_feature_input)
        seg_logits = output['segmentation']
        
        # Resize target to match output
        target_resized = torch.nn.functional.interpolate(
            mock_target_masks.float().unsqueeze(1), 
            size=seg_logits.shape[2:], 
            mode='nearest'
        ).squeeze(1).long()
        
        # Act
        loss = densepose_head.compute_segmentation_loss(seg_logits, target_resized)
        
        # Assert
        assert loss is not None
        assert isinstance(loss, torch.Tensor)
        assert loss.dim() == 0  # Scalar loss
        assert loss.item() >= 0  # Loss should be non-negative
    
    def test_compute_uv_loss_measures_coordinate_regression(self, densepose_head, mock_feature_input, mock_target_uv):
        """Test that compute_uv_loss measures UV coordinate regression accuracy"""
        # Arrange
        output = densepose_head(mock_feature_input)
        uv_pred = output['uv_coordinates']
        
        # Resize target to match output
        target_resized = torch.nn.functional.interpolate(
            mock_target_uv, 
            size=uv_pred.shape[2:], 
            mode='bilinear', 
            align_corners=False
        )
        
        # Act
        loss = densepose_head.compute_uv_loss(uv_pred, target_resized)
        
        # Assert
        assert loss is not None
        assert isinstance(loss, torch.Tensor)
        assert loss.dim() == 0  # Scalar loss
        assert loss.item() >= 0  # Loss should be non-negative
    
    def test_compute_total_loss_combines_segmentation_and_uv_losses(self, densepose_head, mock_feature_input, mock_target_masks, mock_target_uv):
        """Test that compute_total_loss combines segmentation and UV losses"""
        # Arrange
        output = densepose_head(mock_feature_input)
        
        # Resize targets to match outputs
        seg_target = torch.nn.functional.interpolate(
            mock_target_masks.float().unsqueeze(1), 
            size=output['segmentation'].shape[2:], 
            mode='nearest'
        ).squeeze(1).long()
        
        uv_target = torch.nn.functional.interpolate(
            mock_target_uv, 
            size=output['uv_coordinates'].shape[2:], 
            mode='bilinear', 
            align_corners=False
        )
        
        # Act
        total_loss = densepose_head.compute_total_loss(output, seg_target, uv_target)
        seg_loss = densepose_head.compute_segmentation_loss(output['segmentation'], seg_target)
        uv_loss = densepose_head.compute_uv_loss(output['uv_coordinates'], uv_target)
        
        # Assert
        assert total_loss is not None
        assert isinstance(total_loss, torch.Tensor)
        assert total_loss.item() > 0
        # Total loss should be combination of individual losses
        expected_total = seg_loss + uv_loss
        assert torch.allclose(total_loss, expected_total, atol=1e-6)
    
    def test_fpn_integration_enhances_multi_scale_features(self, mock_config, mock_feature_input):
        """Test that FPN integration enhances multi-scale feature processing"""
        # Arrange
        config_with_fpn = mock_config.copy()
        config_with_fpn['use_fpn'] = True
        
        config_without_fpn = mock_config.copy()
        config_without_fpn['use_fpn'] = False
        
        head_with_fpn = DensePoseHead(config_with_fpn)
        head_without_fpn = DensePoseHead(config_without_fpn)
        
        # Act
        output_with_fpn = head_with_fpn(mock_feature_input)
        output_without_fpn = head_without_fpn(mock_feature_input)
        
        # Assert
        assert output_with_fpn['segmentation'].shape == output_without_fpn['segmentation'].shape
        assert output_with_fpn['uv_coordinates'].shape == output_without_fpn['uv_coordinates'].shape
        # Outputs should be different due to FPN
        assert not torch.allclose(output_with_fpn['segmentation'], output_without_fpn['segmentation'], atol=1e-6)
    
    def test_get_prediction_confidence_provides_uncertainty_estimates(self, densepose_head, mock_feature_input):
        """Test that get_prediction_confidence provides uncertainty estimates"""
        # Arrange
        output = densepose_head(mock_feature_input)
        
        # Act
        confidence = densepose_head.get_prediction_confidence(output)
        
        # Assert
        assert confidence is not None
        assert isinstance(confidence, dict)
        assert 'segmentation_confidence' in confidence
        assert 'uv_confidence' in confidence
        
        seg_conf = confidence['segmentation_confidence']
        uv_conf = confidence['uv_confidence']
        
        assert isinstance(seg_conf, torch.Tensor)
        assert isinstance(uv_conf, torch.Tensor)
        assert seg_conf.shape[0] == mock_feature_input.shape[0]
        assert uv_conf.shape[0] == mock_feature_input.shape[0]
    
    def test_post_process_predictions_formats_output(self, densepose_head, mock_feature_input):
        """Test that post_process_predictions formats output correctly"""
        # Arrange
        raw_output = densepose_head(mock_feature_input)
        
        # Act
        processed = densepose_head.post_process_predictions(raw_output)
        
        # Assert
        assert processed is not None
        assert isinstance(processed, dict)
        assert 'body_parts' in processed
        assert 'uv_coordinates' in processed
        assert 'confidence_scores' in processed
    
    def test_training_mode_enables_dropout(self, densepose_head, mock_feature_input):
        """Test that training mode enables dropout for regularization"""
        # Arrange
        densepose_head.train()
        
        # Act
        output1 = densepose_head(mock_feature_input)
        output2 = densepose_head(mock_feature_input)
        
        # Assert - outputs should be different due to dropout
        assert not torch.allclose(output1['segmentation'], output2['segmentation'], atol=1e-6)
        assert not torch.allclose(output1['uv_coordinates'], output2['uv_coordinates'], atol=1e-6)
    
    def test_evaluation_mode_disables_dropout(self, densepose_head, mock_feature_input):
        """Test that evaluation mode disables dropout for consistent inference"""
        # Arrange
        densepose_head.eval()
        
        # Act
        output1 = densepose_head(mock_feature_input)
        output2 = densepose_head(mock_feature_input)
        
        # Assert - outputs should be identical in eval mode
        assert torch.allclose(output1['segmentation'], output2['segmentation'], atol=1e-6)
        assert torch.allclose(output1['uv_coordinates'], output2['uv_coordinates'], atol=1e-6)
    
    def test_head_validates_input_dimensions(self, densepose_head):
        """Test that head validates input dimensions"""
        # Arrange
        invalid_input = torch.randn(2, 128, 56, 56)  # Wrong number of channels
        
        # Act & Assert
        with pytest.raises(DensePoseError):
            densepose_head(invalid_input)
    
    def test_head_handles_different_input_sizes(self, densepose_head):
        """Test that head handles different input sizes"""
        # Arrange
        small_input = torch.randn(1, 256, 28, 28)
        large_input = torch.randn(1, 256, 112, 112)
        
        # Act
        small_output = densepose_head(small_input)
        large_output = densepose_head(large_input)
        
        # Assert
        assert small_output['segmentation'].shape[2:] != large_output['segmentation'].shape[2:]
        assert small_output['uv_coordinates'].shape[2:] != large_output['uv_coordinates'].shape[2:]
    
    def test_head_supports_gradient_computation(self, densepose_head, mock_feature_input, mock_target_masks, mock_target_uv):
        """Test that head supports gradient computation for training"""
        # Arrange
        densepose_head.train()
        optimizer = torch.optim.Adam(densepose_head.parameters(), lr=0.001)
        
        output = densepose_head(mock_feature_input)
        
        # Resize targets
        seg_target = torch.nn.functional.interpolate(
            mock_target_masks.float().unsqueeze(1), 
            size=output['segmentation'].shape[2:], 
            mode='nearest'
        ).squeeze(1).long()
        
        uv_target = torch.nn.functional.interpolate(
            mock_target_uv, 
            size=output['uv_coordinates'].shape[2:], 
            mode='bilinear', 
            align_corners=False
        )
        
        # Act
        loss = densepose_head.compute_total_loss(output, seg_target, uv_target)
        
        optimizer.zero_grad()
        loss.backward()
        
        # Assert
        for param in densepose_head.parameters():
            if param.requires_grad:
                assert param.grad is not None
                assert not torch.allclose(param.grad, torch.zeros_like(param.grad))
    
    def test_head_configuration_validation(self):
        """Test that head validates configuration parameters"""
        # Arrange
        invalid_config = {
            'input_channels': 0,  # Invalid
            'num_body_parts': -1,  # Invalid
            'num_uv_coordinates': 2
        }
        
        # Act & Assert
        with pytest.raises(ValueError):
            DensePoseHead(invalid_config)
    
    def test_save_and_load_model_state(self, densepose_head, mock_feature_input):
        """Test that model state can be saved and loaded"""
        # Arrange
        original_output = densepose_head(mock_feature_input)
        
        # Act - Save state
        state_dict = densepose_head.state_dict()
        
        # Create new head and load state
        new_head = DensePoseHead(densepose_head.config)
        new_head.load_state_dict(state_dict)
        new_output = new_head(mock_feature_input)
        
        # Assert
        assert torch.allclose(original_output['segmentation'], new_output['segmentation'], atol=1e-6)
        assert torch.allclose(original_output['uv_coordinates'], new_output['uv_coordinates'], atol=1e-6)