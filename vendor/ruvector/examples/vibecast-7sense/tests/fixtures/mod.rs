//! Test fixtures and factories for 7sense bioacoustics platform
//!
//! This module provides reusable test data generators, builders, and fixtures
//! for testing the six bounded contexts of the 7sense system.

use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ============================================================================
// Shared Kernel Types (mirroring sevensense-core)
// ============================================================================

/// Recording identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecordingId(pub Uuid);

impl RecordingId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RecordingId {
    fn default() -> Self {
        Self::new()
    }
}

/// Segment identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SegmentId(pub Uuid);

impl SegmentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SegmentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Embedding identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmbeddingId(pub Uuid);

impl EmbeddingId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EmbeddingId {
    fn default() -> Self {
        Self::new()
    }
}

/// Cluster identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterId(pub Uuid);

impl ClusterId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ClusterId {
    fn default() -> Self {
        Self::new()
    }
}

/// Vector identifier for HNSW index
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VectorId(pub Uuid);

impl VectorId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for VectorId {
    fn default() -> Self {
        Self::new()
    }
}

/// Motif identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotifId(pub Uuid);

impl MotifId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MotifId {
    fn default() -> Self {
        Self::new()
    }
}

/// Evidence pack identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EvidencePackId(pub Uuid);

impl EvidencePackId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EvidencePackId {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Audio Ingestion Context Fixtures
// ============================================================================

/// Quality grade for audio segments
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityGrade {
    Excellent,
    Good,
    Fair,
    Poor,
    Unusable,
}

impl QualityGrade {
    pub fn from_snr(snr: f32) -> Self {
        if snr > 20.0 {
            QualityGrade::Excellent
        } else if snr > 10.0 {
            QualityGrade::Good
        } else if snr > 5.0 {
            QualityGrade::Fair
        } else if snr > 0.0 {
            QualityGrade::Poor
        } else {
            QualityGrade::Unusable
        }
    }
}

/// Geographic location
#[derive(Clone, Debug, Default)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f32>,
}

/// Audio format specification
#[derive(Clone, Debug)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 32000, // Perch 2.0 requirement
            channels: 1,
            bit_depth: 16,
        }
    }
}

/// Recording aggregate for testing
#[derive(Clone, Debug)]
pub struct Recording {
    pub id: RecordingId,
    pub sensor_id: String,
    pub location: GeoLocation,
    pub start_timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub format: AudioFormat,
    pub habitat: Option<String>,
    pub file_path: String,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            id: RecordingId::new(),
            sensor_id: "SENSOR_001".to_string(),
            location: GeoLocation {
                latitude: 37.7749,
                longitude: -122.4194,
                altitude: Some(10.0),
            },
            start_timestamp: Utc::now(),
            duration_ms: 60000, // 1 minute
            format: AudioFormat::default(),
            habitat: Some("wetland".to_string()),
            file_path: "/data/recordings/test.wav".to_string(),
        }
    }
}

/// Call segment entity for testing
#[derive(Clone, Debug)]
pub struct CallSegment {
    pub id: SegmentId,
    pub recording_id: RecordingId,
    pub start_ms: u64,
    pub end_ms: u64,
    pub snr: f32,
    pub energy: f32,
    pub clipping_score: f32,
    pub overlap_score: f32,
    pub quality_grade: QualityGrade,
}

impl Default for CallSegment {
    fn default() -> Self {
        Self {
            id: SegmentId::new(),
            recording_id: RecordingId::new(),
            start_ms: 0,
            end_ms: 5000, // 5 seconds (Perch window)
            snr: 15.0,
            energy: 0.5,
            clipping_score: 0.0,
            overlap_score: 0.0,
            quality_grade: QualityGrade::Good,
        }
    }
}

/// Factory function to create a test recording
pub fn create_test_recording() -> Recording {
    Recording::default()
}

/// Factory function to create a test recording with custom duration
pub fn create_test_recording_with_duration(duration_ms: u64) -> Recording {
    Recording {
        duration_ms,
        ..Default::default()
    }
}

/// Factory function to create a test segment
pub fn create_test_segment() -> CallSegment {
    CallSegment::default()
}

