//! # P2P Federated Learning with Gradient Gossip
//!
//! Decentralized federated learning without a central coordinator.
//! Uses gossip protocol for gradient sharing with reputation-weighted aggregation.
//!
//! ## Features
//!
//! - **TopK Sparsification**: 90% gradient compression with error feedback
//! - **Reputation-Weighted FedAvg**: High-reputation peers have more influence
//! - **Byzantine Tolerance**: Outlier detection, gradient clipping, and validation
//! - **Privacy Preservation**: Optional differential privacy noise injection
//! - **Gossip Protocol**: Eventually consistent, fully decentralized
//!
//! ## Architecture
//!
//! ```text
//! +------------------+     gossipsub      +------------------+
//! |   Node A         |<------------------>|   Node B         |
//! |  +-----------+   |                    |  +-----------+   |
//! |  | Local     |   |   GradientMessage  |  | Local     |   |
//! |  | Gradients |---+------------------->|  | Gradients |   |
//! |  +-----------+   |                    |  +-----------+   |
//! |       |          |                    |       |          |
//! |  +-----------+   |                    |  +-----------+   |
//! |  | Sparsifier|   |                    |  | Sparsifier|   |
//! |  | (TopK)    |   |                    |  | (TopK)    |   |
//! |  +-----------+   |                    |  +-----------+   |
//! |       |          |                    |       |          |
//! |  +-----------+   |                    |  +-----------+   |
//! |  | Aggregator|   |                    |  | Aggregator|   |
//! |  | (FedAvg)  |   |                    |  | (FedAvg)  |   |
//! |  +-----------+   |                    |  +-----------+   |
//! +------------------+                    +------------------+
//! ```
//!
//! ## References
//!
//! - [TopK Gradient Compression](https://arxiv.org/abs/1712.01887)
//! - [Gossip Learning](https://arxiv.org/abs/1109.1396)
//! - [Byzantine-Robust FL](https://arxiv.org/abs/1912.00137)

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::{RwLock, atomic::{AtomicU64, Ordering}};

// ============================================================================
// Cross-Platform Utilities
// ============================================================================

/// Get current timestamp in milliseconds (works in both WASM and native)
#[inline]
fn current_timestamp_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// ============================================================================
// Types
// ============================================================================

/// Peer identifier (32-byte public key)
pub type PeerId = [u8; 32];

/// Gossipsub topic for gradient sharing
pub const TOPIC_GRADIENT_GOSSIP: &str = "/ruvector/federated/gradients/1.0.0";

/// Gossipsub topic for model synchronization
pub const TOPIC_MODEL_SYNC: &str = "/ruvector/federated/model/1.0.0";

// ============================================================================
// Sparse Gradient Representation
// ============================================================================

/// Sparse gradient representation for efficient transmission
/// Only stores top-k indices and values (90% compression)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SparseGradient {
    /// Indices of non-zero gradients
    pub indices: Vec<u32>,
    /// Values at those indices
    pub values: Vec<f32>,
    /// Original vector dimension
    pub dimension: usize,
    /// Compression ratio achieved
    pub compression_ratio: f32,
}

impl SparseGradient {
    /// Create a new sparse gradient
    pub fn new(dimension: usize) -> Self {
        Self {
            indices: Vec::new(),
            values: Vec::new(),
            dimension,
            compression_ratio: 0.0,
        }
    }

    /// Decompress to full dense gradient vector
    pub fn decompress(&self) -> Vec<f32> {
        let mut dense = vec![0.0f32; self.dimension];
        for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
            if (idx as usize) < self.dimension {
                dense[idx as usize] = val;
            }
        }
        dense
    }

    /// Number of non-zero entries
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

// ============================================================================
// TopK Sparsifier with Error Feedback
// ============================================================================

/// TopK gradient sparsifier with error feedback for accuracy preservation
///
/// Error feedback accumulates residuals from previous rounds to prevent
/// information loss from aggressive compression.
#[wasm_bindgen]
pub struct TopKSparsifier {
    /// Fraction of gradients to keep (e.g., 0.1 = top 10%)
    k_ratio: f32,
    /// Error feedback buffer (accumulated residuals)
    error_feedback: RwLock<Vec<f32>>,
    /// Whether to use absolute value for selection
    use_abs: bool,
    /// Minimum threshold for including a gradient
    min_threshold: f32,
}

#[wasm_bindgen]
impl TopKSparsifier {
    /// Create a new TopK sparsifier
    ///
    /// # Arguments
    /// * `k_ratio` - Fraction of gradients to keep (0.1 = top 10%)
    #[wasm_bindgen(constructor)]
    pub fn new(k_ratio: f32) -> Self {
        Self {
            k_ratio: k_ratio.clamp(0.01, 1.0),
            error_feedback: RwLock::new(Vec::new()),
            use_abs: true,
            min_threshold: 1e-8,
        }
    }

