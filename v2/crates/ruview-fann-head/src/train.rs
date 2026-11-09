//! Bounded fitting of a head on frozen features (ADR-425 D3).
//!
//! The encoder is frozen; only the head is fitted, with ruv-FANN's reverse-mode
//! dense training: full-batch AdamW on **MSE of the sigmoid probability**
//! against 0/1 targets. That objective differs from the incumbent's binary
//! cross-entropy, so every comparison must include [`FitConfig::linear_control`]:
//! a linear head refitted the same way on the same rows. A gain over that
//! control is attributable to capacity; a gain only over the incumbent may just
//! be the change of loss or fit.
//!
//! Fitting is deterministic for a given config and row order.

use ruv_fann::{ActivationFunction, AdamW, AdamWConfig, AutogradNetwork, DenseLayer};

use crate::{
    FannHead, HeadActivation, HeadContext, HeadError, LayerSpec, MAX_FEATURES, MAX_HIDDEN,
};

/// Most rows accepted by one fit. The helper's fit contract is 4 to 64 Train
/// cohorts per condition; this bound covers offline replay as well.
pub const MAX_ROWS: usize = 4096;
const TRAIN_BYTES: usize = 8 << 20;

/// Fixed fitting recipe. Tune on Validation only, then freeze before HeldOut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitConfig {
    /// Hidden ReLU units; `0` fits the linear control.
    pub hidden: usize,
    pub epochs: usize,
    pub learning_rate: f32,
    /// Decoupled weight decay (applies to weights and biases).
    pub weight_decay: f32,
    pub seed: u64,
}

impl FitConfig {
    /// The first candidate: `n -> 4 -> 1`, ReLU then sigmoid.
    pub fn candidate() -> Self {
        Self {
            hidden: 4,
            epochs: 400,
            learning_rate: 0.02,
            weight_decay: 0.01,
            seed: 0x5EED,
        }
    }

    /// Feature-matched linear control, fitted with the same loss and optimiser.
    pub fn linear_control() -> Self {
        Self {
            hidden: 0,
            ..Self::candidate()
        }
    }
}

/// Outcome of a fit.
#[derive(Debug)]
pub struct FitReport {
    pub head: FannHead,
    /// MSE of the last epoch, on the fitting rows (not a generalisation figure).
    pub final_train_mse: f32,
    pub epochs: usize,
    pub rows: usize,
}

struct SplitMix(u64);

impl SplitMix {
    fn next_unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // 24 random bits -> [-1, 1)
        ((z >> 40) as f32 / (1u64 << 23) as f32) - 1.0
    }
}

fn engine(e: impl std::fmt::Display) -> HeadError {
    HeadError::Engine(e.to_string())
}

fn init_layer(
    inputs: usize,
    outputs: usize,
    activation: HeadActivation,
    rng: &mut SplitMix,
) -> LayerSpec {
    // Glorot-uniform weights, zero biases.
    let bound = (6.0 / (inputs + outputs) as f32).sqrt();
    LayerSpec {
        inputs,
        outputs,
        weights: (0..inputs * outputs)
            .map(|_| rng.next_unit() * bound)
            .collect(),
        bias: vec![0.0; outputs],
        activation,
    }
}

/// Fit a head on `rows` of frozen features. `features` is row-major,
/// `rows * n`; `labels[i]` is true when row `i` is occupied.
pub fn fit(
    features: &[f32],
    labels: &[bool],
    context: HeadContext,
    config: FitConfig,
) -> Result<FitReport, HeadError> {
    let rows = labels.len();
    if rows == 0 || rows > MAX_ROWS || features.len() % rows != 0 {
        return Err(HeadError::Invalid(
            "row count out of range or ragged features",
        ));
    }
    let n = features.len() / rows;
    if n == 0 || n > MAX_FEATURES || config.hidden > MAX_HIDDEN {
        return Err(HeadError::Invalid("feature or hidden width out of range"));
    }
    if config.epochs == 0 || config.epochs > 10_000 {
        return Err(HeadError::Invalid("epochs out of range"));
    }
    if !features.iter().all(|x| x.is_finite()) {
        return Err(HeadError::NonFinite);
    }
    if labels.iter().all(|&l| l) || labels.iter().all(|&l| !l) {
        return Err(HeadError::Invalid(
            "fitting needs both occupied and empty rows",
        ));
    }

    let mut rng = SplitMix(config.seed);
    let mut layers = Vec::with_capacity(2);
    if config.hidden > 0 {
        layers.push(init_layer(n, config.hidden, HeadActivation::Relu, &mut rng));
        layers.push(init_layer(
            config.hidden,
            1,
            HeadActivation::Sigmoid,
            &mut rng,
        ));
    } else {
        layers.push(init_layer(n, 1, HeadActivation::Sigmoid, &mut rng));
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
    let mut network = AutogradNetwork::from_dense_layers(&dense).map_err(engine)?;
    let mut workspace = network.workspace(rows, TRAIN_BYTES).map_err(engine)?;
    let mut optimizer = AdamW::new(
        &network,
        AdamWConfig {
            learning_rate: config.learning_rate,
            weight_decay: config.weight_decay,
            ..AdamWConfig::default()
        },
        TRAIN_BYTES,
    )
    .map_err(engine)?;

    let targets: Vec<f32> = labels.iter().map(|&l| if l { 1.0 } else { 0.0 }).collect();
    let mut loss = f32::NAN;
    for _ in 0..config.epochs {
        loss = network
            .backward_mse(features, &targets, rows, &mut workspace)
            .map_err(engine)?;
        network
            .adamw_step(&mut workspace, &mut optimizer)
            .map_err(engine)?;
    }

    // Read the fitted parameters back through the engine's own interchange
    // document, so this crate never assumes its internal parameter layout.
    let exported: serde_json::Value =
        serde_json::from_str(&network.to_dense_layers_json().map_err(engine)?)
            .map_err(|_| HeadError::Json)?;
    let exported = exported["layers"].as_array().ok_or(HeadError::Json)?;
    if exported.len() != layers.len() {
        return Err(HeadError::Json);
    }
    for (layer, doc) in layers.iter_mut().zip(exported) {
        let read = |key: &str, len: usize| -> Result<Vec<f32>, HeadError> {
            let values: Vec<f32> =
                serde_json::from_value(doc[key].clone()).map_err(|_| HeadError::Json)?;
            if values.len() == len {
                Ok(values)
            } else {
                Err(HeadError::Json)
            }
        };
        layer.weights = read("weights", layer.inputs * layer.outputs)?;
        layer.bias = read("bias", layer.outputs)?;
    }
    Ok(FitReport {
        head: FannHead::new(layers, context)?,
        final_train_mse: loss,
        epochs: config.epochs,
        rows,
    })
}