/// Factory function to create a segment with specific time range
pub fn create_test_segment_at(start_ms: u64, end_ms: u64) -> CallSegment {
    CallSegment {
        start_ms,
        end_ms,
        ..Default::default()
    }
}

/// Factory function to create a segment with specific SNR
pub fn create_test_segment_with_snr(snr: f32) -> CallSegment {
    CallSegment {
        snr,
        quality_grade: QualityGrade::from_snr(snr),
        ..Default::default()
    }
}

/// Factory function to create multiple consecutive segments
pub fn create_segment_sequence(count: usize, gap_ms: u64) -> Vec<CallSegment> {
    let recording_id = RecordingId::new();
    let segment_duration_ms = 5000u64; // 5 seconds per segment

    (0..count)
        .map(|i| {
            let start = i as u64 * (segment_duration_ms + gap_ms);
            CallSegment {
                id: SegmentId::new(),
                recording_id,
                start_ms: start,
                end_ms: start + segment_duration_ms,
                snr: 15.0 + (i as f32 * 0.5), // Varying SNR
                energy: 0.4 + (i as f32 * 0.05),
                clipping_score: 0.0,
                overlap_score: 0.0,
                quality_grade: QualityGrade::Good,
            }
        })
        .collect()
}

// ============================================================================
// Embedding Context Fixtures
// ============================================================================

/// Model version for embeddings
#[derive(Clone, Debug)]
pub struct ModelVersion {
    pub name: String,
    pub version: String,
    pub dimensions: usize,
}

impl Default for ModelVersion {
    fn default() -> Self {
        Self {
            name: "perch".to_string(),
            version: "2.0".to_string(),
            dimensions: 1536,
        }
    }
}

/// Embedding entity for testing
#[derive(Clone, Debug)]
pub struct Embedding {
    pub id: EmbeddingId,
    pub segment_id: SegmentId,
    pub vector: Vec<f32>,
    pub model_version: ModelVersion,
    pub norm: f32,
    pub created_at: DateTime<Utc>,
}

impl Default for Embedding {
    fn default() -> Self {
        let vector = create_random_vector(1536);
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        Self {
            id: EmbeddingId::new(),
            segment_id: SegmentId::new(),
            vector,
            model_version: ModelVersion::default(),
            norm,
            created_at: Utc::now(),
        }
    }
}

/// Factory function to create a test embedding
pub fn create_test_embedding() -> Embedding {
    Embedding::default()
}

/// Factory function to create an embedding with specific vector
pub fn create_test_embedding_with_vector(vector: Vec<f32>) -> Embedding {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    Embedding {
        vector,
        norm,
        ..Default::default()
    }
}

/// Factory function to create a random vector with specified dimensions
pub fn create_random_vector(dims: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Deterministic pseudo-random for reproducibility
    let mut hasher = DefaultHasher::new();
    dims.hash(&mut hasher);
    let seed = hasher.finish();

    (0..dims)
        .map(|i| {
            let x = ((seed.wrapping_mul(i as u64 + 1)) % 10000) as f32 / 10000.0;
            x * 2.0 - 1.0 // Range [-1, 1]
        })
        .collect()
}

/// Factory function to create a deterministic vector based on index
pub fn create_deterministic_vector(dims: usize, index: usize) -> Vec<f32> {
    (0..dims)
        .map(|i| {
            let phase = (index as f32 * 0.1 + i as f32 * 0.01).sin();
            phase
        })
        .collect()
}

/// Factory function to create an L2-normalized vector
pub fn create_normalized_vector(dims: usize) -> Vec<f32> {
    let vector = create_random_vector(dims);
    l2_normalize(&vector)
}

/// L2 normalize a vector
pub fn l2_normalize(vector: &[f32]) -> Vec<f32> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        vector.iter().map(|x| x / norm).collect()
    } else {
        vector.to_vec()
    }
}

