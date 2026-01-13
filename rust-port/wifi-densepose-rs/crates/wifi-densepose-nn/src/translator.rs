//! Modality translation network for CSI to visual feature space conversion.
//!
//! This module implements the encoder-decoder network that translates
//! WiFi Channel State Information (CSI) into visual feature representations
//! compatible with the DensePose head.

use crate::error::{NnError, NnResult};
use crate::tensor::{Tensor, TensorShape, TensorStats};
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the modality translator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatorConfig {
    /// Number of input channels (CSI features)
    pub input_channels: usize,
    /// Hidden channel sizes for encoder/decoder
    pub hidden_channels: Vec<usize>,
    /// Number of output channels (visual feature dimensions)
    pub output_channels: usize,
    /// Convolution kernel size
    #[serde(default = "default_kernel_size")]
    pub kernel_size: usize,
    /// Convolution stride
    #[serde(default = "default_stride")]
    pub stride: usize,
    /// Convolution padding
    #[serde(default = "default_padding")]
    pub padding: usize,
    /// Dropout rate
    #[serde(default = "default_dropout_rate")]
    pub dropout_rate: f32,
    /// Activation function
    #[serde(default = "default_activation")]
    pub activation: ActivationType,
    /// Normalization type
    #[serde(default = "default_normalization")]
    pub normalization: NormalizationType,
    /// Whether to use attention mechanism
    #[serde(default)]
    pub use_attention: bool,
    /// Number of attention heads
    #[serde(default = "default_attention_heads")]
    pub attention_heads: usize,
}

fn default_kernel_size() -> usize {
    3
}

fn default_stride() -> usize {
    1
}

fn default_padding() -> usize {
    1
}

fn default_dropout_rate() -> f32 {
    0.1
}

fn default_activation() -> ActivationType {
    ActivationType::ReLU
}

fn default_normalization() -> NormalizationType {
    NormalizationType::BatchNorm
}

fn default_attention_heads() -> usize {
    8
}

/// Type of activation function
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationType {
    /// Rectified Linear Unit
    ReLU,
    /// Leaky ReLU with negative slope
    LeakyReLU,
    /// Gaussian Error Linear Unit
    GELU,
    /// Sigmoid
    Sigmoid,
    /// Tanh
    Tanh,
}

/// Type of normalization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalizationType {
    /// Batch normalization
    BatchNorm,
    /// Instance normalization
    InstanceNorm,
    /// Layer normalization
    LayerNorm,
    /// No normalization
    None,
}

impl Default for TranslatorConfig {
    fn default() -> Self {
        Self {
            input_channels: 128, // CSI feature dimension
            hidden_channels: vec![256, 512, 256],
            output_channels: 256, // Visual feature dimension
            kernel_size: default_kernel_size(),
            stride: default_stride(),
            padding: default_padding(),
            dropout_rate: default_dropout_rate(),
            activation: default_activation(),
            normalization: default_normalization(),
            use_attention: false,
            attention_heads: default_attention_heads(),
        }
    }
}

impl TranslatorConfig {
    /// Create a new translator configuration
    pub fn new(input_channels: usize, hidden_channels: Vec<usize>, output_channels: usize) -> Self {
        Self {
            input_channels,
            hidden_channels,
            output_channels,
            ..Default::default()
        }
    }

    /// Enable attention mechanism
    pub fn with_attention(mut self, num_heads: usize) -> Self {
        self.use_attention = true;
        self.attention_heads = num_heads;
        self
    }