    /// Get compression ratio
    #[wasm_bindgen(js_name = getCompressionRatio)]
    pub fn get_compression_ratio(&self) -> f32 {
        1.0 - self.k_ratio
    }

    /// Get error feedback buffer size
    #[wasm_bindgen(js_name = getErrorBufferSize)]
    pub fn get_error_buffer_size(&self) -> usize {
        self.error_feedback.read().unwrap().len()
    }

    /// Reset error feedback buffer
    #[wasm_bindgen(js_name = resetErrorFeedback)]
    pub fn reset_error_feedback(&self) {
        self.error_feedback.write().unwrap().clear();
    }
}

impl TopKSparsifier {
    /// Create with custom threshold
    pub fn with_threshold(k_ratio: f32, min_threshold: f32) -> Self {
        Self {
            k_ratio: k_ratio.clamp(0.01, 1.0),
            error_feedback: RwLock::new(Vec::new()),
            use_abs: true,
            min_threshold,
        }
    }

    /// Compress gradients using TopK selection with error feedback
    ///
    /// This implements the error feedback mechanism from "Deep Gradient Compression"
    /// which accumulates residuals to prevent information loss.
    pub fn compress(&self, gradients: &[f32]) -> SparseGradient {
        let n = gradients.len();
        let k = ((n as f32) * self.k_ratio).ceil() as usize;
        let k = k.max(1).min(n);

        // Add error feedback from previous round
        let mut accumulated = {
            let error = self.error_feedback.read().unwrap();
            if error.len() == n {
                gradients.iter()
                    .zip(error.iter())
                    .map(|(g, e)| g + e)
                    .collect::<Vec<_>>()
            } else {
                gradients.to_vec()
            }
        };

        // Create index-value pairs with absolute values for sorting
        let mut indexed: Vec<(usize, f32, f32)> = accumulated.iter()
            .enumerate()
            .map(|(i, &v)| (i, v, if self.use_abs { v.abs() } else { v }))
            .filter(|(_, _, abs_v)| *abs_v >= self.min_threshold)
            .collect();

        // Sort by absolute magnitude (descending)
        indexed.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        indexed.truncate(k);

        // Build sparse gradient
        let mut sparse = SparseGradient::new(n);
        sparse.indices.reserve(indexed.len());
        sparse.values.reserve(indexed.len());

        for (idx, val, _) in &indexed {
            sparse.indices.push(*idx as u32);
            sparse.values.push(*val);
            // Zero out selected entries in accumulated for error calculation
            accumulated[*idx] = 0.0;
        }

        sparse.compression_ratio = if n > 0 {
            1.0 - (sparse.nnz() as f32 / n as f32)
        } else {
            0.0
        };

        // Store residuals as error feedback for next round
        *self.error_feedback.write().unwrap() = accumulated;

        sparse
    }

    /// Compress without error feedback (stateless)
    pub fn compress_stateless(&self, gradients: &[f32]) -> SparseGradient {
        let n = gradients.len();
        let k = ((n as f32) * self.k_ratio).ceil() as usize;
        let k = k.max(1).min(n);

        let mut indexed: Vec<(usize, f32, f32)> = gradients.iter()
            .enumerate()
            .map(|(i, &v)| (i, v, if self.use_abs { v.abs() } else { v }))
            .filter(|(_, _, abs_v)| *abs_v >= self.min_threshold)
            .collect();

        indexed.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        indexed.truncate(k);

        let mut sparse = SparseGradient::new(n);
        for (idx, val, _) in indexed {
            sparse.indices.push(idx as u32);
            sparse.values.push(val);
        }

        sparse.compression_ratio = if n > 0 {
            1.0 - (sparse.nnz() as f32 / n as f32)
        } else {
            0.0
        };

        sparse
    }
}

// ============================================================================
// Gradient Message Protocol
// ============================================================================

/// Gradient message for gossip protocol
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientMessage {
    /// Sender's peer ID
    pub sender: PeerId,
    /// Consensus round number
    pub round: u64,
    /// Sparse gradients
    pub gradients: SparseGradient,
    /// Ed25519 signature of the message
    pub signature: Vec<u8>,
    /// Timestamp (ms since epoch)
    pub timestamp: u64,
    /// Model version/hash for compatibility check
    pub model_hash: [u8; 32],
}

impl GradientMessage {
    /// Create a new unsigned gradient message
    pub fn new(sender: PeerId, round: u64, gradients: SparseGradient, model_hash: [u8; 32]) -> Self {
        Self {
            sender,
            round,
            gradients,
            signature: Vec::new(),
            timestamp: current_timestamp_ms(),
            model_hash,
        }
    }