/// Factory function to create similar embeddings (clustered)
pub fn create_similar_embeddings(base_vector: &[f32], count: usize, noise: f32) -> Vec<Embedding> {
    // Scale noise by sqrt(dims) to maintain reasonable angular distance
    // In high-dimensional spaces, random perturbations cause large angular changes
    let dims = base_vector.len();
    let scaled_noise = noise / (dims as f32).sqrt();

    (0..count)
        .map(|i| {
            let noisy_vector: Vec<f32> = base_vector
                .iter()
                .enumerate()
                .map(|(j, &v)| {
                    // Use (i+1) to ensure first embedding also has noise
                    let noise_val = (((i + 1) * (j + 1)) as f32 * 0.01).sin() * scaled_noise;
                    v + noise_val
                })
                .collect();
            let normalized = l2_normalize(&noisy_vector);
            create_test_embedding_with_vector(normalized)
        })
        .collect()
}

/// Factory function for batch embeddings
pub fn create_embedding_batch(count: usize) -> Vec<Embedding> {
    (0..count).map(|_| create_test_embedding()).collect()
}

// ============================================================================
// Vector Space Context Fixtures
// ============================================================================

/// HNSW configuration for testing
#[derive(Clone, Debug)]
pub struct HnswConfig {
    pub m: usize,              // Max connections per node per layer
    pub ef_construction: usize, // Build-time search width
    pub ef_search: usize,      // Query-time search width
    pub max_layers: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 100,
            max_layers: 6,
        }
    }
}

/// Distance metric types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    Poincare,
}

/// Indexed vector for HNSW
#[derive(Clone, Debug)]
pub struct IndexedVector {
    pub id: VectorId,
    pub embedding_id: EmbeddingId,
    pub vector: Vec<f32>,
    pub layer: usize,
}

impl Default for IndexedVector {
    fn default() -> Self {
        Self {
            id: VectorId::new(),
            embedding_id: EmbeddingId::new(),
            vector: create_normalized_vector(1536),
            layer: 0,
        }
    }
}

/// Similarity edge between vectors
#[derive(Clone, Debug)]
pub struct SimilarityEdge {
    pub source_id: VectorId,
    pub target_id: VectorId,
    pub distance: f32,
    pub edge_type: String,
}

/// Search result from k-NN query
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub vector_id: VectorId,
    pub distance: f32,
    pub rank: usize,
}

/// Factory function to create indexed vectors for testing
pub fn create_indexed_vectors(count: usize) -> Vec<IndexedVector> {
    (0..count)
        .map(|i| IndexedVector {
            id: VectorId::new(),
            embedding_id: EmbeddingId::new(),
            vector: create_deterministic_vector(1536, i),
            layer: i % 4, // Distribute across layers
        })
        .collect()
}

/// Factory function to create test search results
pub fn create_search_results(count: usize) -> Vec<SearchResult> {
    (0..count)
        .map(|i| SearchResult {
            vector_id: VectorId::new(),
            distance: 0.1 + (i as f32 * 0.05),
            rank: i + 1,
        })
        .collect()
}

/// Compute cosine distance between two vectors
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must have same dimensions");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        1.0 - (dot / (norm_a * norm_b))
    } else {
        1.0
    }
}

/// Compute euclidean distance between two vectors
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must have same dimensions");
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

// ============================================================================
// Analysis Context Fixtures
// ============================================================================

/// Clustering method enumeration
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusteringMethod {
    Hdbscan,
    KMeans,
    Spectral,
}

/// Cluster for testing
#[derive(Clone, Debug)]
pub struct Cluster {
    pub id: ClusterId,
    pub method: ClusteringMethod,
    pub member_ids: Vec<EmbeddingId>,
    pub centroid: Vec<f32>,
    pub cohesion: f32,
    pub separation: f32,
}

impl Default for Cluster {
    fn default() -> Self {
        Self {
            id: ClusterId::new(),
            method: ClusteringMethod::Hdbscan,
            member_ids: vec![EmbeddingId::new(); 10],
            centroid: create_normalized_vector(1536),
            cohesion: 0.8,
            separation: 0.6,
        }
    }
}

/// Cluster assignment
#[derive(Clone, Debug)]
pub struct ClusterAssignment {
    pub segment_id: SegmentId,
    pub cluster_id: ClusterId,
    pub confidence: f32,
    pub distance_to_centroid: f32,
}

