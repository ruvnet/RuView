//! Graph Transformer + GNN for WiFi CSI-to-Pose estimation (ADR-023 Phase 2).
//!
//! Cross-attention bottleneck between antenna-space CSI features and COCO 17-keypoint
//! body graph, followed by GCN message passing. All math is pure `std`.

/// Xorshift64 PRNG for deterministic weight initialization.
#[derive(Debug, Clone)]
struct Rng64 { state: u64 }

impl Rng64 {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 0xDEAD_BEEF_CAFE_1234 } else { seed } }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.state = x; x
    }
    /// Uniform f32 in (-1, 1).
    fn next_f32(&mut self) -> f32 {
        let f = (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32;
        f * 2.0 - 1.0
    }
}

#[inline]
fn relu(x: f32) -> f32 { if x > 0.0 { x } else { 0.0 } }

#[inline]
fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 { 1.0 / (1.0 + (-x).exp()) }
    else { let ex = x.exp(); ex / (1.0 + ex) }
}

/// Numerically stable softmax. Writes normalised weights into `out`.
fn softmax(scores: &[f32], out: &mut [f32]) {
    debug_assert_eq!(scores.len(), out.len());
    if scores.is_empty() { return; }
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0f32;
    for (o, &s) in out.iter_mut().zip(scores) {
        let e = (s - max).exp(); *o = e; sum += e;
    }
    let inv = if sum > 1e-10 { 1.0 / sum } else { 0.0 };
    for o in out.iter_mut() { *o *= inv; }
}

// ── Linear layer ─────────────────────────────────────────────────────────

/// Dense linear transformation y = Wx + b (row-major weights).
#[derive(Debug, Clone)]
pub struct Linear {
    in_features: usize,
    out_features: usize,
    weights: Vec<Vec<f32>>,
    bias: Vec<f32>,
}

impl Linear {
    /// Xavier/Glorot uniform init with default seed.
    pub fn new(in_features: usize, out_features: usize) -> Self {
        Self::with_seed(in_features, out_features, 42)
    }
    /// Xavier/Glorot uniform init with explicit seed.
    pub fn with_seed(in_features: usize, out_features: usize, seed: u64) -> Self {
        let mut rng = Rng64::new(seed);
        let limit = (6.0 / (in_features + out_features) as f32).sqrt();
        let weights = (0..out_features)
            .map(|_| (0..in_features).map(|_| rng.next_f32() * limit).collect())
            .collect();
        Self { in_features, out_features, weights, bias: vec![0.0; out_features] }
    }
    /// All-zero weights (for testing).
    pub fn zeros(in_features: usize, out_features: usize) -> Self {
        Self {
            in_features, out_features,
            weights: vec![vec![0.0; in_features]; out_features],
            bias: vec![0.0; out_features],
        }
    }
    /// Forward pass: y = Wx + b.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.in_features,
            "Linear input mismatch: expected {}, got {}", self.in_features, input.len());
        let mut out = vec![0.0f32; self.out_features];
        for (i, row) in self.weights.iter().enumerate() {
            let mut s = self.bias[i];
            for (w, x) in row.iter().zip(input) { s += w * x; }
            out[i] = s;
        }
        out
    }
    pub fn weights(&self) -> &[Vec<f32>] { &self.weights }
    pub fn set_weights(&mut self, w: Vec<Vec<f32>>) {
        assert_eq!(w.len(), self.out_features);
        for row in &w { assert_eq!(row.len(), self.in_features); }
        self.weights = w;
    }
    pub fn set_bias(&mut self, b: Vec<f32>) {
        assert_eq!(b.len(), self.out_features);
        self.bias = b;
    }

    /// Push all weights (row-major) then bias into a flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        for row in &self.weights {
            out.extend_from_slice(row);
        }
        out.extend_from_slice(&self.bias);
    }

    /// Restore from a flat slice. Returns (Self, number of f32s consumed).
    pub fn unflatten_from(data: &[f32], in_f: usize, out_f: usize) -> (Self, usize) {
        let n = in_f * out_f + out_f;
        assert!(data.len() >= n, "unflatten_from: need {n} floats, got {}", data.len());
        let mut weights = Vec::with_capacity(out_f);
        for r in 0..out_f {
            let start = r * in_f;
            weights.push(data[start..start + in_f].to_vec());
        }
        let bias = data[in_f * out_f..n].to_vec();
        (Self { in_features: in_f, out_features: out_f, weights, bias }, n)
    }

    /// Total number of trainable parameters.
    pub fn param_count(&self) -> usize {
        self.in_features * self.out_features + self.out_features
    }
}

