//! DensePose head for body part segmentation and UV coordinate regression.
//!
//! This module implements the DensePose prediction head that takes feature maps
//! from a backbone network and produces body part segmentation masks and UV
//! coordinate predictions for each pixel.

use crate::error::{NnError, NnResult};
use crate::tensor::{Tensor, TensorShape, TensorStats};
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the DensePose head
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensePoseConfig {
    /// Number of input channels from backbone
    pub input_channels: usize,
    /// Number of body parts to predict (excluding background)
    pub num_body_parts: usize,
    /// Number of UV coordinates (typically 2 for U and V)
    pub num_uv_coordinates: usize,
    /// Hidden channel sizes for shared convolutions
    #[serde(default = "default_hidden_channels")]
    pub hidden_channels: Vec<usize>,
    /// Convolution kernel size
    #[serde(default = "default_kernel_size")]
    pub kernel_size: usize,
    /// Convolution padding
    #[serde(default = "default_padding")]
    pub padding: usize,
    /// Dropout rate
    #[serde(default = "default_dropout_rate")]
    pub dropout_rate: f32,
    /// Whether to use Feature Pyramid Network
    #[serde(default)]
    pub use_fpn: bool,
    /// FPN levels to use
    #[serde(default = "default_fpn_levels")]
    pub fpn_levels: Vec<usize>,
    /// Output stride
    #[serde(default = "default_output_stride")]
    pub output_stride: usize,
}

fn default_hidden_channels() -> Vec<usize> {
    vec![128, 64]
}

fn default_kernel_size() -> usize {
    3
}

fn default_padding() -> usize {
    1
}

fn default_dropout_rate() -> f32 {
    0.1
}

fn default_fpn_levels() -> Vec<usize> {
    vec![2, 3, 4, 5]
}

fn default_output_stride() -> usize {
    4
}

impl Default for DensePoseConfig {
    fn default() -> Self {
        Self {
            input_channels: 256,
            num_body_parts: 24,
            num_uv_coordinates: 2,
            hidden_channels: default_hidden_channels(),
            kernel_size: default_kernel_size(),
            padding: default_padding(),
            dropout_rate: default_dropout_rate(),
            use_fpn: false,
            fpn_levels: default_fpn_levels(),
            output_stride: default_output_stride(),
        }
    }
}

