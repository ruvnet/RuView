"""Modality translation network for WiFi-DensePose system."""

import torch
import torch.nn as nn
import torch.nn.functional as F
from typing import Dict, Any


class ModalityTranslationNetwork(nn.Module):
    """Neural network for translating CSI data to visual feature space."""
    
    def __init__(self, config: Dict[str, Any]):
        """Initialize modality translation network.
        
        Args:
            config: Configuration dictionary with network parameters
        """
        super().__init__()
        
        self.input_channels = config['input_channels']
        self.hidden_dim = config['hidden_dim']
        self.output_dim = config['output_dim']
        self.num_layers = config['num_layers']
        self.dropout_rate = config['dropout_rate']
        
        # Encoder: CSI -> Feature space
        self.encoder = self._build_encoder()
        
        # Decoder: Feature space -> Visual-like features
        self.decoder = self._build_decoder()
        
        # Initialize weights
        self._initialize_weights()
    
    def _build_encoder(self) -> nn.Module:
        """Build encoder network."""
        layers = []
        
        # Initial convolution
        layers.append(nn.Conv2d(self.input_channels, 64, kernel_size=3, padding=1))
        layers.append(nn.BatchNorm2d(64))
        layers.append(nn.ReLU(inplace=True))
        layers.append(nn.Dropout2d(self.dropout_rate))
        
        # Progressive downsampling
        in_channels = 64
        for i in range(self.num_layers - 1):
            out_channels = min(in_channels * 2, self.hidden_dim)
            layers.extend([
                nn.Conv2d(in_channels, out_channels, kernel_size=3, stride=2, padding=1),
                nn.BatchNorm2d(out_channels),
                nn.ReLU(inplace=True),
                nn.Dropout2d(self.dropout_rate)
            ])
            in_channels = out_channels
        
        return nn.Sequential(*layers)
    
    def _build_decoder(self) -> nn.Module:
        """Build decoder network."""
        layers = []
        
        # Get the actual output channels from encoder (should be hidden_dim)
        encoder_out_channels = self.hidden_dim
        
        # Progressive upsampling
        in_channels = encoder_out_channels
        for i in range(self.num_layers - 1):
            out_channels = max(in_channels // 2, 64)
            layers.extend([
                nn.ConvTranspose2d(in_channels, out_channels, kernel_size=3, stride=2, padding=1, output_padding=1),
                nn.BatchNorm2d(out_channels),
                nn.ReLU(inplace=True),
                nn.Dropout2d(self.dropout_rate)
            ])
            in_channels = out_channels
        
        # Final output layer
        layers.append(nn.Conv2d(in_channels, self.output_dim, kernel_size=3, padding=1))
        layers.append(nn.Tanh())  # Normalize output
        
        return nn.Sequential(*layers)
    
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
            raise RuntimeError(f"Expected {self.input_channels} input channels, got {x.shape[1]}")
        
        # Encode CSI data
        encoded = self.encoder(x)
        
        # Decode to visual-like features
        decoded = self.decoder(encoded)
        
        return decoded