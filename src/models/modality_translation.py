"""Modality translation network for WiFi-DensePose system."""

import torch
import torch.nn as nn
import torch.nn.functional as F
from typing import Dict, Any, List


class ModalityTranslationError(Exception):
    """Exception raised for modality translation errors."""
    pass


class ModalityTranslationNetwork(nn.Module):
    """Neural network for translating CSI data to visual feature space."""
    
    def __init__(self, config: Dict[str, Any]):
        """Initialize modality translation network.
        
        Args:
            config: Configuration dictionary with network parameters
        """
        super().__init__()
        
        self._validate_config(config)
        self.config = config
        
        self.input_channels = config['input_channels']
        self.hidden_channels = config['hidden_channels']
        self.output_channels = config['output_channels']
        self.kernel_size = config.get('kernel_size', 3)
        self.stride = config.get('stride', 1)
        self.padding = config.get('padding', 1)
        self.dropout_rate = config.get('dropout_rate', 0.1)
        self.activation = config.get('activation', 'relu')
        self.normalization = config.get('normalization', 'batch')
        self.use_attention = config.get('use_attention', False)
        self.attention_heads = config.get('attention_heads', 8)
        
        # Encoder: CSI -> Feature space
        self.encoder = self._build_encoder()
        
        # Decoder: Feature space -> Visual-like features
        self.decoder = self._build_decoder()
        
        # Attention mechanism
        if self.use_attention:
            self.attention = self._build_attention()
        
        # Initialize weights
        self._initialize_weights()
    
    def _validate_config(self, config: Dict[str, Any]):
        """Validate configuration parameters."""
        required_fields = ['input_channels', 'hidden_channels', 'output_channels']
        for field in required_fields:
            if field not in config:
                raise ValueError(f"Missing required field: {field}")
        
        if config['input_channels'] <= 0:
            raise ValueError("input_channels must be positive")
        
        if not config['hidden_channels'] or len(config['hidden_channels']) == 0:
            raise ValueError("hidden_channels must be a non-empty list")
        
        if config['output_channels'] <= 0:
            raise ValueError("output_channels must be positive")
    
    def _build_encoder(self) -> nn.ModuleList:
        """Build encoder network."""
        layers = nn.ModuleList()
        
        # Initial convolution
        in_channels = self.input_channels
        
        for i, out_channels in enumerate(self.hidden_channels):
            layer_block = nn.Sequential(
                nn.Conv2d(in_channels, out_channels,
                         kernel_size=self.kernel_size,
                         stride=self.stride if i == 0 else 2,
                         padding=self.padding),
                self._get_normalization(out_channels),
                self._get_activation(),
                nn.Dropout2d(self.dropout_rate)
            )
            layers.append(layer_block)
            in_channels = out_channels
        
        return layers
    
    def _build_decoder(self) -> nn.ModuleList:
        """Build decoder network."""
        layers = nn.ModuleList()
        
        # Start with the last hidden channel size
        in_channels = self.hidden_channels[-1]
        
        # Progressive upsampling (reverse of encoder)
        for i, out_channels in enumerate(reversed(self.hidden_channels[:-1])):
            layer_block = nn.Sequential(
                nn.ConvTranspose2d(in_channels, out_channels,
                                 kernel_size=self.kernel_size,
                                 stride=2,
                                 padding=self.padding,
                                 output_padding=1),
                self._get_normalization(out_channels),
                self._get_activation(),
                nn.Dropout2d(self.dropout_rate)
            )
            layers.append(layer_block)
            in_channels = out_channels
        
        # Final output layer
        final_layer = nn.Sequential(
            nn.Conv2d(in_channels, self.output_channels,
                     kernel_size=self.kernel_size,
                     padding=self.padding),
            nn.Tanh()  # Normalize output
        )
        layers.append(final_layer)
        
        return layers
    
    def _get_normalization(self, channels: int) -> nn.Module:
        """Get normalization layer."""
        if self.normalization == 'batch':
            return nn.BatchNorm2d(channels)
        elif self.normalization == 'instance':
            return nn.InstanceNorm2d(channels)
        elif self.normalization == 'layer':
            return nn.GroupNorm(1, channels)
        else:
            return nn.Identity()
    
    def _get_activation(self) -> nn.Module:
        """Get activation function."""
        if self.activation == 'relu':
            return nn.ReLU(inplace=True)
        elif self.activation == 'leaky_relu':
            return nn.LeakyReLU(0.2, inplace=True)
        elif self.activation == 'gelu':
            return nn.GELU()
        else:
            return nn.ReLU(inplace=True)
    
    def _build_attention(self) -> nn.Module:
        """Build attention mechanism."""
        return nn.MultiheadAttention(
            embed_dim=self.hidden_channels[-1],
            num_heads=self.attention_heads,
            dropout=self.dropout_rate,
            batch_first=True
        )
    
    def _initialize_weights(self):
        """Initialize network weights."""
        for m in self.modules():
            if isinstance(m, (nn.Conv2d, nn.ConvTranspose2d)):
                nn.init.kaiming_normal_(m.weight, mode='fan_out', nonlinearity='relu')
                if m.bias is not None:
                    nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.constant_(m.weight, 1)
                nn.init.constant_(m.bias, 0)
    
    def forward(self, x: torch.Tensor) -> torch.Tensor:
        """Forward pass through the network.
        
        Args:
            x: Input CSI tensor of shape (batch_size, channels, height, width)
            
        Returns:
            Translated features tensor
        """
        # Validate input shape
        if x.shape[1] != self.input_channels:
            raise ModalityTranslationError(f"Expected {self.input_channels} input channels, got {x.shape[1]}")
        
        # Encode CSI data
        encoded_features = self.encode(x)
        
        # Decode to visual-like features
        decoded = self.decode(encoded_features)
        
        return decoded
    
    def encode(self, x: torch.Tensor) -> List[torch.Tensor]:
        """Encode input through encoder layers.
        
        Args:
            x: Input tensor
            
        Returns:
            List of feature maps from each encoder layer
        """
        features = []
        current = x
        
        for layer in self.encoder:
            current = layer(current)
            features.append(current)
        
        return features
    
    def decode(self, encoded_features: List[torch.Tensor]) -> torch.Tensor:
        """Decode features through decoder layers.
        
        Args:
            encoded_features: List of encoded feature maps
            
        Returns:
            Decoded output tensor
        """
        # Start with the last encoded feature
        current = encoded_features[-1]
        
        # Apply attention if enabled
        if self.use_attention:
            batch_size, channels, height, width = current.shape
            # Reshape for attention: (batch, seq_len, embed_dim)
            current_flat = current.view(batch_size, channels, -1).transpose(1, 2)
            attended, _ = self.attention(current_flat, current_flat, current_flat)
            current = attended.transpose(1, 2).view(batch_size, channels, height, width)
        
        # Apply decoder layers
        for layer in self.decoder:
            current = layer(current)
        
        return current
    
    def compute_translation_loss(self, predicted: torch.Tensor, target: torch.Tensor, loss_type: str = 'mse') -> torch.Tensor:
        """Compute translation loss between predicted and target features.
        
        Args:
            predicted: Predicted feature tensor
            target: Target feature tensor
            loss_type: Type of loss ('mse', 'l1', 'smooth_l1')
            
        Returns:
            Computed loss tensor
        """
        if loss_type == 'mse':
            return F.mse_loss(predicted, target)
        elif loss_type == 'l1':
            return F.l1_loss(predicted, target)
        elif loss_type == 'smooth_l1':
            return F.smooth_l1_loss(predicted, target)
        else:
            return F.mse_loss(predicted, target)
    
    def get_feature_statistics(self, features: torch.Tensor) -> Dict[str, float]:
        """Get statistics of feature tensor.
        
        Args:
            features: Feature tensor to analyze
            
        Returns:
            Dictionary of feature statistics
        """
        with torch.no_grad():
            return {
                'mean': features.mean().item(),
                'std': features.std().item(),
                'min': features.min().item(),
                'max': features.max().item(),
                'sparsity': (features == 0).float().mean().item()
            }
    
    def get_intermediate_features(self, x: torch.Tensor) -> Dict[str, Any]:
        """Get intermediate features for visualization.
        
        Args:
            x: Input tensor
            
        Returns:
            Dictionary containing intermediate features
        """
        result = {}
        
        # Get encoder features
        encoder_features = self.encode(x)
        result['encoder_features'] = encoder_features
        
        # Get decoder features
        decoder_features = []
        current = encoder_features[-1]
        
        if self.use_attention:
            batch_size, channels, height, width = current.shape
            current_flat = current.view(batch_size, channels, -1).transpose(1, 2)
            attended, attention_weights = self.attention(current_flat, current_flat, current_flat)
            current = attended.transpose(1, 2).view(batch_size, channels, height, width)
            result['attention_weights'] = attention_weights
        
        for layer in self.decoder:
            current = layer(current)
            decoder_features.append(current)
        
        result['decoder_features'] = decoder_features
        
        return result