/// Motif pattern
#[derive(Clone, Debug)]
pub struct Motif {
    pub id: MotifId,
    pub pattern: Vec<ClusterId>,
    pub occurrence_count: usize,
    pub confidence: f32,
    pub avg_duration_ms: u64,
}

impl Default for Motif {
    fn default() -> Self {
        Self {
            id: MotifId::new(),
            pattern: vec![ClusterId::new(); 3],
            occurrence_count: 5,
            confidence: 0.85,
            avg_duration_ms: 15000,
        }
    }
}

/// Transition matrix for sequence analysis
#[derive(Clone, Debug)]
pub struct TransitionMatrix {
    pub cluster_ids: Vec<ClusterId>,
    pub probabilities: Vec<Vec<f32>>,
    pub observations: Vec<Vec<u32>>,
}

/// Factory function to create a test cluster
pub fn create_test_cluster() -> Cluster {
    Cluster::default()
}

/// Factory function to create a cluster with specific members
pub fn create_test_cluster_with_members(member_count: usize) -> Cluster {
    let members: Vec<EmbeddingId> = (0..member_count).map(|_| EmbeddingId::new()).collect();

    // Create centroid by averaging random vectors
    let vectors: Vec<Vec<f32>> = (0..member_count)
        .map(|i| create_deterministic_vector(1536, i))
        .collect();

    let centroid: Vec<f32> = (0..1536)
        .map(|dim| {
            let sum: f32 = vectors.iter().map(|v| v[dim]).sum();
            sum / member_count as f32
        })
        .collect();

    Cluster {
        member_ids: members,
        centroid: l2_normalize(&centroid),
        ..Default::default()
    }
}

/// Factory function to create clusters
pub fn create_test_clusters(count: usize) -> Vec<Cluster> {
    (0..count)
        .map(|i| {
            let base_vector = create_deterministic_vector(1536, i * 100);
            Cluster {
                id: ClusterId::new(),
                method: ClusteringMethod::Hdbscan,
                member_ids: (0..10).map(|_| EmbeddingId::new()).collect(),
                centroid: l2_normalize(&base_vector),
                cohesion: 0.7 + (i as f32 * 0.02),
                separation: 0.5 + (i as f32 * 0.03),
            }
        })
        .collect()
}

/// Factory function to create a test motif
pub fn create_test_motif() -> Motif {
    Motif::default()
}

/// Factory function to create a transition matrix
pub fn create_test_transition_matrix(cluster_count: usize) -> TransitionMatrix {
    let cluster_ids: Vec<ClusterId> = (0..cluster_count).map(|_| ClusterId::new()).collect();

    // Create random transition probabilities (rows sum to 1.0)
    let probabilities: Vec<Vec<f32>> = (0..cluster_count)
        .map(|i| {
            let raw: Vec<f32> = (0..cluster_count)
                .map(|j| ((i + j) as f32 * 0.1).sin().abs() + 0.1)
                .collect();
            let sum: f32 = raw.iter().sum();
            raw.iter().map(|p| p / sum).collect()
        })
        .collect();

    let observations: Vec<Vec<u32>> = (0..cluster_count)
        .map(|i| (0..cluster_count).map(|j| ((i + j) % 10 + 1) as u32).collect())
        .collect();

    TransitionMatrix {
        cluster_ids,
        probabilities,
        observations,
    }
}

/// Compute entropy rate from transition matrix
pub fn compute_entropy_rate(matrix: &TransitionMatrix) -> f32 {
    let n = matrix.probabilities.len();
    if n == 0 {
        return 0.0;
    }

    // Compute stationary distribution (simplified: uniform)
    let stationary: Vec<f32> = vec![1.0 / n as f32; n];

    // H(X) = -sum_i pi_i * sum_j p_ij * log(p_ij)
    let mut entropy = 0.0;
    for i in 0..n {
        let mut row_entropy = 0.0;
        for j in 0..n {
            let p = matrix.probabilities[i][j];
            if p > 0.0 {
                row_entropy -= p * p.ln();
            }
        }
        entropy += stationary[i] * row_entropy;
    }

    entropy / (2.0_f32).ln() // Convert to bits
}

// ============================================================================
// Interpretation Context Fixtures
// ============================================================================

