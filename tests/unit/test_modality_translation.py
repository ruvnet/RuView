import pytest
import torch
import torch.nn as nn
import numpy as np
from unittest.mock import Mock, patch
from src.models.modality_translation import ModalityTranslationNetwork


class TestModalityTranslationNetwork:
    """Test suite for Modality Translation Network following London School TDD principles"""
    
    @pytest.fixture
    def mock_csi_input(self):
        """Generate synthetic CSI input tensor for testing"""
        # Batch size 2, 3 antennas, 56 subcarriers, 100 temporal samples
        return torch.randn(2, 3, 56, 100)
    
    @pytest.fixture
    def mock_config(self):
        """Configuration for modality translation network"""
        return {
            'input_channels': 3,
            'hidden_dim': 256,
            'output_dim': 512,
            'num_layers': 3,
            'dropout_rate': 0.1
        }
    
    @pytest.fixture
    def translation_network(self, mock_config):
        """Create modality translation network instance for testing"""
        return ModalityTranslationNetwork(mock_config)
    
    def test_network_initialization_creates_correct_architecture(self, mock_config):
        """Test that network initializes with correct architecture"""
        # Act
        network = ModalityTranslationNetwork(mock_config)
        
        # Assert
        assert network is not None
        assert isinstance(network, nn.Module)
        assert hasattr(network, 'encoder')
        assert hasattr(network, 'decoder')
        assert network.input_channels == mock_config['input_channels']
        assert network.hidden_dim == mock_config['hidden_dim']
        assert network.output_dim == mock_config['output_dim']
    
    def test_forward_pass_produces_correct_output_shape(self, translation_network, mock_csi_input):
        """Test that forward pass produces correctly shaped output"""
        # Act
        with torch.no_grad():
            output = translation_network(mock_csi_input)
        
        # Assert
        assert output is not None
        assert isinstance(output, torch.Tensor)
        assert output.shape[0] == mock_csi_input.shape[0]  # Batch size preserved
        assert output.shape[1] == translation_network.output_dim  # Correct output dimension
        assert len(output.shape) == 4  # Should maintain spatial dimensions
    
    def test_forward_pass_handles_different_batch_sizes(self, translation_network):
        """Test that network handles different batch sizes correctly"""
        # Arrange
        batch_sizes = [1, 4, 8]
        
        for batch_size in batch_sizes:
            input_tensor = torch.randn(batch_size, 3, 56, 100)
            
            # Act
            with torch.no_grad():
                output = translation_network(input_tensor)
            
            # Assert
            assert output.shape[0] == batch_size
            assert output.shape[1] == translation_network.output_dim
    
    def test_network_is_trainable(self, translation_network, mock_csi_input):
        """Test that network parameters are trainable"""
        # Arrange
        criterion = nn.MSELoss()
        
        # Act
        output = translation_network(mock_csi_input)
        # Create target with same shape as output
        target = torch.randn_like(output)
        loss = criterion(output, target)
        loss.backward()
        
        # Assert
        assert loss.item() > 0
        # Check that gradients are computed
        for param in translation_network.parameters():
            if param.requires_grad:
                assert param.grad is not None
    
    def test_network_handles_invalid_input_shape(self, translation_network):
        """Test that network handles invalid input shapes gracefully"""
        # Arrange
        invalid_input = torch.randn(2, 5, 56, 100)  # Wrong number of channels
        
        # Act & Assert
        with pytest.raises(RuntimeError):
            translation_network(invalid_input)
    
    def test_network_supports_evaluation_mode(self, translation_network, mock_csi_input):
        """Test that network supports evaluation mode"""
        # Act
        translation_network.eval()
        
        with torch.no_grad():
            output1 = translation_network(mock_csi_input)
            output2 = translation_network(mock_csi_input)
        
        # Assert - In eval mode with same input, outputs should be identical
        assert torch.allclose(output1, output2, atol=1e-6)
    
    def test_network_feature_extraction_quality(self, translation_network, mock_csi_input):
        """Test that network extracts meaningful features"""
        # Act
        with torch.no_grad():
            output = translation_network(mock_csi_input)
        
        # Assert
        # Features should have reasonable statistics
        assert not torch.isnan(output).any()
        assert not torch.isinf(output).any()
        assert output.std() > 0.01  # Features should have some variance
        assert output.std() < 10.0  # But not be too extreme