// ── AntennaGraph ─────────────────────────────────────────────────────────

/// Spatial topology graph over TX-RX antenna pairs. Nodes = pairs, edges connect
/// pairs sharing a TX or RX antenna.
#[derive(Debug, Clone)]
pub struct AntennaGraph {
    n_tx: usize, n_rx: usize, n_pairs: usize,
    adjacency: Vec<Vec<f32>>,
}

impl AntennaGraph {
    /// Build antenna graph. pair_id = tx * n_rx + rx. Adjacent if shared TX or RX.
    pub fn new(n_tx: usize, n_rx: usize) -> Self {
        let n_pairs = n_tx * n_rx;
        let mut adj = vec![vec![0.0f32; n_pairs]; n_pairs];
        for i in 0..n_pairs {
            let (tx_i, rx_i) = (i / n_rx, i % n_rx);
            adj[i][i] = 1.0;
            for j in (i + 1)..n_pairs {
                let (tx_j, rx_j) = (j / n_rx, j % n_rx);
                if tx_i == tx_j || rx_i == rx_j {
                    adj[i][j] = 1.0; adj[j][i] = 1.0;
                }
            }
        }
        Self { n_tx, n_rx, n_pairs, adjacency: adj }
    }
    pub fn n_nodes(&self) -> usize { self.n_pairs }
    pub fn adjacency_matrix(&self) -> &Vec<Vec<f32>> { &self.adjacency }
    pub fn n_tx(&self) -> usize { self.n_tx }
    pub fn n_rx(&self) -> usize { self.n_rx }
}

// ── BodyGraph ────────────────────────────────────────────────────────────

/// COCO 17-keypoint skeleton graph with 16 anatomical edges.
///
/// Indices: 0=nose 1=l_eye 2=r_eye 3=l_ear 4=r_ear 5=l_shoulder 6=r_shoulder
/// 7=l_elbow 8=r_elbow 9=l_wrist 10=r_wrist 11=l_hip 12=r_hip 13=l_knee
/// 14=r_knee 15=l_ankle 16=r_ankle
#[derive(Debug, Clone)]
pub struct BodyGraph {
    adjacency: [[f32; 17]; 17],
    edges: Vec<(usize, usize)>,
}

pub const COCO_KEYPOINT_NAMES: [&str; 17] = [
    "nose","left_eye","right_eye","left_ear","right_ear",
    "left_shoulder","right_shoulder","left_elbow","right_elbow",
    "left_wrist","right_wrist","left_hip","right_hip",
    "left_knee","right_knee","left_ankle","right_ankle",
];

const COCO_EDGES: [(usize, usize); 16] = [
    (0,1),(0,2),(1,3),(2,4),(5,6),(5,7),(7,9),(6,8),
    (8,10),(5,11),(6,12),(11,12),(11,13),(13,15),(12,14),(14,16),
];

impl BodyGraph {
    pub fn new() -> Self {
        let mut adjacency = [[0.0f32; 17]; 17];
        for i in 0..17 { adjacency[i][i] = 1.0; }
        for &(u, v) in &COCO_EDGES { adjacency[u][v] = 1.0; adjacency[v][u] = 1.0; }
        Self { adjacency, edges: COCO_EDGES.to_vec() }
    }
    pub fn adjacency_matrix(&self) -> &[[f32; 17]; 17] { &self.adjacency }
    pub fn edge_list(&self) -> &Vec<(usize, usize)> { &self.edges }
    pub fn n_nodes(&self) -> usize { 17 }
    pub fn n_edges(&self) -> usize { self.edges.len() }

    /// Degree of each node (including self-loop).
    pub fn degrees(&self) -> [f32; 17] {
        let mut deg = [0.0f32; 17];
        for i in 0..17 { for j in 0..17 { deg[i] += self.adjacency[i][j]; } }
        deg
    }
    /// Symmetric normalised adjacency D^{-1/2} A D^{-1/2}.
    pub fn normalized_adjacency(&self) -> [[f32; 17]; 17] {
        let deg = self.degrees();
        let inv_sqrt: Vec<f32> = deg.iter()
            .map(|&d| if d > 0.0 { 1.0 / d.sqrt() } else { 0.0 }).collect();
        let mut norm = [[0.0f32; 17]; 17];
        for i in 0..17 { for j in 0..17 {
            norm[i][j] = inv_sqrt[i] * self.adjacency[i][j] * inv_sqrt[j];
        }}
        norm
    }
}

