//! # RuVector Adversarial Coherence (RAC)
//!
//! **Adversarial Coherence Thesis (circa 2076):**
//!
//! In a browser-scale, adversarial world, the only sustainable definition of "correctness" is:
//! *claims survive continuous challenge, remain traceable, and can be repaired without global resets.*
//!
//! Structural integrity (high min-cut, stable connectivity) is necessary but not sufficient.
//! The core runtime for all large-scale intelligence becomes a second control loop:
//! an adversarial coherence layer that treats disagreement as a first-class signal,
//! keeps an append-only history of what was believed and why, and makes correction
//! a normal operation rather than an exception.
//!
//! ## The 12 Axioms
//!
//! 1. **Connectivity is not truth.** Structural metrics bound failure modes, not correctness.
//! 2. **Everything is an event.** Assertions, challenges, model updates, and decisions are all logged events.
//! 3. **No destructive edits.** Incorrect learning is deprecated, never erased.
//! 4. **Every claim is scoped.** Claims are always tied to a context: task, domain, time window, and authority boundary.
//! 5. **Semantics drift is expected.** Drift is measured and managed, not denied.
//! 6. **Disagreement is signal.** Sustained contradictions increase epistemic temperature and trigger escalation.
//! 7. **Authority is scoped, not global.** Only specific keys can correct specific contexts, ideally thresholded.
//! 8. **Witnesses matter.** Confidence comes from independent, diverse witness paths, not repetition.
//! 9. **Quarantine is mandatory.** Contested claims cannot freely drive downstream decisions.
//! 10. **All decisions are replayable.** A decision must reference the exact events it depended on.
//! 11. **Equivocation is detectable.** The system must make it hard to show different histories to different peers.
//! 12. **Local learning is allowed.** But learning outputs must be attributable, challengeable, and rollbackable via deprecation.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    RAC Adversarial Coherence Layer                  │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │
//! │  │ Event Log   │  │  Coherence  │  │  Authority  │  │  Dispute  │  │
//! │  │ (Merkle)    │──│   Engine    │──│   Policy    │──│   Engine  │  │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │
//! │  │  Ruvector   │  │  Quarantine │  │   Audit     │  │  Witness  │  │
//! │  │  Routing    │  │   Manager   │  │   Proofs    │  │  Tracker  │  │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## References
//!
//! - [FLP Impossibility](https://groups.csail.mit.edu/tds/papers/Lynch/jacm85.pdf) - Distributed consensus limits
//! - [PBFT](https://css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf) - Byzantine fault tolerance
//! - [CRDTs](https://pages.lip6.fr/Marc.Shapiro/papers/RR-7687.pdf) - Conflict-free replicated data types
//! - [RFC 6962](https://www.rfc-editor.org/rfc/rfc6962.html) - Certificate Transparency (Merkle logs)

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::RwLock;
use ed25519_dalek::{VerifyingKey, Signature, Verifier as Ed25519Verifier};
use sha2::{Sha256, Digest};

// Economic layer with staking, reputation, and rewards
pub mod economics;
pub use economics::{
    RacEconomicEngine, StakeManager, ReputationManager, RewardManager,
    SlashReason, StakeRecord, ReputationRecord, RewardRecord,
};

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
// Core Types (from Adversarial Coherence Thesis)
// ============================================================================

/// 32-byte context identifier
pub type ContextId = [u8; 32];

/// 32-byte event identifier (hash of event bytes)
pub type EventId = [u8; 32];

/// 32-byte public key bytes
pub type PublicKeyBytes = [u8; 32];

/// 64-byte signature bytes (Ed25519) - using Vec for serde compatibility
pub type SignatureBytes = Vec<u8>;

/// RuVector embedding for semantic routing and clustering
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ruvector {
    /// Vector dimensions (quantized for efficiency)
    pub dims: Vec<f32>,
}

impl Ruvector {
    /// Create a new RuVector
    pub fn new(dims: Vec<f32>) -> Self {
        Self { dims }
    }

    /// Create a zero vector of given dimension
    pub fn zeros(dim: usize) -> Self {
        Self { dims: vec![0.0; dim] }
    }

    /// Calculate cosine similarity to another RuVector
    pub fn similarity(&self, other: &Ruvector) -> f64 {
        if self.dims.len() != other.dims.len() {
            return 0.0;
        }

        let dot: f32 = self.dims.iter().zip(&other.dims).map(|(a, b)| a * b).sum();
        let norm_a: f32 = self.dims.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.dims.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)) as f64
    }

    /// Compute semantic drift from a baseline
    pub fn drift_from(&self, baseline: &Ruvector) -> f64 {
        1.0 - self.similarity(baseline)
    }

    /// L2 distance to another vector
    pub fn distance(&self, other: &Ruvector) -> f64 {
        if self.dims.len() != other.dims.len() {
            return f64::MAX;
        }
        self.dims.iter()
            .zip(&other.dims)
            .map(|(a, b)| (a - b).powi(2) as f64)
            .sum::<f64>()
            .sqrt()
    }
}

/// Evidence reference for claims
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Kind of evidence: "url", "hash", "sensor", "dataset", "log"
    pub kind: String,
    /// Pointer bytes (hash/uri/etc)
    pub pointer: Vec<u8>,
}

impl EvidenceRef {
    /// Create a hash evidence reference
    pub fn hash(hash: &[u8]) -> Self {
        Self {
            kind: "hash".to_string(),
            pointer: hash.to_vec(),
        }
    }

    /// Create a URL evidence reference
    pub fn url(url: &str) -> Self {
        Self {
            kind: "url".to_string(),
            pointer: url.as_bytes().to_vec(),
        }
    }

    /// Create a log evidence reference
    pub fn log(log_id: &[u8]) -> Self {
        Self {
            kind: "log".to_string(),
            pointer: log_id.to_vec(),
        }
    }
}

// ============================================================================
// Event Types (Axiom 2: Everything is an event)
// ============================================================================

/// Assertion event - a claim being made
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssertEvent {
    /// Proposition bytes (CBOR/JSON/protobuf)
    pub proposition: Vec<u8>,
    /// Evidence supporting the claim
    pub evidence: Vec<EvidenceRef>,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,
    /// Expiration timestamp (optional)
    pub expires_at_unix_ms: Option<u64>,
}

/// Challenge event - opening a dispute
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChallengeEvent {
    /// Conflict identifier
    pub conflict_id: [u8; 32],
    /// Claim IDs involved in the conflict
    pub claim_ids: Vec<EventId>,
    /// Reason for the challenge
    pub reason: String,
    /// Requested proof types
    pub requested_proofs: Vec<String>,
}

/// Support event - providing evidence for a disputed claim
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupportEvent {
    /// Conflict being supported
    pub conflict_id: [u8; 32],
    /// Claim being supported
    pub claim_id: EventId,
    /// Supporting evidence
    pub evidence: Vec<EvidenceRef>,
    /// Cost/stake/work score
    pub cost: u64,
}

/// Resolution event - concluding a dispute
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolutionEvent {
    /// Conflict being resolved
    pub conflict_id: [u8; 32],
    /// Accepted claim IDs
    pub accepted: Vec<EventId>,
    /// Deprecated claim IDs
    pub deprecated: Vec<EventId>,
    /// Rationale references
    pub rationale: Vec<EvidenceRef>,
    /// Authority signatures
    pub authority_sigs: Vec<SignatureBytes>,
}

/// Deprecation event (Axiom 3: No destructive edits)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeprecateEvent {
    /// Claim being deprecated
    pub claim_id: EventId,
    /// Resolution that triggered deprecation
    pub by_resolution: [u8; 32],
    /// Superseding claim (if any)
    pub superseded_by: Option<EventId>,
}

// ============================================================================
// AI Model Consensus Types (Axiom 2: Everything is an event)
// ============================================================================

/// Task types for LoRA adapter classification
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// Text generation/completion tasks
    TextGeneration,
    /// Code generation and analysis
    CodeGeneration,
    /// Image classification/analysis
    VisionClassification,
    /// Embedding generation
    Embedding,
    /// Retrieval augmented generation
    RAG,
    /// Reinforcement learning from feedback
    RLHF,
    /// Multi-modal tasks
    MultiModal,
    /// Custom task type
    Custom(String),
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::TextGeneration
    }
}

/// Model weight claim for AI consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelWeightClaim {
    /// Unique model identifier
    pub model_id: String,
    /// Layer identifier (e.g., "transformer.h.0.attn")
    pub layer: String,
    /// SHA-256 hash of the weight tensor bytes
    pub weights_hash: [u8; 32],
    /// Version number (monotonically increasing)
    pub version: u64,
    /// Optional quantization info (e.g., "int8", "fp16")
    pub quantization: Option<String>,
    /// Number of parameters in this layer
    pub param_count: usize,
}

/// LoRA adapter claim for per-task fine-tuning
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoraAdapterClaim {
    /// Unique adapter identifier
    pub adapter_id: String,
    /// Task type this adapter specializes in
    pub task_type: TaskType,
    /// LoRA rank (typically 2-64)
    pub rank: u8,
    /// SHA-256 hash of adapter weights
    pub weights_hash: [u8; 32],
    /// Base model this adapter applies to
    pub base_model_id: String,
    /// Training metrics (loss, accuracy, etc.)
    pub metrics: Option<AdapterMetrics>,
}

/// Metrics for LoRA adapter performance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterMetrics {
    /// Final training loss
    pub final_loss: f32,
    /// Validation accuracy (0.0 - 1.0)
    pub val_accuracy: f32,
    /// Number of training samples
    pub train_samples: usize,
    /// Training epochs completed
    pub epochs: u32,
}

/// Learning pattern claim for collective memory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningPatternClaim {
    /// Unique pattern identifier
    pub pattern_id: String,
    /// Vector embedding representing the pattern
    pub embedding: Vec<f32>,
    /// Quality score from validation (0.0 - 1.0)
    pub quality_score: f32,
    /// Number of samples used to learn this pattern
    pub sample_count: usize,
    /// Context/domain this pattern applies to
    pub domain: String,
    /// Confidence interval (low, high)
    pub confidence_interval: (f32, f32),
}

/// Gradient contribution claim for federated learning
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientContributionClaim {
    /// Federated learning round number
    pub round: u64,
    /// Contributor's public key
    pub contributor: PublicKeyBytes,
    /// SHA-256 hash of the gradient tensor
    pub gradient_hash: [u8; 32],
    /// Contributor's reputation at contribution time
    pub reputation_at_time: f32,
    /// Number of local samples used
    pub local_samples: usize,
    /// Gradient norm (for anomaly detection)
    pub gradient_norm: f32,
    /// Model ID this gradient applies to
    pub model_id: String,
    /// Signature proving ownership
    pub signature: SignatureBytes,
}

/// Claim type enumeration for AI/model consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClaimType {
    /// Standard text/data assertion
    Standard(AssertEvent),
    /// Model weight claim
    ModelWeight(ModelWeightClaim),
    /// LoRA adapter claim
    LoraAdapter(LoraAdapterClaim),
    /// Learned pattern claim
    LearningPattern(LearningPatternClaim),
    /// Gradient contribution claim
    GradientContribution(GradientContributionClaim),
}

impl ClaimType {
    /// Get claim type as string for logging
    pub fn type_name(&self) -> &'static str {
        match self {
            ClaimType::Standard(_) => "standard",
            ClaimType::ModelWeight(_) => "model_weight",
            ClaimType::LoraAdapter(_) => "lora_adapter",
            ClaimType::LearningPattern(_) => "learning_pattern",
            ClaimType::GradientContribution(_) => "gradient_contribution",
        }
    }
}

/// Event kind enumeration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventKind {
    Assert(AssertEvent),
    Challenge(ChallengeEvent),
    Support(SupportEvent),
    Resolution(ResolutionEvent),
    Deprecate(DeprecateEvent),
    /// AI model claim (extends assertions for ML consensus)
    ModelClaim(ClaimType),
}