    /// Serialize message for signing (excludes signature field)
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(&self.sender);
        bytes.extend_from_slice(&self.round.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.model_hash);

        // Include gradient data in signature
        bytes.extend_from_slice(&(self.gradients.dimension as u64).to_le_bytes());
        for (&idx, &val) in self.gradients.indices.iter().zip(self.gradients.values.iter()) {
            bytes.extend_from_slice(&idx.to_le_bytes());
            bytes.extend_from_slice(&val.to_le_bytes());
        }

        bytes
    }

    /// Serialize to bytes for network transmission
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Deserialization failed: {}", e))
    }
}

// ============================================================================
// Peer Gradient State
// ============================================================================

/// Stored gradient state from a peer
#[derive(Clone)]
struct PeerGradientState {
    /// Dense gradient vector
    gradients: Vec<f32>,
    /// Peer's reputation score
    reputation: f64,
    /// When received
    received_at: u64,
    /// Consensus round
    round: u64,
}

// ============================================================================
// Byzantine Detection
// ============================================================================

/// Byzantine gradient detection using statistical methods
#[wasm_bindgen]
pub struct ByzantineDetector {
    /// Maximum allowed gradient magnitude
    max_magnitude: f32,
    /// Z-score threshold for outlier detection
    zscore_threshold: f32,
    /// Minimum samples needed for statistical detection
    min_samples: usize,
}

#[wasm_bindgen]
impl ByzantineDetector {
    /// Create a new Byzantine detector
    #[wasm_bindgen(constructor)]
    pub fn new(max_magnitude: f32, zscore_threshold: f32) -> Self {
        Self {
            max_magnitude,
            zscore_threshold,
            min_samples: 3,
        }
    }

    /// Get maximum allowed magnitude
    #[wasm_bindgen(js_name = getMaxMagnitude)]
    pub fn get_max_magnitude(&self) -> f32 {
        self.max_magnitude
    }
}

impl ByzantineDetector {
    /// Check if gradients are within valid bounds
    pub fn is_valid_magnitude(&self, gradients: &[f32]) -> bool {
        gradients.iter().all(|&g| g.abs() <= self.max_magnitude && g.is_finite())
    }

    /// Clip gradients to maximum magnitude
    pub fn clip_gradients(&self, gradients: &mut [f32]) {
        for g in gradients.iter_mut() {
            if !g.is_finite() {
                *g = 0.0;
            } else if *g > self.max_magnitude {
                *g = self.max_magnitude;
            } else if *g < -self.max_magnitude {
                *g = -self.max_magnitude;
            }
        }
    }

    /// Detect outlier gradients using coordinate-wise median
    /// Returns indices of suspected Byzantine peers
    pub fn detect_outliers(&self, peer_gradients: &[(&PeerId, &[f32])]) -> Vec<PeerId> {
        if peer_gradients.len() < self.min_samples {
            return Vec::new();
        }

        let dim = peer_gradients.first().map(|(_, g)| g.len()).unwrap_or(0);
        if dim == 0 {
            return Vec::new();
        }

        // Compute coordinate-wise median and MAD
        let mut outlier_scores: FxHashMap<PeerId, f32> = FxHashMap::default();

        for coord in 0..dim {
            // Collect values at this coordinate
            let mut values: Vec<f32> = peer_gradients.iter()
                .filter_map(|(_, g)| g.get(coord).copied())
                .filter(|v| v.is_finite())
                .collect();

            if values.len() < self.min_samples {
                continue;
            }

            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Median
            let median = if values.len() % 2 == 0 {
                (values[values.len()/2 - 1] + values[values.len()/2]) / 2.0
            } else {
                values[values.len()/2]
            };

            // Median Absolute Deviation (MAD)
            let mut deviations: Vec<f32> = values.iter()
                .map(|v| (v - median).abs())
                .collect();
            deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let mad = if deviations.len() % 2 == 0 {
                (deviations[deviations.len()/2 - 1] + deviations[deviations.len()/2]) / 2.0
            } else {
                deviations[deviations.len()/2]
            };

            // Avoid division by zero
            let mad = if mad < 1e-8 { 1e-8 } else { mad };

            // Check each peer's deviation
            for (peer_id, grads) in peer_gradients {
                if let Some(&val) = grads.get(coord) {
                    let zscore = (val - median).abs() / (1.4826 * mad); // 1.4826 for normal distribution
                    if zscore > self.zscore_threshold {
                        *outlier_scores.entry(**peer_id).or_insert(0.0) += 1.0;
                    }
                }
            }
        }

        // Flag peers with too many outlier coordinates
        let outlier_threshold = (dim as f32) * 0.1; // More than 10% outlier coordinates
        outlier_scores.into_iter()
            .filter(|(_, score)| *score > outlier_threshold)
            .map(|(peer_id, _)| peer_id)
            .collect()
    }
}