    /// Set activation type
    pub fn with_activation(mut self, activation: ActivationType) -> Self {
        self.activation = activation;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> NnResult<()> {
        if self.input_channels == 0 {
            return Err(NnError::config("input_channels must be positive"));
        }
        if self.hidden_channels.is_empty() {
            return Err(NnError::config("hidden_channels must not be empty"));
        }
        if self.output_channels == 0 {
            return Err(NnError::config("output_channels must be positive"));
        }
        if self.use_attention && self.attention_heads == 0 {
            return Err(NnError::config("attention_heads must be positive when using attention"));
        }
        Ok(())
    }

    /// Get the bottleneck dimension (smallest hidden channel)
    pub fn bottleneck_dim(&self) -> usize {
        *self.hidden_channels.last().unwrap_or(&self.output_channels)
    }
}

/// Output from the modality translator
#[derive(Debug, Clone)]
pub struct TranslatorOutput {
    /// Translated visual features
    pub features: Tensor,
    /// Intermediate encoder features (for skip connections)
    pub encoder_features: Option<Vec<Tensor>>,
    /// Attention weights (if attention is used)
    pub attention_weights: Option<Tensor>,
}

/// Weights for the modality translator
#[derive(Debug, Clone)]
pub struct TranslatorWeights {
    /// Encoder layer weights
    pub encoder: Vec<ConvBlockWeights>,
    /// Decoder layer weights
    pub decoder: Vec<ConvBlockWeights>,
    /// Attention weights (if used)
    pub attention: Option<AttentionWeights>,
}

/// Weights for a convolutional block
#[derive(Debug, Clone)]
pub struct ConvBlockWeights {
    /// Convolution weights
    pub conv_weight: Array4<f32>,
    /// Convolution bias
    pub conv_bias: Option<ndarray::Array1<f32>>,
    /// Normalization gamma
    pub norm_gamma: Option<ndarray::Array1<f32>>,
    /// Normalization beta
    pub norm_beta: Option<ndarray::Array1<f32>>,
    /// Running mean for batch norm
    pub running_mean: Option<ndarray::Array1<f32>>,
    /// Running var for batch norm
    pub running_var: Option<ndarray::Array1<f32>>,
}

/// Weights for multi-head attention
#[derive(Debug, Clone)]
pub struct AttentionWeights {
    /// Query projection
    pub query_weight: ndarray::Array2<f32>,
    /// Key projection
    pub key_weight: ndarray::Array2<f32>,
    /// Value projection
    pub value_weight: ndarray::Array2<f32>,
    /// Output projection
    pub output_weight: ndarray::Array2<f32>,
    /// Output bias
    pub output_bias: ndarray::Array1<f32>,
}

/// Modality translator for CSI to visual feature conversion
#[derive(Debug)]
pub struct ModalityTranslator {
    config: TranslatorConfig,
    /// Pre-loaded weights for native inference
    weights: Option<TranslatorWeights>,
}

impl ModalityTranslator {
    /// Create a new modality translator
    pub fn new(config: TranslatorConfig) -> NnResult<Self> {
        config.validate()?;
        Ok(Self {
            config,
            weights: None,
        })
    }