/// A signed, logged event
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Event ID (hash of content)
    pub id: EventId,
    /// Previous event in chain (optional)
    pub prev: Option<EventId>,
    /// Timestamp (ms since epoch)
    pub ts_unix_ms: u64,
    /// Author's public key
    pub author: PublicKeyBytes,
    /// Context binding (Axiom 4: Every claim is scoped)
    pub context: ContextId,
    /// Semantic embedding for routing
    pub ruvector: Ruvector,
    /// Event payload
    pub kind: EventKind,
    /// Author's signature
    pub sig: SignatureBytes,
}

impl Event {
    /// Create a new event with auto-generated ID and timestamp
    pub fn new(
        author: PublicKeyBytes,
        context: ContextId,
        ruvector: Ruvector,
        kind: EventKind,
        prev: Option<EventId>,
    ) -> Self {
        use sha2::{Sha256, Digest};

        let ts_unix_ms = current_timestamp_ms();

        // Generate event ID from content
        let mut hasher = Sha256::new();
        hasher.update(&author);
        hasher.update(&context);
        hasher.update(&ts_unix_ms.to_le_bytes());
        if let Some(prev_id) = &prev {
            hasher.update(prev_id);
        }
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);

        Self {
            id,
            prev,
            ts_unix_ms,
            author,
            context,
            ruvector,
            kind,
            sig: Vec::new(), // Signature added separately
        }
    }
}

// ============================================================================
// Merkle Event Log (Axiom 2, Axiom 3: Append-only, tamper-evident)
// ============================================================================

/// Append-only Merkle log for audit (FIXED: proper event storage)
#[wasm_bindgen]
pub struct EventLog {
    /// Events in order (main storage)
    events: RwLock<Vec<Event>>,
    /// Current Merkle root
    root: RwLock<[u8; 32]>,
    /// Event index by ID for O(1) lookups
    index: RwLock<FxHashMap<[u8; 32], usize>>,
}

#[wasm_bindgen]
impl EventLog {
    /// Create a new event log
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::with_capacity(1000)),
            root: RwLock::new([0u8; 32]),
            index: RwLock::new(FxHashMap::default()),
        }
    }

    /// Get current event count (includes all events)
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.events.read().unwrap().len()
    }

    /// Check if log is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.events.read().unwrap().is_empty()
    }

    /// Get current Merkle root as hex string
    #[wasm_bindgen(js_name = getRoot)]
    pub fn get_root(&self) -> String {
        let root = self.root.read().unwrap();
        hex::encode(&*root)
    }

    /// Get total event count
    #[wasm_bindgen(js_name = totalEvents)]
    pub fn total_events(&self) -> usize {
        self.events.read().unwrap().len()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    /// Append an event to the log (FIXED: immediate storage + incremental Merkle)
    pub fn append(&self, event: Event) -> EventId {
        let id = event.id;

        let mut events = self.events.write().unwrap();
        let mut index = self.index.write().unwrap();
        let mut root = self.root.write().unwrap();

        // Store event
        let event_idx = events.len();
        events.push(event);
        index.insert(id, event_idx);

        // Incremental Merkle root update
        *root = self.compute_incremental_root(&id, &root);

        id
    }

    /// Get current root (no flushing needed - immediate storage)
    pub fn get_root_bytes(&self) -> [u8; 32] {
        *self.root.read().unwrap()
    }

    /// Get event by ID (O(1) lookup via index)
    pub fn get(&self, id: &EventId) -> Option<Event> {
        let index = self.index.read().unwrap();
        let events = self.events.read().unwrap();

        index.get(id)
            .and_then(|&idx| events.get(idx))
            .cloned()
    }

    /// Get events since a timestamp
    pub fn since(&self, timestamp: u64) -> Vec<Event> {
        let events = self.events.read().unwrap();
        events.iter()
            .filter(|e| e.ts_unix_ms >= timestamp)
            .cloned()
            .collect()
    }

    /// Get events for a context
    pub fn for_context(&self, context: &ContextId) -> Vec<Event> {
        let events = self.events.read().unwrap();
        events.iter()
            .filter(|e| &e.context == context)
            .cloned()
            .collect()
    }

    /// Get all events (for iteration)
    pub fn all_events(&self) -> Vec<Event> {
        self.events.read().unwrap().clone()
    }

    /// Compute incremental Merkle root (chain new event ID to existing root)
    fn compute_incremental_root(&self, new_id: &EventId, prev_root: &[u8; 32]) -> [u8; 32] {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(prev_root);
        hasher.update(new_id);
        let result = hasher.finalize();
        let mut root = [0u8; 32];
        root.copy_from_slice(&result);
        root
    }

    /// Generate inclusion proof for an event (Axiom 11: Equivocation detectable)
    pub fn prove_inclusion(&self, event_id: &EventId) -> Option<InclusionProof> {
        let index = self.index.read().unwrap();
        let events = self.events.read().unwrap();
        let root = *self.root.read().unwrap();

        let &event_idx = index.get(event_id)?;

        // Build Merkle path (simplified chain proof)
        let mut path = Vec::with_capacity(32);
        let mut current_hash = [0u8; 32];

        // Compute path from genesis to this event
        for (i, event) in events.iter().take(event_idx + 1).enumerate() {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(&current_hash);
            hasher.update(&event.id);
            let result = hasher.finalize();
            current_hash.copy_from_slice(&result);

            if i < event_idx {
                path.push(current_hash);
            }
        }

        Some(InclusionProof {
            event_id: *event_id,
            index: event_idx,
            root,
            path,
        })
    }

    /// Verify an inclusion proof
    pub fn verify_proof(&self, proof: &InclusionProof) -> bool {
        use sha2::{Sha256, Digest};

        let events = self.events.read().unwrap();

        if proof.index >= events.len() {
            return false;
        }

        // Recompute root from genesis to claimed index
        let mut current = [0u8; 32];
        for event in events.iter().take(proof.index + 1) {
            let mut hasher = Sha256::new();
            hasher.update(&current);
            hasher.update(&event.id);
            let result = hasher.finalize();
            current.copy_from_slice(&result);
        }

        current == proof.root || current == self.get_root_bytes()
    }
}

/// Proof of event inclusion in log
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InclusionProof {
    pub event_id: EventId,
    pub index: usize,
    pub root: [u8; 32],
    pub path: Vec<[u8; 32]>,
}

// ============================================================================
// Witness Tracking (Axiom 8: Witnesses matter)
// ============================================================================

/// Witness record for a claim
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Claim being witnessed
    pub claim_id: EventId,
    /// Witness public key
    pub witness: PublicKeyBytes,
    /// Witness path (how the witness learned of the claim)
    pub path: Vec<PublicKeyBytes>,
    /// Timestamp of witnessing
    pub witnessed_at: u64,
    /// Signature of witness
    pub signature: SignatureBytes,
}

/// Manages witness tracking for claims
#[wasm_bindgen]
pub struct WitnessTracker {
    /// Witnesses by claim ID
    witnesses: RwLock<FxHashMap<String, Vec<WitnessRecord>>>,
    /// Minimum independent witnesses required
    min_witnesses: usize,
}

#[wasm_bindgen]
impl WitnessTracker {
    /// Create a new witness tracker
    #[wasm_bindgen(constructor)]
    pub fn new(min_witnesses: usize) -> Self {
        Self {
            witnesses: RwLock::new(FxHashMap::default()),
            min_witnesses: min_witnesses.max(1),
        }
    }

    /// Get witness count for a claim
    #[wasm_bindgen(js_name = witnessCount)]
    pub fn witness_count(&self, claim_id: &str) -> usize {
        self.witnesses.read().unwrap()
            .get(claim_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Check if claim has sufficient independent witnesses
    #[wasm_bindgen(js_name = hasSufficientWitnesses)]
    pub fn has_sufficient_witnesses(&self, claim_id: &str) -> bool {
        let witnesses = self.witnesses.read().unwrap();
        if let Some(records) = witnesses.get(claim_id) {
            // Count independent witness paths (no common intermediate nodes)
            let independent = self.count_independent_paths(records);
            independent >= self.min_witnesses
        } else {
            false
        }
    }

    /// Get confidence score based on witness diversity
    #[wasm_bindgen(js_name = witnessConfidence)]
    pub fn witness_confidence(&self, claim_id: &str) -> f32 {
        let witnesses = self.witnesses.read().unwrap();
        if let Some(records) = witnesses.get(claim_id) {
            let independent = self.count_independent_paths(records);
            // Confidence scales with independent witnesses, capped at 1.0
            (independent as f32 / (self.min_witnesses as f32 * 2.0)).min(1.0)
        } else {
            0.0
        }
    }
}

impl WitnessTracker {
    /// Add a witness record
    pub fn add_witness(&self, record: WitnessRecord) {
        let claim_key = hex::encode(&record.claim_id);
        let mut witnesses = self.witnesses.write().unwrap();
        witnesses.entry(claim_key).or_default().push(record);
    }

    /// Get all witnesses for a claim
    pub fn get_witnesses(&self, claim_id: &EventId) -> Vec<WitnessRecord> {
        let claim_key = hex::encode(claim_id);
        self.witnesses.read().unwrap()
            .get(&claim_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Count independent witness paths (no common intermediate nodes)
    fn count_independent_paths(&self, records: &[WitnessRecord]) -> usize {
        if records.is_empty() {
            return 0;
        }

        let mut independent_count = 1;
        let mut seen_intermediates: FxHashMap<[u8; 32], bool> = FxHashMap::default();

        // First witness path is always independent
        for key in &records[0].path {
            seen_intermediates.insert(*key, true);
        }

        // Check remaining witnesses for path independence
        for record in records.iter().skip(1) {
            let mut has_common = false;
            for key in &record.path {
                if seen_intermediates.contains_key(key) {
                    has_common = true;
                    break;
                }
            }

            if !has_common {
                independent_count += 1;
                // Add this path's intermediates
                for key in &record.path {
                    seen_intermediates.insert(*key, true);
                }
            }
        }

        independent_count
    }
}

impl Default for WitnessTracker {
    fn default() -> Self {
        Self::new(3)
    }
}

// ============================================================================
// Drift Tracking (Axiom 5: Semantics drift is expected)
// ============================================================================

/// Semantic drift record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftRecord {
    /// Context being tracked
    pub context: ContextId,
    /// Baseline embedding
    pub baseline: Ruvector,
    /// Current centroid
    pub current: Ruvector,
    /// Drift magnitude (0.0 - 1.0)
    pub drift: f64,
    /// Last updated timestamp
    pub updated_at: u64,
    /// Sample count
    pub sample_count: usize,
}

/// Manages semantic drift tracking
#[wasm_bindgen]
pub struct DriftTracker {
    /// Drift records by context
    records: RwLock<FxHashMap<String, DriftRecord>>,
    /// Drift threshold for alerts
    drift_threshold: f64,
}

#[wasm_bindgen]
impl DriftTracker {
    /// Create a new drift tracker
    #[wasm_bindgen(constructor)]
    pub fn new(drift_threshold: f64) -> Self {
        Self {
            records: RwLock::new(FxHashMap::default()),
            drift_threshold: drift_threshold.clamp(0.0, 1.0),
        }
    }

    /// Get drift for a context
    #[wasm_bindgen(js_name = getDrift)]
    pub fn get_drift(&self, context_hex: &str) -> f64 {
        self.records.read().unwrap()
            .get(context_hex)
            .map(|r| r.drift)
            .unwrap_or(0.0)
    }

    /// Check if context has drifted beyond threshold
    #[wasm_bindgen(js_name = hasDrifted)]
    pub fn has_drifted(&self, context_hex: &str) -> bool {
        self.get_drift(context_hex) > self.drift_threshold
    }

    /// Get contexts with significant drift
    #[wasm_bindgen(js_name = getDriftedContexts)]
    pub fn get_drifted_contexts(&self) -> String {
        let records = self.records.read().unwrap();
        let drifted: Vec<&str> = records.iter()
            .filter(|(_, r)| r.drift > self.drift_threshold)
            .map(|(k, _)| k.as_str())
            .collect();
        serde_json::to_string(&drifted).unwrap_or_else(|_| "[]".to_string())
    }
}

impl DriftTracker {
    /// Update drift tracking for a context with new embedding
    pub fn update(&self, context: &ContextId, embedding: &Ruvector) {
        let context_key = hex::encode(context);
        let mut records = self.records.write().unwrap();

        let now = current_timestamp_ms();

        records.entry(context_key)
            .and_modify(|r| {
                // Update running centroid with exponential moving average
                let alpha = 0.1; // Smoothing factor
                for (i, dim) in r.current.dims.iter_mut().enumerate() {
                    if i < embedding.dims.len() {
                        *dim = *dim * (1.0 - alpha as f32) + embedding.dims[i] * alpha as f32;
                    }
                }
                r.drift = r.current.drift_from(&r.baseline);
                r.updated_at = now;
                r.sample_count += 1;
            })
            .or_insert_with(|| DriftRecord {
                context: *context,
                baseline: embedding.clone(),
                current: embedding.clone(),
                drift: 0.0,
                updated_at: now,
                sample_count: 1,
            });
    }

    /// Reset baseline for a context
    pub fn reset_baseline(&self, context: &ContextId) {
        let context_key = hex::encode(context);
        let mut records = self.records.write().unwrap();

        if let Some(record) = records.get_mut(&context_key) {
            record.baseline = record.current.clone();
            record.drift = 0.0;
        }
    }
}

impl Default for DriftTracker {
    fn default() -> Self {
        Self::new(0.3)
    }
}

// ============================================================================
// Conflict Detection (Axiom 6: Disagreement is signal)
// ============================================================================

/// A detected conflict between claims
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conflict {
    /// Conflict identifier
    pub id: [u8; 32],
    /// Context where conflict occurs
    pub context: ContextId,
    /// Conflicting claim IDs
    pub claim_ids: Vec<EventId>,
    /// Detected timestamp
    pub detected_at: u64,
    /// Current status
    pub status: ConflictStatus,
    /// Epistemic temperature (how heated the dispute is)
    pub temperature: f32,
    /// Escalation count
    pub escalation_count: u32,
}

/// Status of a conflict
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConflictStatus {
    /// Conflict detected, awaiting challenge
    Detected,
    /// Challenge opened, collecting evidence
    Challenged,
    /// Resolution proposed
    Resolving,
    /// Conflict resolved
    Resolved,
    /// Escalated to higher authority
    Escalated,
}

/// Escalation configuration
#[derive(Clone, Debug)]
pub struct EscalationConfig {
    /// Temperature threshold for escalation
    pub temperature_threshold: f32,
    /// Duration threshold in ms for escalation
    pub duration_threshold_ms: u64,
    /// Maximum escalation levels
    pub max_escalation: u32,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            temperature_threshold: 0.8,
            duration_threshold_ms: 3600_000, // 1 hour
            max_escalation: 3,
        }
    }
}

// ============================================================================
// Quarantine Manager (Axiom 9: Quarantine is mandatory)
// ============================================================================

/// Quarantine levels for contested claims
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum QuarantineLevel {
    /// Claim can be used normally
    None = 0,
    /// Claim can be used with conservative bounds
    Conservative = 1,
    /// Claim requires multiple independent confirmations
    RequiresWitness = 2,
    /// Claim cannot be used in decisions
    Blocked = 3,
}

/// Manages quarantine status of contested claims
#[wasm_bindgen]
pub struct QuarantineManager {
    /// Quarantine levels by claim ID
    levels: RwLock<FxHashMap<String, QuarantineLevel>>,
    /// Active conflicts by context
    conflicts: RwLock<FxHashMap<String, Vec<Conflict>>>,
}

#[wasm_bindgen]
impl QuarantineManager {
    /// Create a new quarantine manager
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            levels: RwLock::new(FxHashMap::default()),
            conflicts: RwLock::new(FxHashMap::default()),
        }
    }

    /// Check quarantine level for a claim
    #[wasm_bindgen(js_name = getLevel)]
    pub fn get_level(&self, claim_id: &str) -> u8 {
        let levels = self.levels.read().unwrap();
        levels.get(claim_id)
            .map(|&l| l as u8)
            .unwrap_or(0)
    }

    /// Set quarantine level
    #[wasm_bindgen(js_name = setLevel)]
    pub fn set_level(&self, claim_id: &str, level: u8) {
        let quarantine_level = match level {
            0 => QuarantineLevel::None,
            1 => QuarantineLevel::Conservative,
            2 => QuarantineLevel::RequiresWitness,
            _ => QuarantineLevel::Blocked,
        };
        self.levels.write().unwrap().insert(claim_id.to_string(), quarantine_level);
    }

    /// Check if claim can be used in decisions
    #[wasm_bindgen(js_name = canUse)]
    pub fn can_use(&self, claim_id: &str) -> bool {
        self.get_level(claim_id) < QuarantineLevel::Blocked as u8
    }

    /// Get number of quarantined claims
    #[wasm_bindgen(js_name = quarantinedCount)]
    pub fn quarantined_count(&self) -> usize {
        let levels = self.levels.read().unwrap();
        levels.values().filter(|&&l| l != QuarantineLevel::None).count()
    }
}

