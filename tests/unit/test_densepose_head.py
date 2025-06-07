import pytest
import torch
import torch.nn as nn
import numpy as np
from unittest.mock import Mock, patch
from src.models.densepose_head import DensePoseHead


class TestDensePoseHead:
    """Test suite for DensePose Head following London School TDD principles"""
    
    @pytest.fixture
    def mock_feature_input(self):
        """Generate synthetic feature input tensor for testing"""
        # Batch size 2, 512 channels, 56 height, 100 width (from modality translation)
        return torch.randn(2, 512, 56, 100)
    
    @pytest.fixture
    def mock_config(self):
        """Configuration for DensePose head"""
        return {
            'input_channels': 512,
            'num_body_parts': 24,  # Standard DensePose body parts
            'num_uv_coordinates': 2,  # U and V coordinates
            'hidden_dim': 256,
            'dropout_rate': 0.1
        }
    
    @pytest.fixture
    def densepose_head(self, mock_config):
        """Create DensePose head instance for testing"""
        return DensePoseHead(mock_config)
    
    def test_head_initialization_creates_correct_architecture(self, mock_config):
        """Test that DensePose head initializes with correct architecture"""
        # Act
        head = DensePoseHead(mock_config)
        
        # Assert
        assert head is not None
        assert isinstance(head, nn.Module)
        assert hasattr(head, 'segmentation_head')
        assert hasattr(head, 'uv_regression_head')
        assert head.input_channels == mock_config['input_channels']
        assert head.num_body_parts == mock_config['num_body_parts']
        assert head.num_uv_coordinates == mock_config['num_uv_coordinates']
    
    def test_forward_pass_produces_correct_output_shapes(self, densepose_head, mock_feature_input):
        """Test that forward pass produces correctly shaped outputs"""
        # Act
        with torch.no_grad():
            segmentation, uv_coords = densepose_head(mock_feature_input)
        
        # Assert
        assert segmentation is not None
        assert uv_coords is not None
        assert isinstance(segmentation, torch.Tensor)
        assert isinstance(uv_coords, torch.Tensor)
        
        # Check segmentation output shape
        assert segmentation.shape[0] == mock_feature_input.shape[0]  # Batch size preserved
        assert segmentation.shape[1] == densepose_head.num_body_parts  # Correct number of body parts
        assert segmentation.shape[2:] == mock_feature_input.shape[2:]  # Spatial dimensions preserved
        
        # Check UV coordinates output shape
        assert uv_coords.shape[0] == mock_feature_input.shape[0]  # Batch size preserved
        assert uv_coords.shape[1] == densepose_head.num_uv_coordinates  # U and V coordinates
        assert uv_coords.shape[2:] == mock_feature_input.shape[2:]  # Spatial dimensions preserved
    
    def test_segmentation_output_has_valid_probabilities(self, densepose_head, mock_feature_input):
        """Test that segmentation output has valid probability distributions"""
        # Act
        with torch.no_grad():
            segmentation, _ = densepose_head(mock_feature_input)
        
        # Assert
        # After softmax, values should be between 0 and 1
        assert torch.all(segmentation >= 0.0)
        assert torch.all(segmentation <= 1.0)
        
        # Sum across body parts dimension should be approximately 1
        part_sums = torch.sum(segmentation, dim=1)
        assert torch.allclose(part_sums, torch.ones_like(part_sums), atol=1e-5)
    
    def test_uv_coordinates_output_in_valid_range(self, densepose_head, mock_feature_input):
        """Test that UV coordinates are in valid range [0, 1]"""
        # Act
        with torch.no_grad():
            _, uv_coords = densepose_head(mock_feature_input)
        
        # Assert
        # UV coordinates should be in range [0, 1] after sigmoid
        assert torch.all(uv_coords >= 0.0)
        assert torch.all(uv_coords <= 1.0)
    
    def test_head_handles_different_batch_sizes(self, densepose_head):
        """Test that head handles different batch sizes correctly"""
        # Arrange
        batch_sizes = [1, 4, 8]
        
        for batch_size in batch_sizes:
            input_tensor = torch.randn(batch_size, 512, 56, 100)
            
            # Act
            with torch.no_grad():
                segmentation, uv_coords = densepose_head(input_tensor)
            
            # Assert
            assert segmentation.shape[0] == batch_size
            assert uv_coords.shape[0] == batch_size
    
    def test_head_is_trainable(self, densepose_head, mock_feature_input):
        """Test that head parameters are trainable"""
        # Arrange
        seg_criterion = nn.CrossEntropyLoss()
        uv_criterion = nn.MSELoss()
        
        # Create targets with correct shapes
        seg_target = torch.randint(0, 24, (2, 56, 100))  # Class indices for segmentation
        uv_target = torch.rand(2, 2, 56, 100)  # UV coordinates target
        
        # Act
        segmentation, uv_coords = densepose_head(mock_feature_input)
        seg_loss = seg_criterion(segmentation, seg_target)
        uv_loss = uv_criterion(uv_coords, uv_target)
        total_loss = seg_loss + uv_loss
        total_loss.backward()
        
        # Assert
        assert total_loss.item() > 0
        # Check that gradients are computed
        for param in densepose_head.parameters():
            if param.requires_grad:
                assert param.grad is not None
    
    def test_head_handles_invalid_input_shape(self, densepose_head):
        """Test that head handles invalid input shapes gracefully"""
        # Arrange
        invalid_input = torch.randn(2, 256, 56, 100)  # Wrong number of channels
        
        # Act & Assert
        with pytest.raises(RuntimeError):
            densepose_head(invalid_input)
    
    def test_head_supports_evaluation_mode(self, densepose_head, mock_feature_input):
        """Test that head supports evaluation mode"""
        # Act
        densepose_head.eval()
        
        with torch.no_grad():
            seg1, uv1 = densepose_head(mock_feature_input)
            seg2, uv2 = densepose_head(mock_feature_input)
        
        # Assert - In eval mode with same input, outputs should be identical
        assert torch.allclose(seg1, seg2, atol=1e-6)
        assert torch.allclose(uv1, uv2, atol=1e-6)
    
    def test_head_output_quality(self, densepose_head, mock_feature_input):
        """Test that head produces meaningful outputs"""
        # Act
        with torch.no_grad():
            segmentation, uv_coords = densepose_head(mock_feature_input)
        
        # Assert
        # Outputs should not contain NaN or Inf values
        assert not torch.isnan(segmentation).any()
        assert not torch.isinf(segmentation).any()
        assert not torch.isnan(uv_coords).any()
        assert not torch.isinf(uv_coords).any()
        
        # Outputs should have reasonable variance (not all zeros or ones)
        assert segmentation.std() > 0.01
        assert uv_coords.std() > 0.01