/// Evidence type for citations
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EvidenceType {
    Neighbor,
    Exemplar,
    Motif,
    Cluster,
}

/// Citation linking interpretation to evidence
#[derive(Clone, Debug)]
pub struct Citation {
    pub claim: String,
    pub evidence_type: EvidenceType,
    pub evidence_id: String,
    pub strength: f32,
}

/// Retrieved neighbor for evidence pack
#[derive(Clone, Debug)]
pub struct RetrievedNeighbor {
    pub segment_id: SegmentId,
    pub distance: f32,
    pub cluster_id: Option<ClusterId>,
    pub relevance: f32,
}

/// Signal quality assessment
#[derive(Clone, Debug, Default)]
pub struct SignalQuality {
    pub snr: f32,
    pub clipping_score: f32,
    pub overlap_score: f32,
    pub quality_grade: Option<QualityGrade>,
}

/// Evidence pack for RAB interpretation
#[derive(Clone, Debug)]
pub struct EvidencePack {
    pub id: EvidencePackId,
    pub query_segment_id: SegmentId,
    pub neighbors: Vec<RetrievedNeighbor>,
    pub exemplars: Vec<EmbeddingId>,
    pub signal_quality: SignalQuality,
    pub created_at: DateTime<Utc>,
}

impl Default for EvidencePack {
    fn default() -> Self {
        Self {
            id: EvidencePackId::new(),
            query_segment_id: SegmentId::new(),
            neighbors: create_test_neighbors(5),
            exemplars: (0..3).map(|_| EmbeddingId::new()).collect(),
            signal_quality: SignalQuality {
                snr: 15.0,
                clipping_score: 0.02,
                overlap_score: 0.1,
                quality_grade: Some(QualityGrade::Good),
            },
            created_at: Utc::now(),
        }
    }
}

/// Interpretation with citations
#[derive(Clone, Debug)]
pub struct Interpretation {
    pub id: Uuid,
    pub evidence_pack_id: EvidencePackId,
    pub statements: Vec<String>,
    pub citations: Vec<Citation>,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
}

/// Factory function to create test neighbors
pub fn create_test_neighbors(count: usize) -> Vec<RetrievedNeighbor> {
    (0..count)
        .map(|i| RetrievedNeighbor {
            segment_id: SegmentId::new(),
            distance: 0.1 + (i as f32 * 0.05),
            cluster_id: if i % 2 == 0 {
                Some(ClusterId::new())
            } else {
                None
            },
            relevance: 1.0 / (1.0 + 0.1 + (i as f32 * 0.05)),
        })
        .collect()
}

/// Factory function to create test citations
pub fn create_test_citations(count: usize) -> Vec<Citation> {
    (0..count)
        .map(|i| Citation {
            claim: format!("Test claim {}", i + 1),
            evidence_type: match i % 4 {
                0 => EvidenceType::Neighbor,
                1 => EvidenceType::Exemplar,
                2 => EvidenceType::Cluster,
                _ => EvidenceType::Motif,
            },
            evidence_id: Uuid::new_v4().to_string(),
            // Spread strength evenly across [0.5, 1.0] range
            strength: 0.5 + (i as f32 / count.max(1) as f32) * 0.5,
        })
        .collect()
}

/// Factory function to create a test evidence pack
pub fn create_test_evidence_pack() -> EvidencePack {
    EvidencePack::default()
}

/// Factory function to create an evidence pack with specific neighbor count
pub fn create_test_evidence_pack_with_neighbors(neighbor_count: usize) -> EvidencePack {
    EvidencePack {
        neighbors: create_test_neighbors(neighbor_count),
        ..Default::default()
    }
}

/// Factory function to create a test interpretation
pub fn create_test_interpretation(evidence_pack_id: EvidencePackId) -> Interpretation {
    let citations = create_test_citations(3);
    Interpretation {
        id: Uuid::new_v4(),
        evidence_pack_id,
        statements: vec![
            "This vocalization exhibits a descending frequency contour.".to_string(),
            "Similar calls were detected in wetland habitat.".to_string(),
            "The call matches cluster A with high confidence.".to_string(),
        ],
        citations,
        confidence: 0.85,
        created_at: Utc::now(),
    }
}