impl Default for QuarantineManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QuarantineManager {
    /// Get all quarantined claims
    pub fn get_quarantined(&self) -> Vec<(String, QuarantineLevel)> {
        let levels = self.levels.read().unwrap();
        levels.iter()
            .filter(|(_, &l)| l != QuarantineLevel::None)
            .map(|(k, &v)| (k.clone(), v))
            .collect()
    }
}

// ============================================================================
// Authority Policy (Axiom 7: Authority is scoped, not global)
// ============================================================================

/// Authority policy for a context
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScopedAuthority {
    /// Context this policy applies to
    pub context: ContextId,
    /// Authorized keys
    pub authorized_keys: Vec<PublicKeyBytes>,
    /// Threshold (k-of-n)
    pub threshold: usize,
    /// Allowed evidence types
    pub allowed_evidence: Vec<String>,
}

impl ScopedAuthority {
    /// Create a new scoped authority
    pub fn new(context: ContextId, authorized_keys: Vec<PublicKeyBytes>, threshold: usize) -> Self {
        Self {
            context,
            authorized_keys,
            threshold: threshold.max(1),
            allowed_evidence: vec!["hash".to_string(), "url".to_string(), "log".to_string()],
        }
    }

    /// Compute the canonical message to sign for a resolution
    fn resolution_sign_message(resolution: &ResolutionEvent, context: &ContextId) -> Vec<u8> {
        let mut message = Vec::with_capacity(128);
        message.extend_from_slice(b"RAC_RESOLUTION_V1:");
        message.extend_from_slice(context);
        message.extend_from_slice(&resolution.conflict_id);
        for claim_id in &resolution.accepted {
            message.extend_from_slice(claim_id);
        }
        for claim_id in &resolution.deprecated {
            message.extend_from_slice(claim_id);
        }
        message
    }

    /// Verify a single Ed25519 signature against a public key
    fn verify_ed25519_signature(public_key: &PublicKeyBytes, message: &[u8], signature: &[u8]) -> bool {
        if signature.len() != 64 {
            return false;
        }

        let verifying_key = match VerifyingKey::from_bytes(public_key) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };

        let sig = Signature::from_bytes(&sig_bytes);
        Ed25519Verifier::verify(&verifying_key, message, &sig).is_ok()
    }

    /// Check if resolution has sufficient authorized signatures (Ed25519 verified)
    pub fn verify_resolution(&self, resolution: &ResolutionEvent) -> bool {
        if resolution.authority_sigs.len() < self.threshold {
            return false;
        }

        // Compute the canonical message that should have been signed
        let message = Self::resolution_sign_message(resolution, &self.context);

        // Count valid signatures from authorized keys
        let mut valid_sigs = 0;
        let mut used_keys: Vec<PublicKeyBytes> = Vec::new();

        for sig in &resolution.authority_sigs {
            // Try each authorized key to find a match
            for auth_key in &self.authorized_keys {
                // Prevent same key being used twice
                if used_keys.contains(auth_key) {
                    continue;
                }

                if Self::verify_ed25519_signature(auth_key, &message, sig) {
                    valid_sigs += 1;
                    used_keys.push(*auth_key);
                    break;
                }
            }

            // Early exit if we have enough valid signatures
            if valid_sigs >= self.threshold {
                return true;
            }
        }

        valid_sigs >= self.threshold
    }

    /// Sign a resolution with the given signing key (utility for testing/creating valid resolutions)
    pub fn sign_resolution(resolution: &ResolutionEvent, context: &ContextId, signing_key_bytes: &[u8; 32]) -> Vec<u8> {
        use ed25519_dalek::SigningKey;

        let signing_key = SigningKey::from_bytes(signing_key_bytes);
        let message = Self::resolution_sign_message(resolution, context);

        use ed25519_dalek::Signer;
        signing_key.sign(&message).to_bytes().to_vec()
    }
}

/// Trait for authority policy verification
pub trait AuthorityPolicy: Send + Sync {
    /// Check if a resolution is authorized for this context
    fn authorized(&self, context: &ContextId, resolution: &ResolutionEvent) -> bool;

    /// Get quarantine level for a conflict
    fn quarantine_level(&self, context: &ContextId, conflict_id: &[u8; 32]) -> QuarantineLevel;
}

/// Default authority policy that allows all resolutions (for testing)
pub struct DefaultAuthorityPolicy;

impl AuthorityPolicy for DefaultAuthorityPolicy {
    fn authorized(&self, _context: &ContextId, resolution: &ResolutionEvent) -> bool {
        // Require at least one signature
        !resolution.authority_sigs.is_empty()
    }

    fn quarantine_level(&self, _context: &ContextId, _conflict_id: &[u8; 32]) -> QuarantineLevel {
        QuarantineLevel::RequiresWitness
    }
}

/// Trait for semantic verification
pub trait Verifier: Send + Sync {
    /// Check if two assertions are incompatible
    fn incompatible(&self, context: &ContextId, a: &AssertEvent, b: &AssertEvent) -> bool;
}

/// Default verifier that checks proposition equality
pub struct DefaultVerifier;

impl Verifier for DefaultVerifier {
    fn incompatible(&self, _context: &ContextId, a: &AssertEvent, b: &AssertEvent) -> bool {
        // Simple: different propositions with high confidence are incompatible
        a.proposition != b.proposition && a.confidence > 0.7 && b.confidence > 0.7
    }
}

// ============================================================================
// Coherence Engine (The Core Loop)
// ============================================================================

/// Statistics from the coherence engine
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CoherenceStats {
    pub events_processed: usize,
    pub conflicts_detected: usize,
    pub conflicts_resolved: usize,
    pub claims_deprecated: usize,
    pub quarantined_claims: usize,
    pub escalations: usize,
    pub unauthorized_resolutions: usize,
}

/// Result of event ingestion
#[derive(Clone, Debug)]
pub enum IngestResult {
    /// Event ingested successfully
    Success(EventId),
    /// Resolution was unauthorized
    UnauthorizedResolution,
    /// Event was invalid
    Invalid(String),
}

/// The main coherence engine running the RAC protocol
#[wasm_bindgen]
pub struct CoherenceEngine {
    /// Event log
    log: EventLog,
    /// Quarantine manager
    quarantine: QuarantineManager,
    /// Witness tracker
    witnesses: WitnessTracker,
    /// Drift tracker
    drift: DriftTracker,
    /// Statistics
    stats: RwLock<CoherenceStats>,
    /// Active conflicts by context
    conflicts: RwLock<FxHashMap<String, Vec<Conflict>>>,
    /// Semantic clusters for conflict detection
    clusters: RwLock<FxHashMap<String, Vec<EventId>>>,
    /// Authority policies by context
    authorities: RwLock<FxHashMap<String, ScopedAuthority>>,
    /// Escalation configuration
    escalation_config: EscalationConfig,
}

