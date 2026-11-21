//! Dataset-backed offline embedding and fingerprint index workflows (#2591).

use crate::{dataset, embedding, graph_transformer, rvf_container::RvfReader};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub type CsiWindows = Vec<Vec<Vec<f32>>>;

/// Use the same windowing/normalization for pretraining, extraction and indexing.
/// Missing, short, malformed and non-finite datasets are errors, never demos.
pub fn load_windows(path: Option<&Path>, dataset_type: &str) -> Result<CsiWindows, String> {
    let path = path.ok_or("--dataset <path> is required")?;
    let source = match dataset_type {
        "mmfi" => dataset::DataSource::MmFi(path.to_owned()),
        "wipose" => dataset::DataSource::WiPose(path.to_owned()),
        other => {
            return Err(format!(
                "unknown dataset type {other:?}; use mmfi or wipose"
            ))
        }
    };
    let samples = dataset::DataPipeline::new(dataset::DataConfig {
        source,
        ..Default::default()
    })
    .load()
    .map_err(|e| format!("failed to load dataset {}: {e}", path.display()))?;
    let windows: CsiWindows = samples.into_iter().map(|s| s.csi_window).collect();
    window_width(&windows)?;
    Ok(windows)
}

pub fn window_width(windows: &[Vec<Vec<f32>>]) -> Result<usize, String> {
    let width = windows.first().and_then(|w| w.first()).map_or(0, Vec::len);
    if width == 0 {
        return Err("dataset contains no complete non-empty CSI windows".into());
    }
    if windows.iter().any(|w| {
        w.is_empty()
            || w.iter()
                .any(|f| f.len() != width || f.iter().any(|x| !x.is_finite()))
    }) {
        return Err("dataset has inconsistent shapes or non-finite CSI values".into());
    }
    Ok(width)
}

#[derive(Deserialize)]
struct ModelConfig {
    d_model: usize,
    d_proj: usize,
    temperature: f32,
    normalize: bool,
    // Legacy ADR-024 RVFs omitted the transformer shape. Validate their exact
    // weight count against the dataset width and the original architecture.
    n_subcarriers: Option<usize>,
    n_heads: Option<usize>,
    n_gnn_layers: Option<usize>,
}

/// Require both trained weight segments and validate counts before unflattening.
pub fn load_extractor(
    model: Option<&Path>,
    dataset_width: usize,
) -> Result<embedding::EmbeddingExtractor, String> {
    let path = model.ok_or("--model <path> to a pretrained embedding RVF is required")?;
    let reader = RvfReader::from_file(path).map_err(|e| format!("failed to load model: {e}"))?;
    let weights = reader
        .weights()
        .ok_or("model has no valid transformer weights")?;
    let (json, projection) = reader
        .embedding()
        .ok_or("model has no valid embedding segment")?;
    let c: ModelConfig = serde_json::from_value(json)
        .map_err(|e| format!("invalid embedding configuration: {e}"))?;
    let width = c.n_subcarriers.unwrap_or(dataset_width);
    let heads = c.n_heads.unwrap_or(4);
    let layers = c.n_gnn_layers.unwrap_or(2);
    if width != dataset_width {
        return Err(format!(
            "model expects {width} subcarriers, dataset has {dataset_width}"
        ));
    }
    if !(1..=4096).contains(&width)
        || !(1..=4096).contains(&c.d_model)
        || !(1..=4096).contains(&c.d_proj)
        || heads == 0
        || c.d_model % heads != 0
        || !(1..=16).contains(&layers)
        || !c.temperature.is_finite()
        || c.temperature <= 0.0
    {
        return Err("invalid or unsupported embedding model dimensions/temperature".into());
    }
    let d = c.d_model;
    let p = c.d_proj;
    let transformer_count = width * d + d + 17 * d + (4 + layers) * (d * d + d) + 4 * d + 4;
    let projection_count = d * p + p + p * p + p;
    if weights.len() != transformer_count || projection.len() != projection_count {
        return Err(format!(
            "model weight count mismatch: expected {transformer_count} transformer and {projection_count} projection parameters, got {} and {}",
            weights.len(), projection.len()
        ));
    }
    if weights.iter().chain(&projection).any(|x| !x.is_finite()) {
        return Err("model contains non-finite weights".into());
    }
    let mut extractor = embedding::EmbeddingExtractor::new(
        graph_transformer::TransformerConfig {
            n_subcarriers: width,
            n_keypoints: 17,
            d_model: d,
            n_heads: heads,
            n_gnn_layers: layers,
        },
        embedding::EmbeddingConfig {
            d_model: d,
            d_proj: p,
            temperature: c.temperature,
            normalize: c.normalize,
        },
    );
    let mut all_weights = weights;
    all_weights.extend(projection);
    extractor.unflatten_weights(&all_weights)?;
    Ok(extractor)
}

pub fn index_type(name: &str) -> Result<embedding::IndexType, String> {
    match name {
        "env" | "environment" => Ok(embedding::IndexType::EnvironmentFingerprint),
        "activity" => Ok(embedding::IndexType::ActivityPattern),
        "temporal" => Ok(embedding::IndexType::TemporalBaseline),
        "person" => Ok(embedding::IndexType::PersonTrack),
        _ => Err(format!(
            "unknown index type {name:?}; use env, activity, temporal, person"
        )),
    }
}

/// Portable index payload; embeddings can be reloaded into FingerprintIndex.
/// Window ordinals identify source windows, without inventing capture times.
#[derive(Debug, Serialize, Deserialize)]
pub struct FingerprintFile {
    pub version: u32,
    pub index_type: String,
    pub embeddings: Vec<Vec<f32>>,
}

impl FingerprintFile {
    pub fn new(index_type_name: &str, embeddings: Vec<Vec<f32>>) -> Result<Self, String> {
        index_type(index_type_name)?;
        window_width(std::slice::from_ref(&embeddings))?;
        Ok(Self {
            version: 1,
            index_type: index_type_name.into(),
            embeddings,
        })
    }

    pub fn to_index(&self) -> Result<embedding::FingerprintIndex, String> {
        if self.version != 1 {
            return Err("unsupported fingerprint index version".into());
        }
        window_width(std::slice::from_ref(&self.embeddings))?;
        let mut index = embedding::FingerprintIndex::new(index_type(&self.index_type)?);
        for (i, emb) in self.embeddings.iter().enumerate() {
            index.insert(emb.clone(), format!("window_{i}"), 0);
        }
        Ok(index)
    }

    /// Refuse to overwrite an existing model, dataset, or index.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("cannot create index {}: {e}", path.display()))?;
        use std::io::Write;
        file.write_all(&bytes)
            .map_err(|e| format!("cannot write index {}: {e}", path.display()))
    }
}
