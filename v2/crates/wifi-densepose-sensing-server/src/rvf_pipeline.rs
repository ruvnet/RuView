//! Extended RVF build pipeline — ADR-023 Phases 7-8.
//!
//! Adds HNSW index, overlay graph, SONA profile, and progressive loading
//! segments on top of the base `rvf_container` module.

use std::path::Path;

use crate::rvf_container::{RvfBuilder, RvfReader};

// ── Additional segment type discriminators ──────────────────────────────────

/// HNSW index layers for sparse neuron routing.
pub const SEG_INDEX: u8 = 0x02;
/// Pre-computed min-cut graph structures.
pub const SEG_OVERLAY: u8 = 0x03;
/// SONA LoRA deltas per environment.
pub const SEG_AGGREGATE_WEIGHTS: u8 = 0x36;
/// Integrity signatures.
pub const SEG_CRYPTO: u8 = 0x0C;
/// WASM inference engine bytes.
pub const SEG_WASM: u8 = 0x10;
/// Embedded UI dashboard assets.
pub const SEG_DASHBOARD: u8 = 0x11;

// ── HnswIndex ───────────────────────────────────────────────────────────────

/// A single node in an HNSW layer.
#[derive(Debug, Clone)]
pub struct HnswNode {
    pub id: usize,
    pub neighbors: Vec<usize>,
    pub vector: Vec<f32>,
}

/// One layer of the HNSW graph.
#[derive(Debug, Clone)]
pub struct HnswLayer {
    pub nodes: Vec<HnswNode>,
}

/// Serializable HNSW index used for sparse inference neuron routing.
#[derive(Debug, Clone)]
pub struct HnswIndex {
    pub layers: Vec<HnswLayer>,
    pub entry_point: usize,
    pub ef_construction: usize,
    pub m: usize,
}

impl HnswIndex {
    /// Serialize the index to a byte vector.
    ///
    /// Wire format (all little-endian):
    /// ```text
    /// [entry_point: u64][ef_construction: u64][m: u64][n_layers: u32]
    /// per layer:
    ///   [n_nodes: u32]
    ///   per node:
    ///     [id: u64][n_neighbors: u32][neighbors: u64*n][vec_len: u32][vector: f32*vec_len]
    /// ```
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(self.entry_point as u64).to_le_bytes());
        buf.extend_from_slice(&(self.ef_construction as u64).to_le_bytes());
        buf.extend_from_slice(&(self.m as u64).to_le_bytes());
        buf.extend_from_slice(&(self.layers.len() as u32).to_le_bytes());

        for layer in &self.layers {
            buf.extend_from_slice(&(layer.nodes.len() as u32).to_le_bytes());
            for node in &layer.nodes {
                buf.extend_from_slice(&(node.id as u64).to_le_bytes());
                buf.extend_from_slice(&(node.neighbors.len() as u32).to_le_bytes());
                for &n in &node.neighbors {
                    buf.extend_from_slice(&(n as u64).to_le_bytes());
                }
                buf.extend_from_slice(&(node.vector.len() as u32).to_le_bytes());
                for &v in &node.vector {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        buf
    }

    /// Deserialize an HNSW index from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut off = 0usize;
        let read_u32 = |o: &mut usize| -> Result<u32, String> {
            if *o + 4 > data.len() {
                return Err("truncated u32".into());
            }
            let v = u32::from_le_bytes(data[*o..*o + 4].try_into().unwrap());
            *o += 4;
            Ok(v)
        };
        let read_u64 = |o: &mut usize| -> Result<u64, String> {
            if *o + 8 > data.len() {
                return Err("truncated u64".into());
            }
            let v = u64::from_le_bytes(data[*o..*o + 8].try_into().unwrap());
            *o += 8;
            Ok(v)
        };
        let read_f32 = |o: &mut usize| -> Result<f32, String> {
            if *o + 4 > data.len() {
                return Err("truncated f32".into());
            }
            let v = f32::from_le_bytes(data[*o..*o + 4].try_into().unwrap());
            *o += 4;
            Ok(v)
        };

        let entry_point = read_u64(&mut off)? as usize;
        let ef_construction = read_u64(&mut off)? as usize;
        let m = read_u64(&mut off)? as usize;
        let n_layers = read_u32(&mut off)? as usize;

        let mut layers = Vec::with_capacity(n_layers);
        for _ in 0..n_layers {
            let n_nodes = read_u32(&mut off)? as usize;
            let mut nodes = Vec::with_capacity(n_nodes);
            for _ in 0..n_nodes {
                let id = read_u64(&mut off)? as usize;
                let n_neigh = read_u32(&mut off)? as usize;
                let mut neighbors = Vec::with_capacity(n_neigh);
                for _ in 0..n_neigh {
                    neighbors.push(read_u64(&mut off)? as usize);
                }
                let vec_len = read_u32(&mut off)? as usize;
                let mut vector = Vec::with_capacity(vec_len);
                for _ in 0..vec_len {
                    vector.push(read_f32(&mut off)?);
                }
                nodes.push(HnswNode { id, neighbors, vector });
            }
            layers.push(HnswLayer { nodes });
        }

        Ok(Self { layers, entry_point, ef_construction, m })
    }
}