impl Default for BodyGraph { fn default() -> Self { Self::new() } }

// ── CrossAttention ───────────────────────────────────────────────────────

/// Multi-head scaled dot-product cross-attention.
/// Attn(Q,K,V) = softmax(QK^T / sqrt(d_k)) V, split into n_heads.
#[derive(Debug, Clone)]
pub struct CrossAttention {
    d_model: usize, n_heads: usize, d_k: usize,
    w_q: Linear, w_k: Linear, w_v: Linear, w_o: Linear,
}

impl CrossAttention {
    pub fn new(d_model: usize, n_heads: usize) -> Self {
        assert!(d_model % n_heads == 0,
            "d_model ({d_model}) must be divisible by n_heads ({n_heads})");
        let d_k = d_model / n_heads;
        let s = 123u64;
        Self { d_model, n_heads, d_k,
            w_q: Linear::with_seed(d_model, d_model, s),
            w_k: Linear::with_seed(d_model, d_model, s+1),
            w_v: Linear::with_seed(d_model, d_model, s+2),
            w_o: Linear::with_seed(d_model, d_model, s+3),
        }
    }
    /// query [n_q, d_model], key/value [n_kv, d_model] -> [n_q, d_model].
    pub fn forward(&self, query: &[Vec<f32>], key: &[Vec<f32>], value: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let (n_q, n_kv) = (query.len(), key.len());
        if n_q == 0 || n_kv == 0 { return vec![vec![0.0; self.d_model]; n_q]; }

        let q_proj: Vec<Vec<f32>> = query.iter().map(|q| self.w_q.forward(q)).collect();
        let k_proj: Vec<Vec<f32>> = key.iter().map(|k| self.w_k.forward(k)).collect();
        let v_proj: Vec<Vec<f32>> = value.iter().map(|v| self.w_v.forward(v)).collect();

        let scale = (self.d_k as f32).sqrt();
        let mut output = vec![vec![0.0f32; self.d_model]; n_q];

        for qi in 0..n_q {
            let mut concat = Vec::with_capacity(self.d_model);
            for h in 0..self.n_heads {
                let (start, end) = (h * self.d_k, (h + 1) * self.d_k);
                let q_h = &q_proj[qi][start..end];
                let mut scores = vec![0.0f32; n_kv];
                for ki in 0..n_kv {
                    let dot: f32 = q_h.iter().zip(&k_proj[ki][start..end]).map(|(a,b)| a*b).sum();
                    scores[ki] = dot / scale;
                }
                let mut wts = vec![0.0f32; n_kv];
                softmax(&scores, &mut wts);
                let mut head_out = vec![0.0f32; self.d_k];
                for ki in 0..n_kv {
                    for (o, &v) in head_out.iter_mut().zip(&v_proj[ki][start..end]) {
                        *o += wts[ki] * v;
                    }
                }
                concat.extend_from_slice(&head_out);
            }
            output[qi] = self.w_o.forward(&concat);
        }
        output
    }
    pub fn d_model(&self) -> usize { self.d_model }
    pub fn n_heads(&self) -> usize { self.n_heads }

    /// Push all cross-attention weights (w_q, w_k, w_v, w_o) into flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        self.w_q.flatten_into(out);
        self.w_k.flatten_into(out);
        self.w_v.flatten_into(out);
        self.w_o.flatten_into(out);
    }

    /// Restore cross-attention weights from flat slice. Returns (Self, consumed).
    pub fn unflatten_from(data: &[f32], d_model: usize, n_heads: usize) -> (Self, usize) {
        let mut offset = 0;
        let (w_q, n) = Linear::unflatten_from(&data[offset..], d_model, d_model);
        offset += n;
        let (w_k, n) = Linear::unflatten_from(&data[offset..], d_model, d_model);
        offset += n;
        let (w_v, n) = Linear::unflatten_from(&data[offset..], d_model, d_model);
        offset += n;
        let (w_o, n) = Linear::unflatten_from(&data[offset..], d_model, d_model);
        offset += n;
        let d_k = d_model / n_heads;
        (Self { d_model, n_heads, d_k, w_q, w_k, w_v, w_o }, offset)
    }

    /// Total trainable params in cross-attention.
    pub fn param_count(&self) -> usize {
        self.w_q.param_count() + self.w_k.param_count()
            + self.w_v.param_count() + self.w_o.param_count()
    }
}