// ============================================================================
// Differential Privacy
// ============================================================================

/// Differential privacy noise generator
#[wasm_bindgen]
pub struct DifferentialPrivacy {
    /// Privacy budget epsilon
    epsilon: f64,
    /// Gradient sensitivity (L2 norm bound)
    sensitivity: f64,
    /// Whether DP is enabled
    enabled: bool,
}

#[wasm_bindgen]
impl DifferentialPrivacy {
    /// Create a new differential privacy module
    #[wasm_bindgen(constructor)]
    pub fn new(epsilon: f64, sensitivity: f64) -> Self {
        Self {
            epsilon: epsilon.max(0.01),
            sensitivity: sensitivity.max(0.001),
            enabled: true,
        }
    }

    /// Get epsilon value
    #[wasm_bindgen(js_name = getEpsilon)]
    pub fn get_epsilon(&self) -> f64 {
        self.epsilon
    }

    /// Check if DP is enabled
    #[wasm_bindgen(js_name = isEnabled)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable differential privacy
    #[wasm_bindgen(js_name = setEnabled)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl DifferentialPrivacy {
    /// Compute noise scale for Gaussian mechanism
    fn noise_scale(&self) -> f64 {
        // For (epsilon, delta)-DP with delta = 1e-5
        let delta = 1e-5_f64;
        self.sensitivity * (2.0 * (1.25 / delta).ln()).sqrt() / self.epsilon
    }

    /// Add Gaussian noise to gradients for differential privacy
    pub fn add_noise(&self, gradients: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let scale = self.noise_scale() as f32;

        // Use simple PRNG seeded from timestamp for WASM compatibility
        let mut seed = current_timestamp_ms();

        for g in gradients.iter_mut() {
            // Box-Muller transform for Gaussian noise
            let u1 = {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                ((seed >> 16) & 0x7fff) as f32 / 32767.0
            }.max(1e-10);

            let u2 = {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                ((seed >> 16) & 0x7fff) as f32 / 32767.0
            };

            let noise = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
            *g += noise * scale;
        }
    }

    /// Clip gradients to bound sensitivity
    pub fn clip_l2(&self, gradients: &mut [f32]) {
        let l2_norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();

        if l2_norm > self.sensitivity as f32 {
            let scale = self.sensitivity as f32 / l2_norm;
            for g in gradients.iter_mut() {
                *g *= scale;
            }
        }
    }
}

// ============================================================================
// Gradient Gossip (Main Module)
// ============================================================================

/// P2P Gradient Gossip for decentralized federated learning
///
/// This is the main coordinator for federated learning without a central server.
#[wasm_bindgen]
pub struct GradientGossip {
    /// Local node's gradients
    local_gradients: RwLock<Vec<f32>>,
    /// Peer gradients: PeerId -> (gradients, reputation, received_at)
    peer_gradients: RwLock<FxHashMap<PeerId, PeerGradientState>>,
    /// Current consensus round
    consensus_round: AtomicU64,
    /// Gradient sparsifier
    sparsifier: TopKSparsifier,
    /// Byzantine detector
    byzantine_detector: ByzantineDetector,
    /// Differential privacy module
    dp: RwLock<DifferentialPrivacy>,
    /// Model hash for version compatibility
    model_hash: RwLock<[u8; 32]>,
    /// Our peer ID
    local_peer_id: PeerId,
    /// Maximum gradient staleness in rounds
    max_staleness: u64,
    /// Minimum reputation for participation
    min_reputation: f64,
}

#[wasm_bindgen]
impl GradientGossip {
    /// Create a new GradientGossip instance
    ///
    /// # Arguments
    /// * `local_peer_id` - 32-byte peer identifier
    /// * `dimension` - Gradient vector dimension
    /// * `k_ratio` - TopK sparsification ratio (0.1 = keep top 10%)
    #[wasm_bindgen(constructor)]
    pub fn new(local_peer_id: &[u8], dimension: usize, k_ratio: f32) -> Result<GradientGossip, JsValue> {
        if local_peer_id.len() != 32 {
            return Err(JsValue::from_str("Peer ID must be 32 bytes"));
        }

        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(local_peer_id);

        Ok(GradientGossip {
            local_gradients: RwLock::new(vec![0.0f32; dimension]),
            peer_gradients: RwLock::new(FxHashMap::default()),
            consensus_round: AtomicU64::new(0),
            sparsifier: TopKSparsifier::new(k_ratio),
            byzantine_detector: ByzantineDetector::new(100.0, 3.0),
            dp: RwLock::new(DifferentialPrivacy::new(1.0, 1.0)),
            model_hash: RwLock::new([0u8; 32]),
            local_peer_id: peer_id,
            max_staleness: 5,
            min_reputation: 0.1,
        })
    }