#[wasm_bindgen]
impl CoherenceEngine {
    /// Create a new coherence engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            log: EventLog::new(),
            quarantine: QuarantineManager::new(),
            witnesses: WitnessTracker::new(3),
            drift: DriftTracker::new(0.3),
            stats: RwLock::new(CoherenceStats::default()),
            conflicts: RwLock::new(FxHashMap::default()),
            clusters: RwLock::new(FxHashMap::default()),
            authorities: RwLock::new(FxHashMap::default()),
            escalation_config: EscalationConfig::default(),
        }
    }

    /// Get event log length
    #[wasm_bindgen(js_name = eventCount)]
    pub fn event_count(&self) -> usize {
        self.log.len()
    }

    /// Get current Merkle root
    #[wasm_bindgen(js_name = getMerkleRoot)]
    pub fn get_merkle_root(&self) -> String {
        self.log.get_root()
    }

    /// Get quarantined claim count
    #[wasm_bindgen(js_name = quarantinedCount)]
    pub fn quarantined_count(&self) -> usize {
        self.quarantine.quarantined_count()
    }

    /// Get conflict count
    #[wasm_bindgen(js_name = conflictCount)]
    pub fn conflict_count(&self) -> usize {
        self.conflicts.read().unwrap().values().map(|v| v.len()).sum()
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let stats = self.stats.read().unwrap();
        serde_json::to_string(&*stats).unwrap_or_else(|_| "{}".to_string())
    }

    /// Check quarantine level for a claim
    #[wasm_bindgen(js_name = getQuarantineLevel)]
    pub fn get_quarantine_level(&self, claim_id: &str) -> u8 {
        self.quarantine.get_level(claim_id)
    }

    /// Check if a claim can be used in decisions
    #[wasm_bindgen(js_name = canUseClaim)]
    pub fn can_use_claim(&self, claim_id: &str) -> bool {
        self.quarantine.can_use(claim_id)
    }

    /// Get witness count for a claim
    #[wasm_bindgen(js_name = witnessCount)]
    pub fn witness_count(&self, claim_id: &str) -> usize {
        self.witnesses.witness_count(claim_id)
    }

    /// Check if claim has sufficient witnesses
    #[wasm_bindgen(js_name = hasSufficientWitnesses)]
    pub fn has_sufficient_witnesses(&self, claim_id: &str) -> bool {
        self.witnesses.has_sufficient_witnesses(claim_id)
    }

    /// Get drift for a context
    #[wasm_bindgen(js_name = getDrift)]
    pub fn get_drift(&self, context_hex: &str) -> f64 {
        self.drift.get_drift(context_hex)
    }

    /// Check if context has drifted
    #[wasm_bindgen(js_name = hasDrifted)]
    pub fn has_drifted(&self, context_hex: &str) -> bool {
        self.drift.has_drifted(context_hex)
    }
}

impl Default for CoherenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CoherenceEngine {
    /// Register an authority policy for a context
    pub fn register_authority(&self, authority: ScopedAuthority) {
        let context_key = hex::encode(&authority.context);
        self.authorities.write().unwrap().insert(context_key, authority);
    }

    /// Check if a resolution is authorized (Axiom 7)
    fn verify_authority(&self, context: &ContextId, resolution: &ResolutionEvent) -> bool {
        let context_key = hex::encode(context);
        let authorities = self.authorities.read().unwrap();

        if let Some(authority) = authorities.get(&context_key) {
            authority.verify_resolution(resolution)
        } else {
            // No registered authority - require at least one signature
            !resolution.authority_sigs.is_empty()
        }
    }

    /// Ingest an event into the coherence engine with full validation
    pub fn ingest(&mut self, event: Event) -> IngestResult {
        // Track drift for all events (Axiom 5)
        self.drift.update(&event.context, &event.ruvector);

        // Handle based on event type
        match &event.kind {
            EventKind::Resolution(resolution) => {
                // CRITICAL: Verify authority before applying resolution (Axiom 7)
                if !self.verify_authority(&event.context, resolution) {
                    let mut stats = self.stats.write().unwrap();
                    stats.unauthorized_resolutions += 1;
                    return IngestResult::UnauthorizedResolution;
                }
            }
            _ => {}
        }

        // Append to log
        let event_id = self.log.append(event.clone());

        // Update statistics
        let mut stats = self.stats.write().unwrap();
        stats.events_processed += 1;

        // Handle based on event type
        match &event.kind {
            EventKind::Assert(_) => {
                // Add to semantic cluster for conflict detection
                let context_key = hex::encode(&event.context);
                let mut clusters = self.clusters.write().unwrap();
                clusters.entry(context_key).or_default().push(event_id);
            }
            EventKind::Challenge(challenge) => {
                // Record conflict with escalation tracking
                let context_key = hex::encode(&event.context);
                let conflict = Conflict {
                    id: challenge.conflict_id,
                    context: event.context,
                    claim_ids: challenge.claim_ids.clone(),
                    detected_at: event.ts_unix_ms,
                    status: ConflictStatus::Challenged,
                    temperature: 0.5,
                    escalation_count: 0,
                };

                let mut conflicts = self.conflicts.write().unwrap();
                conflicts.entry(context_key).or_default().push(conflict);

                // Quarantine disputed claims (Axiom 9)
                for claim_id in &challenge.claim_ids {
                    self.quarantine.set_level(&hex::encode(claim_id), 2);
                }

                stats.conflicts_detected += 1;
            }
            EventKind::Support(support) => {
                // Update conflict temperature based on support (Axiom 6)
                let context_key = hex::encode(&event.context);
                let mut conflicts = self.conflicts.write().unwrap();

                if let Some(context_conflicts) = conflicts.get_mut(&context_key) {
                    for conflict in context_conflicts.iter_mut() {
                        if conflict.id == support.conflict_id {
                            // Increase temperature based on support cost/weight
                            conflict.temperature = (conflict.temperature + 0.1).min(1.0);

                            // Check for escalation (Axiom 6)
                            if conflict.temperature > self.escalation_config.temperature_threshold
                                && conflict.escalation_count < self.escalation_config.max_escalation
                            {
                                conflict.status = ConflictStatus::Escalated;
                                conflict.escalation_count += 1;
                                stats.escalations += 1;
                            }
                        }
                    }
                }
            }
            EventKind::Resolution(resolution) => {
                // Apply resolution (already verified above)
                for claim_id in &resolution.deprecated {
                    self.quarantine.set_level(&hex::encode(claim_id), 3);
                    stats.claims_deprecated += 1;
                }

                // Remove quarantine from accepted claims
                for claim_id in &resolution.accepted {
                    self.quarantine.set_level(&hex::encode(claim_id), 0);
                }

                // Update conflict status
                let context_key = hex::encode(&event.context);
                let mut conflicts = self.conflicts.write().unwrap();
                if let Some(context_conflicts) = conflicts.get_mut(&context_key) {
                    for conflict in context_conflicts.iter_mut() {
                        if conflict.id == resolution.conflict_id {
                            conflict.status = ConflictStatus::Resolved;
                        }
                    }
                }

                stats.conflicts_resolved += 1;
            }
            EventKind::Deprecate(deprecate) => {
                self.quarantine.set_level(&hex::encode(&deprecate.claim_id), 3);
                stats.claims_deprecated += 1;
            }
            EventKind::ModelClaim(_) => {
                // Model claims are handled separately by validate_weight_consensus
            }
        }

        stats.quarantined_claims = self.quarantine.quarantined_count();

        IngestResult::Success(event_id)
    }

    /// Legacy ingest method for compatibility (does not return result)
    pub fn ingest_event(&mut self, event: Event) {
        let _ = self.ingest(event);
    }

    /// Add a witness record for a claim
    pub fn add_witness(&self, record: WitnessRecord) {
        self.witnesses.add_witness(record);
    }

    /// Detect conflicts in a context
    pub fn detect_conflicts<V: Verifier>(
        &self,
        context: &ContextId,
        verifier: &V,
    ) -> Vec<Conflict> {
        let context_key = hex::encode(context);
        let clusters = self.clusters.read().unwrap();

        let Some(event_ids) = clusters.get(&context_key) else {
            return Vec::new();
        };

        let mut conflicts = Vec::new();
        let now = current_timestamp_ms();

        // Check all pairs for incompatibility
        for (i, id_a) in event_ids.iter().enumerate() {
            let Some(event_a) = self.log.get(id_a) else { continue };
            let EventKind::Assert(assert_a) = &event_a.kind else { continue };

            for id_b in event_ids.iter().skip(i + 1) {
                let Some(event_b) = self.log.get(id_b) else { continue };
                let EventKind::Assert(assert_b) = &event_b.kind else { continue };

                if verifier.incompatible(context, assert_a, assert_b) {
                    use sha2::{Sha256, Digest};
                    let mut hasher = Sha256::new();
                    hasher.update(id_a);
                    hasher.update(id_b);
                    let result = hasher.finalize();
                    let mut conflict_id = [0u8; 32];
                    conflict_id.copy_from_slice(&result);

                    conflicts.push(Conflict {
                        id: conflict_id,
                        context: *context,
                        claim_ids: vec![*id_a, *id_b],
                        detected_at: now,
                        status: ConflictStatus::Detected,
                        temperature: 0.3,
                        escalation_count: 0,
                    });
                }
            }
        }

        conflicts
    }

    /// Get all conflicts for a context
    pub fn get_conflicts(&self, context: &ContextId) -> Vec<Conflict> {
        let context_key = hex::encode(context);
        self.conflicts.read().unwrap()
            .get(&context_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Get audit proof for event inclusion
    pub fn prove_inclusion(&self, event_id: &EventId) -> Option<InclusionProof> {
        self.log.prove_inclusion(event_id)
    }

    /// Verify an inclusion proof
    pub fn verify_proof(&self, proof: &InclusionProof) -> bool {
        self.log.verify_proof(proof)
    }

    /// Get event by ID
    pub fn get_event(&self, id: &EventId) -> Option<Event> {
        self.log.get(id)
    }

    /// Get all events for a context
    pub fn get_context_events(&self, context: &ContextId) -> Vec<Event> {
        self.log.for_context(context)
    }
}

// ============================================================================
// Decision Trace (Axiom 10: All decisions are replayable)
// ============================================================================

/// A replayable decision trace
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionTrace {
    /// Decision ID
    pub id: [u8; 32],
    /// Events this decision depends on
    pub dependencies: Vec<EventId>,
    /// Decision timestamp
    pub timestamp: u64,
    /// Whether any dependencies are disputed
    pub has_disputed: bool,
    /// Quarantine policy used
    pub quarantine_policy: String,
    /// Decision outcome
    pub outcome: Vec<u8>,
}

impl DecisionTrace {
    /// Create a new decision trace
    pub fn new(dependencies: Vec<EventId>, outcome: Vec<u8>) -> Self {
        use sha2::{Sha256, Digest};

        // Generate decision ID from dependencies
        let mut hasher = Sha256::new();
        for dep in &dependencies {
            hasher.update(dep);
        }
        hasher.update(&outcome);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);

        Self {
            id,
            dependencies,
            timestamp: current_timestamp_ms(),
            has_disputed: false,
            quarantine_policy: "default".to_string(),
            outcome,
        }
    }

    /// Create with explicit timestamp (for testing)
    pub fn with_timestamp(dependencies: Vec<EventId>, outcome: Vec<u8>, timestamp: u64) -> Self {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        for dep in &dependencies {
            hasher.update(dep);
        }
        hasher.update(&outcome);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);

        Self {
            id,
            dependencies,
            timestamp,
            has_disputed: false,
            quarantine_policy: "default".to_string(),
            outcome,
        }
    }

    /// Check if decision can be replayed given current state
    /// For decisions, any quarantine level blocks replay (Axiom 9)
    pub fn can_replay(&self, engine: &CoherenceEngine) -> bool {
        // All dependencies must exist and have no quarantine (any level)
        for dep in &self.dependencies {
            let dep_hex = hex::encode(dep);
            // Decisions cannot use any disputed claims (stricter than general can_use)
            if engine.get_quarantine_level(&dep_hex) > 0 {
                return false;
            }
        }
        true
    }

    /// Mark disputed dependencies
    pub fn check_disputes(&mut self, engine: &CoherenceEngine) {
        for dep in &self.dependencies {
            let dep_hex = hex::encode(dep);
            if engine.get_quarantine_level(&dep_hex) > 0 {
                self.has_disputed = true;
                return;
            }
        }
        self.has_disputed = false;
    }
}

// ============================================================================
// Semantic Gossip Routing
// ============================================================================

/// Peer routing entry for semantic gossip
#[derive(Clone, Debug)]
pub struct PeerRoute {
    /// Peer public key
    pub peer_id: PublicKeyBytes,
    /// Peer's semantic centroid
    pub centroid: Ruvector,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Latency estimate in ms
    pub latency_ms: u32,
}