// ── GraphMessagePassing ──────────────────────────────────────────────────

/// GCN layer: H' = ReLU(A_norm H W) where A_norm = D^{-1/2} A D^{-1/2}.
#[derive(Debug, Clone)]
pub struct GraphMessagePassing {
    pub(crate) in_features: usize,
    pub(crate) out_features: usize,
    pub(crate) weight: Linear,
    norm_adj: [[f32; 17]; 17],
}

impl GraphMessagePassing {
    pub fn new(in_features: usize, out_features: usize, graph: &BodyGraph) -> Self {
        Self { in_features, out_features,
            weight: Linear::with_seed(in_features, out_features, 777),
            norm_adj: graph.normalized_adjacency() }
    }
    /// node_features [17, in_features] -> [17, out_features].
    pub fn forward(&self, node_features: &[Vec<f32>]) -> Vec<Vec<f32>> {
        assert_eq!(node_features.len(), 17, "expected 17 nodes, got {}", node_features.len());
        let mut agg = vec![vec![0.0f32; self.in_features]; 17];
        for i in 0..17 { for j in 0..17 {
            let a = self.norm_adj[i][j];
            if a.abs() > 1e-10 {
                for (ag, &f) in agg[i].iter_mut().zip(&node_features[j]) { *ag += a * f; }
            }
        }}
        agg.iter().map(|a| self.weight.forward(a).into_iter().map(relu).collect()).collect()
    }
    pub fn in_features(&self) -> usize { self.in_features }
    pub fn out_features(&self) -> usize { self.out_features }

    /// Push all layer weights into a flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        self.weight.flatten_into(out);
    }

    /// Restore from a flat slice. Returns number of f32s consumed.
    pub fn unflatten_from(&mut self, data: &[f32]) -> usize {
        let (lin, consumed) = Linear::unflatten_from(data, self.in_features, self.out_features);
        self.weight = lin;
        consumed
    }

    /// Total trainable params in this GCN layer.
    pub fn param_count(&self) -> usize { self.weight.param_count() }
}

/// Stack of GCN layers.
#[derive(Debug, Clone)]
pub struct GnnStack { pub(crate) layers: Vec<GraphMessagePassing> }

impl GnnStack {
    pub fn new(in_f: usize, out_f: usize, n: usize, g: &BodyGraph) -> Self {
        assert!(n >= 1);
        let mut layers = vec![GraphMessagePassing::new(in_f, out_f, g)];
        for _ in 1..n { layers.push(GraphMessagePassing::new(out_f, out_f, g)); }
        Self { layers }
    }
    pub fn forward(&self, feats: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let mut h = feats.to_vec();
        for l in &self.layers { h = l.forward(&h); }
        h
    }
    /// Push all GNN weights into a flat vec.
    pub fn flatten_into(&self, out: &mut Vec<f32>) {
        for l in &self.layers { l.flatten_into(out); }
    }
    /// Restore GNN weights from flat slice. Returns number of f32s consumed.
    pub fn unflatten_from(&mut self, data: &[f32]) -> usize {
        let mut offset = 0;
        for l in &mut self.layers {
            offset += l.unflatten_from(&data[offset..]);
        }
        offset
    }
    /// Total trainable params across all GCN layers.
    pub fn param_count(&self) -> usize {
        self.layers.iter().map(|l| l.param_count()).sum()
    }
}

// ── Transformer config / output / pipeline ───────────────────────────────

/// Configuration for the CSI-to-Pose transformer.
#[derive(Debug, Clone)]
pub struct TransformerConfig {
    pub n_subcarriers: usize,
    pub n_keypoints: usize,
    pub d_model: usize,
    pub n_heads: usize,
    pub n_gnn_layers: usize,
}

impl Default for TransformerConfig {
    fn default() -> Self {
        Self { n_subcarriers: 56, n_keypoints: 17, d_model: 64, n_heads: 4, n_gnn_layers: 2 }
    }
}

/// Output of the CSI-to-Pose transformer.
#[derive(Debug, Clone)]
pub struct PoseOutput {
    /// Predicted (x, y, z) per keypoint.
    pub keypoints: Vec<(f32, f32, f32)>,
    /// Per-keypoint confidence in [0, 1].
    pub confidences: Vec<f32>,
    /// Per-keypoint GNN features for downstream use.
    pub body_part_features: Vec<Vec<f32>>,
}