    /// Get current consensus round
    #[wasm_bindgen(js_name = getCurrentRound)]
    pub fn get_current_round(&self) -> u64 {
        self.consensus_round.load(Ordering::Relaxed)
    }

    /// Advance to next consensus round
    #[wasm_bindgen(js_name = advanceRound)]
    pub fn advance_round(&self) -> u64 {
        self.consensus_round.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get number of active peers
    #[wasm_bindgen(js_name = peerCount)]
    pub fn peer_count(&self) -> usize {
        self.peer_gradients.read().unwrap().len()
    }

    /// Get gradient dimension
    #[wasm_bindgen(js_name = getDimension)]
    pub fn get_dimension(&self) -> usize {
        self.local_gradients.read().unwrap().len()
    }

    /// Set local gradients from JavaScript
    #[wasm_bindgen(js_name = setLocalGradients)]
    pub fn set_local_gradients(&self, gradients: &[f32]) -> Result<(), JsValue> {
        let mut local = self.local_gradients.write().unwrap();
        if gradients.len() != local.len() {
            return Err(JsValue::from_str("Gradient dimension mismatch"));
        }
        local.copy_from_slice(gradients);
        Ok(())
    }

    /// Get aggregated gradients as JavaScript array
    #[wasm_bindgen(js_name = getAggregatedGradients)]
    pub fn get_aggregated_gradients(&self) -> Vec<f32> {
        self.aggregate()
    }

    /// Set model hash for version compatibility
    #[wasm_bindgen(js_name = setModelHash)]
    pub fn set_model_hash(&self, hash: &[u8]) -> Result<(), JsValue> {
        if hash.len() != 32 {
            return Err(JsValue::from_str("Model hash must be 32 bytes"));
        }
        let mut model_hash = self.model_hash.write().unwrap();
        model_hash.copy_from_slice(hash);
        Ok(())
    }

    /// Enable/disable differential privacy
    #[wasm_bindgen(js_name = setDPEnabled)]
    pub fn set_dp_enabled(&self, enabled: bool) {
        self.dp.write().unwrap().set_enabled(enabled);
    }

    /// Configure differential privacy
    #[wasm_bindgen(js_name = configureDifferentialPrivacy)]
    pub fn configure_dp(&self, epsilon: f64, sensitivity: f64) {
        let mut dp = self.dp.write().unwrap();
        *dp = DifferentialPrivacy::new(epsilon, sensitivity);
    }

    /// Get compression ratio achieved
    #[wasm_bindgen(js_name = getCompressionRatio)]
    pub fn get_compression_ratio(&self) -> f32 {
        self.sparsifier.get_compression_ratio()
    }

    /// Prune stale peer gradients
    #[wasm_bindgen(js_name = pruneStale)]
    pub fn prune_stale(&self) -> usize {
        let current_round = self.consensus_round.load(Ordering::Relaxed);
        let mut peers = self.peer_gradients.write().unwrap();
        let before = peers.len();

        peers.retain(|_, state| {
            current_round.saturating_sub(state.round) <= self.max_staleness
        });

        before - peers.len()
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let peers = self.peer_gradients.read().unwrap();
        let current_round = self.consensus_round.load(Ordering::Relaxed);
        let dimension = self.local_gradients.read().unwrap().len();

        let avg_reputation: f64 = if peers.is_empty() {
            0.0
        } else {
            peers.values().map(|s| s.reputation).sum::<f64>() / peers.len() as f64
        };

        format!(
            r#"{{"peers":{},"round":{},"dimension":{},"avg_reputation":{:.4},"compression":{:.2},"dp_enabled":{}}}"#,
            peers.len(),
            current_round,
            dimension,
            avg_reputation,
            self.get_compression_ratio() * 100.0,
            self.dp.read().unwrap().is_enabled()
        )
    }
}

impl GradientGossip {
    /// Create gradient message for sharing via gossipsub
    pub fn create_message(&self) -> Result<GradientMessage, String> {
        let gradients = self.local_gradients.read().unwrap();
        let model_hash = *self.model_hash.read().unwrap();
        let round = self.consensus_round.load(Ordering::Relaxed);

        // Apply differential privacy if enabled
        let mut grads = gradients.clone();
        {
            let dp = self.dp.read().unwrap();
            dp.clip_l2(&mut grads);
            dp.add_noise(&mut grads);
        }

        // Sparsify for compression
        let sparse = self.sparsifier.compress(&grads);

        Ok(GradientMessage::new(
            self.local_peer_id,
            round,
            sparse,
            model_hash,
        ))
    }

