# WiFi DensePose Implementation in PyTorch
# Based on "DensePose From WiFi" by Carnegie Mellon University
# Paper: https://arxiv.org/pdf/2301.00250

import torch
import torch.nn as nn
import torch.nn.functional as F
import numpy as np
import math
from typing import Dict, List, Tuple, Optional
from collections import OrderedDict

class CSIPhaseProcessor:
    """
    Processes raw CSI phase data through unwrapping, filtering, and linear fitting
    Based on the phase sanitization methodology from the paper
    """
    
    def __init__(self, num_subcarriers: int = 30):
        self.num_subcarriers = num_subcarriers
    
    def unwrap_phase(self, phase_data: torch.Tensor) -> torch.Tensor:
        """
        Unwrap phase values to handle discontinuities
        Args:
            phase_data: Raw phase data of shape (batch, freq_samples, tx, rx)
        Returns:
            Unwrapped phase data
        """
        unwrapped = phase_data.clone()
        
        # Unwrap along frequency dimension (groups of 30 frequencies)
        for sample_group in range(5):  # 5 consecutive samples
            start_idx = sample_group * 30
            end_idx = start_idx + 30
            
            for i in range(start_idx + 1, end_idx):
                diff = unwrapped[:, i] - unwrapped[:, i-1]
                
                # Apply unwrapping logic
                unwrapped[:, i] = torch.where(diff > math.pi,
                                            unwrapped[:, i-1] + diff - 2*math.pi,
                                            unwrapped[:, i])
                unwrapped[:, i] = torch.where(diff < -math.pi,
                                            unwrapped[:, i-1] + diff + 2*math.pi,
                                            unwrapped[:, i])
        
        return unwrapped
    
    def apply_filters(self, phase_data: torch.Tensor) -> torch.Tensor:
        """
        Apply median and uniform filters to eliminate outliers
        """
        # Simple smoothing in frequency dimension
        filtered = phase_data.clone()
        for i in range(1, phase_data.shape[1]-1):
            filtered[:, i] = (phase_data[:, i-1] + phase_data[:, i] + phase_data[:, i+1]) / 3
        
        return filtered
    
    def linear_fitting(self, phase_data: torch.Tensor) -> torch.Tensor:
        """
        Apply linear fitting to remove systematic phase drift
        """
        fitted_data = phase_data.clone()
        F = self.num_subcarriers
        
        # Process each sample group (5 consecutive samples)
        for sample_group in range(5):
            start_idx = sample_group * 30
            end_idx = start_idx + 30
            
            for batch_idx in range(phase_data.shape[0]):
                for tx in range(phase_data.shape[2]):
                    for rx in range(phase_data.shape[3]):
                        phase_seq = phase_data[batch_idx, start_idx:end_idx, tx, rx]
                        
                        if len(phase_seq) > 1:
                            # Calculate linear coefficients
                            alpha1 = (phase_seq[-1] - phase_seq[0]) / (2 * math.pi * F)
                            alpha0 = torch.mean(phase_seq)
                            
                            # Apply linear fitting
                            frequencies = torch.arange(1, len(phase_seq) + 1, dtype=phase_seq.dtype, device=phase_seq.device)
                            linear_trend = alpha1 * frequencies + alpha0
                            fitted_data[batch_idx, start_idx:end_idx, tx, rx] = phase_seq - linear_trend
        
        return fitted_data
    
    def sanitize_phase(self, raw_phase: torch.Tensor) -> torch.Tensor:
        """
        Complete phase sanitization pipeline
        """
        # Step 1: Unwrap phase
        unwrapped = self.unwrap_phase(raw_phase)
        
        # Step 2: Apply filters
        filtered = self.apply_filters(unwrapped)
        
        # Step 3: Linear fitting
        sanitized = self.linear_fitting(filtered)
        
        return sanitized