// ── OverlayGraph ────────────────────────────────────────────────────────────

/// Weighted adjacency list: `(src, dst, weight)` edges.
#[derive(Debug, Clone)]
pub struct AdjacencyList {
    pub n_nodes: usize,
    pub edges: Vec<(usize, usize, f32)>,
}

/// Min-cut partition result.
#[derive(Debug, Clone)]
pub struct Partition {
    pub sensitive: Vec<usize>,
    pub insensitive: Vec<usize>,
}

/// Pre-computed graph overlay structures for the sensing pipeline.
#[derive(Debug, Clone)]
pub struct OverlayGraph {
    pub subcarrier_graph: AdjacencyList,
    pub antenna_graph: AdjacencyList,
    pub body_graph: AdjacencyList,
    pub mincut_partitions: Vec<Partition>,
}

impl OverlayGraph {
    /// Serialize overlay graph to bytes.
    ///
    /// Format: three adjacency lists followed by partitions.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        Self::write_adj(&mut buf, &self.subcarrier_graph);
        Self::write_adj(&mut buf, &self.antenna_graph);
        Self::write_adj(&mut buf, &self.body_graph);

        buf.extend_from_slice(&(self.mincut_partitions.len() as u32).to_le_bytes());
        for p in &self.mincut_partitions {
            buf.extend_from_slice(&(p.sensitive.len() as u32).to_le_bytes());
            for &s in &p.sensitive {
                buf.extend_from_slice(&(s as u64).to_le_bytes());
            }
            buf.extend_from_slice(&(p.insensitive.len() as u32).to_le_bytes());
            for &i in &p.insensitive {
                buf.extend_from_slice(&(i as u64).to_le_bytes());
            }
        }
        buf
    }

    /// Deserialize overlay graph from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut off = 0usize;
        let subcarrier_graph = Self::read_adj(data, &mut off)?;
        let antenna_graph = Self::read_adj(data, &mut off)?;
        let body_graph = Self::read_adj(data, &mut off)?;

        let n_part = Self::read_u32(data, &mut off)? as usize;
        let mut mincut_partitions = Vec::with_capacity(n_part);
        for _ in 0..n_part {
            let ns = Self::read_u32(data, &mut off)? as usize;
            let mut sensitive = Vec::with_capacity(ns);
            for _ in 0..ns {
                sensitive.push(Self::read_u64(data, &mut off)? as usize);
            }
            let ni = Self::read_u32(data, &mut off)? as usize;
            let mut insensitive = Vec::with_capacity(ni);
            for _ in 0..ni {
                insensitive.push(Self::read_u64(data, &mut off)? as usize);
            }
            mincut_partitions.push(Partition { sensitive, insensitive });
        }

        Ok(Self { subcarrier_graph, antenna_graph, body_graph, mincut_partitions })
    }

    // -- helpers --

    fn write_adj(buf: &mut Vec<u8>, adj: &AdjacencyList) {
        buf.extend_from_slice(&(adj.n_nodes as u32).to_le_bytes());
        buf.extend_from_slice(&(adj.edges.len() as u32).to_le_bytes());
        for &(s, d, w) in &adj.edges {
            buf.extend_from_slice(&(s as u64).to_le_bytes());
            buf.extend_from_slice(&(d as u64).to_le_bytes());
            buf.extend_from_slice(&w.to_le_bytes());
        }
    }

    fn read_adj(data: &[u8], off: &mut usize) -> Result<AdjacencyList, String> {
        let n_nodes = Self::read_u32(data, off)? as usize;
        let n_edges = Self::read_u32(data, off)? as usize;
        let mut edges = Vec::with_capacity(n_edges);
        for _ in 0..n_edges {
            let s = Self::read_u64(data, off)? as usize;
            let d = Self::read_u64(data, off)? as usize;
            let w = Self::read_f32(data, off)?;
            edges.push((s, d, w));
        }
        Ok(AdjacencyList { n_nodes, edges })
    }

    fn read_u32(data: &[u8], off: &mut usize) -> Result<u32, String> {
        if *off + 4 > data.len() {
            return Err("overlay: truncated u32".into());
        }
        let v = u32::from_le_bytes(data[*off..*off + 4].try_into().unwrap());
        *off += 4;
        Ok(v)
    }

    fn read_u64(data: &[u8], off: &mut usize) -> Result<u64, String> {
        if *off + 8 > data.len() {
            return Err("overlay: truncated u64".into());
        }
        let v = u64::from_le_bytes(data[*off..*off + 8].try_into().unwrap());
        *off += 8;
        Ok(v)
    }

    fn read_f32(data: &[u8], off: &mut usize) -> Result<f32, String> {
        if *off + 4 > data.len() {
            return Err("overlay: truncated f32".into());
        }
        let v = f32::from_le_bytes(data[*off..*off + 4].try_into().unwrap());
        *off += 4;
        Ok(v)
    }
}

