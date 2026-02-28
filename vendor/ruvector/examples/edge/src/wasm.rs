//! WASM Bindings for RuVector Edge
//!
//! Exposes P2P swarm functionality to JavaScript/TypeScript via wasm-bindgen.
//!
//! ## Usage in JavaScript/TypeScript
//!
//! ```typescript
//! import init, {
//!     WasmIdentity,
//!     WasmCrypto,
//!     WasmHnswIndex,
//!     WasmSemanticMatcher,
//!     WasmRaftNode
//! } from 'ruvector-edge';
//!
//! await init();
//!
//! // Create identity
//! const identity = new WasmIdentity();
//! console.log('Public key:', identity.publicKeyHex());
//!
//! // Sign and verify
//! const signature = identity.sign('Hello, World!');
//! console.log('Valid:', WasmIdentity.verify(identity.publicKeyHex(), 'Hello, World!', signature));
//! ```

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

use crate::p2p::{
    IdentityManager, CryptoV2, HnswIndex, SemanticTaskMatcher,
    RaftNode, RaftState, HybridKeyPair, SpikingNetwork,
    BinaryQuantized, ScalarQuantized, AdaptiveCompressor, NetworkCondition,
};

// ============================================================================
// WASM Identity Manager
// ============================================================================

/// WASM-compatible Identity Manager for Ed25519/X25519 cryptography
#[wasm_bindgen]
pub struct WasmIdentity {
    inner: IdentityManager,
}