// ============================================================================
// Audio Data Fixtures
// ============================================================================

/// Generate test audio samples at 32kHz
pub fn create_test_audio_samples(duration_ms: u64, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_ms as f64 * sample_rate as f64 / 1000.0) as usize;

    // Generate a simple sine wave with varying frequency (simulating a bird call)
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            // Frequency sweep from 2000Hz to 4000Hz
            let freq = 2000.0 + 2000.0 * (t * 0.5).sin();
            // Amplitude envelope
            let envelope = (std::f32::consts::PI * t * 200.0).sin().abs();
            envelope * (2.0 * std::f32::consts::PI * freq * t).sin()
        })
        .collect()
}

/// Generate test mel spectrogram (500 frames x 128 mel bins)
pub fn create_test_spectrogram() -> Vec<Vec<f32>> {
    let frames = 500;
    let mel_bins = 128;

    (0..frames)
        .map(|frame| {
            // Vary amplitude across frames (simulates signal onset/offset)
            let amplitude = 0.3 + 0.7 * ((frame as f32 / 50.0).sin().abs());
            (0..mel_bins)
                .map(|bin| {
                    // Create a pattern that simulates a frequency sweep
                    let center = 64.0 + 30.0 * (frame as f32 / 100.0).sin();
                    let distance = (bin as f32 - center).abs();
                    amplitude * (-distance / 20.0).exp()
                })
                .collect()
        })
        .collect()
}

/// Create test WAV file bytes (simplified format)
pub fn create_test_wav_bytes(duration_ms: u64) -> Vec<u8> {
    let sample_rate = 32000u32;
    let samples = create_test_audio_samples(duration_ms, sample_rate);

    // Convert to i16 samples
    let i16_samples: Vec<i16> = samples
        .iter()
        .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect();

    // Create minimal WAV header
    let data_size = (i16_samples.len() * 2) as u32;
    let file_size = data_size + 36;

    let mut bytes = Vec::with_capacity(file_size as usize + 8);

    // RIFF header
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&file_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");

    // fmt chunk
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    bytes.extend_from_slice(&1u16.to_le_bytes()); // audio format (PCM)
    bytes.extend_from_slice(&1u16.to_le_bytes()); // num channels
    bytes.extend_from_slice(&sample_rate.to_le_bytes()); // sample rate
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());

    // audio data
    for sample in i16_samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    bytes
}

// ============================================================================
// Test Context and Utilities
// ============================================================================

/// Test context for managing test state
pub struct TestContext {
    pub recordings: HashMap<RecordingId, Recording>,
    pub segments: HashMap<SegmentId, CallSegment>,
    pub embeddings: HashMap<EmbeddingId, Embedding>,
    pub clusters: HashMap<ClusterId, Cluster>,
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            recordings: HashMap::new(),
            segments: HashMap::new(),
            embeddings: HashMap::new(),
            clusters: HashMap::new(),
        }
    }

    pub fn with_recording(mut self, recording: Recording) -> Self {
        self.recordings.insert(recording.id, recording);
        self
    }

    pub fn with_segment(mut self, segment: CallSegment) -> Self {
        self.segments.insert(segment.id, segment);
        self
    }

    pub fn with_embedding(mut self, embedding: Embedding) -> Self {
        self.embeddings.insert(embedding.id, embedding);
        self
    }

    pub fn with_cluster(mut self, cluster: Cluster) -> Self {
        self.clusters.insert(cluster.id, cluster);
        self
    }

    /// Create a fully populated test context
    pub fn fully_populated(num_recordings: usize, segments_per_recording: usize) -> Self {
        let mut ctx = Self::new();

        for _ in 0..num_recordings {
            let recording = create_test_recording();
            let recording_id = recording.id;
            ctx.recordings.insert(recording_id, recording);

            for i in 0..segments_per_recording {
                let start_ms = i as u64 * 5500; // 5s segment + 500ms gap
                let segment = CallSegment {
                    id: SegmentId::new(),
                    recording_id,
                    start_ms,
                    end_ms: start_ms + 5000,
                    ..Default::default()
                };
                let segment_id = segment.id;
                ctx.segments.insert(segment_id, segment);

                let embedding = Embedding {
                    segment_id,
                    ..Default::default()
                };
                ctx.embeddings.insert(embedding.id, embedding);
            }
        }

        // Create some clusters
        for _ in 0..5 {
            let cluster = create_test_cluster();
            ctx.clusters.insert(cluster.id, cluster);
        }

        ctx
    }
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assert that a vector is L2-normalized (within epsilon)
pub fn assert_normalized(vector: &[f32], epsilon: f32) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < epsilon,
        "Vector norm {} is not within {} of 1.0",
        norm,
        epsilon
    );
}