// ── RvfBuildInfo ────────────────────────────────────────────────────────────

/// Summary returned by `RvfModelBuilder::build_info()`.
#[derive(Debug, Clone)]
pub struct RvfBuildInfo {
    pub segments: Vec<(String, usize)>,
    pub total_size: usize,
    pub model_name: String,
}

// ── RvfModelBuilder ─────────────────────────────────────────────────────────

/// High-level model packaging builder that wraps `RvfBuilder` with
/// domain-specific helpers for the WiFi-DensePose pipeline.
pub struct RvfModelBuilder {
    model_name: String,
    version: String,
    weights: Option<Vec<f32>>,
    hnsw: Option<HnswIndex>,
    overlay: Option<OverlayGraph>,
    quant_mode: Option<String>,
    quant_scale: f32,
    quant_zero: i32,
    sona_profiles: Vec<(String, Vec<f32>, Vec<f32>)>,
    training_hash: Option<String>,
    training_metrics: Option<serde_json::Value>,
    vital_config: Option<(f32, f32, f32, f32)>,
    model_profile: Option<(String, String, String)>,
}

impl RvfModelBuilder {
    /// Create a new model builder.
    pub fn new(model_name: &str, version: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            version: version.to_string(),
            weights: None,
            hnsw: None,
            overlay: None,
            quant_mode: None,
            quant_scale: 1.0,
            quant_zero: 0,
            sona_profiles: Vec::new(),
            training_hash: None,
            training_metrics: None,
            vital_config: None,
            model_profile: None,
        }
    }

    /// Set model weights.
    pub fn set_weights(&mut self, weights: &[f32]) -> &mut Self {
        self.weights = Some(weights.to_vec());
        self
    }

    /// Attach an HNSW index for sparse neuron routing.
    pub fn set_hnsw_index(&mut self, index: HnswIndex) -> &mut Self {
        self.hnsw = Some(index);
        self
    }

    /// Attach pre-computed overlay graph structures.
    pub fn set_overlay(&mut self, overlay: OverlayGraph) -> &mut Self {
        self.overlay = Some(overlay);
        self
    }

    /// Set quantization parameters.
    pub fn set_quantization(&mut self, mode: &str, scale: f32, zero_point: i32) -> &mut Self {
        self.quant_mode = Some(mode.to_string());
        self.quant_scale = scale;
        self.quant_zero = zero_point;
        self
    }

    /// Add a SONA environment adaptation profile (LoRA delta pair).
    pub fn add_sona_profile(
        &mut self,
        env_name: &str,
        lora_a: &[f32],
        lora_b: &[f32],
    ) -> &mut Self {
        self.sona_profiles
            .push((env_name.to_string(), lora_a.to_vec(), lora_b.to_vec()));
        self
    }

    /// Set training provenance (witness).
    pub fn set_training_proof(
        &mut self,
        hash: &str,
        metrics: serde_json::Value,
    ) -> &mut Self {
        self.training_hash = Some(hash.to_string());
        self.training_metrics = Some(metrics);
        self
    }

    /// Set vital sign detector bounds.
    pub fn set_vital_config(
        &mut self,
        breathing_min: f32,
        breathing_max: f32,
        heart_min: f32,
        heart_max: f32,
    ) -> &mut Self {
        self.vital_config = Some((breathing_min, breathing_max, heart_min, heart_max));
        self
    }

    /// Set model profile (input/output spec and requirements).
    pub fn set_model_profile(
        &mut self,
        input_spec: &str,
        output_spec: &str,
        requirements: &str,
    ) -> &mut Self {
        self.model_profile = Some((
            input_spec.to_string(),
            output_spec.to_string(),
            requirements.to_string(),
        ));
        self
    }

    /// Build the final RVF binary.
    pub fn build(&self) -> Result<Vec<u8>, String> {
        let mut rvf = RvfBuilder::new();

        // 1) Manifest
        rvf.add_manifest(&self.model_name, &self.version, "RvfModelBuilder output");

        // 2) Weights
        if let Some(ref w) = self.weights {
            rvf.add_weights(w);
        }

        // 3) HNSW index segment
        if let Some(ref idx) = self.hnsw {
            rvf.add_raw_segment(SEG_INDEX, &idx.to_bytes());
        }

        // 4) Overlay graph segment
        if let Some(ref ov) = self.overlay {
            rvf.add_raw_segment(SEG_OVERLAY, &ov.to_bytes());
        }

        // 5) Quantization
        if let Some(ref mode) = self.quant_mode {
            rvf.add_quant_info(mode, self.quant_scale, self.quant_zero);
        }

        // 6) SONA aggregate-weights segments
        for (env, lora_a, lora_b) in &self.sona_profiles {
            let payload = serde_json::to_vec(&serde_json::json!({
                "env": env,
                "lora_a": lora_a,
                "lora_b": lora_b,
            }))
            .map_err(|e| format!("sona serialize: {e}"))?;
            rvf.add_raw_segment(SEG_AGGREGATE_WEIGHTS, &payload);
        }

        // 7) Witness / training proof
        if let Some(ref hash) = self.training_hash {
            let metrics = self.training_metrics.clone().unwrap_or(serde_json::json!({}));
            rvf.add_witness(hash, &metrics);
        }

        // 8) Vital sign config (as profile segment)
        if let Some((br_lo, br_hi, hr_lo, hr_hi)) = self.vital_config {
            let cfg = crate::rvf_container::VitalSignConfig {
                breathing_low_hz: br_lo as f64,
                breathing_high_hz: br_hi as f64,
                heartrate_low_hz: hr_lo as f64,
                heartrate_high_hz: hr_hi as f64,
                ..Default::default()
            };
            rvf.add_vital_config(&cfg);
        }

        // 9) Model profile metadata
        if let Some((ref inp, ref out, ref req)) = self.model_profile {
            rvf.add_metadata(&serde_json::json!({
                "model_profile": {
                    "input_spec": inp,
                    "output_spec": out,
                    "requirements": req,
                }
            }));
        }

        // 10) Crypto placeholder (empty signature)
        rvf.add_raw_segment(SEG_CRYPTO, &[]);

        Ok(rvf.build())
    }

    /// Build and write to a file.
    pub fn write_to_file(&self, path: &Path) -> Result<(), String> {
        let data = self.build()?;
        std::fs::write(path, &data)
            .map_err(|e| format!("write {}: {e}", path.display()))
    }

    /// Return build info (segment names + sizes) without fully building.
    pub fn build_info(&self) -> RvfBuildInfo {
        // Build once to get accurate sizes.
        let data = self.build().unwrap_or_default();
        let reader = RvfReader::from_bytes(&data).ok();

        let segments: Vec<(String, usize)> = reader
            .as_ref()
            .map(|r| {
                r.segments()
                    .map(|(h, p)| (seg_type_name(h.seg_type), p.len()))
                    .collect()
            })
            .unwrap_or_default();

        RvfBuildInfo {
            segments,
            total_size: data.len(),
            model_name: self.model_name.clone(),
        }
    }
}

