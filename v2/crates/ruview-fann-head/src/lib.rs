//! Default-off ruv-FANN presence head for RuView (ADR-425).
//!
//! The room learner's presence decision is a small head over frozen features:
//! `sigmoid(sum(x_i * w_i) + bias)`. This crate runs that head, and a slightly
//! larger candidate, on ruv-FANN's prepared inference engine:
//!
//! - [`LinearHead`] is the incumbent contract (weights, bias last);
//! - [`FannHead`] is an immutable prepared model. [`FannHead::from_linear`]
//!   reproduces the incumbent exactly; [`train`] fits a bounded candidate;
//! - [`shadow`] scores a candidate next to the incumbent without authority;
//! - [`replay`] compares models on held-out sessions with fixed gates.
//!
//! Nothing here publishes a result. A candidate's score is a diagnostic until
//! the existing owner qualification and selection flow selects it.

pub mod replay;
pub mod shadow;
pub mod train;

use ruv_fann::{
    ActivationFunction, DenseLayer, InferenceWorkspace, PreparationLimits, PreparedNetwork,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Exact ruv-FANN source revision this crate is built and tested against.
pub const RUV_FANN_REV: &str = "d94b47d8e3a5dcb92d6a1f0b966593233b40458c";
/// Envelope schema for an exported head.
pub const HEAD_SCHEMA: &str = "ruview.fann-head/1";
/// Largest feature vector accepted (the room learner uses 5 per source, 2-8 sources).
pub const MAX_FEATURES: usize = 64;
/// Largest hidden width accepted.
pub const MAX_HIDDEN: usize = 16;
/// Probabilities inside this closed band abstain, as in the incumbent.
pub const ABSTAIN_LOW: f32 = 0.45;
pub const ABSTAIN_HIGH: f32 = 0.55;

const LIMITS: PreparationLimits = PreparationLimits {
    max_bytes: 1 << 20,
    max_neurons: 4096,
    max_connections: 65_536,
};

#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum HeadError {
    #[error("invalid head: {0}")]
    Invalid(&'static str),
    #[error("feature count {actual} does not match the model's {expected}")]
    FeatureCount { expected: usize, actual: usize },
    #[error("non-finite feature or score")]
    NonFinite,
    #[error("model context does not match: {0}")]
    Context(&'static str),
    #[error("engine: {0}")]
    Engine(String),
    #[error("malformed head document")]
    Json,
}

/// The incumbent's three-way decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceDecision {
    Occupied,
    Empty,
    Abstain,
}

/// Decision rule of the incumbent head: abstain on `[0.45, 0.55]`, else `> 0.5`.
pub fn decide(probability: f32) -> PresenceDecision {
    if (ABSTAIN_LOW..=ABSTAIN_HIGH).contains(&probability) {
        PresenceDecision::Abstain
    } else if probability > 0.5 {
        PresenceDecision::Occupied
    } else {
        PresenceDecision::Empty
    }
}

/// Overflow-safe logistic, identical to the incumbent's and to ruv-FANN's.
pub fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// The incumbent head: one weight per feature, then the bias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearHead {
    weights: Vec<f32>,
}

impl LinearHead {
    /// `weights` is `[w_0, .., w_{n-1}, bias]`, the room learner's `head_weights`.
    pub fn new(weights: Vec<f32>) -> Result<Self, HeadError> {
        if weights.len() < 2 || weights.len() - 1 > MAX_FEATURES {
            return Err(HeadError::Invalid("feature count out of range"));
        }
        if !weights.iter().all(|w| w.is_finite() && w.abs() <= 1.0e4) {
            return Err(HeadError::Invalid("weights must be finite and bounded"));
        }
        Ok(Self { weights })
    }

    pub fn features(&self) -> usize {
        self.weights.len() - 1
    }

    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Same expression and summation order as the incumbent's `logit`.
    pub fn logit(&self, features: &[f32]) -> Result<f32, HeadError> {
        if features.len() != self.features() {
            return Err(HeadError::FeatureCount {
                expected: self.features(),
                actual: features.len(),
            });
        }
        let logit = features
            .iter()
            .zip(&self.weights)
            .map(|(x, w)| x * w)
            .sum::<f32>()
            + self.weights[self.weights.len() - 1];
        if logit.is_finite() {
            Ok(logit)
        } else {
            Err(HeadError::NonFinite)
        }
    }