impl DensePoseConfig {
    /// Create a new configuration with required parameters
    pub fn new(input_channels: usize, num_body_parts: usize, num_uv_coordinates: usize) -> Self {
        Self {
            input_channels,
            num_body_parts,
            num_uv_coordinates,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> NnResult<()> {
        if self.input_channels == 0 {
            return Err(NnError::config("input_channels must be positive"));
        }
        if self.num_body_parts == 0 {
            return Err(NnError::config("num_body_parts must be positive"));
        }
        if self.num_uv_coordinates == 0 {
            return Err(NnError::config("num_uv_coordinates must be positive"));
        }
        if self.hidden_channels.is_empty() {
            return Err(NnError::config("hidden_channels must not be empty"));
        }
        Ok(())
    }

    /// Get the number of output channels for segmentation (including background)
    pub fn segmentation_channels(&self) -> usize {
        self.num_body_parts + 1 // +1 for background class
    }
}

/// Output from the DensePose head
#[derive(Debug, Clone)]
pub struct DensePoseOutput {
    /// Body part segmentation logits: (batch, num_parts+1, height, width)
    pub segmentation: Tensor,
    /// UV coordinates: (batch, 2, height, width)
    pub uv_coordinates: Tensor,
    /// Optional confidence scores
    pub confidence: Option<ConfidenceScores>,
}

/// Confidence scores for predictions
#[derive(Debug, Clone)]
pub struct ConfidenceScores {
    /// Segmentation confidence per pixel
    pub segmentation_confidence: Tensor,
    /// UV confidence per pixel
    pub uv_confidence: Tensor,
}

/// DensePose head for body part segmentation and UV regression
///
/// This is a pure inference implementation that works with pre-trained
/// weights stored in various formats (ONNX, SafeTensors, etc.)
#[derive(Debug)]
pub struct DensePoseHead {
    config: DensePoseConfig,
    /// Cached weights for native inference (optional)
    weights: Option<DensePoseWeights>,
}

/// Pre-trained weights for native Rust inference
#[derive(Debug, Clone)]
pub struct DensePoseWeights {
    /// Shared conv weights: Vec of (weight, bias) for each layer
    pub shared_conv: Vec<ConvLayerWeights>,
    /// Segmentation head weights
    pub segmentation_head: Vec<ConvLayerWeights>,
    /// UV regression head weights
    pub uv_head: Vec<ConvLayerWeights>,
}

/// Weights for a single conv layer
#[derive(Debug, Clone)]
pub struct ConvLayerWeights {
    /// Convolution weights: (out_channels, in_channels, kernel_h, kernel_w)
    pub weight: Array4<f32>,
    /// Bias: (out_channels,)
    pub bias: Option<ndarray::Array1<f32>>,
    /// Batch norm gamma
    pub bn_gamma: Option<ndarray::Array1<f32>>,
    /// Batch norm beta
    pub bn_beta: Option<ndarray::Array1<f32>>,
    /// Batch norm running mean
    pub bn_mean: Option<ndarray::Array1<f32>>,
    /// Batch norm running var
    pub bn_var: Option<ndarray::Array1<f32>>,
}

impl DensePoseHead {
    /// Create a new DensePose head with configuration
    pub fn new(config: DensePoseConfig) -> NnResult<Self> {
        config.validate()?;
        Ok(Self {
            config,
            weights: None,
        })
    }

    /// Create with pre-loaded weights for native inference
    pub fn with_weights(config: DensePoseConfig, weights: DensePoseWeights) -> NnResult<Self> {
        config.validate()?;
        Ok(Self {
            config,
            weights: Some(weights),
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &DensePoseConfig {
        &self.config
    }

    /// Check if weights are loaded for native inference
    pub fn has_weights(&self) -> bool {
        self.weights.is_some()
    }

    /// Get expected input shape for a given batch size
    pub fn expected_input_shape(&self, batch_size: usize, height: usize, width: usize) -> TensorShape {
        TensorShape::new(vec![batch_size, self.config.input_channels, height, width])
    }

    /// Validate input tensor shape
    pub fn validate_input(&self, input: &Tensor) -> NnResult<()> {
        let shape = input.shape();
        if shape.ndim() != 4 {
            return Err(NnError::shape_mismatch(
                vec![0, self.config.input_channels, 0, 0],
                shape.dims().to_vec(),
            ));
        }
        if shape.dim(1) != Some(self.config.input_channels) {
            return Err(NnError::invalid_input(format!(
                "Expected {} input channels, got {:?}",
                self.config.input_channels,
                shape.dim(1)
            )));
        }
        Ok(())
    }

    /// Forward pass through the DensePose head (native Rust implementation)
    ///
    /// This performs inference using loaded weights. For ONNX-based inference,
    /// use the ONNX backend directly.
    pub fn forward(&self, input: &Tensor) -> NnResult<DensePoseOutput> {
        self.validate_input(input)?;

        // If we have native weights, use them
        if let Some(ref _weights) = self.weights {
            self.forward_native(input)
        } else {
            // Return mock output for testing when no weights are loaded
            self.forward_mock(input)
        }
    }

    /// Native forward pass using loaded weights
    fn forward_native(&self, input: &Tensor) -> NnResult<DensePoseOutput> {
        let weights = self.weights.as_ref().ok_or_else(|| {
            NnError::inference("No weights loaded for native inference")
        })?;

        let input_arr = input.as_array4()?;
        let (batch, _channels, height, width) = input_arr.dim();

        // Apply shared convolutions
        let mut current = input_arr.clone();
        for layer_weights in &weights.shared_conv {
            current = self.apply_conv_layer(&current, layer_weights)?;
            current = self.apply_relu(&current);
        }

        // Segmentation branch
        let mut seg_features = current.clone();
        for layer_weights in &weights.segmentation_head {
            seg_features = self.apply_conv_layer(&seg_features, layer_weights)?;
        }

        // UV regression branch
        let mut uv_features = current;
        for layer_weights in &weights.uv_head {
            uv_features = self.apply_conv_layer(&uv_features, layer_weights)?;
        }
        // Apply sigmoid to normalize UV to [0, 1]
        uv_features = self.apply_sigmoid(&uv_features);

        Ok(DensePoseOutput {
            segmentation: Tensor::Float4D(seg_features),
            uv_coordinates: Tensor::Float4D(uv_features),
            confidence: None,
        })
    }

    /// Mock forward pass for testing
    fn forward_mock(&self, input: &Tensor) -> NnResult<DensePoseOutput> {
        let shape = input.shape();
        let batch = shape.dim(0).unwrap_or(1);
        let height = shape.dim(2).unwrap_or(64);
        let width = shape.dim(3).unwrap_or(64);

        // Output dimensions after upsampling (2x)
        let out_height = height * 2;
        let out_width = width * 2;

        // Create mock segmentation output
        let seg_shape = [batch, self.config.segmentation_channels(), out_height, out_width];
        let segmentation = Tensor::zeros_4d(seg_shape);

        // Create mock UV output
        let uv_shape = [batch, self.config.num_uv_coordinates, out_height, out_width];
        let uv_coordinates = Tensor::zeros_4d(uv_shape);

        Ok(DensePoseOutput {
            segmentation,
            uv_coordinates,
            confidence: None,
        })
    }

    /// Apply a convolution layer
    fn apply_conv_layer(&self, input: &Array4<f32>, weights: &ConvLayerWeights) -> NnResult<Array4<f32>> {
        let (batch, in_channels, in_height, in_width) = input.dim();
        let (out_channels, _, kernel_h, kernel_w) = weights.weight.dim();

        let pad_h = self.config.padding;
        let pad_w = self.config.padding;
        let out_height = in_height + 2 * pad_h - kernel_h + 1;
        let out_width = in_width + 2 * pad_w - kernel_w + 1;

        let mut output = Array4::zeros((batch, out_channels, out_height, out_width));

        // Simple convolution implementation (not optimized)
        for b in 0..batch {
            for oc in 0..out_channels {
                for oh in 0..out_height {
                    for ow in 0..out_width {
                        let mut sum = 0.0f32;
                        for ic in 0..in_channels {
                            for kh in 0..kernel_h {
                                for kw in 0..kernel_w {
                                    let ih = oh + kh;
                                    let iw = ow + kw;
                                    if ih >= pad_h && ih < in_height + pad_h
                                        && iw >= pad_w && iw < in_width + pad_w
                                    {
                                        let input_val = input[[b, ic, ih - pad_h, iw - pad_w]];
                                        sum += input_val * weights.weight[[oc, ic, kh, kw]];
                                    }
                                }
                            }
                        }
                        if let Some(ref bias) = weights.bias {
                            sum += bias[oc];
                        }
                        output[[b, oc, oh, ow]] = sum;
                    }
                }
            }
        }

        // Apply batch normalization if weights are present
        if let (Some(gamma), Some(beta), Some(mean), Some(var)) = (
            &weights.bn_gamma,
            &weights.bn_beta,
            &weights.bn_mean,
            &weights.bn_var,
        ) {
            let eps = 1e-5;
            for b in 0..batch {
                for c in 0..out_channels {
                    let scale = gamma[c] / (var[c] + eps).sqrt();
                    let shift = beta[c] - mean[c] * scale;
                    for h in 0..out_height {
                        for w in 0..out_width {
                            output[[b, c, h, w]] = output[[b, c, h, w]] * scale + shift;
                        }
                    }
                }
            }
        }

        Ok(output)
    }

    /// Apply ReLU activation
    fn apply_relu(&self, input: &Array4<f32>) -> Array4<f32> {
        input.mapv(|x| x.max(0.0))
    }

    /// Apply sigmoid activation
    fn apply_sigmoid(&self, input: &Array4<f32>) -> Array4<f32> {
        input.mapv(|x| 1.0 / (1.0 + (-x).exp()))
    }

    /// Post-process predictions to get final output
    pub fn post_process(&self, output: &DensePoseOutput) -> NnResult<PostProcessedOutput> {
        // Get body part predictions (argmax over channels)
        let body_parts = output.segmentation.argmax(1)?;

        // Compute confidence scores
        let seg_confidence = self.compute_segmentation_confidence(&output.segmentation)?;
        let uv_confidence = self.compute_uv_confidence(&output.uv_coordinates)?;

        Ok(PostProcessedOutput {
            body_parts,
            uv_coordinates: output.uv_coordinates.clone(),
            segmentation_confidence: seg_confidence,
            uv_confidence,
        })
    }

    /// Compute segmentation confidence from logits
    fn compute_segmentation_confidence(&self, logits: &Tensor) -> NnResult<Tensor> {
        // Apply softmax and take max probability
        let probs = logits.softmax(1)?;
        // For simplicity, return the softmax output
        // In a full implementation, we'd compute max along channel axis
        Ok(probs)
    }

    /// Compute UV confidence from predictions
    fn compute_uv_confidence(&self, uv: &Tensor) -> NnResult<Tensor> {
        // UV confidence based on prediction variance
        // Higher confidence where predictions are more consistent
        let std = uv.std()?;
        let confidence_val = 1.0 / (1.0 + std);

        // Return a tensor with constant confidence for now
        let shape = uv.shape();
        let arr = Array4::from_elem(
            (shape.dim(0).unwrap_or(1), 1, shape.dim(2).unwrap_or(1), shape.dim(3).unwrap_or(1)),
            confidence_val,
        );
        Ok(Tensor::Float4D(arr))
    }

    /// Get feature statistics for debugging
    pub fn get_output_stats(&self, output: &DensePoseOutput) -> NnResult<HashMap<String, TensorStats>> {
        let mut stats = HashMap::new();
        stats.insert("segmentation".to_string(), TensorStats::from_tensor(&output.segmentation)?);
        stats.insert("uv_coordinates".to_string(), TensorStats::from_tensor(&output.uv_coordinates)?);
        Ok(stats)
    }
}

/// Post-processed output with final predictions
#[derive(Debug, Clone)]
pub struct PostProcessedOutput {
    /// Body part labels per pixel
    pub body_parts: Tensor,
    /// UV coordinates
    pub uv_coordinates: Tensor,
    /// Segmentation confidence
    pub segmentation_confidence: Tensor,
    /// UV confidence
    pub uv_confidence: Tensor,
}

/// Body part labels according to DensePose specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BodyPart {
    /// Background (no body)
    Background = 0,
    /// Torso
    Torso = 1,
    /// Right hand
    RightHand = 2,
    /// Left hand
    LeftHand = 3,
    /// Left foot
    LeftFoot = 4,
    /// Right foot
    RightFoot = 5,
    /// Upper leg right
    UpperLegRight = 6,
    /// Upper leg left
    UpperLegLeft = 7,
    /// Lower leg right
    LowerLegRight = 8,
    /// Lower leg left
    LowerLegLeft = 9,
    /// Upper arm left
    UpperArmLeft = 10,
    /// Upper arm right
    UpperArmRight = 11,
    /// Lower arm left
    LowerArmLeft = 12,
    /// Lower arm right
    LowerArmRight = 13,
    /// Head
    Head = 14,
}

impl BodyPart {
    /// Get body part from index
    pub fn from_index(idx: u8) -> Option<Self> {
        match idx {
            0 => Some(BodyPart::Background),
            1 => Some(BodyPart::Torso),
            2 => Some(BodyPart::RightHand),
            3 => Some(BodyPart::LeftHand),
            4 => Some(BodyPart::LeftFoot),
            5 => Some(BodyPart::RightFoot),
            6 => Some(BodyPart::UpperLegRight),
            7 => Some(BodyPart::UpperLegLeft),
            8 => Some(BodyPart::LowerLegRight),
            9 => Some(BodyPart::LowerLegLeft),
            10 => Some(BodyPart::UpperArmLeft),
            11 => Some(BodyPart::UpperArmRight),
            12 => Some(BodyPart::LowerArmLeft),
            13 => Some(BodyPart::LowerArmRight),
            14 => Some(BodyPart::Head),
            _ => None,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            BodyPart::Background => "Background",
            BodyPart::Torso => "Torso",
            BodyPart::RightHand => "Right Hand",
            BodyPart::LeftHand => "Left Hand",
            BodyPart::LeftFoot => "Left Foot",
            BodyPart::RightFoot => "Right Foot",
            BodyPart::UpperLegRight => "Upper Leg Right",
            BodyPart::UpperLegLeft => "Upper Leg Left",
            BodyPart::LowerLegRight => "Lower Leg Right",
            BodyPart::LowerLegLeft => "Lower Leg Left",
            BodyPart::UpperArmLeft => "Upper Arm Left",
            BodyPart::UpperArmRight => "Upper Arm Right",
            BodyPart::LowerArmLeft => "Lower Arm Left",
            BodyPart::LowerArmRight => "Lower Arm Right",
            BodyPart::Head => "Head",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = DensePoseConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = DensePoseConfig {
            input_channels: 0,
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_densepose_head_creation() {
        let config = DensePoseConfig::new(256, 24, 2);
        let head = DensePoseHead::new(config).unwrap();
        assert!(!head.has_weights());
    }

    #[test]
    fn test_mock_forward_pass() {
        let config = DensePoseConfig::new(256, 24, 2);
        let head = DensePoseHead::new(config).unwrap();

        let input = Tensor::zeros_4d([1, 256, 64, 64]);
        let output = head.forward(&input).unwrap();

        // Check output shapes
        assert_eq!(output.segmentation.shape().dim(1), Some(25)); // 24 + 1 background
        assert_eq!(output.uv_coordinates.shape().dim(1), Some(2));
    }

    #[test]
    fn test_body_part_enum() {
        assert_eq!(BodyPart::from_index(0), Some(BodyPart::Background));
        assert_eq!(BodyPart::from_index(14), Some(BodyPart::Head));
        assert_eq!(BodyPart::from_index(100), None);

        assert_eq!(BodyPart::Torso.name(), "Torso");
    }
}