/// RAC-specific semantic gossip router for event propagation
#[wasm_bindgen(js_name = RacSemanticRouter)]
pub struct RacSemanticRouter {
    /// Known peers
    peers: RwLock<Vec<PeerRoute>>,
    /// Random peer sample size
    random_sample: usize,
    /// Semantic neighbor count
    semantic_neighbors: usize,
}

#[wasm_bindgen]
impl RacSemanticRouter {
    /// Create a new semantic router
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(Vec::new()),
            random_sample: 3,
            semantic_neighbors: 5,
        }
    }

    /// Get peer count
    #[wasm_bindgen(js_name = peerCount)]
    pub fn peer_count(&self) -> usize {
        self.peers.read().unwrap().len()
    }
}

impl Default for RacSemanticRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl RacSemanticRouter {
    /// Register a peer
    pub fn register_peer(&self, peer_id: PublicKeyBytes, centroid: Ruvector, latency_ms: u32) {
        let mut peers = self.peers.write().unwrap();

        // Update existing or add new
        if let Some(peer) = peers.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.centroid = centroid;
            peer.last_seen = current_timestamp_ms();
            peer.latency_ms = latency_ms;
        } else {
            peers.push(PeerRoute {
                peer_id,
                centroid,
                last_seen: current_timestamp_ms(),
                latency_ms,
            });
        }
    }

    /// Get routing targets for an event (semantic neighbors + random sample)
    pub fn get_routes(&self, event: &Event) -> Vec<PublicKeyBytes> {
        let peers = self.peers.read().unwrap();

        if peers.is_empty() {
            return Vec::new();
        }

        let mut routes = Vec::with_capacity(self.semantic_neighbors + self.random_sample);

        // Sort by semantic similarity
        let mut scored: Vec<_> = peers.iter()
            .map(|p| (p, event.ruvector.similarity(&p.centroid)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take semantic neighbors
        for (peer, _) in scored.iter().take(self.semantic_neighbors) {
            routes.push(peer.peer_id);
        }

        // Add random sample for robustness
        use std::collections::HashSet;
        let selected: HashSet<_> = routes.iter().cloned().collect();

        // Simple deterministic "random" selection based on event ID
        let mut seed = 0u64;
        for byte in event.id.iter() {
            seed = seed.wrapping_mul(31).wrapping_add(*byte as u64);
        }

        for (i, peer) in peers.iter().enumerate() {
            if routes.len() >= self.semantic_neighbors + self.random_sample {
                break;
            }
            let pseudo_random = (seed.wrapping_add(i as u64)) % (peers.len() as u64);
            if pseudo_random < self.random_sample as u64 && !selected.contains(&peer.peer_id) {
                routes.push(peer.peer_id);
            }
        }

        routes
    }

    /// Prune stale peers
    pub fn prune_stale(&self, max_age_ms: u64) {
        let now = current_timestamp_ms();
        let mut peers = self.peers.write().unwrap();
        peers.retain(|p| now - p.last_seen < max_age_ms);
    }
}

// ============================================================================
// AI Model Consensus (Axiom 2, 7, 8, 9: Events, Authority, Witnesses, Quarantine)
// ============================================================================

/// Result of model weight consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightConsensus {
    /// Model ID that reached consensus
    pub model_id: String,
    /// Agreed-upon weight version
    pub agreed_version: u64,
    /// Agreed-upon weights hash
    pub agreed_hash: [u8; 32],
    /// Number of witnesses supporting this version
    pub witness_count: usize,
    /// Confidence in consensus (0.0 - 1.0)
    pub confidence: f32,
    /// Timestamp when consensus was reached
    pub consensus_time: u64,
    /// Event IDs that contributed to consensus
    pub contributing_events: Vec<EventId>,
    /// Any conflicting claims that were quarantined
    pub quarantined_claims: Vec<EventId>,
}

/// Dispute record for model updates
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelDispute {
    /// Model being disputed
    pub model_id: String,
    /// Conflicting version claims
    pub version_conflicts: Vec<(EventId, u64)>,
    /// Conflicting hash claims
    pub hash_conflicts: Vec<(EventId, [u8; 32])>,
    /// Dispute severity (0.0 - 1.0)
    pub severity: f32,
    /// When dispute was detected
    pub detected_at: u64,
    /// Resolution status
    pub resolved: bool,
}

/// Gradient validation result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientValidation {
    /// Whether gradient is valid
    pub valid: bool,
    /// Validation score (0.0 - 1.0)
    pub score: f32,
    /// Reason if invalid
    pub rejection_reason: Option<String>,
    /// Detected anomalies
    pub anomalies: Vec<String>,
    /// Contributor reputation factor
    pub reputation_factor: f32,
}

/// Model consensus manager for federated learning integration
#[wasm_bindgen]
pub struct ModelConsensusManager {
    /// Model weight claims by model_id -> layer -> versions
    model_claims: RwLock<FxHashMap<String, FxHashMap<String, Vec<(EventId, ModelWeightClaim)>>>>,
    /// Gradient contributions by round -> contributor
    gradient_claims: RwLock<FxHashMap<u64, FxHashMap<PublicKeyBytes, Vec<(EventId, GradientContributionClaim)>>>>,
    /// LoRA adapter claims by adapter_id
    lora_claims: RwLock<FxHashMap<String, Vec<(EventId, LoraAdapterClaim)>>>,
    /// Learning pattern claims by pattern_id
    pattern_claims: RwLock<FxHashMap<String, Vec<(EventId, LearningPatternClaim)>>>,
    /// Active disputes
    disputes: RwLock<Vec<ModelDispute>>,
    /// Quarantined model updates
    quarantined_updates: RwLock<FxHashMap<String, Vec<EventId>>>,
    /// Minimum witnesses for consensus
    min_witnesses: usize,
    /// Equivocation detection window (ms)
    equivocation_window_ms: u64,
    /// Maximum gradient norm (for anomaly detection)
    max_gradient_norm: f32,
}

#[wasm_bindgen]
impl ModelConsensusManager {
    /// Create a new model consensus manager
    #[wasm_bindgen(constructor)]
    pub fn new(min_witnesses: usize) -> Self {
        Self {
            model_claims: RwLock::new(FxHashMap::default()),
            gradient_claims: RwLock::new(FxHashMap::default()),
            lora_claims: RwLock::new(FxHashMap::default()),
            pattern_claims: RwLock::new(FxHashMap::default()),
            disputes: RwLock::new(Vec::new()),
            quarantined_updates: RwLock::new(FxHashMap::default()),
            min_witnesses: min_witnesses.max(1),
            equivocation_window_ms: 60_000, // 1 minute
            max_gradient_norm: 100.0,
        }
    }

    /// Get number of tracked models
    #[wasm_bindgen(js_name = modelCount)]
    pub fn model_count(&self) -> usize {
        self.model_claims.read().unwrap().len()
    }

    /// Get number of active disputes
    #[wasm_bindgen(js_name = disputeCount)]
    pub fn dispute_count(&self) -> usize {
        self.disputes.read().unwrap().iter().filter(|d| !d.resolved).count()
    }

    /// Get number of quarantined updates
    #[wasm_bindgen(js_name = quarantinedUpdateCount)]
    pub fn quarantined_update_count(&self) -> usize {
        self.quarantined_updates.read().unwrap()
            .values()
            .map(|v| v.len())
            .sum()
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let model_count = self.model_count();
        let dispute_count = self.dispute_count();
        let quarantined = self.quarantined_update_count();
        let gradient_rounds = self.gradient_claims.read().unwrap().len();
        let lora_count = self.lora_claims.read().unwrap().len();
        let pattern_count = self.pattern_claims.read().unwrap().len();

        format!(
            r#"{{"models":{},"disputes":{},"quarantined":{},"gradient_rounds":{},"lora_adapters":{},"patterns":{}}}"#,
            model_count, dispute_count, quarantined, gradient_rounds, lora_count, pattern_count
        )
    }
}

impl Default for ModelConsensusManager {
    fn default() -> Self {
        Self::new(3)
    }
}

impl ModelConsensusManager {
    /// Register a model weight claim
    pub fn register_model_claim(&self, event_id: EventId, claim: ModelWeightClaim) {
        let mut claims = self.model_claims.write().unwrap();
        claims
            .entry(claim.model_id.clone())
            .or_default()
            .entry(claim.layer.clone())
            .or_default()
            .push((event_id, claim));
    }

    /// Register a gradient contribution claim
    pub fn register_gradient_claim(&self, event_id: EventId, claim: GradientContributionClaim) {
        let mut claims = self.gradient_claims.write().unwrap();
        claims
            .entry(claim.round)
            .or_default()
            .entry(claim.contributor)
            .or_default()
            .push((event_id, claim));
    }

    /// Register a LoRA adapter claim
    pub fn register_lora_claim(&self, event_id: EventId, claim: LoraAdapterClaim) {
        let mut claims = self.lora_claims.write().unwrap();
        claims
            .entry(claim.adapter_id.clone())
            .or_default()
            .push((event_id, claim));
    }

    /// Register a learning pattern claim
    pub fn register_pattern_claim(&self, event_id: EventId, claim: LearningPatternClaim) {
        let mut claims = self.pattern_claims.write().unwrap();
        claims
            .entry(claim.pattern_id.clone())
            .or_default()
            .push((event_id, claim));
    }

    /// Attempt to reach consensus on model weights
    pub fn model_consensus(&self, model_id: &str, layer: &str) -> Option<WeightConsensus> {
        let claims = self.model_claims.read().unwrap();
        let quarantined = self.quarantined_updates.read().unwrap();

        let model_claims = claims.get(model_id)?;
        let layer_claims = model_claims.get(layer)?;

        if layer_claims.is_empty() {
            return None;
        }

        // Get quarantined events for this model
        let quarantined_events: std::collections::HashSet<EventId> = quarantined
            .get(model_id)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        // Filter out quarantined claims
        let valid_claims: Vec<_> = layer_claims
            .iter()
            .filter(|(id, _)| !quarantined_events.contains(id))
            .collect();

        if valid_claims.len() < self.min_witnesses {
            return None;
        }

        // Group by (version, hash) and count witnesses
        let mut version_counts: FxHashMap<(u64, [u8; 32]), Vec<EventId>> = FxHashMap::default();
        for (event_id, claim) in &valid_claims {
            let key = (claim.version, claim.weights_hash);
            version_counts.entry(key).or_default().push(*event_id);
        }

        // Find version with most witnesses
        let best = version_counts
            .iter()
            .max_by_key(|(_, events)| events.len())?;

        let ((agreed_version, agreed_hash), contributing_events) = best;
        let witness_count = contributing_events.len();

        if witness_count < self.min_witnesses {
            return None;
        }

        // Calculate confidence based on witness agreement
        let total_claims = valid_claims.len();
        let confidence = (witness_count as f32) / (total_claims as f32);

        // Identify quarantined claims (those that disagree with consensus)
        let quarantined_claims: Vec<EventId> = valid_claims
            .iter()
            .filter(|(_, claim)| {
                claim.version != *agreed_version || claim.weights_hash != *agreed_hash
            })
            .map(|(id, _)| *id)
            .collect();

        Some(WeightConsensus {
            model_id: model_id.to_string(),
            agreed_version: *agreed_version,
            agreed_hash: *agreed_hash,
            witness_count,
            confidence,
            consensus_time: current_timestamp_ms(),
            contributing_events: contributing_events.clone(),
            quarantined_claims,
        })
    }