    /// Process received gradient message
    pub fn receive_message(&self, msg: &GradientMessage, sender_reputation: f64) -> Result<(), String> {
        // Check model compatibility
        let model_hash = *self.model_hash.read().unwrap();
        if msg.model_hash != model_hash && model_hash != [0u8; 32] {
            return Err("Model version mismatch".to_string());
        }

        // Check staleness
        let current_round = self.consensus_round.load(Ordering::Relaxed);
        if current_round.saturating_sub(msg.round) > self.max_staleness {
            return Err("Gradient too stale".to_string());
        }

        // Check reputation
        if sender_reputation < self.min_reputation {
            return Err("Sender reputation too low".to_string());
        }

        // Decompress gradients
        let gradients = msg.gradients.decompress();

        // Validate magnitude
        if !self.byzantine_detector.is_valid_magnitude(&gradients) {
            return Err("Invalid gradient magnitude".to_string());
        }

        // Store peer gradients
        let state = PeerGradientState {
            gradients,
            reputation: sender_reputation,
            received_at: current_timestamp_ms(),
            round: msg.round,
        };

        self.peer_gradients.write().unwrap().insert(msg.sender, state);
        Ok(())
    }

    /// Aggregate gradients using reputation-weighted FedAvg
    ///
    /// Returns the aggregated gradient vector combining local and peer gradients
    /// with reputation-based weighting.
    pub fn aggregate(&self) -> Vec<f32> {
        let local = self.local_gradients.read().unwrap();
        let peers = self.peer_gradients.read().unwrap();
        let dim = local.len();

        if peers.is_empty() {
            return local.clone();
        }

        let mut result = vec![0.0f32; dim];
        let mut total_weight = 0.0f64;

        // Add local gradients with weight 1.0
        let local_weight = 1.0f64;
        for (i, &g) in local.iter().enumerate() {
            result[i] += g * local_weight as f32;
        }
        total_weight += local_weight;

        // Prepare peer gradients for Byzantine detection
        let peer_list: Vec<(&PeerId, &[f32])> = peers.iter()
            .map(|(id, state)| (id, state.gradients.as_slice()))
            .collect();

        // Detect Byzantine peers
        let outliers = self.byzantine_detector.detect_outliers(&peer_list);
        let outlier_set: std::collections::HashSet<_> = outliers.into_iter().collect();

        // Add peer gradients with reputation weight (excluding outliers)
        for (peer_id, state) in peers.iter() {
            // Skip detected Byzantine peers
            if outlier_set.contains(peer_id) {
                continue;
            }

            // Superlinear reputation weight: rep^1.5
            // This gives high-reputation peers more influence
            let weight = state.reputation.powf(1.5);

            for (i, &g) in state.gradients.iter().enumerate() {
                if i < dim {
                    result[i] += g * weight as f32;
                }
            }
            total_weight += weight;
        }

        // Normalize by total weight
        if total_weight > 0.0 {
            let scale = 1.0 / total_weight as f32;
            for r in result.iter_mut() {
                *r *= scale;
            }
        }

        result
    }

    /// Get Byzantine-detected peers
    pub fn get_byzantine_peers(&self) -> Vec<PeerId> {
        let peers = self.peer_gradients.read().unwrap();

        let peer_list: Vec<(&PeerId, &[f32])> = peers.iter()
            .map(|(id, state)| (id, state.gradients.as_slice()))
            .collect();

        self.byzantine_detector.detect_outliers(&peer_list)
    }

    /// Update peer reputation after aggregation round
    pub fn update_peer_reputation(&self, peer_id: &PeerId, new_reputation: f64) {
        let mut peers = self.peer_gradients.write().unwrap();
        if let Some(state) = peers.get_mut(peer_id) {
            state.reputation = new_reputation.clamp(0.0, 1.0);
        }
    }

    /// Get peer reputations
    pub fn get_peer_reputations(&self) -> Vec<(PeerId, f64)> {
        self.peer_gradients.read().unwrap()
            .iter()
            .map(|(id, state)| (*id, state.reputation))
            .collect()
    }
}

// ============================================================================
// Federated Model State
// ============================================================================

/// Federated model state for tracking learning progress
#[wasm_bindgen]
pub struct FederatedModel {
    /// Model parameters (flattened)
    parameters: RwLock<Vec<f32>>,
    /// Learning rate
    learning_rate: f32,
    /// Momentum
    momentum: f32,
    /// Velocity for momentum-based updates
    velocity: RwLock<Vec<f32>>,
    /// Number of local epochs per round
    local_epochs: u32,
    /// Training round
    round: AtomicU64,
}

#[wasm_bindgen]
impl FederatedModel {
    /// Create a new federated model
    #[wasm_bindgen(constructor)]
    pub fn new(dimension: usize, learning_rate: f32, momentum: f32) -> Self {
        Self {
            parameters: RwLock::new(vec![0.0f32; dimension]),
            learning_rate,
            momentum: momentum.clamp(0.0, 0.99),
            velocity: RwLock::new(vec![0.0f32; dimension]),
            local_epochs: 1,
            round: AtomicU64::new(0),
        }
    }