/// Full CSI-to-Pose pipeline: CSI embed -> cross-attention -> GNN -> regression heads.
#[derive(Debug, Clone)]
pub struct CsiToPoseTransformer {
    config: TransformerConfig,
    csi_embed: Linear,
    keypoint_queries: Vec<Vec<f32>>,
    cross_attn: CrossAttention,
    gnn: GnnStack,
    xyz_head: Linear,
    conf_head: Linear,
}

impl CsiToPoseTransformer {
    pub fn new(config: TransformerConfig) -> Self {
        let d = config.d_model;
        let bg = BodyGraph::new();
        let mut rng = Rng64::new(999);
        let limit = (6.0 / (config.n_keypoints + d) as f32).sqrt();
        let kq: Vec<Vec<f32>> = (0..config.n_keypoints)
            .map(|_| (0..d).map(|_| rng.next_f32() * limit).collect()).collect();
        Self {
            csi_embed: Linear::with_seed(config.n_subcarriers, d, 500),
            keypoint_queries: kq,
            cross_attn: CrossAttention::new(d, config.n_heads),
            gnn: GnnStack::new(d, d, config.n_gnn_layers, &bg),
            xyz_head: Linear::with_seed(d, 3, 600),
            conf_head: Linear::with_seed(d, 1, 700),
            config,
        }
    }
    /// Construct with zero-initialized weights (faster than Xavier init).
    /// Use with `unflatten_weights()` when you plan to overwrite all weights.
    pub fn zeros(config: TransformerConfig) -> Self {
        let d = config.d_model;
        let bg = BodyGraph::new();
        let kq = vec![vec![0.0f32; d]; config.n_keypoints];
        Self {
            csi_embed: Linear::zeros(config.n_subcarriers, d),
            keypoint_queries: kq,
            cross_attn: CrossAttention::new(d, config.n_heads), // small; kept for correct structure
            gnn: GnnStack::new(d, d, config.n_gnn_layers, &bg),
            xyz_head: Linear::zeros(d, 3),
            conf_head: Linear::zeros(d, 1),
            config,
        }
    }

    /// csi_features [n_antenna_pairs, n_subcarriers] -> PoseOutput with 17 keypoints.
    pub fn forward(&self, csi_features: &[Vec<f32>]) -> PoseOutput {
        let embedded: Vec<Vec<f32>> = csi_features.iter()
            .map(|f| self.csi_embed.forward(f)).collect();
        let attended = self.cross_attn.forward(&self.keypoint_queries, &embedded, &embedded);
        let gnn_out = self.gnn.forward(&attended);
        let mut kps = Vec::with_capacity(self.config.n_keypoints);
        let mut confs = Vec::with_capacity(self.config.n_keypoints);
        for nf in &gnn_out {
            let xyz = self.xyz_head.forward(nf);
            kps.push((xyz[0], xyz[1], xyz[2]));
            confs.push(sigmoid(self.conf_head.forward(nf)[0]));
        }
        PoseOutput { keypoints: kps, confidences: confs, body_part_features: gnn_out }
    }
    pub fn config(&self) -> &TransformerConfig { &self.config }

    /// Extract body-part feature embeddings without regression heads.
    /// Returns 17 vectors of dimension d_model (same as forward() but stops
    /// before xyz_head/conf_head).
    pub fn embed(&self, csi_features: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let embedded: Vec<Vec<f32>> = csi_features.iter()
            .map(|f| self.csi_embed.forward(f)).collect();
        let attended = self.cross_attn.forward(&self.keypoint_queries, &embedded, &embedded);
        self.gnn.forward(&attended)
    }