    /// Validate a gradient contribution (Axiom 8, 11: Witnesses, Equivocation)
    pub fn validate_gradient(&self, event: &GradientContributionClaim, reputation_manager: Option<&ReputationManager>) -> GradientValidation {
        let mut anomalies = Vec::new();
        let mut score = 1.0f32;

        // Check 1: Gradient norm within bounds
        if event.gradient_norm > self.max_gradient_norm {
            anomalies.push(format!(
                "Gradient norm {} exceeds maximum {}",
                event.gradient_norm, self.max_gradient_norm
            ));
            score *= 0.5;
        }

        // Check 2: Signature present
        if event.signature.is_empty() {
            return GradientValidation {
                valid: false,
                score: 0.0,
                rejection_reason: Some("Missing signature".to_string()),
                anomalies: vec!["No signature provided".to_string()],
                reputation_factor: 0.0,
            };
        }

        // Check 3: Verify signature matches contributor
        let sig_valid = if event.signature.len() == 64 {
            // Compute expected message
            let mut message = Vec::with_capacity(64);
            message.extend_from_slice(&event.round.to_le_bytes());
            message.extend_from_slice(&event.gradient_hash);
            message.extend_from_slice(event.model_id.as_bytes());

            // Verify Ed25519 signature
            ScopedAuthority::verify_ed25519_signature(
                &event.contributor,
                &message,
                &event.signature,
            )
        } else {
            false
        };

        if !sig_valid {
            anomalies.push("Signature verification failed".to_string());
            score *= 0.3;
        }

        // Check 4: Reputation at time matches current (within tolerance)
        let reputation_factor = if let Some(rep_mgr) = reputation_manager {
            let current_rep = rep_mgr.get_reputation(&event.contributor);
            let rep_diff = (current_rep as f32 - event.reputation_at_time).abs();

            if rep_diff > 0.2 {
                anomalies.push(format!(
                    "Reputation mismatch: claimed {} vs current {:.2}",
                    event.reputation_at_time, current_rep
                ));
                score *= 0.8;
            }

            current_rep as f32
        } else {
            event.reputation_at_time
        };

        // Check 5: Detect equivocation (same contributor, same round, different gradients)
        let equivocation = self.detect_gradient_equivocation(event);
        if equivocation {
            return GradientValidation {
                valid: false,
                score: 0.0,
                rejection_reason: Some("Equivocation detected: multiple gradients for same round".to_string()),
                anomalies: vec!["Contributor submitted conflicting gradients".to_string()],
                reputation_factor,
            };
        }

        // Check 6: Local samples reasonable
        if event.local_samples == 0 {
            anomalies.push("Zero local samples".to_string());
            score *= 0.7;
        }

        let valid = score >= 0.5 && anomalies.len() < 3;

        GradientValidation {
            valid,
            score,
            rejection_reason: if valid { None } else { Some("Multiple validation failures".to_string()) },
            anomalies,
            reputation_factor,
        }
    }

    /// Detect gradient equivocation (Axiom 11)
    fn detect_gradient_equivocation(&self, event: &GradientContributionClaim) -> bool {
        let claims = self.gradient_claims.read().unwrap();

        if let Some(round_claims) = claims.get(&event.round) {
            if let Some(contributor_claims) = round_claims.get(&event.contributor) {
                // Check if any existing claim has a different hash
                for (_, existing) in contributor_claims {
                    if existing.gradient_hash != event.gradient_hash {
                        return true; // Equivocation detected
                    }
                }
            }
        }

        false
    }

    /// Quarantine a disputed model update (Axiom 9)
    pub fn quarantine_model_update(&self, model_id: &str, event_id: EventId, dispute: Option<&ModelDispute>) {
        let mut quarantined = self.quarantined_updates.write().unwrap();
        quarantined
            .entry(model_id.to_string())
            .or_default()
            .push(event_id);

        // If dispute provided, register it
        if let Some(d) = dispute {
            self.disputes.write().unwrap().push(d.clone());
        }
    }

    /// Check if a model update is quarantined
    pub fn is_update_quarantined(&self, model_id: &str, event_id: &EventId) -> bool {
        self.quarantined_updates
            .read()
            .unwrap()
            .get(model_id)
            .map(|v| v.contains(event_id))
            .unwrap_or(false)
    }

    /// Lift quarantine on a model update (after dispute resolution)
    pub fn lift_quarantine(&self, model_id: &str, event_id: &EventId) -> bool {
        let mut quarantined = self.quarantined_updates.write().unwrap();
        if let Some(events) = quarantined.get_mut(model_id) {
            if let Some(pos) = events.iter().position(|e| e == event_id) {
                events.remove(pos);
                return true;
            }
        }
        false
    }

    /// Detect conflicts in model weight claims (Axiom 6)
    pub fn detect_model_conflicts(&self, model_id: &str) -> Vec<ModelDispute> {
        let claims = self.model_claims.read().unwrap();
        let mut disputes = Vec::new();

        if let Some(model_claims) = claims.get(model_id) {
            for (layer, layer_claims) in model_claims {
                if layer_claims.len() < 2 {
                    continue;
                }

                // Group by version
                let mut version_groups: FxHashMap<u64, Vec<(EventId, [u8; 32])>> = FxHashMap::default();
                for (event_id, claim) in layer_claims {
                    version_groups
                        .entry(claim.version)
                        .or_default()
                        .push((*event_id, claim.weights_hash));
                }

                // Check for hash conflicts within same version
                for (version, entries) in &version_groups {
                    if entries.len() < 2 {
                        continue;
                    }

                    let first_hash = entries[0].1;
                    let has_conflict = entries.iter().any(|(_, h)| *h != first_hash);

                    if has_conflict {
                        let version_conflicts: Vec<_> = entries.iter().map(|(id, _)| (*id, *version)).collect();
                        let hash_conflicts: Vec<_> = entries.iter().map(|(id, h)| (*id, *h)).collect();

                        disputes.push(ModelDispute {
                            model_id: format!("{}:{}", model_id, layer),
                            version_conflicts,
                            hash_conflicts,
                            severity: 0.8,
                            detected_at: current_timestamp_ms(),
                            resolved: false,
                        });
                    }
                }
            }
        }

        disputes
    }