/// Assert that vectors have expected dimensions
pub fn assert_dimensions(vector: &[f32], expected: usize) {
    assert_eq!(
        vector.len(),
        expected,
        "Vector has {} dimensions, expected {}",
        vector.len(),
        expected
    );
}

/// Assert that all embeddings in a batch have valid structure
pub fn assert_valid_embeddings(embeddings: &[Embedding], expected_dims: usize) {
    for (i, emb) in embeddings.iter().enumerate() {
        assert_dimensions(&emb.vector, expected_dims);
        assert!(
            !emb.vector.iter().any(|x| x.is_nan() || x.is_infinite()),
            "Embedding {} contains NaN or Inf values",
            i
        );
    }
}

/// Assert recall meets threshold
pub fn assert_recall_at_k(retrieved: &[VectorId], relevant: &[VectorId], k: usize, min_recall: f32) {
    let retrieved_set: std::collections::HashSet<_> = retrieved.iter().take(k).collect();
    let relevant_set: std::collections::HashSet<_> = relevant.iter().collect();

    let intersection_count = retrieved_set.intersection(&relevant_set).count();
    let recall = intersection_count as f32 / relevant.len().min(k) as f32;

    assert!(
        recall >= min_recall,
        "Recall@{} is {}, expected >= {}",
        k,
        recall,
        min_recall
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_creation() {
        let recording = create_test_recording();
        assert!(recording.duration_ms > 0);
        assert_eq!(recording.format.sample_rate, 32000);
    }

    #[test]
    fn test_random_vector_determinism() {
        let v1 = create_random_vector(1536);
        let v2 = create_random_vector(1536);
        assert_eq!(v1, v2, "Random vectors should be deterministic");
    }

    #[test]
    fn test_l2_normalization() {
        let vector = vec![3.0, 4.0];
        let normalized = l2_normalize(&vector);
        assert_normalized(&normalized, 0.0001);
        assert!((normalized[0] - 0.6).abs() < 0.0001);
        assert!((normalized[1] - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = cosine_distance(&a, &b);
        assert!((dist - 1.0).abs() < 0.0001, "Perpendicular vectors should have distance 1.0");

        let c = vec![1.0, 0.0];
        let same_dist = cosine_distance(&a, &c);
        assert!(same_dist < 0.0001, "Same vectors should have distance 0.0");
    }

    #[test]
    fn test_quality_grade_from_snr() {
        assert_eq!(QualityGrade::from_snr(25.0), QualityGrade::Excellent);
        assert_eq!(QualityGrade::from_snr(15.0), QualityGrade::Good);
        assert_eq!(QualityGrade::from_snr(7.0), QualityGrade::Fair);
        assert_eq!(QualityGrade::from_snr(2.0), QualityGrade::Poor);
        assert_eq!(QualityGrade::from_snr(-5.0), QualityGrade::Unusable);
    }

    #[test]
    fn test_test_context_builder() {
        let ctx = TestContext::fully_populated(2, 3);
        assert_eq!(ctx.recordings.len(), 2);
        assert_eq!(ctx.segments.len(), 6);
        assert_eq!(ctx.embeddings.len(), 6);
        assert_eq!(ctx.clusters.len(), 5);
    }

    #[test]
    fn test_entropy_rate_computation() {
        let matrix = create_test_transition_matrix(4);
        let entropy = compute_entropy_rate(&matrix);
        assert!(entropy >= 0.0, "Entropy should be non-negative");
        assert!(entropy < 10.0, "Entropy should be reasonable");
    }
}