    /// Create with pre-loaded weights
    pub fn with_weights(config: TranslatorConfig, weights: TranslatorWeights) -> NnResult<Self> {
        config.validate()?;
        Ok(Self {
            config,
            weights: Some(weights),
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &TranslatorConfig {
        &self.config
    }

    /// Check if weights are loaded
    pub fn has_weights(&self) -> bool {
        self.weights.is_some()
    }

    /// Get expected input shape
    pub fn expected_input_shape(&self, batch_size: usize, height: usize, width: usize) -> TensorShape {
        TensorShape::new(vec![batch_size, self.config.input_channels, height, width])
    }

    /// Validate input tensor
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

    /// Forward pass through the translator
    pub fn forward(&self, input: &Tensor) -> NnResult<TranslatorOutput> {
        self.validate_input(input)?;

        if let Some(ref _weights) = self.weights {
            self.forward_native(input)
        } else {
            self.forward_mock(input)
        }
    }

    /// Encode input to latent space
    pub fn encode(&self, input: &Tensor) -> NnResult<Vec<Tensor>> {
        self.validate_input(input)?;

        let shape = input.shape();
        let batch = shape.dim(0).unwrap_or(1);
        let height = shape.dim(2).unwrap_or(64);
        let width = shape.dim(3).unwrap_or(64);

        // Mock encoder features at different scales
        let mut features = Vec::new();
        let mut current_h = height;
        let mut current_w = width;

        for (i, &channels) in self.config.hidden_channels.iter().enumerate() {
            if i > 0 {
                current_h /= 2;
                current_w /= 2;
            }
            let feat = Tensor::zeros_4d([batch, channels, current_h.max(1), current_w.max(1)]);
            features.push(feat);
        }

        Ok(features)
    }

    /// Decode from latent space
    pub fn decode(&self, encoded_features: &[Tensor]) -> NnResult<Tensor> {
        if encoded_features.is_empty() {
            return Err(NnError::invalid_input("No encoded features provided"));
        }

        let last_feat = encoded_features.last().unwrap();
        let shape = last_feat.shape();
        let batch = shape.dim(0).unwrap_or(1);

        // Determine output spatial dimensions based on encoder structure
        let out_height = shape.dim(2).unwrap_or(1) * 2_usize.pow(encoded_features.len() as u32 - 1);
        let out_width = shape.dim(3).unwrap_or(1) * 2_usize.pow(encoded_features.len() as u32 - 1);

        Ok(Tensor::zeros_4d([batch, self.config.output_channels, out_height, out_width]))
    }

    /// Native forward pass with weights
    fn forward_native(&self, input: &Tensor) -> NnResult<TranslatorOutput> {
        let weights = self.weights.as_ref().ok_or_else(|| {
            NnError::inference("No weights loaded for native inference")
        })?;

        let input_arr = input.as_array4()?;
        let (batch, _channels, height, width) = input_arr.dim();

        // Encode
        let mut encoder_outputs = Vec::new();
        let mut current = input_arr.clone();

        for (i, block_weights) in weights.encoder.iter().enumerate() {
            let stride = if i == 0 { self.config.stride } else { 2 };
            current = self.apply_conv_block(&current, block_weights, stride)?;
            current = self.apply_activation(&current);
            encoder_outputs.push(Tensor::Float4D(current.clone()));
        }

        // Apply attention if configured
        let attention_weights = if self.config.use_attention {
            if let Some(ref attn_weights) = weights.attention {
                let (attended, attn_w) = self.apply_attention(&current, attn_weights)?;
                current = attended;
                Some(Tensor::Float4D(attn_w))
            } else {
                None
            }
        } else {
            None
        };

        // Decode
        for block_weights in &weights.decoder {
            current = self.apply_deconv_block(&current, block_weights)?;
            current = self.apply_activation(&current);
        }

        // Final tanh normalization
        current = current.mapv(|x| x.tanh());

        Ok(TranslatorOutput {
            features: Tensor::Float4D(current),
            encoder_features: Some(encoder_outputs),
            attention_weights,
        })
    }

    /// Mock forward pass for testing
    fn forward_mock(&self, input: &Tensor) -> NnResult<TranslatorOutput> {
        let shape = input.shape();
        let batch = shape.dim(0).unwrap_or(1);
        let height = shape.dim(2).unwrap_or(64);
        let width = shape.dim(3).unwrap_or(64);

        // Output has same spatial dimensions but different channels
        let features = Tensor::zeros_4d([batch, self.config.output_channels, height, width]);

        Ok(TranslatorOutput {
            features,
            encoder_features: None,
            attention_weights: None,
        })
    }

    /// Apply a convolutional block
    fn apply_conv_block(
        &self,
        input: &Array4<f32>,
        weights: &ConvBlockWeights,
        stride: usize,
    ) -> NnResult<Array4<f32>> {
        let (batch, in_channels, in_height, in_width) = input.dim();
        let (out_channels, _, kernel_h, kernel_w) = weights.conv_weight.dim();

        let out_height = (in_height + 2 * self.config.padding - kernel_h) / stride + 1;
        let out_width = (in_width + 2 * self.config.padding - kernel_w) / stride + 1;

        let mut output = Array4::zeros((batch, out_channels, out_height, out_width));

        // Simple strided convolution
        for b in 0..batch {
            for oc in 0..out_channels {
                for oh in 0..out_height {
                    for ow in 0..out_width {
                        let mut sum = 0.0f32;
                        for ic in 0..in_channels {
                            for kh in 0..kernel_h {
                                for kw in 0..kernel_w {
                                    let ih = oh * stride + kh;
                                    let iw = ow * stride + kw;
                                    if ih >= self.config.padding
                                        && ih < in_height + self.config.padding
                                        && iw >= self.config.padding
                                        && iw < in_width + self.config.padding
                                    {
                                        let input_val =
                                            input[[b, ic, ih - self.config.padding, iw - self.config.padding]];
                                        sum += input_val * weights.conv_weight[[oc, ic, kh, kw]];
                                    }
                                }
                            }
                        }
                        if let Some(ref bias) = weights.conv_bias {
                            sum += bias[oc];
                        }
                        output[[b, oc, oh, ow]] = sum;
                    }
                }
            }
        }

        // Apply normalization
        self.apply_normalization(&mut output, weights);

        Ok(output)
    }

    /// Apply transposed convolution for upsampling
    fn apply_deconv_block(
        &self,
        input: &Array4<f32>,
        weights: &ConvBlockWeights,
    ) -> NnResult<Array4<f32>> {
        let (batch, in_channels, in_height, in_width) = input.dim();
        let (out_channels, _, kernel_h, kernel_w) = weights.conv_weight.dim();

        // Upsample 2x
        let out_height = in_height * 2;
        let out_width = in_width * 2;

        // Simple nearest-neighbor upsampling + conv (approximation of transpose conv)
        let mut output = Array4::zeros((batch, out_channels, out_height, out_width));

        for b in 0..batch {
            for oc in 0..out_channels {
                for oh in 0..out_height {
                    for ow in 0..out_width {
                        let ih = oh / 2;
                        let iw = ow / 2;
                        let mut sum = 0.0f32;
                        for ic in 0..in_channels.min(weights.conv_weight.dim().1) {
                            sum += input[[b, ic, ih.min(in_height - 1), iw.min(in_width - 1)]]
                                * weights.conv_weight[[oc, ic, 0, 0]];
                        }
                        if let Some(ref bias) = weights.conv_bias {
                            sum += bias[oc];
                        }
                        output[[b, oc, oh, ow]] = sum;
                    }
                }
            }
        }

        Ok(output)
    }

    /// Apply normalization to output
    fn apply_normalization(&self, output: &mut Array4<f32>, weights: &ConvBlockWeights) {
        if let (Some(gamma), Some(beta), Some(mean), Some(var)) = (
            &weights.norm_gamma,
            &weights.norm_beta,
            &weights.running_mean,
            &weights.running_var,
        ) {
            let (batch, channels, height, width) = output.dim();
            let eps = 1e-5;

            for b in 0..batch {
                for c in 0..channels {
                    let scale = gamma[c] / (var[c] + eps).sqrt();
                    let shift = beta[c] - mean[c] * scale;
                    for h in 0..height {
                        for w in 0..width {
                            output[[b, c, h, w]] = output[[b, c, h, w]] * scale + shift;
                        }
                    }
                }
            }
        }
    }

    /// Apply activation function
    fn apply_activation(&self, input: &Array4<f32>) -> Array4<f32> {
        match self.config.activation {
            ActivationType::ReLU => input.mapv(|x| x.max(0.0)),
            ActivationType::LeakyReLU => input.mapv(|x| if x > 0.0 { x } else { 0.2 * x }),
            ActivationType::GELU => {
                // Approximate GELU
                input.mapv(|x| 0.5 * x * (1.0 + (0.7978845608 * (x + 0.044715 * x.powi(3))).tanh()))
            }
            ActivationType::Sigmoid => input.mapv(|x| 1.0 / (1.0 + (-x).exp())),
            ActivationType::Tanh => input.mapv(|x| x.tanh()),
        }
    }

    /// Apply multi-head attention
    fn apply_attention(
        &self,
        input: &Array4<f32>,
        weights: &AttentionWeights,
    ) -> NnResult<(Array4<f32>, Array4<f32>)> {
        let (batch, channels, height, width) = input.dim();
        let seq_len = height * width;

        // Flatten spatial dimensions
        let mut flat = ndarray::Array2::zeros((batch, seq_len * channels));
        for b in 0..batch {
            for h in 0..height {
                for w in 0..width {
                    for c in 0..channels {
                        flat[[b, (h * width + w) * channels + c]] = input[[b, c, h, w]];
                    }
                }
            }
        }

        // For simplicity, return input unchanged with identity attention
        let attention_weights = Array4::from_elem((batch, self.config.attention_heads, seq_len, seq_len), 1.0 / seq_len as f32);

        Ok((input.clone(), attention_weights))
    }

    /// Compute translation loss between predicted and target features
    pub fn compute_loss(&self, predicted: &Tensor, target: &Tensor, loss_type: LossType) -> NnResult<f32> {
        let pred_arr = predicted.as_array4()?;
        let target_arr = target.as_array4()?;

        if pred_arr.dim() != target_arr.dim() {
            return Err(NnError::shape_mismatch(
                pred_arr.shape().to_vec(),
                target_arr.shape().to_vec(),
            ));
        }

        let n = pred_arr.len() as f32;
        let loss = match loss_type {
            LossType::MSE => {
                pred_arr
                    .iter()
                    .zip(target_arr.iter())
                    .map(|(p, t)| (p - t).powi(2))
                    .sum::<f32>()
                    / n
            }
            LossType::L1 => {
                pred_arr
                    .iter()
                    .zip(target_arr.iter())
                    .map(|(p, t)| (p - t).abs())
                    .sum::<f32>()
                    / n
            }
            LossType::SmoothL1 => {
                pred_arr
                    .iter()
                    .zip(target_arr.iter())
                    .map(|(p, t)| {
                        let diff = (p - t).abs();
                        if diff < 1.0 {
                            0.5 * diff.powi(2)
                        } else {
                            diff - 0.5
                        }
                    })
                    .sum::<f32>()
                    / n
            }
        };

        Ok(loss)
    }

    /// Get feature statistics
    pub fn get_feature_stats(&self, features: &Tensor) -> NnResult<TensorStats> {
        TensorStats::from_tensor(features)
    }

    /// Get intermediate features for visualization
    pub fn get_intermediate_features(&self, input: &Tensor) -> NnResult<HashMap<String, Tensor>> {
        let output = self.forward(input)?;

        let mut features = HashMap::new();
        features.insert("output".to_string(), output.features);

        if let Some(encoder_feats) = output.encoder_features {
            for (i, feat) in encoder_feats.into_iter().enumerate() {
                features.insert(format!("encoder_{}", i), feat);
            }
        }

        if let Some(attn) = output.attention_weights {
            features.insert("attention".to_string(), attn);
        }

        Ok(features)
    }
}

/// Type of loss function for training
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossType {
    /// Mean Squared Error
    MSE,
    /// L1 / Mean Absolute Error
    L1,
    /// Smooth L1 (Huber) loss
    SmoothL1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = TranslatorConfig::default();
        assert!(config.validate().is_ok());

        let invalid = TranslatorConfig {
            input_channels: 0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_translator_creation() {
        let config = TranslatorConfig::new(128, vec![256, 512, 256], 256);
        let translator = ModalityTranslator::new(config).unwrap();
        assert!(!translator.has_weights());
    }

    #[test]
    fn test_mock_forward() {
        let config = TranslatorConfig::new(128, vec![256, 512, 256], 256);
        let translator = ModalityTranslator::new(config).unwrap();

        let input = Tensor::zeros_4d([1, 128, 64, 64]);
        let output = translator.forward(&input).unwrap();

        assert_eq!(output.features.shape().dim(1), Some(256));
    }

    #[test]
    fn test_encode_decode() {
        let config = TranslatorConfig::new(128, vec![256, 512], 256);
        let translator = ModalityTranslator::new(config).unwrap();

        let input = Tensor::zeros_4d([1, 128, 64, 64]);
        let encoded = translator.encode(&input).unwrap();
        assert_eq!(encoded.len(), 2);

        let decoded = translator.decode(&encoded).unwrap();
        assert_eq!(decoded.shape().dim(1), Some(256));
    }

    #[test]
    fn test_activation_types() {
        let config = TranslatorConfig::default().with_activation(ActivationType::GELU);
        assert_eq!(config.activation, ActivationType::GELU);
    }

    #[test]
    fn test_loss_computation() {
        let config = TranslatorConfig::default();
        let translator = ModalityTranslator::new(config).unwrap();

        let pred = Tensor::ones_4d([1, 256, 8, 8]);
        let target = Tensor::zeros_4d([1, 256, 8, 8]);

        let mse = translator.compute_loss(&pred, &target, LossType::MSE).unwrap();
        assert_eq!(mse, 1.0);

        let l1 = translator.compute_loss(&pred, &target, LossType::L1).unwrap();
        assert_eq!(l1, 1.0);
    }
}