class ModalityTranslationNetwork(nn.Module):
    """
    Translates CSI domain features to spatial domain features
    Input: 150x3x3 amplitude and phase tensors
    Output: 3x720x1280 feature map
    """
    
    def __init__(self, input_dim: int = 1350, hidden_dim: int = 512, output_height: int = 720, output_width: int = 1280):
        super(ModalityTranslationNetwork, self).__init__()
        
        self.input_dim = input_dim
        self.output_height = output_height
        self.output_width = output_width
        
        # Amplitude encoder
        self.amplitude_encoder = nn.Sequential(
            nn.Linear(input_dim, hidden_dim),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim, hidden_dim//2),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim//2, hidden_dim//4),
            nn.ReLU()
        )
        
        # Phase encoder
        self.phase_encoder = nn.Sequential(
            nn.Linear(input_dim, hidden_dim),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim, hidden_dim//2),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim//2, hidden_dim//4),
            nn.ReLU()
        )
        
        # Feature fusion
        self.fusion_mlp = nn.Sequential(
            nn.Linear(hidden_dim//2, hidden_dim//4),
            nn.ReLU(),
            nn.Dropout(0.2),
            nn.Linear(hidden_dim//4, 24*24),  # Reshape to 24x24
            nn.ReLU()
        )
        
        # Spatial processing
        self.spatial_conv = nn.Sequential(
            nn.Conv2d(1, 64, kernel_size=3, padding=1),
            nn.BatchNorm2d(64),
            nn.ReLU(),
            nn.Conv2d(64, 128, kernel_size=3, padding=1),
            nn.BatchNorm2d(128),
            nn.ReLU(),
            nn.AdaptiveAvgPool2d((6, 6))  # Compress to 6x6
        )
        
        # Upsampling to target resolution
        self.upsample = nn.Sequential(
            nn.ConvTranspose2d(128, 64, kernel_size=4, stride=2, padding=1),  # 12x12
            nn.BatchNorm2d(64),
            nn.ReLU(),
            nn.ConvTranspose2d(64, 32, kernel_size=4, stride=2, padding=1),   # 24x24
            nn.BatchNorm2d(32),
            nn.ReLU(),
            nn.ConvTranspose2d(32, 16, kernel_size=4, stride=2, padding=1),   # 48x48
            nn.BatchNorm2d(16),
            nn.ReLU(),
            nn.ConvTranspose2d(16, 8, kernel_size=4, stride=2, padding=1),    # 96x96
            nn.BatchNorm2d(8),
            nn.ReLU(),
        )
        
        # Final upsampling to target size
        self.final_conv = nn.Conv2d(8, 3, kernel_size=1)
        
    def forward(self, amplitude_tensor: torch.Tensor, phase_tensor: torch.Tensor) -> torch.Tensor:
        batch_size = amplitude_tensor.shape[0]
        
        # Flatten input tensors
        amplitude_flat = amplitude_tensor.view(batch_size, -1)  # [B, 1350]
        phase_flat = phase_tensor.view(batch_size, -1)          # [B, 1350]
        
        # Encode features
        amp_features = self.amplitude_encoder(amplitude_flat)   # [B, 128]
        phase_features = self.phase_encoder(phase_flat)         # [B, 128]
        
        # Fuse features
        fused_features = torch.cat([amp_features, phase_features], dim=1)  # [B, 256]
        spatial_features = self.fusion_mlp(fused_features)      # [B, 576]
        
        # Reshape to 2D feature map
        spatial_map = spatial_features.view(batch_size, 1, 24, 24)  # [B, 1, 24, 24]
        
        # Apply spatial convolutions
        conv_features = self.spatial_conv(spatial_map)          # [B, 128, 6, 6]
        
        # Upsample
        upsampled = self.upsample(conv_features)                # [B, 8, 96, 96]
        
        # Final convolution
        final_features = self.final_conv(upsampled)             # [B, 3, 96, 96]
        
        # Interpolate to target resolution
        output = F.interpolate(final_features, size=(self.output_height, self.output_width), 
                             mode='bilinear', align_corners=False)
        
        return output

class DensePoseHead(nn.Module):
    """
    DensePose prediction head for estimating UV coordinates
    """
    def __init__(self, input_channels=256, num_parts=24, output_size=(112, 112)):
        super(DensePoseHead, self).__init__()
        
        self.num_parts = num_parts
        self.output_size = output_size
        
        # Shared convolutional layers
        self.shared_conv = nn.Sequential(
            nn.Conv2d(input_channels, 512, kernel_size=3, padding=1),
            nn.ReLU(),
            nn.Conv2d(512, 512, kernel_size=3, padding=1),
            nn.ReLU(),
            nn.Conv2d(512, 512, kernel_size=3, padding=1),
            nn.ReLU(),
        )
        
        # Part classification branch
        self.part_classifier = nn.Conv2d(512, num_parts + 1, kernel_size=1)  # +1 for background
        
        # UV coordinate regression branches
        self.u_regressor = nn.Conv2d(512, num_parts, kernel_size=1)
        self.v_regressor = nn.Conv2d(512, num_parts, kernel_size=1)
        
    def forward(self, x):
        # Shared feature extraction
        features = self.shared_conv(x)
        
        # Upsample features to target size
        features = F.interpolate(features, size=self.output_size, mode='bilinear', align_corners=False)
        
        # Predict part labels
        part_logits = self.part_classifier(features)
        
        # Predict UV coordinates
        u_coords = torch.sigmoid(self.u_regressor(features))  # Sigmoid to ensure [0,1] range
        v_coords = torch.sigmoid(self.v_regressor(features))
        
        return {
            'part_logits': part_logits,
            'u_coords': u_coords,
            'v_coords': v_coords
        }

class KeypointHead(nn.Module):
    """
    Keypoint prediction head for estimating body keypoints
    """
    def __init__(self, input_channels=256, num_keypoints=17, output_size=(56, 56)):
        super(KeypointHead, self).__init__()
        
        self.num_keypoints = num_keypoints
        self.output_size = output_size
        
        # Convolutional layers for keypoint detection
        self.conv_layers = nn.Sequential(
            nn.Conv2d(input_channels, 512, kernel_size=3, padding=1),
            nn.ReLU(),
            nn.Conv2d(512, 512, kernel_size=3, padding=1),
            nn.ReLU(),
            nn.Conv2d(512, 512, kernel_size=3, padding=1),
            nn.ReLU(),
            nn.Conv2d(512, num_keypoints, kernel_size=1)
        )
        
    def forward(self, x):
        # Extract keypoint heatmaps
        heatmaps = self.conv_layers(x)
        
        # Upsample to target size
        heatmaps = F.interpolate(heatmaps, size=self.output_size, mode='bilinear', align_corners=False)
        
        return heatmaps

class WiFiDensePoseRCNN(nn.Module):
    """
    Complete WiFi-DensePose RCNN architecture
    """
    def __init__(self):
        super(WiFiDensePoseRCNN, self).__init__()
        
        # CSI processing
        self.phase_processor = CSIPhaseProcessor()
        
        # Modality translation
        self.modality_translation = ModalityTranslationNetwork()
        
        # Simplified backbone (in practice, use ResNet-FPN)
        self.backbone = nn.Sequential(
            nn.Conv2d(3, 64, kernel_size=7, stride=2, padding=3),
            nn.BatchNorm2d(64),
            nn.ReLU(),
            nn.MaxPool2d(kernel_size=3, stride=2, padding=1),
            
            # Simplified ResNet blocks
            nn.Conv2d(64, 128, kernel_size=3, padding=1),
            nn.BatchNorm2d(128),
            nn.ReLU(),
            nn.Conv2d(128, 256, kernel_size=3, padding=1),
            nn.BatchNorm2d(256),
            nn.ReLU(),
        )
        
        # Prediction heads
        self.densepose_head = DensePoseHead(input_channels=256)
        self.keypoint_head = KeypointHead(input_channels=256)
        
        # Global average pooling for simplified processing
        self.global_pool = nn.AdaptiveAvgPool2d((7, 7))
        
    def forward(self, amplitude_data, phase_data):
        batch_size = amplitude_data.shape[0]
        
        # Process CSI phase data
        sanitized_phase = self.phase_processor.sanitize_phase(phase_data)
        
        # Translate to spatial domain
        spatial_features = self.modality_translation(amplitude_data, sanitized_phase)
        
        # Extract backbone features
        backbone_features = self.backbone(spatial_features)
        
        # Global pooling to get fixed-size features
        pooled_features = self.global_pool(backbone_features)
        
        # Predict DensePose
        densepose_output = self.densepose_head(pooled_features)
        
        # Predict keypoints
        keypoint_heatmaps = self.keypoint_head(pooled_features)
        
        return {
            'spatial_features': spatial_features,
            'densepose': densepose_output,
            'keypoints': keypoint_heatmaps
        }

class WiFiDensePoseLoss(nn.Module):
    """
    Combined loss function for WiFi DensePose training
    """
    def __init__(self, lambda_dp=0.6, lambda_kp=0.3, lambda_tr=0.1):
        super(WiFiDensePoseLoss, self).__init__()
        
        self.lambda_dp = lambda_dp
        self.lambda_kp = lambda_kp
        self.lambda_tr = lambda_tr
        
        # Loss functions
        self.cross_entropy = nn.CrossEntropyLoss()
        self.mse_loss = nn.MSELoss()
        self.smooth_l1 = nn.SmoothL1Loss()
        
    def forward(self, predictions, targets, teacher_features=None):
        total_loss = 0.0
        loss_dict = {}
        
        # DensePose losses
        if 'densepose' in predictions and 'densepose' in targets:
            # Part classification loss
            part_loss = self.cross_entropy(
                predictions['densepose']['part_logits'],
                targets['densepose']['part_labels']
            )
            
            # UV coordinate regression loss
            uv_loss = (self.smooth_l1(predictions['densepose']['u_coords'], targets['densepose']['u_coords']) +
                      self.smooth_l1(predictions['densepose']['v_coords'], targets['densepose']['v_coords'])) / 2
            
            dp_loss = part_loss + uv_loss
            total_loss += self.lambda_dp * dp_loss
            loss_dict['densepose'] = dp_loss
        
        # Keypoint loss
        if 'keypoints' in predictions and 'keypoints' in targets:
            kp_loss = self.mse_loss(predictions['keypoints'], targets['keypoints'])
            total_loss += self.lambda_kp * kp_loss
            loss_dict['keypoint'] = kp_loss
        
        # Transfer learning loss
        if teacher_features is not None and 'backbone_features' in predictions:
            tr_loss = self.mse_loss(predictions['backbone_features'], teacher_features)
            total_loss += self.lambda_tr * tr_loss
            loss_dict['transfer'] = tr_loss
        
        loss_dict['total'] = total_loss
        return total_loss, loss_dict

# Training utilities
class WiFiDensePoseTrainer:
    """
    Training utilities for WiFi DensePose
    """
    def __init__(self, model, device='cuda' if torch.cuda.is_available() else 'cpu'):
        self.model = model.to(device)
        self.device = device
        self.criterion = WiFiDensePoseLoss()
        self.optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
        self.scheduler = torch.optim.lr_scheduler.MultiStepLR(
            self.optimizer, milestones=[48000, 96000], gamma=0.1
        )
        
    def train_step(self, amplitude_data, phase_data, targets):
        self.model.train()
        self.optimizer.zero_grad()
        
        # Forward pass
        outputs = self.model(amplitude_data, phase_data)
        
        # Compute loss
        loss, loss_dict = self.criterion(outputs, targets)
        
        # Backward pass
        loss.backward()
        self.optimizer.step()
        self.scheduler.step()
        
        return loss.item(), loss_dict
    
    def save_model(self, path):
        torch.save({
            'model_state_dict': self.model.state_dict(),
            'optimizer_state_dict': self.optimizer.state_dict(),
        }, path)
    
    def load_model(self, path):
        checkpoint = torch.load(path)
        self.model.load_state_dict(checkpoint['model_state_dict'])
        self.optimizer.load_state_dict(checkpoint['optimizer_state_dict'])

# Example usage
def create_sample_data(batch_size=1, device='cpu'):
    """
    Create sample CSI data for testing
    """
    amplitude = torch.randn(batch_size, 150, 3, 3).to(device)
    phase = torch.randn(batch_size, 150, 3, 3).to(device)
    
    # Sample targets
    targets = {
        'densepose': {
            'part_labels': torch.randint(0, 25, (batch_size, 112, 112)).to(device),
            'u_coords': torch.rand(batch_size, 24, 112, 112).to(device),
            'v_coords': torch.rand(batch_size, 24, 112, 112).to(device)
        },
        'keypoints': torch.rand(batch_size, 17, 56, 56).to(device)
    }
    
    return amplitude, phase, targets

if __name__ == "__main__":
    # Initialize model
    model = WiFiDensePoseRCNN()
    trainer = WiFiDensePoseTrainer(model)
    
    print("WiFi DensePose model initialized!")
    print(f"Model parameters: {sum(p.numel() for p in model.parameters()):,}")
    
    # Create sample data
    amplitude, phase, targets = create_sample_data()
    
    # Run inference
    with torch.no_grad():
        outputs = model(amplitude, phase)
        print(f"Spatial features shape: {outputs['spatial_features'].shape}")
        print(f"DensePose part logits shape: {outputs['densepose']['part_logits'].shape}")
        print(f"Keypoint heatmaps shape: {outputs['keypoints'].shape}")
    
    # Training step
    loss, loss_dict = trainer.train_step(amplitude, phase, targets)
    print(f"Training loss: {loss:.4f}")
    print(f"Loss breakdown: {loss_dict}")