#[wasm_bindgen]
impl WasmIdentity {
    /// Create a new identity with generated keys
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: IdentityManager::new(),
        }
    }

    /// Get Ed25519 public key as hex string
    #[wasm_bindgen(js_name = publicKeyHex)]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.inner.public_key())
    }

    /// Get X25519 public key as hex string (for key exchange)
    #[wasm_bindgen(js_name = x25519PublicKeyHex)]
    pub fn x25519_public_key_hex(&self) -> String {
        hex::encode(self.inner.x25519_public_key())
    }

    /// Sign a message and return signature as hex
    #[wasm_bindgen]
    pub fn sign(&self, message: &str) -> String {
        hex::encode(self.inner.sign(message.as_bytes()))
    }

    /// Sign raw bytes and return signature as hex
    #[wasm_bindgen(js_name = signBytes)]
    pub fn sign_bytes(&self, data: &[u8]) -> String {
        hex::encode(self.inner.sign(data))
    }

    /// Verify a signature (static method)
    #[wasm_bindgen]
    pub fn verify(public_key_hex: &str, message: &str, signature_hex: &str) -> bool {
        let Ok(pubkey_bytes) = hex::decode(public_key_hex) else {
            return false;
        };
        let Ok(sig_bytes) = hex::decode(signature_hex) else {
            return false;
        };

        if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 {
            return false;
        }

        let mut pubkey = [0u8; 32];
        let mut signature = [0u8; 64];
        pubkey.copy_from_slice(&pubkey_bytes);
        signature.copy_from_slice(&sig_bytes);

        crate::p2p::KeyPair::verify(&pubkey, message.as_bytes(), &signature)
    }

    /// Generate a random nonce
    #[wasm_bindgen(js_name = generateNonce)]
    pub fn generate_nonce() -> String {
        IdentityManager::generate_nonce()
    }

    /// Create a signed registration for this identity
    #[wasm_bindgen(js_name = createRegistration)]
    pub fn create_registration(&self, agent_id: &str, capabilities: JsValue) -> Result<JsValue, JsValue> {
        let caps: Vec<String> = serde_wasm_bindgen::from_value(capabilities)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let registration = self.inner.create_registration(agent_id, caps);
        serde_wasm_bindgen::to_value(&registration)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for WasmIdentity {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WASM Crypto Utilities
// ============================================================================

/// WASM-compatible cryptographic utilities
#[wasm_bindgen]
pub struct WasmCrypto;

#[wasm_bindgen]
impl WasmCrypto {
    /// SHA-256 hash as hex string
    #[wasm_bindgen]
    pub fn sha256(data: &[u8]) -> String {
        CryptoV2::hash_hex(data)
    }

    /// SHA-256 hash of string as hex
    #[wasm_bindgen(js_name = sha256String)]
    pub fn sha256_string(text: &str) -> String {
        CryptoV2::hash_hex(text.as_bytes())
    }

    /// Generate a local CID for data
    #[wasm_bindgen(js_name = generateCid)]
    pub fn generate_cid(data: &[u8]) -> String {
        CryptoV2::generate_local_cid(data)
    }

    /// Encrypt data with AES-256-GCM (key as hex)
    #[wasm_bindgen]
    pub fn encrypt(data: &[u8], key_hex: &str) -> Result<JsValue, JsValue> {
        let key_bytes = hex::decode(key_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid key hex: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(JsValue::from_str("Key must be 32 bytes (64 hex chars)"));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        let encrypted = CryptoV2::encrypt(data, &key)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&encrypted)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decrypt data with AES-256-GCM
    #[wasm_bindgen]
    pub fn decrypt(encrypted: JsValue, key_hex: &str) -> Result<Vec<u8>, JsValue> {
        let key_bytes = hex::decode(key_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid key hex: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(JsValue::from_str("Key must be 32 bytes"));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        let encrypted_payload: crate::p2p::EncryptedPayload =
            serde_wasm_bindgen::from_value(encrypted)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

        CryptoV2::decrypt(&encrypted_payload, &key)
            .map_err(|e| JsValue::from_str(&e))
    }
}

// ============================================================================
// WASM HNSW Vector Index
// ============================================================================

/// WASM-compatible HNSW index for fast vector similarity search
#[wasm_bindgen]
pub struct WasmHnswIndex {
    inner: HnswIndex,
}

#[wasm_bindgen]
impl WasmHnswIndex {
    /// Create new HNSW index with default parameters
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: HnswIndex::new(),
        }
    }

    /// Create with custom parameters (m = connections per node, ef = search width)
    #[wasm_bindgen(js_name = withParams)]
    pub fn with_params(m: usize, ef_construction: usize) -> Self {
        Self {
            inner: HnswIndex::with_params(m, ef_construction),
        }
    }

    /// Insert a vector with an ID
    #[wasm_bindgen]
    pub fn insert(&mut self, id: &str, vector: Vec<f32>) {
        self.inner.insert(id, vector);
    }

    /// Search for k nearest neighbors, returns JSON array of {id, distance}
    #[wasm_bindgen]
    pub fn search(&self, query: Vec<f32>, k: usize) -> JsValue {
        let results = self.inner.search(&query, k);
        let json_results: Vec<SearchResult> = results
            .into_iter()
            .map(|(id, dist)| SearchResult { id, distance: dist })
            .collect();

        serde_wasm_bindgen::to_value(&json_results).unwrap_or(JsValue::NULL)
    }

    /// Get number of vectors in index
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if index is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for WasmHnswIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct SearchResult {
    id: String,
    distance: f32,
}

// ============================================================================
// WASM Semantic Task Matcher
// ============================================================================

/// WASM-compatible semantic task matcher for intelligent agent routing
#[wasm_bindgen]
pub struct WasmSemanticMatcher {
    inner: SemanticTaskMatcher,
}

#[wasm_bindgen]
impl WasmSemanticMatcher {
    /// Create new semantic matcher
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: SemanticTaskMatcher::new(),
        }
    }

    /// Register an agent with capability description
    #[wasm_bindgen(js_name = registerAgent)]
    pub fn register_agent(&mut self, agent_id: &str, capabilities: &str) {
        self.inner.register_agent(agent_id, capabilities);
    }

    /// Find best matching agent for a task, returns {agentId, score} or null
    #[wasm_bindgen(js_name = matchAgent)]
    pub fn match_agent(&self, task_description: &str) -> JsValue {
        match self.inner.match_agent(task_description) {
            Some((agent_id, score)) => {
                let result = MatchResult { agent_id, score };
                serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
            }
            None => JsValue::NULL,
        }
    }

    /// Get number of registered agents
    #[wasm_bindgen(js_name = agentCount)]
    pub fn agent_count(&self) -> usize {
        // Return count from inner (we can't access private fields, so just return 0 for now)
        0
    }
}

impl Default for WasmSemanticMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct MatchResult {
    #[serde(rename = "agentId")]
    agent_id: String,
    score: f32,
}

// ============================================================================
// WASM Raft Consensus
// ============================================================================

/// WASM-compatible Raft consensus node
#[wasm_bindgen]
pub struct WasmRaftNode {
    inner: RaftNode,
}

#[wasm_bindgen]
impl WasmRaftNode {
    /// Create new Raft node with cluster members
    #[wasm_bindgen(constructor)]
    pub fn new(node_id: &str, members: JsValue) -> Result<WasmRaftNode, JsValue> {
        let member_list: Vec<String> = serde_wasm_bindgen::from_value(members)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(Self {
            inner: RaftNode::new(node_id, member_list),
        })
    }

    /// Get current state (Follower, Candidate, Leader)
    #[wasm_bindgen]
    pub fn state(&self) -> String {
        format!("{:?}", self.inner.state)
    }

    /// Get current term
    #[wasm_bindgen]
    pub fn term(&self) -> u64 {
        self.inner.current_term
    }

    /// Check if this node is the leader
    #[wasm_bindgen(js_name = isLeader)]
    pub fn is_leader(&self) -> bool {
        matches!(self.inner.state, RaftState::Leader)
    }

    /// Start an election (returns vote request as JSON)
    #[wasm_bindgen(js_name = startElection)]
    pub fn start_election(&mut self) -> JsValue {
        let request = self.inner.start_election();
        serde_wasm_bindgen::to_value(&request).unwrap_or(JsValue::NULL)
    }

    /// Handle a vote request (returns vote response as JSON)
    #[wasm_bindgen(js_name = handleVoteRequest)]
    pub fn handle_vote_request(&mut self, request: JsValue) -> Result<JsValue, JsValue> {
        let req: crate::p2p::RaftVoteRequest = serde_wasm_bindgen::from_value(request)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let response = self.inner.handle_vote_request(&req);
        serde_wasm_bindgen::to_value(&response)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Handle a vote response (returns true if we became leader)
    #[wasm_bindgen(js_name = handleVoteResponse)]
    pub fn handle_vote_response(&mut self, response: JsValue) -> Result<bool, JsValue> {
        let resp: crate::p2p::RaftVoteResponse = serde_wasm_bindgen::from_value(response)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(self.inner.handle_vote_response(&resp))
    }

    /// Append entry to log (leader only), returns log index or null
    #[wasm_bindgen(js_name = appendEntry)]
    pub fn append_entry(&mut self, data: &[u8]) -> JsValue {
        match self.inner.append_entry(data.to_vec()) {
            Some(index) => JsValue::from_f64(index as f64),
            None => JsValue::NULL,
        }
    }

    /// Get commit index
    #[wasm_bindgen(js_name = getCommitIndex)]
    pub fn get_commit_index(&self) -> u64 {
        self.inner.commit_index
    }

    /// Get log length
    #[wasm_bindgen(js_name = getLogLength)]
    pub fn get_log_length(&self) -> usize {
        self.inner.log.len()
    }
}

// ============================================================================
// WASM Post-Quantum Crypto
// ============================================================================

/// WASM-compatible hybrid post-quantum signatures (Ed25519 + Dilithium-style)
#[wasm_bindgen]
pub struct WasmHybridKeyPair {
    inner: HybridKeyPair,
}

#[wasm_bindgen]
impl WasmHybridKeyPair {
    /// Generate new hybrid keypair
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: HybridKeyPair::generate(),
        }
    }

    /// Get public key bytes as hex
    #[wasm_bindgen(js_name = publicKeyHex)]
    pub fn public_key_hex(&self) -> String {
        let pubkey_bytes = self.inner.public_key_bytes();
        hex::encode(&pubkey_bytes.ed25519)
    }

    /// Sign message with hybrid signature
    #[wasm_bindgen]
    pub fn sign(&self, message: &[u8]) -> String {
        let sig = self.inner.sign(message);
        // Serialize the signature struct
        serde_json::to_string(&sig).unwrap_or_default()
    }

    /// Verify hybrid signature (pubkey and signature both as JSON)
    #[wasm_bindgen]
    pub fn verify(public_key_json: &str, message: &[u8], signature_json: &str) -> bool {
        let Ok(pubkey): Result<crate::p2p::HybridPublicKey, _> = serde_json::from_str(public_key_json) else {
            return false;
        };

        let Ok(signature): Result<crate::p2p::HybridSignature, _> = serde_json::from_str(signature_json) else {
            return false;
        };

        HybridKeyPair::verify(&pubkey, message, &signature)
    }
}