    /// Get current round
    #[wasm_bindgen(js_name = getRound)]
    pub fn get_round(&self) -> u64 {
        self.round.load(Ordering::Relaxed)
    }

    /// Get parameter dimension
    #[wasm_bindgen(js_name = getDimension)]
    pub fn get_dimension(&self) -> usize {
        self.parameters.read().unwrap().len()
    }

    /// Get parameters as array
    #[wasm_bindgen(js_name = getParameters)]
    pub fn get_parameters(&self) -> Vec<f32> {
        self.parameters.read().unwrap().clone()
    }

    /// Set parameters from array
    #[wasm_bindgen(js_name = setParameters)]
    pub fn set_parameters(&self, params: &[f32]) -> Result<(), JsValue> {
        let mut parameters = self.parameters.write().unwrap();
        if params.len() != parameters.len() {
            return Err(JsValue::from_str("Parameter dimension mismatch"));
        }
        parameters.copy_from_slice(params);
        Ok(())
    }

    /// Apply aggregated gradients to update model
    #[wasm_bindgen(js_name = applyGradients)]
    pub fn apply_gradients(&self, gradients: &[f32]) -> Result<(), JsValue> {
        let mut parameters = self.parameters.write().unwrap();
        let mut velocity = self.velocity.write().unwrap();

        if gradients.len() != parameters.len() {
            return Err(JsValue::from_str("Gradient dimension mismatch"));
        }

        // SGD with momentum
        for i in 0..parameters.len() {
            velocity[i] = self.momentum * velocity[i] + gradients[i];
            parameters[i] -= self.learning_rate * velocity[i];
        }

        self.round.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Set learning rate
    #[wasm_bindgen(js_name = setLearningRate)]
    pub fn set_learning_rate(&mut self, lr: f32) {
        self.learning_rate = lr.max(0.0);
    }

    /// Set local epochs per round
    #[wasm_bindgen(js_name = setLocalEpochs)]
    pub fn set_local_epochs(&mut self, epochs: u32) {
        self.local_epochs = epochs.max(1);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topk_sparsifier_compression() {
        let sparsifier = TopKSparsifier::new(0.1); // Keep top 10%
        let gradients = vec![0.1, 0.5, 0.2, 0.8, 0.3, 0.1, 0.9, 0.4, 0.2, 0.6];

        let sparse = sparsifier.compress_stateless(&gradients);

        assert!(sparse.nnz() <= 2); // 10% of 10 = 1, but at least 1
        assert!(sparse.compression_ratio > 0.0);
    }

    #[test]
    fn test_topk_error_feedback() {
        let sparsifier = TopKSparsifier::new(0.2);
        let gradients = vec![1.0, 0.5, 0.3, 0.1, 0.05];

        // First compression
        let sparse1 = sparsifier.compress(&gradients);
        assert!(sparse1.nnz() > 0);

        // Error buffer should now have residuals
        assert!(sparsifier.get_error_buffer_size() > 0);

        // Second compression should use error feedback
        let gradients2 = vec![0.1, 0.1, 0.1, 0.1, 0.1];
        let sparse2 = sparsifier.compress(&gradients2);

        // Decompress and verify
        let decompressed = sparse2.decompress();
        assert_eq!(decompressed.len(), 5);
    }

    #[test]
    fn test_sparse_gradient_decompress() {
        let mut sparse = SparseGradient::new(5);
        sparse.indices = vec![1, 3];
        sparse.values = vec![0.5, 0.8];

        let dense = sparse.decompress();

        assert_eq!(dense.len(), 5);
        assert_eq!(dense[0], 0.0);
        assert_eq!(dense[1], 0.5);
        assert_eq!(dense[2], 0.0);
        assert_eq!(dense[3], 0.8);
        assert_eq!(dense[4], 0.0);
    }

    #[test]
    fn test_byzantine_detector_clipping() {
        let detector = ByzantineDetector::new(1.0, 3.0);
        let mut gradients = vec![0.5, 1.5, -2.0, f32::NAN, f32::INFINITY];

        detector.clip_gradients(&mut gradients);

        assert_eq!(gradients[0], 0.5);
        assert_eq!(gradients[1], 1.0);
        assert_eq!(gradients[2], -1.0);
        assert_eq!(gradients[3], 0.0); // NaN clipped to 0
        // Note: The implementation clips non-finite values to 0.0 first,
        // so Infinity becomes 0.0, not 1.0
        assert_eq!(gradients[4], 0.0); // Inf clipped to 0 (non-finite handling)
    }

    #[test]
    fn test_byzantine_outlier_detection() {
        let detector = ByzantineDetector::new(100.0, 2.0);

        let honest1 = vec![1.0, 1.0, 1.0];
        let honest2 = vec![1.1, 0.9, 1.0];
        let honest3 = vec![0.9, 1.1, 1.0];
        let byzantine = vec![100.0, 100.0, 100.0]; // Obvious outlier

        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];
        let peer3 = [3u8; 32];
        let peer4 = [4u8; 32];

        let peer_grads: Vec<(&PeerId, &[f32])> = vec![
            (&peer1, &honest1),
            (&peer2, &honest2),
            (&peer3, &honest3),
            (&peer4, &byzantine),
        ];

        let outliers = detector.detect_outliers(&peer_grads);

        // The Byzantine peer should be detected
        assert!(outliers.contains(&peer4));
        assert!(!outliers.contains(&peer1));
    }

    #[test]
    fn test_differential_privacy_clipping() {
        let dp = DifferentialPrivacy::new(1.0, 1.0);
        let mut gradients = vec![3.0, 4.0]; // L2 norm = 5

        dp.clip_l2(&mut gradients);

        let l2_norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
        assert!(l2_norm <= 1.001); // Within sensitivity bound
    }

    #[test]
    fn test_gradient_message_serialization() {
        let sender = [1u8; 32];
        let sparse = SparseGradient {
            indices: vec![0, 5],
            values: vec![0.1, 0.2],
            dimension: 10,
            compression_ratio: 0.8,
        };
        let model_hash = [0u8; 32];

        let msg = GradientMessage::new(sender, 1, sparse, model_hash);

        let bytes = msg.to_bytes().unwrap();
        let decoded = GradientMessage::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.sender, sender);
        assert_eq!(decoded.round, 1);
        assert_eq!(decoded.gradients.nnz(), 2);
    }

