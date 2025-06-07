import pytest
import torch
import torch.nn as nn
import numpy as np
from unittest.mock import Mock, patch
from src.models.modality_translation import ModalityTranslationNetwork, ModalityTranslationError


class TestModalityTranslationNetwork:
    """Test suite for Modality Translation Network following London School TDD principles"""
    
    @pytest.fixture
    def mock_config(self):
        """Configuration for modality translation network"""
        return {
            'input_channels': 6,  # Real and imaginary parts for 3 antennas
            'hidden_channels': [64, 128, 256],
            'output_channels': 256,
            'kernel_size': 3,
            'stride': 1,
            'padding': 1,
            'dropout_rate': 0.1,
            'activation': 'relu',
            'normalization': 'batch',
            'use_attention': True,
            'attention_heads': 8
        }
    
    @pytest.fixture
    def translation_network(self, mock_config):
        """Create modality translation network instance for testing"""
        return ModalityTranslationNetwork(mock_config)
    
    @pytest.fixture
    def mock_csi_input(self):
        """Generate mock CSI input tensor"""
        batch_size = 4
        channels = 6  # Real and imaginary parts for 3 antennas
        height = 56  # Number of subcarriers
        width = 100  # Time samples
        return torch.randn(batch_size, channels, height, width)
    
    @pytest.fixture
    def mock_target_features(self):
        """Generate mock target feature tensor for training"""
        batch_size = 4
        feature_dim = 256
        spatial_height = 56
        spatial_width = 100
        return torch.randn(batch_size, feature_dim, spatial_height, spatial_width)
    
    def test_network_initialization_creates_correct_architecture(self, mock_config):
        """Test that modality translation network initializes with correct architecture"""
        # Act
        network = ModalityTranslationNetwork(mock_config)
        
        # Assert
        assert network is not None
        assert isinstance(network, nn.Module)
        assert network.input_channels == mock_config['input_channels']
        assert network.output_channels == mock_config['output_channels']
        assert network.use_attention == mock_config['use_attention']
        assert hasattr(network, 'encoder')
        assert hasattr(network, 'decoder')
        if mock_config['use_attention']:
            assert hasattr(network, 'attention')
    
    def test_forward_pass_produces_correct_output_shape(self, translation_network, mock_csi_input):
        """Test that forward pass produces correctly shaped output"""
        # Act
        output = translation_network(mock_csi_input)
        
        # Assert
        assert output is not None
        assert isinstance(output, torch.Tensor)
        assert output.shape[0] == mock_csi_input.shape[0]  # Batch size preserved
        assert output.shape[1] == translation_network.output_channels  # Correct output channels
        assert output.shape[2] == mock_csi_input.shape[2]  # Spatial height preserved
        assert output.shape[3] == mock_csi_input.shape[3]  # Spatial width preserved
    
    def test_forward_pass_handles_different_input_sizes(self, translation_network):
        """Test that forward pass handles different input sizes"""
        # Arrange
        small_input = torch.randn(2, 6, 28, 50)
        large_input = torch.randn(8, 6, 112, 200)
        
        # Act
        small_output = translation_network(small_input)
        large_output = translation_network(large_input)
        
        # Assert
        assert small_output.shape == (2, 256, 28, 50)
        assert large_output.shape == (8, 256, 112, 200)
    
    def test_encoder_extracts_hierarchical_features(self, translation_network, mock_csi_input):
        """Test that encoder extracts hierarchical features"""
        # Act
        features = translation_network.encode(mock_csi_input)
        
        # Assert
        assert features is not None
        assert isinstance(features, list)
        assert len(features) == len(translation_network.encoder)
        
        # Check feature map sizes decrease with depth
        for i in range(1, len(features)):
            assert features[i].shape[2] <= features[i-1].shape[2]  # Height decreases or stays same
            assert features[i].shape[3] <= features[i-1].shape[3]  # Width decreases or stays same
    
    def test_decoder_reconstructs_target_features(self, translation_network, mock_csi_input):
        """Test that decoder reconstructs target feature representation"""
        # Arrange
        encoded_features = translation_network.encode(mock_csi_input)
        
        # Act
        decoded_output = translation_network.decode(encoded_features)
        
        # Assert
        assert decoded_output is not None
        assert isinstance(decoded_output, torch.Tensor)
        assert decoded_output.shape[1] == translation_network.output_channels
        assert decoded_output.shape[2:] == mock_csi_input.shape[2:]
    
    def test_attention_mechanism_enhances_features(self, mock_config, mock_csi_input):
        """Test that attention mechanism enhances feature representation"""
        # Arrange
        config_with_attention = mock_config.copy()
        config_with_attention['use_attention'] = True
        
        config_without_attention = mock_config.copy()
        config_without_attention['use_attention'] = False
        
        network_with_attention = ModalityTranslationNetwork(config_with_attention)
        network_without_attention = ModalityTranslationNetwork(config_without_attention)
        
        # Act
        output_with_attention = network_with_attention(mock_csi_input)
        output_without_attention = network_without_attention(mock_csi_input)
        
        # Assert
        assert output_with_attention.shape == output_without_attention.shape
        # Outputs should be different due to attention mechanism
        assert not torch.allclose(output_with_attention, output_without_attention, atol=1e-6)
    
    def test_training_mode_enables_dropout(self, translation_network, mock_csi_input):
        """Test that training mode enables dropout for regularization"""
        # Arrange
        translation_network.train()
        
        # Act
        output1 = translation_network(mock_csi_input)
        output2 = translation_network(mock_csi_input)
        
        # Assert - outputs should be different due to dropout
        assert not torch.allclose(output1, output2, atol=1e-6)
    
    def test_evaluation_mode_disables_dropout(self, translation_network, mock_csi_input):
        """Test that evaluation mode disables dropout for consistent inference"""
        # Arrange
        translation_network.eval()
        
        # Act
        output1 = translation_network(mock_csi_input)
        output2 = translation_network(mock_csi_input)
        
        # Assert - outputs should be identical in eval mode
        assert torch.allclose(output1, output2, atol=1e-6)
    
    def test_compute_translation_loss_measures_feature_alignment(self, translation_network, mock_csi_input, mock_target_features):
        """Test that compute_translation_loss measures feature alignment"""
        # Arrange
        predicted_features = translation_network(mock_csi_input)
        
        # Act
        loss = translation_network.compute_translation_loss(predicted_features, mock_target_features)
        
        # Assert
        assert loss is not None
        assert isinstance(loss, torch.Tensor)
        assert loss.dim() == 0  # Scalar loss
        assert loss.item() >= 0  # Loss should be non-negative
    
    def test_compute_translation_loss_handles_different_loss_types(self, translation_network, mock_csi_input, mock_target_features):
        """Test that compute_translation_loss handles different loss types"""
        # Arrange
        predicted_features = translation_network(mock_csi_input)
        
        # Act
        mse_loss = translation_network.compute_translation_loss(predicted_features, mock_target_features, loss_type='mse')
        l1_loss = translation_network.compute_translation_loss(predicted_features, mock_target_features, loss_type='l1')
        
        # Assert
        assert mse_loss is not None
        assert l1_loss is not None
        assert mse_loss.item() != l1_loss.item()  # Different loss types should give different values
    
    def test_get_feature_statistics_provides_analysis(self, translation_network, mock_csi_input):
        """Test that get_feature_statistics provides feature analysis"""
        # Arrange
        output = translation_network(mock_csi_input)
        
        # Act
        stats = translation_network.get_feature_statistics(output)
        
        # Assert
        assert stats is not None
        assert isinstance(stats, dict)
        assert 'mean' in stats
        assert 'std' in stats
        assert 'min' in stats
        assert 'max' in stats
        assert 'sparsity' in stats
    
    def test_network_supports_gradient_computation(self, translation_network, mock_csi_input, mock_target_features):
        """Test that network supports gradient computation for training"""
        # Arrange
        translation_network.train()
        optimizer = torch.optim.Adam(translation_network.parameters(), lr=0.001)
        
        # Act
        output = translation_network(mock_csi_input)
        loss = translation_network.compute_translation_loss(output, mock_target_features)
        
        optimizer.zero_grad()
        loss.backward()
        
        # Assert
        for param in translation_network.parameters():
            if param.requires_grad:
                assert param.grad is not None
                assert not torch.allclose(param.grad, torch.zeros_like(param.grad))
    
    def test_network_validates_input_dimensions(self, translation_network):
        """Test that network validates input dimensions"""
        # Arrange
        invalid_input = torch.randn(4, 3, 56, 100)  # Wrong number of channels
        
        # Act & Assert
        with pytest.raises(ModalityTranslationError):
            translation_network(invalid_input)
    
    def test_network_handles_batch_size_one(self, translation_network):
        """Test that network handles single sample inference"""
        # Arrange
        single_input = torch.randn(1, 6, 56, 100)
        
        # Act
        output = translation_network(single_input)
        
        # Assert
        assert output.shape == (1, 256, 56, 100)
    
    def test_save_and_load_model_state(self, translation_network, mock_csi_input):
        """Test that model state can be saved and loaded"""
        # Arrange
        original_output = translation_network(mock_csi_input)
        
        # Act - Save state
        state_dict = translation_network.state_dict()
        
        # Create new network and load state
        new_network = ModalityTranslationNetwork(translation_network.config)
        new_network.load_state_dict(state_dict)
        new_output = new_network(mock_csi_input)
        
        # Assert
        assert torch.allclose(original_output, new_output, atol=1e-6)
    
    def test_network_configuration_validation(self):
        """Test that network validates configuration parameters"""
        # Arrange
        invalid_config = {
            'input_channels': 0,  # Invalid
            'hidden_channels': [],  # Invalid
            'output_channels': 256
        }
        
        # Act & Assert
        with pytest.raises(ValueError):
            ModalityTranslationNetwork(invalid_config)
    
    def test_feature_visualization_support(self, translation_network, mock_csi_input):
        """Test that network supports feature visualization"""
        # Act
        features = translation_network.get_intermediate_features(mock_csi_input)
        
        # Assert
        assert features is not None
        assert isinstance(features, dict)
        assert 'encoder_features' in features
        assert 'decoder_features' in features
        if translation_network.use_attention:
            assert 'attention_weights' in features