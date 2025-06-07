"""DensePose head for WiFi-DensePose system."""

import torch
import torch.nn as nn
import torch.nn.functional as F
from typing import Dict, Any, Tuple, List


class DensePoseError(Exception):
    """Exception raised for DensePose head errors."""
    pass


class DensePoseHead(nn.Module):
    """DensePose head for body part segmentation and UV coordinate regression."""
    
    def __init__(self, config: Dict[str, Any]):
        """Initialize DensePose head.
        
        Args:
            config: Configuration dictionary with head parameters
        """
        super().__init__()
        
        self._validate_config(config)
        self.config = config
        
        self.input_channels = config['input_channels']
        self.num_body_parts = config['num_body_parts']
        self.num_uv_coordinates = config['num_uv_coordinates']
        self.hidden_channels = config.get('hidden_channels', [128, 64])
        self.kernel_size = config.get('kernel_size', 3)
        self.padding = config.get('padding', 1)
        self.dropout_rate = config.get('dropout_rate', 0.1)
        self.use_deformable_conv = config.get('use_deformable_conv', False)
        self.use_fpn = config.get('use_fpn', False)
        self.fpn_levels = config.get('fpn_levels', [2, 3, 4, 5])
        self.output_stride = config.get('output_stride', 4)
        
        # Feature Pyramid Network (optional)
        if self.use_fpn:
            self.fpn = self._build_fpn()
        
        # Shared feature processing
        self.shared_conv = self._build_shared_layers()
        
        # Segmentation head for body part classification
        self.segmentation_head = self._build_segmentation_head()
        
        # UV regression head for coordinate prediction
        self.uv_regression_head = self._build_uv_regression_head()
        
        # Initialize weights
        self._initialize_weights()
    
    def _validate_config(self, config: Dict[str, Any]):
        """Validate configuration parameters."""
        required_fields = ['input_channels', 'num_body_parts', 'num_uv_coordinates']
        for field in required_fields:
            if field not in config:
                raise ValueError(f"Missing required field: {field}")
        
        if config['input_channels'] <= 0:
            raise ValueError("input_channels must be positive")
        
        if config['num_body_parts'] <= 0:
            raise ValueError("num_body_parts must be positive")
        
        if config['num_uv_coordinates'] <= 0:
            raise ValueError("num_uv_coordinates must be positive")
    
    def _build_fpn(self) -> nn.Module:
        """Build Feature Pyramid Network."""
        return nn.ModuleDict({
            f'level_{level}': nn.Conv2d(self.input_channels, self.input_channels, 1)
            for level in self.fpn_levels
        })
    
    def _build_shared_layers(self) -> nn.Module:
        """Build shared feature processing layers."""
        layers = []
        in_channels = self.input_channels
        
        for hidden_dim in self.hidden_channels:
            layers.extend([
                nn.Conv2d(in_channels, hidden_dim,
                         kernel_size=self.kernel_size,
                         padding=self.padding),
                nn.BatchNorm2d(hidden_dim),
                nn.ReLU(inplace=True),
                nn.Dropout2d(self.dropout_rate)
            ])
            in_channels = hidden_dim
        
        return nn.Sequential(*layers)
    
    def _build_segmentation_head(self) -> nn.Module:
        """Build segmentation head for body part classification."""
        final_hidden = self.hidden_channels[-1] if self.hidden_channels else self.input_channels
        
        return nn.Sequential(
            nn.Conv2d(final_hidden, final_hidden // 2,
                     kernel_size=self.kernel_size,
                     padding=self.padding),
            nn.BatchNorm2d(final_hidden // 2),
            nn.ReLU(inplace=True),
            nn.Dropout2d(self.dropout_rate),
            
            # Upsampling to increase resolution
            nn.ConvTranspose2d(final_hidden // 2, final_hidden // 4,
                             kernel_size=4, stride=2, padding=1),
            nn.BatchNorm2d(final_hidden // 4),
            nn.ReLU(inplace=True),
            
            nn.Conv2d(final_hidden // 4, self.num_body_parts + 1, kernel_size=1),
            # +1 for background class
        )
    
    def _build_uv_regression_head(self) -> nn.Module:
        """Build UV regression head for coordinate prediction."""
        final_hidden = self.hidden_channels[-1] if self.hidden_channels else self.input_channels
        
        return nn.Sequential(
            nn.Conv2d(final_hidden, final_hidden // 2,
                     kernel_size=self.kernel_size,
                     padding=self.padding),
            nn.BatchNorm2d(final_hidden // 2),
            nn.ReLU(inplace=True),
            nn.Dropout2d(self.dropout_rate),
            
            # Upsampling to increase resolution
            nn.ConvTranspose2d(final_hidden // 2, final_hidden // 4,
                             kernel_size=4, stride=2, padding=1),
            nn.BatchNorm2d(final_hidden // 4),
            nn.ReLU(inplace=True),
            
            nn.Conv2d(final_hidden // 4, self.num_uv_coordinates, kernel_size=1),
        )
    
    def _initialize_weights(self):
        """Initialize network weights."""
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode='fan_out', nonlinearity='relu')
                if m.bias is not None:
                    nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.constant_(m.weight, 1)
                nn.init.constant_(m.bias, 0)
    
    def forward(self, x: torch.Tensor) -> Dict[str, torch.Tensor]:
        """Forward pass through the DensePose head.
        
        Args:
            x: Input feature tensor of shape (batch_size, channels, height, width)
            
        Returns:
            Dictionary containing:
            - segmentation: Body part logits (batch_size, num_parts+1, height, width)
            - uv_coordinates: UV coordinates (batch_size, 2, height, width)
        """
        # Validate input shape
        if x.shape[1] != self.input_channels:
            raise DensePoseError(f"Expected {self.input_channels} input channels, got {x.shape[1]}")
        
        # Apply FPN if enabled
        if self.use_fpn:
            # Simple FPN processing - in practice this would be more sophisticated
            x = self.fpn['level_2'](x)
        
        # Shared feature processing
        shared_features = self.shared_conv(x)
        
        # Segmentation branch
        segmentation_logits = self.segmentation_head(shared_features)
        
        # UV regression branch
        uv_coordinates = self.uv_regression_head(shared_features)
        uv_coordinates = torch.sigmoid(uv_coordinates)  # Normalize to [0, 1]
        
        return {
            'segmentation': segmentation_logits,
            'uv_coordinates': uv_coordinates
        }
    
    def compute_segmentation_loss(self, pred_logits: torch.Tensor, target: torch.Tensor) -> torch.Tensor:
        """Compute segmentation loss.
        
        Args:
            pred_logits: Predicted segmentation logits
            target: Target segmentation masks
            
        Returns:
            Computed cross-entropy loss
        """
        return F.cross_entropy(pred_logits, target, ignore_index=-1)
    
    def compute_uv_loss(self, pred_uv: torch.Tensor, target_uv: torch.Tensor) -> torch.Tensor:
        """Compute UV coordinate regression loss.
        
        Args:
            pred_uv: Predicted UV coordinates
            target_uv: Target UV coordinates
            
        Returns:
            Computed L1 loss
        """
        return F.l1_loss(pred_uv, target_uv)
    
    def compute_total_loss(self, predictions: Dict[str, torch.Tensor],
                          seg_target: torch.Tensor,
                          uv_target: torch.Tensor,
                          seg_weight: float = 1.0,
                          uv_weight: float = 1.0) -> torch.Tensor:
        """Compute total loss combining segmentation and UV losses.
        
        Args:
            predictions: Dictionary of predictions
            seg_target: Target segmentation masks
            uv_target: Target UV coordinates
            seg_weight: Weight for segmentation loss
            uv_weight: Weight for UV loss
            
        Returns:
            Combined loss
        """
        seg_loss = self.compute_segmentation_loss(predictions['segmentation'], seg_target)
        uv_loss = self.compute_uv_loss(predictions['uv_coordinates'], uv_target)
        
        return seg_weight * seg_loss + uv_weight * uv_loss
    
    def get_prediction_confidence(self, predictions: Dict[str, torch.Tensor]) -> Dict[str, torch.Tensor]:
        """Get prediction confidence scores.
        
        Args:
            predictions: Dictionary of predictions
            
        Returns:
            Dictionary of confidence scores
        """
        seg_logits = predictions['segmentation']
        uv_coords = predictions['uv_coordinates']
        
        # Segmentation confidence: max probability
        seg_probs = F.softmax(seg_logits, dim=1)
        seg_confidence = torch.max(seg_probs, dim=1)[0]
        
        # UV confidence: inverse of prediction variance
        uv_variance = torch.var(uv_coords, dim=1, keepdim=True)
        uv_confidence = 1.0 / (1.0 + uv_variance)
        
        return {
            'segmentation_confidence': seg_confidence,
            'uv_confidence': uv_confidence.squeeze(1)
        }
    
    def post_process_predictions(self, predictions: Dict[str, torch.Tensor]) -> Dict[str, torch.Tensor]:
        """Post-process predictions for final output.
        
        Args:
            predictions: Raw predictions from forward pass
            
        Returns:
            Post-processed predictions
        """
        seg_logits = predictions['segmentation']
        uv_coords = predictions['uv_coordinates']
        
        # Convert logits to class predictions
        body_parts = torch.argmax(seg_logits, dim=1)
        
        # Get confidence scores
        confidence = self.get_prediction_confidence(predictions)
        
        return {
            'body_parts': body_parts,
            'uv_coordinates': uv_coords,
            'confidence_scores': confidence
        }