    #[test]
    fn test_gradient_gossip_aggregation() {
        let local_peer = [0u8; 32];
        let gossip = GradientGossip::new(&local_peer, 4, 0.5).unwrap();

        // Set local gradients
        let local_grads = vec![1.0, 2.0, 3.0, 4.0];
        gossip.set_local_gradients(&local_grads).unwrap();

        // Add peer gradients
        let peer1 = [1u8; 32];
        let peer1_grads = SparseGradient {
            indices: vec![0, 1, 2, 3],
            values: vec![2.0, 4.0, 6.0, 8.0],
            dimension: 4,
            compression_ratio: 0.0,
        };
        let model_hash = [0u8; 32];
        let msg1 = GradientMessage::new(peer1, 0, peer1_grads, model_hash);
        gossip.receive_message(&msg1, 1.0).unwrap();

        // Aggregate
        let aggregated = gossip.aggregate();

        assert_eq!(aggregated.len(), 4);
        // Should be weighted average of local and peer
        assert!(aggregated[0] > 1.0 && aggregated[0] < 2.0);
    }

    #[test]
    fn test_federated_model_update() {
        let model = FederatedModel::new(3, 0.1, 0.9);

        // Initialize with some values
        model.set_parameters(&[1.0, 2.0, 3.0]).unwrap();

        // Apply gradients
        model.apply_gradients(&[0.1, 0.2, 0.3]).unwrap();

        let params = model.get_parameters();

        // Parameters should have decreased (gradient descent)
        assert!(params[0] < 1.0);
        assert!(params[1] < 2.0);
        assert!(params[2] < 3.0);

        // Round should have incremented
        assert_eq!(model.get_round(), 1);
    }

    #[test]
    fn test_staleness_pruning() {
        let local_peer = [0u8; 32];
        let gossip = GradientGossip::new(&local_peer, 4, 0.5).unwrap();

        // Add peer at round 0
        let peer1 = [1u8; 32];
        let grads = SparseGradient::new(4);
        let model_hash = [0u8; 32];
        let msg = GradientMessage::new(peer1, 0, grads, model_hash);
        gossip.receive_message(&msg, 0.5).unwrap();

        assert_eq!(gossip.peer_count(), 1);

        // Advance many rounds
        for _ in 0..10 {
            gossip.advance_round();
        }

        // Prune stale
        let pruned = gossip.prune_stale();
        assert_eq!(pruned, 1);
        assert_eq!(gossip.peer_count(), 0);
    }
}