/// Human-readable segment type name.
fn seg_type_name(t: u8) -> String {
    match t {
        0x01 => "vec".into(),
        0x02 => "index".into(),
        0x03 => "overlay".into(),
        0x05 => "manifest".into(),
        0x06 => "quant".into(),
        0x07 => "meta".into(),
        0x0A => "witness".into(),
        0x0B => "profile".into(),
        0x0C => "crypto".into(),
        0x10 => "wasm".into(),
        0x11 => "dashboard".into(),
        0x36 => "aggregate_weights".into(),
        other => format!("0x{other:02X}"),
    }
}

// ── ProgressiveLoader ───────────────────────────────────────────────────────

/// Data returned by Layer A (instant startup).
#[derive(Debug, Clone)]
pub struct LayerAData {
    pub manifest: serde_json::Value,
    pub model_name: String,
    pub version: String,
    pub n_segments: usize,
}

/// Data returned by Layer B (hot neuron weights).
#[derive(Debug, Clone)]
pub struct LayerBData {
    pub weights_subset: Vec<f32>,
    pub hot_neuron_ids: Vec<usize>,
}

/// Data returned by Layer C (full model).
#[derive(Debug, Clone)]
pub struct LayerCData {
    pub all_weights: Vec<f32>,
    pub overlay: Option<OverlayGraph>,
    pub sona_profiles: Vec<(String, Vec<f32>)>,
}