    pub fn probability(&self, features: &[f32]) -> Result<f32, HeadError> {
        self.logit(features).map(sigmoid)
    }
}

/// What a head was fitted for. A prepared model and any cached score are only
/// valid for the exact context they were bound to (ADR-425 D4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadContext {
    /// Feature schema id, e.g. `aether-skip/1` (embedding + DC + temporal energy).
    pub feature_schema: String,
    /// Ordered source ids; order is part of the contract.
    pub sources: Vec<String>,
    /// Room revision the head was fitted for.
    pub room_revision: String,
}

impl HeadContext {
    pub fn validate(&self) -> Result<(), HeadError> {
        let ok = |s: &str| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control);
        if !ok(&self.feature_schema) || !ok(&self.room_revision) {
            return Err(HeadError::Context("empty or oversized identifier"));
        }
        if self.sources.is_empty() || self.sources.len() > 8 || !self.sources.iter().all(|s| ok(s))
        {
            return Err(HeadError::Context("1 to 8 named sources required"));
        }
        Ok(())
    }
}

/// One dense layer of a head: `outputs x inputs` row-major weights plus bias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    pub inputs: usize,
    pub outputs: usize,
    pub weights: Vec<f32>,
    pub bias: Vec<f32>,
    pub activation: HeadActivation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadActivation {
    Relu,
    Sigmoid,
}

/// Exported head: schema, engine pin, context, layers and a digest over them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadDocument {
    pub schema: String,
    pub engine_rev: String,
    pub context: HeadContext,
    pub layers: Vec<LayerSpec>,
    /// SHA-256 over engine, context and every parameter's little-endian bytes.
    pub digest: String,
}

/// An immutable prepared head. Cheap to share; each scorer owns a workspace.
#[derive(Debug)]
pub struct FannHead {
    network: PreparedNetwork<f32>,
    layers: Vec<LayerSpec>,
    context: HeadContext,
    digest: String,
}

/// Per-thread scratch for [`FannHead::score`]. Scoring does not allocate.
#[derive(Debug)]
pub struct HeadScorer {
    workspace: InferenceWorkspace<f32>,
    out: [f32; 1],
}