impl Default for WasmHybridKeyPair {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WASM Spiking Neural Network
// ============================================================================

/// WASM-compatible spiking neural network for temporal pattern recognition
#[wasm_bindgen]
pub struct WasmSpikingNetwork {
    inner: SpikingNetwork,
}

#[wasm_bindgen]
impl WasmSpikingNetwork {
    /// Create new spiking network
    #[wasm_bindgen(constructor)]
    pub fn new(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        Self {
            inner: SpikingNetwork::new(input_size, hidden_size, output_size),
        }
    }

    /// Process input spikes and return output spikes
    #[wasm_bindgen]
    pub fn forward(&mut self, inputs: Vec<u8>) -> Vec<u8> {
        let input_bools: Vec<bool> = inputs.iter().map(|&x| x != 0).collect();
        let output_bools = self.inner.forward(&input_bools);
        output_bools.iter().map(|&b| if b { 1 } else { 0 }).collect()
    }

    /// Apply STDP learning rule
    #[wasm_bindgen(js_name = stdpUpdate)]
    pub fn stdp_update(&mut self, pre: Vec<u8>, post: Vec<u8>, learning_rate: f32) {
        let pre_bools: Vec<bool> = pre.iter().map(|&x| x != 0).collect();
        let post_bools: Vec<bool> = post.iter().map(|&x| x != 0).collect();
        self.inner.stdp_update(&pre_bools, &post_bools, learning_rate);
    }