    /// Collect all trainable parameters into a flat vec.
    ///
    /// Layout: csi_embed | keypoint_queries (flat) | cross_attn | gnn | xyz_head | conf_head
    pub fn flatten_weights(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.param_count());
        self.csi_embed.flatten_into(&mut out);
        for kq in &self.keypoint_queries {
            out.extend_from_slice(kq);
        }
        self.cross_attn.flatten_into(&mut out);
        self.gnn.flatten_into(&mut out);
        self.xyz_head.flatten_into(&mut out);
        self.conf_head.flatten_into(&mut out);
        out
    }

    /// Restore all trainable parameters from a flat slice.
    pub fn unflatten_weights(&mut self, params: &[f32]) -> Result<(), String> {
        let expected = self.param_count();
        if params.len() != expected {
            return Err(format!("expected {expected} params, got {}", params.len()));
        }
        let mut offset = 0;

        // csi_embed
        let (embed, n) = Linear::unflatten_from(&params[offset..],
            self.config.n_subcarriers, self.config.d_model);
        self.csi_embed = embed;
        offset += n;

        // keypoint_queries
        let d = self.config.d_model;
        for kq in &mut self.keypoint_queries {
            kq.copy_from_slice(&params[offset..offset + d]);
            offset += d;
        }

        // cross_attn
        let (ca, n) = CrossAttention::unflatten_from(&params[offset..],
            self.config.d_model, self.cross_attn.n_heads());
        self.cross_attn = ca;
        offset += n;

        // gnn
        let n = self.gnn.unflatten_from(&params[offset..]);
        offset += n;

        // xyz_head
        let (xyz, n) = Linear::unflatten_from(&params[offset..], self.config.d_model, 3);
        self.xyz_head = xyz;
        offset += n;

        // conf_head
        let (conf, n) = Linear::unflatten_from(&params[offset..], self.config.d_model, 1);
        self.conf_head = conf;
        offset += n;

        debug_assert_eq!(offset, expected);
        Ok(())
    }

    /// Total number of trainable parameters.
    pub fn param_count(&self) -> usize {
        self.csi_embed.param_count()
            + self.config.n_keypoints * self.config.d_model  // keypoint queries
            + self.cross_attn.param_count()
            + self.gnn.param_count()
            + self.xyz_head.param_count()
            + self.conf_head.param_count()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_graph_has_17_nodes() {
        assert_eq!(BodyGraph::new().n_nodes(), 17);
    }

    #[test]
    fn body_graph_has_16_edges() {
        let g = BodyGraph::new();
        assert_eq!(g.n_edges(), 16);
        assert_eq!(g.edge_list().len(), 16);
    }

    #[test]
    fn body_graph_adjacency_symmetric() {
        let bg = BodyGraph::new();
        let adj = bg.adjacency_matrix();
        for i in 0..17 { for j in 0..17 {
            assert_eq!(adj[i][j], adj[j][i], "asymmetric at ({i},{j})");
        }}
    }

    #[test]
    fn body_graph_self_loops_and_specific_edges() {
        let bg = BodyGraph::new();
        let adj = bg.adjacency_matrix();
        for i in 0..17 { assert_eq!(adj[i][i], 1.0); }
        assert_eq!(adj[0][1], 1.0);   // nose-left_eye
        assert_eq!(adj[5][6], 1.0);   // l_shoulder-r_shoulder
        assert_eq!(adj[14][16], 1.0); // r_knee-r_ankle
        assert_eq!(adj[0][15], 0.0);  // nose should NOT connect to l_ankle
    }

    #[test]
    fn antenna_graph_node_count() {
        assert_eq!(AntennaGraph::new(3, 3).n_nodes(), 9);
    }

    #[test]
    fn antenna_graph_adjacency() {
        let ag = AntennaGraph::new(2, 2);
        let adj = ag.adjacency_matrix();
        assert_eq!(adj[0][1], 1.0); // share tx=0
        assert_eq!(adj[0][2], 1.0); // share rx=0
        assert_eq!(adj[0][3], 0.0); // share neither
    }

    #[test]
    fn cross_attention_output_shape() {
        let ca = CrossAttention::new(16, 4);
        let out = ca.forward(&vec![vec![0.5; 16]; 5], &vec![vec![0.3; 16]; 3], &vec![vec![0.7; 16]; 3]);
        assert_eq!(out.len(), 5);
        for r in &out { assert_eq!(r.len(), 16); }
    }

    #[test]
    fn cross_attention_single_head_vs_multi() {
        let (q, k, v) = (vec![vec![1.0f32; 8]; 2], vec![vec![0.5; 8]; 3], vec![vec![0.5; 8]; 3]);
        let o1 = CrossAttention::new(8, 1).forward(&q, &k, &v);
        let o2 = CrossAttention::new(8, 2).forward(&q, &k, &v);
        assert_eq!(o1.len(), o2.len());
        assert_eq!(o1[0].len(), o2[0].len());
    }

    #[test]
    fn scaled_dot_product_softmax_sums_to_one() {
        let scores = vec![1.0f32, 2.0, 3.0, 0.5];
        let mut w = vec![0.0f32; 4];
        softmax(&scores, &mut w);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        for &wi in &w { assert!(wi > 0.0); }
        assert!(w[2] > w[0] && w[2] > w[1] && w[2] > w[3]);
    }

    #[test]
    fn gnn_message_passing_shape() {
        let g = BodyGraph::new();
        let out = GraphMessagePassing::new(32, 16, &g).forward(&vec![vec![1.0; 32]; 17]);
        assert_eq!(out.len(), 17);
        for r in &out { assert_eq!(r.len(), 16); }
    }

    #[test]
    fn gnn_preserves_isolated_node() {
        let g = BodyGraph::new();
        let gmp = GraphMessagePassing::new(8, 8, &g);
        let mut feats: Vec<Vec<f32>> = vec![vec![0.0; 8]; 17];
        feats[0] = vec![1.0; 8]; // only nose has signal
        let out = gmp.forward(&feats);
        let ankle_e: f32 = out[15].iter().map(|x| x*x).sum();
        let nose_e: f32 = out[0].iter().map(|x| x*x).sum();
        assert!(nose_e > ankle_e, "nose ({nose_e}) should > ankle ({ankle_e})");
    }

    #[test]
    fn linear_layer_output_size() {
        assert_eq!(Linear::new(10, 5).forward(&vec![1.0; 10]).len(), 5);
    }

    #[test]
    fn linear_layer_zero_weights() {
        let out = Linear::zeros(4, 3).forward(&[1.0, 2.0, 3.0, 4.0]);
        for &v in &out { assert_eq!(v, 0.0); }
    }

    #[test]
    fn linear_layer_set_weights_identity() {
        let mut lin = Linear::zeros(2, 2);
        lin.set_weights(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        let out = lin.forward(&[3.0, 7.0]);
        assert!((out[0] - 3.0).abs() < 1e-6 && (out[1] - 7.0).abs() < 1e-6);
    }

    #[test]
    fn transformer_config_defaults() {
        let c = TransformerConfig::default();
        assert_eq!((c.n_subcarriers, c.n_keypoints, c.d_model, c.n_heads, c.n_gnn_layers),
                   (56, 17, 64, 4, 2));
    }

    #[test]
    fn transformer_forward_output_17_keypoints() {
        let t = CsiToPoseTransformer::new(TransformerConfig {
            n_subcarriers: 16, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 1,
        });
        let out = t.forward(&vec![vec![0.5; 16]; 4]);
        assert_eq!(out.keypoints.len(), 17);
        assert_eq!(out.confidences.len(), 17);
        assert_eq!(out.body_part_features.len(), 17);
    }

    #[test]
    fn transformer_keypoints_are_finite() {
        let t = CsiToPoseTransformer::new(TransformerConfig {
            n_subcarriers: 8, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 2,
        });
        let out = t.forward(&vec![vec![1.0; 8]; 6]);
        for (i, &(x, y, z)) in out.keypoints.iter().enumerate() {
            assert!(x.is_finite() && y.is_finite() && z.is_finite(), "kp {i} not finite");
        }
        for (i, &c) in out.confidences.iter().enumerate() {
            assert!(c.is_finite() && (0.0..=1.0).contains(&c), "conf {i} invalid: {c}");
        }
    }

    #[test]
    fn relu_activation() {
        assert_eq!(relu(-5.0), 0.0);
        assert_eq!(relu(-0.001), 0.0);
        assert_eq!(relu(0.0), 0.0);
        assert_eq!(relu(3.14), 3.14);
        assert_eq!(relu(100.0), 100.0);
    }

    #[test]
    fn sigmoid_bounds() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(100.0) > 0.999);
        assert!(sigmoid(-100.0) < 0.001);
    }

    #[test]
    fn deterministic_rng_and_linear() {
        let (mut r1, mut r2) = (Rng64::new(42), Rng64::new(42));
        for _ in 0..100 { assert_eq!(r1.next_u64(), r2.next_u64()); }
        let inp = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(Linear::with_seed(4, 3, 99).forward(&inp),
                   Linear::with_seed(4, 3, 99).forward(&inp));
    }

    #[test]
    fn body_graph_normalized_adjacency_finite() {
        let norm = BodyGraph::new().normalized_adjacency();
        for i in 0..17 {
            let s: f32 = norm[i].iter().sum();
            assert!(s.is_finite() && s > 0.0, "row {i} sum={s}");
        }
    }

    #[test]
    fn cross_attention_empty_keys() {
        let out = CrossAttention::new(8, 2).forward(
            &vec![vec![1.0; 8]; 3], &vec![], &vec![]);
        assert_eq!(out.len(), 3);
        for r in &out { for &v in r { assert_eq!(v, 0.0); } }
    }

    #[test]
    fn softmax_edge_cases() {
        let mut w1 = vec![0.0f32; 1];
        softmax(&[42.0], &mut w1);
        assert!((w1[0] - 1.0).abs() < 1e-6);

        let mut w3 = vec![0.0f32; 3];
        softmax(&[1000.0, 1001.0, 999.0], &mut w3);
        let sum: f32 = w3.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        for &wi in &w3 { assert!(wi.is_finite()); }
    }

    // ── Weight serialization integration tests ────────────────────────

    #[test]
    fn linear_flatten_unflatten_roundtrip() {
        let lin = Linear::with_seed(8, 4, 42);
        let mut flat = Vec::new();
        lin.flatten_into(&mut flat);
        assert_eq!(flat.len(), lin.param_count());
        let (restored, consumed) = Linear::unflatten_from(&flat, 8, 4);
        assert_eq!(consumed, flat.len());
        let inp = vec![1.0f32; 8];
        assert_eq!(lin.forward(&inp), restored.forward(&inp));
    }

    #[test]
    fn cross_attention_flatten_unflatten_roundtrip() {
        let ca = CrossAttention::new(16, 4);
        let mut flat = Vec::new();
        ca.flatten_into(&mut flat);
        assert_eq!(flat.len(), ca.param_count());
        let (restored, consumed) = CrossAttention::unflatten_from(&flat, 16, 4);
        assert_eq!(consumed, flat.len());
        let q = vec![vec![0.5f32; 16]; 3];
        let k = vec![vec![0.3f32; 16]; 5];
        let v = vec![vec![0.7f32; 16]; 5];
        let orig = ca.forward(&q, &k, &v);
        let rest = restored.forward(&q, &k, &v);
        for (a, b) in orig.iter().zip(rest.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert!((x - y).abs() < 1e-6, "mismatch: {x} vs {y}");
            }
        }
    }

    #[test]
    fn transformer_weight_roundtrip() {
        let config = TransformerConfig {
            n_subcarriers: 16, n_keypoints: 17, d_model: 8, n_heads: 2, n_gnn_layers: 1,
        };
        let t = CsiToPoseTransformer::new(config.clone());
        let weights = t.flatten_weights();
        assert_eq!(weights.len(), t.param_count());

        let mut t2 = CsiToPoseTransformer::new(config);
        t2.unflatten_weights(&weights).expect("unflatten should succeed");

        // Forward pass should produce identical results
        let csi = vec![vec![0.5f32; 16]; 4];
        let out1 = t.forward(&csi);
        let out2 = t2.forward(&csi);
        for (a, b) in out1.keypoints.iter().zip(out2.keypoints.iter()) {
            assert!((a.0 - b.0).abs() < 1e-6);
            assert!((a.1 - b.1).abs() < 1e-6);
            assert!((a.2 - b.2).abs() < 1e-6);
        }
        for (a, b) in out1.confidences.iter().zip(out2.confidences.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn transformer_param_count_positive() {
        let t = CsiToPoseTransformer::new(TransformerConfig::default());
        assert!(t.param_count() > 1000, "expected many params, got {}", t.param_count());
        let flat = t.flatten_weights();
        assert_eq!(flat.len(), t.param_count());
    }

    #[test]
    fn gnn_stack_flatten_unflatten() {
        let bg = BodyGraph::new();
        let gnn = GnnStack::new(8, 8, 2, &bg);
        let mut flat = Vec::new();
        gnn.flatten_into(&mut flat);
        assert_eq!(flat.len(), gnn.param_count());

        let mut gnn2 = GnnStack::new(8, 8, 2, &bg);
        let consumed = gnn2.unflatten_from(&flat);
        assert_eq!(consumed, flat.len());

        let feats = vec![vec![1.0f32; 8]; 17];
        let o1 = gnn.forward(&feats);
        let o2 = gnn2.forward(&feats);
        for (a, b) in o1.iter().zip(o2.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert!((x - y).abs() < 1e-6);
            }
        }
    }
}