/// Progressive loader that reads an RVF container in three layers of
/// increasing completeness.
pub struct ProgressiveLoader {
    reader: RvfReader,
    layer_a_loaded: bool,
    layer_b_loaded: bool,
    layer_c_loaded: bool,
}

impl ProgressiveLoader {
    /// Create a new progressive loader from raw RVF bytes.
    pub fn new(data: &[u8]) -> Result<Self, String> {
        let reader = RvfReader::from_bytes(data)?;
        Ok(Self {
            reader,
            layer_a_loaded: false,
            layer_b_loaded: false,
            layer_c_loaded: false,
        })
    }

    /// Load Layer A: manifest + index only (target: <5ms).
    pub fn load_layer_a(&mut self) -> Result<LayerAData, String> {
        let manifest = self.reader.manifest().unwrap_or(serde_json::json!({}));
        let model_name = manifest
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let version = manifest
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        let n_segments = self.reader.segment_count();

        self.layer_a_loaded = true;
        Ok(LayerAData { manifest, model_name, version, n_segments })
    }

    /// Load Layer B: hot neuron weights subset.
    pub fn load_layer_b(&mut self) -> Result<LayerBData, String> {
        // Load HNSW index to find hot neuron IDs.
        let hot_neuron_ids: Vec<usize> = self
            .reader
            .find_segment(SEG_INDEX)
            .and_then(|data| HnswIndex::from_bytes(data).ok())
            .map(|idx| {
                // Hot neurons = all nodes in layer 0 (most connected).
                idx.layers
                    .first()
                    .map(|l| l.nodes.iter().map(|n| n.id).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        // Extract a subset of weights corresponding to hot neurons.
        let all_w = self.reader.weights().unwrap_or_default();
        let weights_subset: Vec<f32> = if hot_neuron_ids.is_empty() {
            // No index — take first 25% of weights as "hot" subset.
            let n = all_w.len() / 4;
            all_w.iter().take(n.max(1)).copied().collect()
        } else {
            hot_neuron_ids
                .iter()
                .filter_map(|&id| all_w.get(id).copied())
                .collect()
        };

        self.layer_b_loaded = true;
        Ok(LayerBData { weights_subset, hot_neuron_ids })
    }

    /// Load Layer C: all remaining weights and structures (full accuracy).
    pub fn load_layer_c(&mut self) -> Result<LayerCData, String> {
        let all_weights = self.reader.weights().unwrap_or_default();

        let overlay = self
            .reader
            .find_segment(SEG_OVERLAY)
            .and_then(|data| OverlayGraph::from_bytes(data).ok());

        // Collect SONA profiles from aggregate-weight segments.
        let mut sona_profiles = Vec::new();
        for (h, payload) in self.reader.segments() {
            if h.seg_type == SEG_AGGREGATE_WEIGHTS {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(payload) {
                    let env = v
                        .get("env")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let lora_a: Vec<f32> = v
                        .get("lora_a")
                        .and_then(|a| serde_json::from_value(a.clone()).ok())
                        .unwrap_or_default();
                    sona_profiles.push((env, lora_a));
                }
            }
        }

        self.layer_c_loaded = true;
        Ok(LayerCData { all_weights, overlay, sona_profiles })
    }

    /// Current loading progress (0.0 to 1.0).
    pub fn loading_progress(&self) -> f32 {
        let mut p = 0.0f32;
        if self.layer_a_loaded {
            p += 0.33;
        }
        if self.layer_b_loaded {
            p += 0.34;
        }
        if self.layer_c_loaded {
            p += 0.33;
        }
        p.min(1.0)
    }

    /// Per-layer status for the REST API.
    pub fn layer_status(&self) -> (bool, bool, bool) {
        (self.layer_a_loaded, self.layer_b_loaded, self.layer_c_loaded)
    }

    /// Collect segment info list for the REST API.
    pub fn segment_list(&self) -> Vec<serde_json::Value> {
        self.reader
            .segments()
            .map(|(h, p)| {
                serde_json::json!({
                    "type": seg_type_name(h.seg_type),
                    "size": p.len(),
                    "segment_id": h.segment_id,
                })
            })
            .collect()
    }

    /// List available SONA profile names.
    pub fn sona_profile_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for (h, payload) in self.reader.segments() {
            if h.seg_type == SEG_AGGREGATE_WEIGHTS {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(payload) {
                    if let Some(env) = v.get("env").and_then(|e| e.as_str()) {
                        names.push(env.to_string());
                    }
                }
            }
        }
        names
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_hnsw() -> HnswIndex {
        HnswIndex {
            layers: vec![
                HnswLayer {
                    nodes: vec![
                        HnswNode { id: 0, neighbors: vec![1, 2], vector: vec![1.0, 2.0] },
                        HnswNode { id: 1, neighbors: vec![0], vector: vec![3.0, 4.0] },
                        HnswNode { id: 2, neighbors: vec![0], vector: vec![5.0, 6.0] },
                    ],
                },
                HnswLayer {
                    nodes: vec![
                        HnswNode { id: 0, neighbors: vec![2], vector: vec![1.0, 2.0] },
                    ],
                },
            ],
            entry_point: 0,
            ef_construction: 200,
            m: 16,
        }
    }

    fn sample_overlay() -> OverlayGraph {
        OverlayGraph {
            subcarrier_graph: AdjacencyList {
                n_nodes: 3,
                edges: vec![(0, 1, 0.5), (1, 2, 0.8)],
            },
            antenna_graph: AdjacencyList {
                n_nodes: 2,
                edges: vec![(0, 1, 1.0)],
            },
            body_graph: AdjacencyList {
                n_nodes: 4,
                edges: vec![(0, 1, 0.3), (2, 3, 0.9), (0, 3, 0.1)],
            },
            mincut_partitions: vec![Partition {
                sensitive: vec![0, 1],
                insensitive: vec![2, 3],
            }],
        }
    }

    #[test]
    fn hnsw_index_round_trip() {
        let idx = sample_hnsw();
        let bytes = idx.to_bytes();
        let decoded = HnswIndex::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.entry_point, 0);
        assert_eq!(decoded.ef_construction, 200);
        assert_eq!(decoded.m, 16);
        assert_eq!(decoded.layers.len(), 2);
        assert_eq!(decoded.layers[0].nodes.len(), 3);
        assert_eq!(decoded.layers[0].nodes[0].neighbors, vec![1, 2]);
        assert!((decoded.layers[0].nodes[1].vector[0] - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hnsw_index_empty_layers() {
        let idx = HnswIndex {
            layers: vec![],
            entry_point: 0,
            ef_construction: 64,
            m: 8,
        };
        let bytes = idx.to_bytes();
        let decoded = HnswIndex::from_bytes(&bytes).unwrap();
        assert!(decoded.layers.is_empty());
        assert_eq!(decoded.ef_construction, 64);
    }

    #[test]
    fn overlay_graph_round_trip() {
        let ov = sample_overlay();
        let bytes = ov.to_bytes();
        let decoded = OverlayGraph::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.subcarrier_graph.n_nodes, 3);
        assert_eq!(decoded.subcarrier_graph.edges.len(), 2);
        assert_eq!(decoded.antenna_graph.n_nodes, 2);
        assert_eq!(decoded.body_graph.edges.len(), 3);
        assert_eq!(decoded.mincut_partitions.len(), 1);
    }

    #[test]
    fn overlay_adjacency_list_edges() {
        let ov = sample_overlay();
        let bytes = ov.to_bytes();
        let decoded = OverlayGraph::from_bytes(&bytes).unwrap();
        let e = &decoded.subcarrier_graph.edges[0];
        assert_eq!(e.0, 0);
        assert_eq!(e.1, 1);
        assert!((e.2 - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn overlay_partition_sensitive_insensitive() {
        let ov = sample_overlay();
        let bytes = ov.to_bytes();
        let decoded = OverlayGraph::from_bytes(&bytes).unwrap();
        let p = &decoded.mincut_partitions[0];
        assert_eq!(p.sensitive, vec![0, 1]);
        assert_eq!(p.insensitive, vec![2, 3]);
    }

    #[test]
    fn model_builder_minimal() {
        let mut b = RvfModelBuilder::new("test-min", "0.1.0");
        b.set_weights(&[1.0, 2.0, 3.0]);
        let data = b.build().unwrap();
        assert!(!data.is_empty());

        let reader = RvfReader::from_bytes(&data).unwrap();
        // manifest + weights + crypto = 3 segments minimum
        assert!(reader.segment_count() >= 3);
        assert!(reader.manifest().is_some());
        assert!(reader.weights().is_some());
    }

    #[test]
    fn model_builder_full() {
        let mut b = RvfModelBuilder::new("full-model", "1.0.0");
        b.set_weights(&[0.1, 0.2, 0.3, 0.4]);
        b.set_hnsw_index(sample_hnsw());
        b.set_overlay(sample_overlay());
        b.set_quantization("int8", 0.0078, -128);
        b.add_sona_profile("office-3f", &[0.1, 0.2], &[0.3, 0.4]);
        b.add_sona_profile("warehouse", &[0.5], &[0.6]);
        b.set_training_proof("sha256:abc123", serde_json::json!({"loss": 0.01}));
        b.set_vital_config(0.1, 0.5, 0.8, 2.0);
        b.set_model_profile("csi_56d", "keypoints_17", "gpu_optional");

        let data = b.build().unwrap();
        let reader = RvfReader::from_bytes(&data).unwrap();

        // manifest + vec + index + overlay + quant + 2*agg + witness + profile + meta + crypto = 11
        assert!(reader.segment_count() >= 10, "got {}", reader.segment_count());
        assert!(reader.manifest().is_some());
        assert!(reader.weights().is_some());
        assert!(reader.find_segment(SEG_INDEX).is_some());
        assert!(reader.find_segment(SEG_OVERLAY).is_some());
        assert!(reader.find_segment(SEG_CRYPTO).is_some());
    }

    #[test]
    fn model_builder_build_info_reports_sizes() {
        let mut b = RvfModelBuilder::new("info-test", "2.0.0");
        b.set_weights(&[1.0; 100]);
        let info = b.build_info();
        assert_eq!(info.model_name, "info-test");
        assert!(info.total_size > 0);
        assert!(!info.segments.is_empty());
        // At least one segment should have meaningful size
        assert!(info.segments.iter().any(|(_, sz)| *sz > 0));
    }

    #[test]
    fn model_builder_sona_profiles_stored() {
        let mut b = RvfModelBuilder::new("sona-test", "1.0.0");
        b.set_weights(&[1.0]);
        b.add_sona_profile("env-a", &[0.1, 0.2], &[0.3, 0.4]);
        b.add_sona_profile("env-b", &[0.5], &[0.6]);

        let data = b.build().unwrap();
        let reader = RvfReader::from_bytes(&data).unwrap();

        // Count aggregate-weight segments.
        let agg_count = reader
            .segments()
            .filter(|(h, _)| h.seg_type == SEG_AGGREGATE_WEIGHTS)
            .count();
        assert_eq!(agg_count, 2);

        // Verify first profile content.
        let (_, payload) = reader
            .segments()
            .find(|(h, _)| h.seg_type == SEG_AGGREGATE_WEIGHTS)
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(v["env"], "env-a");
    }

    #[test]
    fn progressive_loader_layer_a_fast() {
        let mut b = RvfModelBuilder::new("prog-a", "1.0.0");
        b.set_weights(&[1.0; 50]);
        let data = b.build().unwrap();

        let mut loader = ProgressiveLoader::new(&data).unwrap();
        let start = std::time::Instant::now();
        let la = loader.load_layer_a().unwrap();
        let elapsed = start.elapsed();

        assert_eq!(la.model_name, "prog-a");
        assert_eq!(la.version, "1.0.0");
        assert!(la.n_segments > 0);
        // Layer A should be very fast (target <5ms, we allow generous 100ms for CI).
        assert!(elapsed.as_millis() < 100, "Layer A took {}ms", elapsed.as_millis());
    }

    #[test]
    fn progressive_loader_all_layers() {
        let mut b = RvfModelBuilder::new("prog-all", "2.0.0");
        b.set_weights(&[0.5; 20]);
        b.set_hnsw_index(sample_hnsw());
        b.set_overlay(sample_overlay());
        b.add_sona_profile("env-x", &[1.0], &[2.0]);

        let data = b.build().unwrap();
        let mut loader = ProgressiveLoader::new(&data).unwrap();

        let la = loader.load_layer_a().unwrap();
        assert_eq!(la.model_name, "prog-all");

        let lb = loader.load_layer_b().unwrap();
        // HNSW has nodes 0,1,2 in layer 0, so hot_neuron_ids should contain those.
        assert!(!lb.hot_neuron_ids.is_empty());
        assert!(!lb.weights_subset.is_empty());

        let lc = loader.load_layer_c().unwrap();
        assert_eq!(lc.all_weights.len(), 20);
        assert!(lc.overlay.is_some());
        assert_eq!(lc.sona_profiles.len(), 1);
        assert_eq!(lc.sona_profiles[0].0, "env-x");
    }

    #[test]
    fn progressive_loader_progress_tracking() {
        let mut b = RvfModelBuilder::new("prog-track", "1.0.0");
        b.set_weights(&[1.0]);
        let data = b.build().unwrap();
        let mut loader = ProgressiveLoader::new(&data).unwrap();

        assert!((loader.loading_progress() - 0.0).abs() < f32::EPSILON);

        loader.load_layer_a().unwrap();
        assert!(loader.loading_progress() > 0.3);

        loader.load_layer_b().unwrap();
        assert!(loader.loading_progress() > 0.6);

        loader.load_layer_c().unwrap();
        assert!((loader.loading_progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn rvf_model_file_round_trip() {
        let dir = std::env::temp_dir().join("rvf_pipeline_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pipeline_model.rvf");

        let mut b = RvfModelBuilder::new("file-rt", "3.0.0");
        b.set_weights(&[42.0, -1.0, 0.0]);
        b.set_hnsw_index(sample_hnsw());
        b.write_to_file(&path).unwrap();

        let reader = RvfReader::from_file(&path).unwrap();
        assert!(reader.segment_count() >= 3);
        let manifest = reader.manifest().unwrap();
        assert_eq!(manifest["model_id"], "file-rt");

        let w = reader.weights().unwrap();
        assert_eq!(w.len(), 3);
        assert!((w[0] - 42.0).abs() < f32::EPSILON);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn segment_type_constants_unique() {
        let types = [
            SEG_INDEX,
            SEG_OVERLAY,
            SEG_AGGREGATE_WEIGHTS,
            SEG_CRYPTO,
            SEG_WASM,
            SEG_DASHBOARD,
        ];
        // Also include the base types from rvf_container to ensure no collision.
        let base_types: [u8; 6] = [0x01, 0x05, 0x06, 0x07, 0x0A, 0x0B];
        let mut all: Vec<u8> = types.to_vec();
        all.extend_from_slice(&base_types);

        let mut seen = std::collections::HashSet::new();
        for t in &all {
            assert!(seen.insert(*t), "duplicate segment type: 0x{t:02X}");
        }
    }

    #[test]
    fn aggregate_weights_multiple_envs() {
        let mut b = RvfModelBuilder::new("multi-env", "1.0.0");
        b.set_weights(&[1.0]);
        b.add_sona_profile("office", &[0.1, 0.2, 0.3], &[0.4, 0.5, 0.6]);
        b.add_sona_profile("warehouse", &[0.7, 0.8], &[0.9, 1.0]);
        b.add_sona_profile("outdoor", &[1.1], &[1.2]);

        let data = b.build().unwrap();
        let mut loader = ProgressiveLoader::new(&data).unwrap();
        let names = loader.sona_profile_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"office".to_string()));
        assert!(names.contains(&"warehouse".to_string()));
        assert!(names.contains(&"outdoor".to_string()));

        let lc = loader.load_layer_c().unwrap();
        assert_eq!(lc.sona_profiles.len(), 3);
    }

    #[test]
    fn crypto_segment_placeholder() {
        let mut b = RvfModelBuilder::new("crypto-test", "1.0.0");
        b.set_weights(&[1.0]);
        let data = b.build().unwrap();
        let reader = RvfReader::from_bytes(&data).unwrap();

        // Crypto segment should exist but be empty (placeholder).
        let crypto = reader.find_segment(SEG_CRYPTO);
        assert!(crypto.is_some(), "crypto segment must be present");
        assert!(crypto.unwrap().is_empty(), "crypto segment should be empty placeholder");
    }
}