fn digest(context: &HeadContext, layers: &[LayerSpec]) -> String {
    let mut h = Sha256::new();
    h.update(HEAD_SCHEMA.as_bytes());
    h.update(RUV_FANN_REV.as_bytes());
    for part in [&context.feature_schema, &context.room_revision] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    for source in &context.sources {
        h.update((source.len() as u64).to_le_bytes());
        h.update(source.as_bytes());
    }
    for layer in layers {
        h.update([layer.activation as u8]);
        h.update((layer.inputs as u64).to_le_bytes());
        h.update((layer.outputs as u64).to_le_bytes());
        for v in layer.weights.iter().chain(&layer.bias) {
            h.update(v.to_le_bytes());
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl FannHead {
    /// Validate and prepare a head. All setup cost is paid here, once.
    pub fn new(layers: Vec<LayerSpec>, context: HeadContext) -> Result<Self, HeadError> {
        context.validate()?;
        let (first, last) = match (layers.first(), layers.last()) {
            (Some(f), Some(l)) if layers.len() <= 2 => (f, l),
            _ => return Err(HeadError::Invalid("one or two layers required")),
        };
        if first.inputs == 0 || first.inputs > MAX_FEATURES {
            return Err(HeadError::Invalid("feature count out of range"));
        }
        if last.outputs != 1 || last.activation != HeadActivation::Sigmoid {
            return Err(HeadError::Invalid(
                "the head must end in one sigmoid output",
            ));
        }
        if layers.len() == 2
            && (first.outputs > MAX_HIDDEN || first.activation != HeadActivation::Relu)
        {
            return Err(HeadError::Invalid(
                "the hidden layer must be ReLU and at most 16 wide",
            ));
        }
        if layers.iter().any(|l| l.bias.len() != l.outputs) {
            return Err(HeadError::Invalid("every layer needs one bias per output"));
        }
        if layers
            .iter()
            .flat_map(|l| l.weights.iter().chain(&l.bias))
            .any(|v| v.abs() > 1.0e4)
        {
            return Err(HeadError::Invalid("parameters must be bounded"));
        }
        let dense: Vec<DenseLayer<'_, f32>> = layers
            .iter()
            .map(|l| DenseLayer {
                inputs: l.inputs,
                outputs: l.outputs,
                weights: &l.weights,
                bias: Some(&l.bias),
                activation: match l.activation {
                    HeadActivation::Relu => ActivationFunction::ReLU,
                    HeadActivation::Sigmoid => ActivationFunction::Sigmoid,
                },
                steepness: 1.0,
            })
            .collect();
        let network = PreparedNetwork::from_dense_layers_with_limits(&dense, LIMITS)
            .map_err(|e| HeadError::Engine(e.to_string()))?;
        let digest = digest(&context, &layers);
        Ok(Self {
            network,
            layers,
            context,
            digest,
        })
    }

    /// The incumbent as a single dense sigmoid layer, same weight order and bias.
    pub fn from_linear(head: &LinearHead, context: HeadContext) -> Result<Self, HeadError> {
        let n = head.features();
        Self::new(
            vec![LayerSpec {
                inputs: n,
                outputs: 1,
                weights: head.weights()[..n].to_vec(),
                bias: vec![head.weights()[n]],
                activation: HeadActivation::Sigmoid,
            }],
            context,
        )
    }

    pub fn features(&self) -> usize {
        self.layers[0].inputs
    }

    pub fn context(&self) -> &HeadContext {
        &self.context
    }

    /// Identity of the exact parameters, engine and context. Use it as the key
    /// of any prepared-model or score cache.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn parameter_count(&self) -> usize {
        self.layers
            .iter()
            .map(|l| l.weights.len() + l.bias.len())
            .sum()
    }

    /// Bytes held by the prepared model and one scorer's workspace.
    pub fn estimated_bytes(&self) -> usize {
        self.network.estimated_bytes()
    }

    pub fn scorer(&self) -> Result<HeadScorer, HeadError> {
        let workspace = self
            .network
            .workspace()
            .map_err(|e| HeadError::Engine(e.to_string()))?;
        Ok(HeadScorer {
            workspace,
            out: [0.0],
        })
    }

    /// Occupied probability for one feature vector. No allocation.
    pub fn score(&self, scorer: &mut HeadScorer, features: &[f32]) -> Result<f32, HeadError> {
        if features.len() != self.features() {
            return Err(HeadError::FeatureCount {
                expected: self.features(),
                actual: features.len(),
            });
        }
        self.network
            .run_into(features, &mut scorer.out, &mut scorer.workspace)
            .map_err(|_| HeadError::NonFinite)?;
        let p = scorer.out[0];
        if p.is_finite() {
            Ok(p)
        } else {
            Err(HeadError::NonFinite)
        }
    }

    /// Refuse to use a head for anything but the context it was fitted for.
    pub fn require_context(&self, context: &HeadContext) -> Result<(), HeadError> {
        if &self.context == context {
            Ok(())
        } else {
            Err(HeadError::Context("context mismatch"))
        }
    }

    pub fn to_document(&self) -> HeadDocument {
        HeadDocument {
            schema: HEAD_SCHEMA.to_string(),
            engine_rev: RUV_FANN_REV.to_string(),
            context: self.context.clone(),
            layers: self.layers.clone(),
            digest: self.digest.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.to_document()).expect("head document serialises")
    }

    /// Load an exported head. The schema, engine pin and digest must all match;
    /// a head exported by another engine revision is refused, not reinterpreted.
    pub fn from_json(json: &str) -> Result<Self, HeadError> {
        if json.len() > 256 * 1024 {
            return Err(HeadError::Json);
        }
        let doc: HeadDocument = serde_json::from_str(json).map_err(|_| HeadError::Json)?;
        if doc.schema != HEAD_SCHEMA {
            return Err(HeadError::Invalid("unsupported head schema"));
        }
        if doc.engine_rev != RUV_FANN_REV {
            return Err(HeadError::Invalid(
                "exported by a different engine revision",
            ));
        }
        let head = Self::new(doc.layers, doc.context)?;
        if head.digest != doc.digest {
            return Err(HeadError::Invalid("digest mismatch"));
        }
        Ok(head)
    }
}