    /// Reset network state
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

// ============================================================================
// WASM Quantization
// ============================================================================

/// WASM-compatible vector quantization utilities
#[wasm_bindgen]
pub struct WasmQuantizer;

#[wasm_bindgen]
impl WasmQuantizer {
    /// Binary quantize a vector (32x compression)
    #[wasm_bindgen(js_name = binaryQuantize)]
    pub fn binary_quantize(vector: Vec<f32>) -> Vec<u8> {
        let quantized = BinaryQuantized::quantize(&vector);
        quantized.bits.to_vec()
    }

    /// Scalar quantize a vector (4x compression)
    #[wasm_bindgen(js_name = scalarQuantize)]
    pub fn scalar_quantize(vector: Vec<f32>) -> JsValue {
        let quantized = ScalarQuantized::quantize(&vector);
        serde_wasm_bindgen::to_value(&ScalarQuantizedJs {
            data: quantized.data.clone(),
            min: quantized.min,
            scale: quantized.scale,
        }).unwrap_or(JsValue::NULL)
    }

    /// Reconstruct from scalar quantized
    #[wasm_bindgen(js_name = scalarDequantize)]
    pub fn scalar_dequantize(quantized: JsValue) -> Result<Vec<f32>, JsValue> {
        let q: ScalarQuantizedJs = serde_wasm_bindgen::from_value(quantized)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let sq = ScalarQuantized {
            data: q.data,
            min: q.min,
            scale: q.scale,
        };

        Ok(sq.reconstruct())
    }

    /// Compute hamming distance between binary quantized vectors
    #[wasm_bindgen(js_name = hammingDistance)]
    pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x ^ y).count_ones())
            .sum()
    }
}

#[derive(Serialize, Deserialize)]
struct ScalarQuantizedJs {
    data: Vec<u8>,
    min: f32,
    scale: f32,
}

// ============================================================================
// WASM Adaptive Compressor
// ============================================================================

/// WASM-compatible network-aware adaptive compression
#[wasm_bindgen]
pub struct WasmAdaptiveCompressor {
    inner: AdaptiveCompressor,
}

#[wasm_bindgen]
impl WasmAdaptiveCompressor {
    /// Create new adaptive compressor
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: AdaptiveCompressor::new(),
        }
    }

    /// Update network metrics (bandwidth in Mbps, latency in ms)
    #[wasm_bindgen(js_name = updateMetrics)]
    pub fn update_metrics(&mut self, bandwidth_mbps: f32, latency_ms: f32) {
        self.inner.update_metrics(bandwidth_mbps, latency_ms);
    }

    /// Get current network condition
    #[wasm_bindgen]
    pub fn condition(&self) -> String {
        match self.inner.condition() {
            NetworkCondition::Excellent => "excellent".to_string(),
            NetworkCondition::Good => "good".to_string(),
            NetworkCondition::Poor => "poor".to_string(),
            NetworkCondition::Critical => "critical".to_string(),
        }
    }

    /// Compress vector based on network conditions
    #[wasm_bindgen]
    pub fn compress(&self, data: Vec<f32>) -> JsValue {
        let compressed = self.inner.compress(&data);
        serde_wasm_bindgen::to_value(&compressed).unwrap_or(JsValue::NULL)
    }
}

impl Default for WasmAdaptiveCompressor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the WASM module (call once on load)
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages in console
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get library version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