    /// Get LoRA adapter consensus for a task type
    pub fn lora_consensus(&self, adapter_id: &str) -> Option<(EventId, LoraAdapterClaim)> {
        let claims = self.lora_claims.read().unwrap();
        let adapter_claims = claims.get(adapter_id)?;

        if adapter_claims.is_empty() {
            return None;
        }

        // For LoRA, prefer latest version with best metrics
        adapter_claims
            .iter()
            .filter(|(_, claim)| claim.metrics.is_some())
            .max_by(|(_, a), (_, b)| {
                let a_score = a.metrics.as_ref().map(|m| m.val_accuracy).unwrap_or(0.0);
                let b_score = b.metrics.as_ref().map(|m| m.val_accuracy).unwrap_or(0.0);
                a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// Get learning pattern consensus
    pub fn pattern_consensus(&self, pattern_id: &str) -> Option<(EventId, LearningPatternClaim)> {
        let claims = self.pattern_claims.read().unwrap();
        let pattern_claims = claims.get(pattern_id)?;

        if pattern_claims.is_empty() {
            return None;
        }

        // Prefer pattern with highest quality score weighted by sample count
        pattern_claims
            .iter()
            .max_by(|(_, a), (_, b)| {
                let a_score = a.quality_score * (a.sample_count as f32).ln().max(1.0);
                let b_score = b.quality_score * (b.sample_count as f32).ln().max(1.0);
                a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// Aggregate gradients for a federated learning round
    pub fn aggregate_round_gradients(&self, round: u64, min_contributors: usize) -> Option<Vec<(PublicKeyBytes, f32)>> {
        let claims = self.gradient_claims.read().unwrap();
        let round_claims = claims.get(&round)?;

        if round_claims.len() < min_contributors {
            return None;
        }

        // Return contributors with their reputation weights
        let contributors: Vec<(PublicKeyBytes, f32)> = round_claims
            .iter()
            .filter_map(|(contributor, claims)| {
                // Take most recent claim per contributor
                claims.last().map(|(_, claim)| (*contributor, claim.reputation_at_time))
            })
            .collect();

        if contributors.len() >= min_contributors {
            Some(contributors)
        } else {
            None
        }
    }
}

// Extension methods for CoherenceEngine to support AI model consensus
impl CoherenceEngine {
    /// Create a model consensus manager for this engine
    pub fn create_model_consensus_manager(&self, min_witnesses: usize) -> ModelConsensusManager {
        ModelConsensusManager::new(min_witnesses)
    }

    /// Ingest a model claim event
    pub fn ingest_model_claim(&mut self, event: Event, manager: &ModelConsensusManager) -> IngestResult {
        // First ingest as normal event
        let result = self.ingest(event.clone());

        // Then register with consensus manager if it's a model claim
        if let IngestResult::Success(event_id) = &result {
            if let EventKind::ModelClaim(claim_type) = &event.kind {
                match claim_type {
                    ClaimType::ModelWeight(claim) => {
                        manager.register_model_claim(*event_id, claim.clone());

                        // Check for conflicts
                        let disputes = manager.detect_model_conflicts(&claim.model_id);
                        for dispute in disputes {
                            // Quarantine all conflicting claims
                            for (conflict_id, _) in &dispute.hash_conflicts {
                                manager.quarantine_model_update(&claim.model_id, *conflict_id, Some(&dispute));
                                self.quarantine.set_level(&hex::encode(conflict_id), 2);
                            }
                        }
                    }
                    ClaimType::GradientContribution(claim) => {
                        manager.register_gradient_claim(*event_id, claim.clone());
                    }
                    ClaimType::LoraAdapter(claim) => {
                        manager.register_lora_claim(*event_id, claim.clone());
                    }
                    ClaimType::LearningPattern(claim) => {
                        manager.register_pattern_claim(*event_id, claim.clone());
                    }
                    ClaimType::Standard(_) => {
                        // Standard claims don't need special handling
                    }
                }
            }
        }

        result
    }

    /// Get model weight consensus through the manager
    pub fn model_consensus(&self, manager: &ModelConsensusManager, model_id: &str, layer: &str) -> Option<WeightConsensus> {
        manager.model_consensus(model_id, layer)
    }

    /// Validate a gradient contribution
    pub fn validate_gradient(&self, manager: &ModelConsensusManager, event: &GradientContributionClaim) -> GradientValidation {
        manager.validate_gradient(event, None)
    }

    /// Quarantine a disputed model update
    pub fn quarantine_model_update(&mut self, manager: &ModelConsensusManager, model_id: &str, event_id: EventId) {
        let dispute = ModelDispute {
            model_id: model_id.to_string(),
            version_conflicts: vec![],
            hash_conflicts: vec![(event_id, [0u8; 32])],
            severity: 0.5,
            detected_at: current_timestamp_ms(),
            resolved: false,
        };

        manager.quarantine_model_update(model_id, event_id, Some(&dispute));
        self.quarantine.set_level(&hex::encode(&event_id), 2);

        let mut stats = self.stats.write().unwrap();
        stats.quarantined_claims += 1;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruvector_similarity() {
        let v1 = Ruvector::new(vec![1.0, 0.0, 0.0]);
        let v2 = Ruvector::new(vec![1.0, 0.0, 0.0]);
        let v3 = Ruvector::new(vec![0.0, 1.0, 0.0]);

        assert!((v1.similarity(&v2) - 1.0).abs() < 0.001);
        assert!((v1.similarity(&v3) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ruvector_drift() {
        let baseline = Ruvector::new(vec![1.0, 0.0, 0.0]);
        let drifted = Ruvector::new(vec![0.707, 0.707, 0.0]);

        let drift = drifted.drift_from(&baseline);
        assert!(drift > 0.2 && drift < 0.4);
    }

    #[test]
    fn test_event_log_append() {
        let log = EventLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);

        // Create and append events
        let event1 = Event::new(
            [1u8; 32],
            [0u8; 32],
            Ruvector::new(vec![1.0, 0.0, 0.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            None,
        );

        let id1 = log.append(event1.clone());
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());

        // Verify event can be retrieved
        let retrieved = log.get(&id1);
        assert!(retrieved.is_some());

        // Append another event
        let event2 = Event::new(
            [2u8; 32],
            [0u8; 32],
            Ruvector::new(vec![0.0, 1.0, 0.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"test2".to_vec(),
                evidence: vec![],
                confidence: 0.8,
                expires_at_unix_ms: None,
            }),
            Some(id1),
        );

        let id2 = log.append(event2);
        assert_eq!(log.len(), 2);

        // Root should have changed
        let root = log.get_root();
        assert!(!root.is_empty());
        assert_ne!(root, hex::encode([0u8; 32]));
    }

    #[test]
    fn test_quarantine_manager() {
        let manager = QuarantineManager::new();

        assert!(manager.can_use("claim-1"));
        assert_eq!(manager.get_level("claim-1"), 0);

        manager.set_level("claim-1", 3);
        assert!(!manager.can_use("claim-1"));
        assert_eq!(manager.get_level("claim-1"), 3);

        assert_eq!(manager.quarantined_count(), 1);
    }

    #[test]
    fn test_coherence_engine_basic() {
        let engine = CoherenceEngine::new();

        assert_eq!(engine.event_count(), 0);
        assert_eq!(engine.conflict_count(), 0);
        assert_eq!(engine.quarantined_count(), 0);
    }

    #[test]
    fn test_coherence_engine_ingest() {
        let mut engine = CoherenceEngine::new();

        let event = Event::new(
            [1u8; 32],
            [0u8; 32],
            Ruvector::new(vec![1.0, 0.0, 0.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            None,
        );

        let result = engine.ingest(event);
        assert!(matches!(result, IngestResult::Success(_)));
        assert_eq!(engine.event_count(), 1);
    }

    #[test]
    fn test_authority_verification() {
        use ed25519_dalek::SigningKey;

        let mut engine = CoherenceEngine::new();
        let context = [42u8; 32];

        // Generate a real Ed25519 keypair for signing
        let signing_key_bytes: [u8; 32] = [
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60,
            0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
            0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19,
            0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
        ];
        let signing_key = SigningKey::from_bytes(&signing_key_bytes);
        let public_key_bytes: [u8; 32] = signing_key.verifying_key().to_bytes();

        // Use the real public key as author/authorized key
        let author = public_key_bytes;

        // Register authority requiring signatures from this public key
        let authority = ScopedAuthority::new(context, vec![author], 1);
        engine.register_authority(authority);

        // Create a resolution without signature - should fail
        let resolution_no_sig = Event::new(
            author,
            context,
            Ruvector::new(vec![1.0, 0.0, 0.0]),
            EventKind::Resolution(ResolutionEvent {
                conflict_id: [0u8; 32],
                accepted: vec![],
                deprecated: vec![[99u8; 32]],
                rationale: vec![],
                authority_sigs: vec![], // No signatures!
            }),
            None,
        );

        let result = engine.ingest(resolution_no_sig);
        assert!(matches!(result, IngestResult::UnauthorizedResolution));

        // Create resolution with REAL Ed25519 signature
        let resolution_event = ResolutionEvent {
            conflict_id: [0u8; 32],
            accepted: vec![],
            deprecated: vec![[99u8; 32]],
            rationale: vec![],
            authority_sigs: vec![], // Will be replaced with real signature
        };

        // Sign the resolution with the real private key
        let signature = ScopedAuthority::sign_resolution(&resolution_event, &context, &signing_key_bytes);

        // Create the resolution with the real signature
        let resolution_with_sig = Event::new(
            author,
            context,
            Ruvector::new(vec![1.0, 0.0, 0.0]),
            EventKind::Resolution(ResolutionEvent {
                conflict_id: [0u8; 32],
                accepted: vec![],
                deprecated: vec![[99u8; 32]],
                rationale: vec![],
                authority_sigs: vec![signature], // Real Ed25519 signature
            }),
            None,
        );

        let result = engine.ingest(resolution_with_sig);
        assert!(matches!(result, IngestResult::Success(_)));
    }

    #[test]
    fn test_witness_tracking() {
        let tracker = WitnessTracker::new(2);
        let claim_id = [1u8; 32];
        let claim_key = hex::encode(&claim_id);

        assert_eq!(tracker.witness_count(&claim_key), 0);
        assert!(!tracker.has_sufficient_witnesses(&claim_key));

        // Add first witness
        tracker.add_witness(WitnessRecord {
            claim_id,
            witness: [1u8; 32],
            path: vec![[10u8; 32]],
            witnessed_at: current_timestamp_ms(),
            signature: vec![],
        });

        assert_eq!(tracker.witness_count(&claim_key), 1);
        assert!(!tracker.has_sufficient_witnesses(&claim_key));

        // Add second independent witness
        tracker.add_witness(WitnessRecord {
            claim_id,
            witness: [2u8; 32],
            path: vec![[20u8; 32]], // Different path
            witnessed_at: current_timestamp_ms(),
            signature: vec![],
        });

        assert_eq!(tracker.witness_count(&claim_key), 2);
        assert!(tracker.has_sufficient_witnesses(&claim_key));
    }

    #[test]
    fn test_drift_tracking() {
        let tracker = DriftTracker::new(0.3);
        let context = [1u8; 32];
        let context_key = hex::encode(&context);

        // Initial embedding
        tracker.update(&context, &Ruvector::new(vec![1.0, 0.0, 0.0]));
        assert!((tracker.get_drift(&context_key) - 0.0).abs() < 0.001);

        // Update with same embedding - no drift
        tracker.update(&context, &Ruvector::new(vec![1.0, 0.0, 0.0]));
        assert!(!tracker.has_drifted(&context_key));

        // Update with very different embedding
        for _ in 0..20 {
            tracker.update(&context, &Ruvector::new(vec![0.0, 1.0, 0.0]));
        }

        // After many updates, drift should be significant
        assert!(tracker.get_drift(&context_key) > 0.1);
    }

    #[test]
    fn test_decision_trace() {
        let deps = vec![[1u8; 32], [2u8; 32]];
        let outcome = b"accepted".to_vec();

        let trace = DecisionTrace::with_timestamp(deps.clone(), outcome.clone(), 1000);

        assert_eq!(trace.dependencies.len(), 2);
        assert_eq!(trace.timestamp, 1000);
        assert!(!trace.has_disputed);
    }

    #[test]
    fn test_semantic_router() {
        let router = RacSemanticRouter::new();

        router.register_peer([1u8; 32], Ruvector::new(vec![1.0, 0.0, 0.0]), 50);
        router.register_peer([2u8; 32], Ruvector::new(vec![0.0, 1.0, 0.0]), 100);
        router.register_peer([3u8; 32], Ruvector::new(vec![0.5, 0.5, 0.0]), 75);

        assert_eq!(router.peer_count(), 3);

        let event = Event::new(
            [0u8; 32],
            [0u8; 32],
            Ruvector::new(vec![1.0, 0.0, 0.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            None,
        );

        let routes = router.get_routes(&event);
        assert!(!routes.is_empty());
        // First route should be most similar peer (peer 1)
        assert_eq!(routes[0], [1u8; 32]);
    }

    #[test]
    fn test_evidence_ref() {
        let hash_evidence = EvidenceRef::hash(&[1, 2, 3]);
        assert_eq!(hash_evidence.kind, "hash");

        let url_evidence = EvidenceRef::url("https://example.com");
        assert_eq!(url_evidence.kind, "url");

        let log_evidence = EvidenceRef::log(&[4, 5, 6]);
        assert_eq!(log_evidence.kind, "log");
    }

    #[test]
    fn test_conflict_status() {
        let status = ConflictStatus::Detected;
        assert_eq!(status, ConflictStatus::Detected);
        assert_ne!(status, ConflictStatus::Resolved);
    }

    #[test]
    fn test_inclusion_proof() {
        let log = EventLog::new();

        let event = Event::new(
            [1u8; 32],
            [0u8; 32],
            Ruvector::new(vec![1.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            None,
        );

        let id = log.append(event);
        let proof = log.prove_inclusion(&id);

        assert!(proof.is_some());
        let proof = proof.unwrap();
        assert_eq!(proof.event_id, id);
        assert_eq!(proof.index, 0);
    }

    #[test]
    fn test_escalation() {
        let mut engine = CoherenceEngine::new();
        let context = [0u8; 32];
        let author = [1u8; 32];

        // Create two conflicting assertions
        let assert1 = Event::new(
            author,
            context,
            Ruvector::new(vec![1.0, 0.0]),
            EventKind::Assert(AssertEvent {
                proposition: b"claim A".to_vec(),
                evidence: vec![],
                confidence: 0.95,
                expires_at_unix_ms: None,
            }),
            None,
        );
        engine.ingest(assert1);

        // Create challenge
        let challenge = Event::new(
            author,
            context,
            Ruvector::new(vec![1.0, 0.0]),
            EventKind::Challenge(ChallengeEvent {
                conflict_id: [99u8; 32],
                claim_ids: vec![[1u8; 32]],
                reason: "Disputed".to_string(),
                requested_proofs: vec![],
            }),
            None,
        );
        engine.ingest(challenge);

        // Add many support events to increase temperature
        for i in 0..10 {
            let support = Event::new(
                [i + 10; 32],
                context,
                Ruvector::new(vec![1.0, 0.0]),
                EventKind::Support(SupportEvent {
                    conflict_id: [99u8; 32],
                    claim_id: [1u8; 32],
                    evidence: vec![],
                    cost: 100,
                }),
                None,
            );
            engine.ingest(support);
        }

        // Check that escalation occurred
        let stats: CoherenceStats = serde_json::from_str(&engine.get_stats()).unwrap();
        assert!(stats.escalations > 0);
    }

    // ========================================================================
    // AI Model Consensus Tests
    // ========================================================================

    #[test]
    fn test_task_type_enum() {
        let text_gen = TaskType::TextGeneration;
        let code_gen = TaskType::CodeGeneration;
        let custom = TaskType::Custom("my-task".to_string());

        assert_eq!(text_gen, TaskType::TextGeneration);
        assert_ne!(text_gen, code_gen);
        assert_eq!(TaskType::default(), TaskType::TextGeneration);

        if let TaskType::Custom(name) = custom {
            assert_eq!(name, "my-task");
        } else {
            panic!("Expected Custom variant");
        }
    }

    #[test]
    fn test_model_weight_claim() {
        let claim = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "transformer.h.0.attn".to_string(),
            weights_hash: [1u8; 32],
            version: 1,
            quantization: Some("int8".to_string()),
            param_count: 1_000_000,
        };

        assert_eq!(claim.model_id, "llama-7b");
        assert_eq!(claim.version, 1);
        assert_eq!(claim.param_count, 1_000_000);
    }

    #[test]
    fn test_lora_adapter_claim() {
        let claim = LoraAdapterClaim {
            adapter_id: "code-adapter-v1".to_string(),
            task_type: TaskType::CodeGeneration,
            rank: 4,
            weights_hash: [2u8; 32],
            base_model_id: "llama-7b".to_string(),
            metrics: Some(AdapterMetrics {
                final_loss: 0.15,
                val_accuracy: 0.92,
                train_samples: 10_000,
                epochs: 3,
            }),
        };

        assert_eq!(claim.rank, 4);
        assert_eq!(claim.task_type, TaskType::CodeGeneration);
        assert!(claim.metrics.is_some());
        assert!((claim.metrics.as_ref().unwrap().val_accuracy - 0.92).abs() < 0.001);
    }

    #[test]
    fn test_learning_pattern_claim() {
        let claim = LearningPatternClaim {
            pattern_id: "pattern-1".to_string(),
            embedding: vec![0.1, 0.2, 0.3, 0.4],
            quality_score: 0.85,
            sample_count: 500,
            domain: "code-completion".to_string(),
            confidence_interval: (0.80, 0.90),
        };

        assert_eq!(claim.embedding.len(), 4);
        assert_eq!(claim.sample_count, 500);
        assert_eq!(claim.confidence_interval, (0.80, 0.90));
    }

    #[test]
    fn test_gradient_contribution_claim() {
        let claim = GradientContributionClaim {
            round: 42,
            contributor: [3u8; 32],
            gradient_hash: [4u8; 32],
            reputation_at_time: 0.8,
            local_samples: 1000,
            gradient_norm: 5.5,
            model_id: "llama-7b".to_string(),
            signature: vec![0u8; 64],
        };

        assert_eq!(claim.round, 42);
        assert_eq!(claim.local_samples, 1000);
        assert!((claim.gradient_norm - 5.5).abs() < 0.001);
    }

    #[test]
    fn test_claim_type_names() {
        let standard = ClaimType::Standard(AssertEvent {
            proposition: vec![],
            evidence: vec![],
            confidence: 0.9,
            expires_at_unix_ms: None,
        });
        assert_eq!(standard.type_name(), "standard");

        let model_weight = ClaimType::ModelWeight(ModelWeightClaim {
            model_id: "test".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [0u8; 32],
            version: 1,
            quantization: None,
            param_count: 100,
        });
        assert_eq!(model_weight.type_name(), "model_weight");

        let gradient = ClaimType::GradientContribution(GradientContributionClaim {
            round: 1,
            contributor: [0u8; 32],
            gradient_hash: [0u8; 32],
            reputation_at_time: 0.5,
            local_samples: 10,
            gradient_norm: 1.0,
            model_id: "test".to_string(),
            signature: vec![],
        });
        assert_eq!(gradient.type_name(), "gradient_contribution");
    }

    #[test]
    fn test_model_consensus_manager_basic() {
        let manager = ModelConsensusManager::new(2);

        assert_eq!(manager.model_count(), 0);
        assert_eq!(manager.dispute_count(), 0);
        assert_eq!(manager.quarantined_update_count(), 0);

        let stats = manager.get_stats();
        assert!(stats.contains("\"models\":0"));
        assert!(stats.contains("\"disputes\":0"));
    }

    #[test]
    fn test_model_weight_registration() {
        let manager = ModelConsensusManager::new(2);

        let event_id_1 = [1u8; 32];
        let event_id_2 = [2u8; 32];

        let claim1 = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [10u8; 32],
            version: 1,
            quantization: None,
            param_count: 1000,
        };

        let claim2 = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [10u8; 32], // Same hash = agreement
            version: 1,
            quantization: None,
            param_count: 1000,
        };

        manager.register_model_claim(event_id_1, claim1);
        manager.register_model_claim(event_id_2, claim2);

        assert_eq!(manager.model_count(), 1);

        // Should reach consensus with 2 agreeing witnesses
        let consensus = manager.model_consensus("llama-7b", "layer0");
        assert!(consensus.is_some());

        let consensus = consensus.unwrap();
        assert_eq!(consensus.agreed_version, 1);
        assert_eq!(consensus.witness_count, 2);
        assert!((consensus.confidence - 1.0).abs() < 0.001); // 100% agreement
    }

    #[test]
    fn test_model_weight_conflict_detection() {
        let manager = ModelConsensusManager::new(1);

        let event_id_1 = [1u8; 32];
        let event_id_2 = [2u8; 32];

        // Same model, same layer, same version, DIFFERENT hash = conflict
        let claim1 = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [10u8; 32],
            version: 1,
            quantization: None,
            param_count: 1000,
        };

        let claim2 = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [20u8; 32], // Different hash!
            version: 1,
            quantization: None,
            param_count: 1000,
        };

        manager.register_model_claim(event_id_1, claim1);
        manager.register_model_claim(event_id_2, claim2);

        let disputes = manager.detect_model_conflicts("llama-7b");
        assert_eq!(disputes.len(), 1);
        assert!(!disputes[0].resolved);
        assert!((disputes[0].severity - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_gradient_validation_missing_signature() {
        let manager = ModelConsensusManager::new(2);

        let claim = GradientContributionClaim {
            round: 1,
            contributor: [1u8; 32],
            gradient_hash: [2u8; 32],
            reputation_at_time: 0.8,
            local_samples: 100,
            gradient_norm: 5.0,
            model_id: "test".to_string(),
            signature: vec![], // Empty signature
        };

        let result = manager.validate_gradient(&claim, None);

        assert!(!result.valid);
        assert_eq!(result.score, 0.0);
        assert!(result.rejection_reason.is_some());
        assert!(result.rejection_reason.unwrap().contains("Missing signature"));
    }

    #[test]
    fn test_gradient_validation_excessive_norm() {
        let manager = ModelConsensusManager::new(2);

        let claim = GradientContributionClaim {
            round: 1,
            contributor: [1u8; 32],
            gradient_hash: [2u8; 32],
            reputation_at_time: 0.8,
            local_samples: 100,
            gradient_norm: 500.0, // Exceeds max of 100.0
            model_id: "test".to_string(),
            signature: vec![0u8; 64],
        };

        let result = manager.validate_gradient(&claim, None);

        // Should have anomaly but might still be valid with reduced score
        assert!(result.anomalies.iter().any(|a| a.contains("Gradient norm")));
        assert!(result.score < 1.0);
    }

    #[test]
    fn test_gradient_equivocation_detection() {
        let manager = ModelConsensusManager::new(2);

        let contributor = [1u8; 32];
        let event_id_1 = [10u8; 32];

        // First gradient for round 1
        let claim1 = GradientContributionClaim {
            round: 1,
            contributor,
            gradient_hash: [2u8; 32],
            reputation_at_time: 0.8,
            local_samples: 100,
            gradient_norm: 5.0,
            model_id: "test".to_string(),
            signature: vec![0u8; 64],
        };

        manager.register_gradient_claim(event_id_1, claim1);

        // Second gradient for same round with DIFFERENT hash = equivocation
        let claim2 = GradientContributionClaim {
            round: 1,
            contributor,
            gradient_hash: [3u8; 32], // Different!
            reputation_at_time: 0.8,
            local_samples: 100,
            gradient_norm: 5.0,
            model_id: "test".to_string(),
            signature: vec![0u8; 64],
        };

        let result = manager.validate_gradient(&claim2, None);

        assert!(!result.valid);
        assert!(result.rejection_reason.is_some());
        assert!(result.rejection_reason.unwrap().contains("Equivocation"));
    }

    #[test]
    fn test_quarantine_model_update() {
        let manager = ModelConsensusManager::new(2);

        let model_id = "llama-7b";
        let event_id = [5u8; 32];

        assert!(!manager.is_update_quarantined(model_id, &event_id));

        manager.quarantine_model_update(model_id, event_id, None);

        assert!(manager.is_update_quarantined(model_id, &event_id));
        assert_eq!(manager.quarantined_update_count(), 1);

        // Lift quarantine
        assert!(manager.lift_quarantine(model_id, &event_id));
        assert!(!manager.is_update_quarantined(model_id, &event_id));
    }

    #[test]
    fn test_lora_consensus() {
        let manager = ModelConsensusManager::new(1);

        let event_id_1 = [1u8; 32];
        let event_id_2 = [2u8; 32];

        // LoRA adapter with lower accuracy
        let claim1 = LoraAdapterClaim {
            adapter_id: "code-adapter".to_string(),
            task_type: TaskType::CodeGeneration,
            rank: 4,
            weights_hash: [10u8; 32],
            base_model_id: "llama-7b".to_string(),
            metrics: Some(AdapterMetrics {
                final_loss: 0.2,
                val_accuracy: 0.85,
                train_samples: 5000,
                epochs: 2,
            }),
        };

        // LoRA adapter with higher accuracy (should win)
        let claim2 = LoraAdapterClaim {
            adapter_id: "code-adapter".to_string(),
            task_type: TaskType::CodeGeneration,
            rank: 4,
            weights_hash: [20u8; 32],
            base_model_id: "llama-7b".to_string(),
            metrics: Some(AdapterMetrics {
                final_loss: 0.1,
                val_accuracy: 0.92,
                train_samples: 10000,
                epochs: 3,
            }),
        };

        manager.register_lora_claim(event_id_1, claim1);
        manager.register_lora_claim(event_id_2, claim2);

        let consensus = manager.lora_consensus("code-adapter");
        assert!(consensus.is_some());

        let (_, best_claim) = consensus.unwrap();
        assert!((best_claim.metrics.unwrap().val_accuracy - 0.92).abs() < 0.001);
    }

    #[test]
    fn test_pattern_consensus() {
        let manager = ModelConsensusManager::new(1);

        let event_id_1 = [1u8; 32];
        let event_id_2 = [2u8; 32];

        // Pattern with lower quality
        let claim1 = LearningPatternClaim {
            pattern_id: "pattern-1".to_string(),
            embedding: vec![0.1, 0.2],
            quality_score: 0.7,
            sample_count: 100,
            domain: "test".to_string(),
            confidence_interval: (0.65, 0.75),
        };

        // Pattern with higher quality and more samples
        let claim2 = LearningPatternClaim {
            pattern_id: "pattern-1".to_string(),
            embedding: vec![0.3, 0.4],
            quality_score: 0.9,
            sample_count: 1000,
            domain: "test".to_string(),
            confidence_interval: (0.85, 0.95),
        };

        manager.register_pattern_claim(event_id_1, claim1);
        manager.register_pattern_claim(event_id_2, claim2);

        let consensus = manager.pattern_consensus("pattern-1");
        assert!(consensus.is_some());

        let (_, best_claim) = consensus.unwrap();
        assert!((best_claim.quality_score - 0.9).abs() < 0.001);
        assert_eq!(best_claim.sample_count, 1000);
    }

    #[test]
    fn test_federated_learning_round_aggregation() {
        let manager = ModelConsensusManager::new(1);

        let round = 42u64;

        // Three different contributors for the same round
        for i in 0..3 {
            let mut contributor = [0u8; 32];
            contributor[0] = i as u8;

            let claim = GradientContributionClaim {
                round,
                contributor,
                gradient_hash: [(i + 10) as u8; 32],
                reputation_at_time: 0.5 + (i as f32 * 0.1),
                local_samples: 100 + i * 50,
                gradient_norm: 5.0,
                model_id: "test".to_string(),
                signature: vec![0u8; 64],
            };

            manager.register_gradient_claim([(i + 100) as u8; 32], claim);
        }

        let result = manager.aggregate_round_gradients(round, 2);
        assert!(result.is_some());

        let contributors = result.unwrap();
        assert_eq!(contributors.len(), 3);
    }

    #[test]
    fn test_coherence_engine_model_consensus_integration() {
        let mut engine = CoherenceEngine::new();
        let manager = engine.create_model_consensus_manager(2);
        let context = [0u8; 32];
        let author = [1u8; 32];

        // Create model weight claim event
        let claim = ModelWeightClaim {
            model_id: "llama-7b".to_string(),
            layer: "layer0".to_string(),
            weights_hash: [10u8; 32],
            version: 1,
            quantization: None,
            param_count: 1000,
        };

        let event = Event::new(
            author,
            context,
            Ruvector::new(vec![1.0, 0.0]),
            EventKind::ModelClaim(ClaimType::ModelWeight(claim)),
            None,
        );

        let result = engine.ingest_model_claim(event, &manager);
        assert!(matches!(result, IngestResult::Success(_)));
        assert_eq!(manager.model_count(), 1);
    }

    #[test]
    fn test_weight_consensus_struct() {
        let consensus = WeightConsensus {
            model_id: "test-model".to_string(),
            agreed_version: 5,
            agreed_hash: [42u8; 32],
            witness_count: 3,
            confidence: 0.95,
            consensus_time: 1234567890,
            contributing_events: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
            quarantined_claims: vec![[4u8; 32]],
        };

        assert_eq!(consensus.agreed_version, 5);
        assert_eq!(consensus.witness_count, 3);
        assert_eq!(consensus.contributing_events.len(), 3);
        assert_eq!(consensus.quarantined_claims.len(), 1);
    }

    #[test]
    fn test_model_dispute_struct() {
        let dispute = ModelDispute {
            model_id: "llama-7b:layer0".to_string(),
            version_conflicts: vec![([1u8; 32], 1), ([2u8; 32], 1)],
            hash_conflicts: vec![([1u8; 32], [10u8; 32]), ([2u8; 32], [20u8; 32])],
            severity: 0.8,
            detected_at: 1234567890,
            resolved: false,
        };

        assert_eq!(dispute.version_conflicts.len(), 2);
        assert_eq!(dispute.hash_conflicts.len(), 2);
        assert!(!dispute.resolved);
    }

    #[test]
    fn test_gradient_validation_struct() {
        let validation = GradientValidation {
            valid: true,
            score: 0.95,
            rejection_reason: None,
            anomalies: vec![],
            reputation_factor: 0.8,
        };

        assert!(validation.valid);
        assert!((validation.score - 0.95).abs() < 0.001);
        assert!(validation.rejection_reason.is_none());
        assert!(validation.anomalies.is_empty());
    }
}
