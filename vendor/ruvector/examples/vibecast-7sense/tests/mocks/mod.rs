//! Mock implementations for testing 7sense bounded contexts
//!
//! This module provides mock implementations of repositories and services
//! for isolated unit and integration testing.

use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Import fixtures
use crate::fixtures::*;

// ============================================================================
// Error Types
// ============================================================================

/// Repository error type for mock implementations
#[derive(Debug, Clone)]
pub enum MockRepositoryError {
    NotFound(String),
    AlreadyExists(String),
    ValidationFailed(String),
    StorageError(String),
}

impl std::fmt::Display for MockRepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::AlreadyExists(msg) => write!(f, "Already exists: {}", msg),
            Self::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for MockRepositoryError {}

/// Result type alias for mock operations
pub type MockResult<T> = Result<T, MockRepositoryError>;

// ============================================================================
// Audio Ingestion Context Mocks
// ============================================================================

/// Mock repository for Recording aggregate
#[derive(Debug, Default)]
pub struct MockRecordingRepository {
    recordings: RwLock<HashMap<RecordingId, Recording>>,
    segments_by_recording: RwLock<HashMap<RecordingId, Vec<SegmentId>>>,
}

impl MockRecordingRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-populated data
    pub fn with_recordings(recordings: Vec<Recording>) -> Self {
        let repo = Self::new();
        for recording in recordings {
            repo.save(recording).unwrap();
        }
        repo
    }

    pub fn save(&self, recording: Recording) -> MockResult<()> {
        let mut store = self.recordings.write().unwrap();
        store.insert(recording.id, recording);
        Ok(())
    }

    pub fn find_by_id(&self, id: &RecordingId) -> MockResult<Option<Recording>> {
        let store = self.recordings.read().unwrap();
        Ok(store.get(id).cloned())
    }

    pub fn find_all(&self) -> MockResult<Vec<Recording>> {
        let store = self.recordings.read().unwrap();
        Ok(store.values().cloned().collect())
    }

    pub fn delete(&self, id: &RecordingId) -> MockResult<()> {
        let mut store = self.recordings.write().unwrap();
        store.remove(id);
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.recordings.read().unwrap().len()
    }

    pub fn find_by_sensor_id(&self, sensor_id: &str) -> MockResult<Vec<Recording>> {
        let store = self.recordings.read().unwrap();
        let results: Vec<Recording> = store
            .values()
            .filter(|r| r.sensor_id == sensor_id)
            .cloned()
            .collect();
        Ok(results)
    }

    pub fn find_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> MockResult<Vec<Recording>> {
        let store = self.recordings.read().unwrap();
        let results: Vec<Recording> = store
            .values()
            .filter(|r| r.start_timestamp >= start && r.start_timestamp <= end)
            .cloned()
            .collect();
        Ok(results)
    }

    /// Link segments to a recording
    pub fn add_segment_link(&self, recording_id: RecordingId, segment_id: SegmentId) {
        let mut links = self.segments_by_recording.write().unwrap();
        links
            .entry(recording_id)
            .or_insert_with(Vec::new)
            .push(segment_id);
    }

    /// Get segment IDs for a recording
    pub fn get_segment_ids(&self, recording_id: &RecordingId) -> Vec<SegmentId> {
        let links = self.segments_by_recording.read().unwrap();
        links.get(recording_id).cloned().unwrap_or_default()
    }
}

/// Mock repository for CallSegment entity
#[derive(Debug, Default)]
pub struct MockSegmentRepository {
    segments: RwLock<HashMap<SegmentId, CallSegment>>,
}

impl MockSegmentRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-populated data
    pub fn with_segments(segments: Vec<CallSegment>) -> Self {
        let repo = Self::new();
        for segment in segments {
            repo.save(segment).unwrap();
        }
        repo
    }

    pub fn save(&self, segment: CallSegment) -> MockResult<()> {
        let mut store = self.segments.write().unwrap();
        store.insert(segment.id, segment);
        Ok(())
    }

    pub fn find_by_id(&self, id: &SegmentId) -> MockResult<Option<CallSegment>> {
        let store = self.segments.read().unwrap();
        Ok(store.get(id).cloned())
    }

    pub fn find_by_recording(&self, recording_id: &RecordingId) -> MockResult<Vec<CallSegment>> {
        let store = self.segments.read().unwrap();
        let results: Vec<CallSegment> = store
            .values()
            .filter(|s| s.recording_id == *recording_id)
            .cloned()
            .collect();
        Ok(results)
    }

    pub fn find_by_time_range(
        &self,
        recording_id: &RecordingId,
        start_ms: u64,
        end_ms: u64,
    ) -> MockResult<Vec<CallSegment>> {
        let store = self.segments.read().unwrap();
        let results: Vec<CallSegment> = store
            .values()
            .filter(|s| {
                s.recording_id == *recording_id && s.start_ms >= start_ms && s.end_ms <= end_ms
            })
            .cloned()
            .collect();
        Ok(results)
    }

    pub fn delete(&self, id: &SegmentId) -> MockResult<()> {
        let mut store = self.segments.write().unwrap();
        store.remove(id);
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.segments.read().unwrap().len()
    }

    pub fn find_by_quality(&self, min_grade: QualityGrade) -> MockResult<Vec<CallSegment>> {
        let store = self.segments.read().unwrap();
        let results: Vec<CallSegment> = store
            .values()
            .filter(|s| {
                match (&s.quality_grade, &min_grade) {
                    (QualityGrade::Excellent, _) => true,
                    (QualityGrade::Good, QualityGrade::Good)
                    | (QualityGrade::Good, QualityGrade::Fair)
                    | (QualityGrade::Good, QualityGrade::Poor)
                    | (QualityGrade::Good, QualityGrade::Unusable) => true,
                    (QualityGrade::Fair, QualityGrade::Fair)
                    | (QualityGrade::Fair, QualityGrade::Poor)
                    | (QualityGrade::Fair, QualityGrade::Unusable) => true,
                    (QualityGrade::Poor, QualityGrade::Poor)
                    | (QualityGrade::Poor, QualityGrade::Unusable) => true,
                    (QualityGrade::Unusable, QualityGrade::Unusable) => true,
                    _ => false,
                }
            })
            .cloned()
            .collect();
        Ok(results)
    }
}

// ============================================================================
// Embedding Context Mocks
// ============================================================================

/// Mock repository for Embedding entity
#[derive(Debug, Default)]
pub struct MockEmbeddingRepository {
    embeddings: RwLock<HashMap<EmbeddingId, Embedding>>,
    by_segment: RwLock<HashMap<SegmentId, EmbeddingId>>,
}

impl MockEmbeddingRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-populated data
    pub fn with_embeddings(embeddings: Vec<Embedding>) -> Self {
        let repo = Self::new();
        for embedding in embeddings {
            repo.save(embedding).unwrap();
        }
        repo
    }

    pub fn save(&self, embedding: Embedding) -> MockResult<()> {
        let segment_id = embedding.segment_id;
        let embedding_id = embedding.id;

        let mut store = self.embeddings.write().unwrap();
        store.insert(embedding_id, embedding);

        let mut by_segment = self.by_segment.write().unwrap();
        by_segment.insert(segment_id, embedding_id);

        Ok(())
    }

    pub fn find_by_id(&self, id: &EmbeddingId) -> MockResult<Option<Embedding>> {
        let store = self.embeddings.read().unwrap();
        Ok(store.get(id).cloned())
    }

    pub fn find_by_segment(&self, segment_id: &SegmentId) -> MockResult<Option<Embedding>> {
        let by_segment = self.by_segment.read().unwrap();
        let embedding_id = by_segment.get(segment_id);

        match embedding_id {
            Some(id) => {
                let store = self.embeddings.read().unwrap();
                Ok(store.get(id).cloned())
            }
            None => Ok(None),
        }
    }

    pub fn find_by_model(&self, model_name: &str) -> MockResult<Vec<Embedding>> {
        let store = self.embeddings.read().unwrap();
        let results: Vec<Embedding> = store
            .values()
            .filter(|e| e.model_version.name == model_name)
            .cloned()
            .collect();
        Ok(results)
    }

    pub fn batch_save(&self, embeddings: Vec<Embedding>) -> MockResult<()> {
        for embedding in embeddings {
            self.save(embedding)?;
        }
        Ok(())
    }

    pub fn delete(&self, id: &EmbeddingId) -> MockResult<()> {
        let mut store = self.embeddings.write().unwrap();
        if let Some(embedding) = store.remove(id) {
            let mut by_segment = self.by_segment.write().unwrap();
            by_segment.remove(&embedding.segment_id);
        }
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.embeddings.read().unwrap().len()
    }

    pub fn get_all_vectors(&self) -> Vec<(EmbeddingId, Vec<f32>)> {
        let store = self.embeddings.read().unwrap();
        store
            .iter()
            .map(|(id, emb)| (*id, emb.vector.clone()))
            .collect()
    }
}

/// Mock embedding model adapter (simulates Perch 2.0)
#[derive(Debug)]
pub struct MockEmbeddingModelAdapter {
    dimensions: usize,
    model_name: String,
    model_version: String,
    _latency_ms: u64,
    fail_rate: f32,
}

impl Default for MockEmbeddingModelAdapter {
    fn default() -> Self {
        Self {
            dimensions: 1536,
            model_name: "perch".to_string(),
            model_version: "2.0".to_string(),
            _latency_ms: 10,
            fail_rate: 0.0,
        }
    }
}

impl MockEmbeddingModelAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create adapter that fails at specified rate
    pub fn with_failure_rate(mut self, rate: f32) -> Self {
        self.fail_rate = rate;
        self
    }

    /// Create adapter with custom dimensions
    pub fn with_dimensions(mut self, dims: usize) -> Self {
        self.dimensions = dims;
        self
    }

    pub fn model_version(&self) -> ModelVersion {
        ModelVersion {
            name: self.model_name.clone(),
            version: self.model_version.clone(),
            dimensions: self.dimensions,
        }
    }

    /// Generate embedding from audio samples
    pub fn embed(&self, audio_samples: &[f32]) -> MockResult<Vec<f32>> {
        // Simulate random failures
        if self.fail_rate > 0.0 {
            let random_val = (audio_samples.len() as f32 * 0.00001) % 1.0;
            if random_val < self.fail_rate {
                return Err(MockRepositoryError::StorageError(
                    "Simulated embedding failure".to_string(),
                ));
            }
        }

        // Generate deterministic embedding based on audio content
        let mut embedding = vec![0.0f32; self.dimensions];

        // Hash-like transformation that is sensitive to actual values, not just scale
        for (i, chunk) in audio_samples.chunks(100).enumerate() {
            let sum: f32 = chunk.iter().sum();
            let sum_squares: f32 = chunk.iter().map(|x| x * x).sum();
            let mean = sum / chunk.len() as f32;
            let variance = sum_squares / chunk.len() as f32 - mean * mean;

            let dim_idx = i % self.dimensions;
            let dim_idx2 = (i + 1) % self.dimensions;
            let dim_idx3 = (i + 2) % self.dimensions;

            // Use different statistics to create distinct embeddings
            embedding[dim_idx] += sum * 0.001;
            embedding[dim_idx2] += variance * 0.01;  // Variance-based component
            embedding[dim_idx3] += (sum.abs() + 0.1).ln() * 0.1;  // Log-scale component
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Ok(embedding)
    }

    /// Batch embedding
    pub fn embed_batch(&self, audio_batch: &[Vec<f32>]) -> MockResult<Vec<Vec<f32>>> {
        audio_batch.iter().map(|audio| self.embed(audio)).collect()
    }
}

// ============================================================================
// Vector Space Context Mocks
// ============================================================================

/// Mock HNSW vector index
#[derive(Debug)]
pub struct MockVectorIndex {
    vectors: RwLock<HashMap<VectorId, IndexedVector>>,
    config: HnswConfig,
    distance_metric: DistanceMetric,
}

impl Default for MockVectorIndex {
    fn default() -> Self {
        Self {
            vectors: RwLock::new(HashMap::new()),
            config: HnswConfig::default(),
            distance_metric: DistanceMetric::Cosine,
        }
    }
}

impl MockVectorIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: HnswConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Insert a vector into the index
    pub fn insert(&self, embedding_id: EmbeddingId, vector: Vec<f32>) -> MockResult<VectorId> {
        let vector_id = VectorId::new();
        let indexed = IndexedVector {
            id: vector_id,
            embedding_id,
            vector,
            layer: self.assign_layer(),
        };

        let mut store = self.vectors.write().unwrap();
        store.insert(vector_id, indexed);

        Ok(vector_id)
    }

    /// Batch insert vectors
    pub fn insert_batch(
        &self,
        embeddings: Vec<(EmbeddingId, Vec<f32>)>,
    ) -> MockResult<Vec<VectorId>> {
        embeddings
            .into_iter()
            .map(|(emb_id, vec)| self.insert(emb_id, vec))
            .collect()
    }

    /// k-NN search
    pub fn search(&self, query: &[f32], k: usize) -> MockResult<Vec<SearchResult>> {
        let store = self.vectors.read().unwrap();

        let mut results: Vec<(VectorId, f32)> = store
            .iter()
            .map(|(id, indexed)| {
                let distance = self.compute_distance(query, &indexed.vector);
                (*id, distance)
            })
            .collect();

        // Sort by distance
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        let search_results: Vec<SearchResult> = results
            .into_iter()
            .take(k)
            .enumerate()
            .map(|(rank, (vector_id, distance))| SearchResult {
                vector_id,
                distance,
                rank: rank + 1,
            })
            .collect();

        Ok(search_results)
    }

    /// Get vector by ID
    pub fn get(&self, id: &VectorId) -> MockResult<Option<IndexedVector>> {
        let store = self.vectors.read().unwrap();
        Ok(store.get(id).cloned())
    }

    /// Remove vector
    pub fn remove(&self, id: &VectorId) -> MockResult<()> {
        let mut store = self.vectors.write().unwrap();
        store.remove(id);
        Ok(())
    }

    /// Get all neighbors of a vector
    pub fn get_neighbors(&self, id: &VectorId, k: usize) -> MockResult<Vec<SearchResult>> {
        let store = self.vectors.read().unwrap();

        let query_vector = store
            .get(id)
            .ok_or_else(|| MockRepositoryError::NotFound(id.0.to_string()))?;

        let mut results: Vec<(VectorId, f32)> = store
            .iter()
            .filter(|(vid, _)| *vid != id) // Exclude self
            .map(|(vid, indexed)| {
                let distance = self.compute_distance(&query_vector.vector, &indexed.vector);
                (*vid, distance)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let search_results: Vec<SearchResult> = results
            .into_iter()
            .take(k)
            .enumerate()
            .map(|(rank, (vector_id, distance))| SearchResult {
                vector_id,
                distance,
                rank: rank + 1,
            })
            .collect();

        Ok(search_results)
    }

    /// Count vectors in index
    pub fn count(&self) -> usize {
        self.vectors.read().unwrap().len()
    }

    /// Save index to bytes (mock persistence)
    pub fn save_to_bytes(&self) -> MockResult<Vec<u8>> {
        // Simplified serialization
        let store = self.vectors.read().unwrap();
        let count = store.len() as u64;
        let mut bytes = count.to_le_bytes().to_vec();

        for (id, indexed) in store.iter() {
            bytes.extend_from_slice(&id.0.as_bytes()[..]);
            bytes.extend_from_slice(&indexed.embedding_id.0.as_bytes()[..]);
            bytes.extend_from_slice(&(indexed.vector.len() as u32).to_le_bytes());
            for v in &indexed.vector {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }

        Ok(bytes)
    }

    /// Load index from bytes
    pub fn load_from_bytes(bytes: &[u8]) -> MockResult<Self> {
        if bytes.len() < 8 {
            return Err(MockRepositoryError::StorageError(
                "Invalid index data".to_string(),
            ));
        }

        // This is a simplified mock - real implementation would parse properly
        Ok(Self::default())
    }

    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.distance_metric {
            DistanceMetric::Cosine => cosine_distance(a, b),
            DistanceMetric::Euclidean => euclidean_distance(a, b),
            DistanceMetric::Poincare => {
                // Simplified Poincare distance
                let eucl = euclidean_distance(a, b);
                2.0 * (eucl / 2.0).atanh()
            }
        }
    }

    fn assign_layer(&self) -> usize {
        // Random layer assignment following HNSW distribution
        let store = self.vectors.read().unwrap();
        let count = store.len();
        if count == 0 {
            return 0;
        }
        (count.trailing_zeros() as usize).min(self.config.max_layers - 1)
    }
}

// ============================================================================
// Analysis Context Mocks
// ============================================================================

/// Mock clustering service
#[derive(Debug, Default)]
pub struct MockClusteringService {
    min_cluster_size: usize,
    _min_samples: usize,
}

impl MockClusteringService {
    pub fn new() -> Self {
        Self {
            min_cluster_size: 5,
            _min_samples: 3,
        }
    }

    pub fn with_params(min_cluster_size: usize, min_samples: usize) -> Self {
        Self {
            min_cluster_size,
            _min_samples: min_samples,
        }
    }

    /// Run mock HDBSCAN clustering
    pub fn cluster_hdbscan(&self, embeddings: &[Embedding]) -> MockResult<Vec<Cluster>> {
        if embeddings.len() < self.min_cluster_size {
            return Ok(vec![]); // No clusters if too few points
        }

        // Simple mock clustering: group by vector similarity
        let mut clusters: Vec<Cluster> = Vec::new();
        let mut assigned: Vec<bool> = vec![false; embeddings.len()];

        for (i, embedding) in embeddings.iter().enumerate() {
            if assigned[i] {
                continue;
            }

            let mut cluster_members = vec![embedding.id];
            assigned[i] = true;

            // Find similar embeddings
            for (j, other) in embeddings.iter().enumerate() {
                if i != j && !assigned[j] {
                    let dist = cosine_distance(&embedding.vector, &other.vector);
                    if dist < 0.3 {
                        // Threshold for similarity
                        cluster_members.push(other.id);
                        assigned[j] = true;
                    }
                }
            }

            if cluster_members.len() >= self.min_cluster_size {
                // Compute centroid
                let dims = embedding.vector.len();
                let mut centroid = vec![0.0f32; dims];
                for member_id in &cluster_members {
                    if let Some(emb) = embeddings.iter().find(|e| e.id == *member_id) {
                        for (k, v) in emb.vector.iter().enumerate() {
                            centroid[k] += v;
                        }
                    }
                }
                let n = cluster_members.len() as f32;
                for v in &mut centroid {
                    *v /= n;
                }
                let centroid = l2_normalize(&centroid);

                clusters.push(Cluster {
                    id: ClusterId::new(),
                    method: ClusteringMethod::Hdbscan,
                    member_ids: cluster_members,
                    centroid,
                    cohesion: 0.8,
                    separation: 0.6,
                });
            }
        }

        Ok(clusters)
    }

    /// Assign embedding to nearest cluster
    pub fn assign_to_cluster(
        &self,
        embedding: &Embedding,
        clusters: &[Cluster],
    ) -> MockResult<Option<ClusterAssignment>> {
        if clusters.is_empty() {
            return Ok(None);
        }

        let mut best_cluster: Option<(ClusterId, f32)> = None;

        for cluster in clusters {
            let distance = cosine_distance(&embedding.vector, &cluster.centroid);
            match &best_cluster {
                None => best_cluster = Some((cluster.id, distance)),
                Some((_, best_dist)) if distance < *best_dist => {
                    best_cluster = Some((cluster.id, distance))
                }
                _ => {}
            }
        }

        Ok(best_cluster.map(|(cluster_id, distance)| ClusterAssignment {
            segment_id: embedding.segment_id,
            cluster_id,
            confidence: 1.0 / (1.0 + distance),
            distance_to_centroid: distance,
        }))
    }
}

/// Mock motif detection service
#[derive(Debug, Default)]
pub struct MockMotifDetectionService {
    min_support: usize,
    max_length: usize,
}

impl MockMotifDetectionService {
    pub fn new() -> Self {
        Self {
            min_support: 3,
            max_length: 5,
        }
    }

    /// Detect motifs in cluster sequences
    pub fn detect_motifs(&self, sequences: &[Vec<ClusterId>]) -> MockResult<Vec<Motif>> {
        if sequences.is_empty() {
            return Ok(vec![]);
        }

        // Count n-gram patterns
        let mut pattern_counts: HashMap<Vec<ClusterId>, usize> = HashMap::new();

        for sequence in sequences {
            for len in 2..=self.max_length.min(sequence.len()) {
                for window in sequence.windows(len) {
                    let pattern = window.to_vec();
                    *pattern_counts.entry(pattern).or_insert(0) += 1;
                }
            }
        }

        // Filter by minimum support
        let motifs: Vec<Motif> = pattern_counts
            .into_iter()
            .filter(|(_, count)| *count >= self.min_support)
            .map(|(pattern, count)| Motif {
                id: MotifId::new(),
                pattern,
                occurrence_count: count,
                confidence: count as f32 / sequences.len() as f32,
                avg_duration_ms: 5000 * count as u64,
            })
            .collect();

        Ok(motifs)
    }
}

// ============================================================================
// Interpretation Context Mocks
// ============================================================================

/// Mock evidence pack builder
#[derive(Debug, Default)]
pub struct MockEvidencePackBuilder {
    neighbor_count: usize,
    exemplar_count: usize,
}

impl MockEvidencePackBuilder {
    pub fn new() -> Self {
        Self {
            neighbor_count: 10,
            exemplar_count: 5,
        }
    }

    pub fn with_neighbor_count(mut self, count: usize) -> Self {
        self.neighbor_count = count;
        self
    }

    pub fn with_exemplar_count(mut self, count: usize) -> Self {
        self.exemplar_count = count;
        self
    }

    /// Build evidence pack from segment and search results
    pub fn build(
        &self,
        segment: &CallSegment,
        search_results: &[SearchResult],
        clusters: &[Cluster],
    ) -> MockResult<EvidencePack> {
        let neighbors: Vec<RetrievedNeighbor> = search_results
            .iter()
            .take(self.neighbor_count)
            .map(|result| RetrievedNeighbor {
                segment_id: SegmentId::new(), // Would map from vector_id in real impl
                distance: result.distance,
                cluster_id: clusters.first().map(|c| c.id),
                relevance: 1.0 / (1.0 + result.distance),
            })
            .collect();

        let exemplars: Vec<EmbeddingId> = clusters
            .iter()
            .flat_map(|c| c.member_ids.iter().take(1).cloned())
            .take(self.exemplar_count)
            .collect();

        Ok(EvidencePack {
            id: EvidencePackId::new(),
            query_segment_id: segment.id,
            neighbors,
            exemplars,
            signal_quality: SignalQuality {
                snr: segment.snr,
                clipping_score: segment.clipping_score,
                overlap_score: segment.overlap_score,
                quality_grade: Some(segment.quality_grade),
            },
            created_at: Utc::now(),
        })
    }
}

/// Mock interpretation generator
#[derive(Debug, Default)]
pub struct MockInterpretationGenerator {
    _min_confidence: f32,
}

impl MockInterpretationGenerator {
    pub fn new() -> Self {
        Self {
            _min_confidence: 0.7,
        }
    }

    /// Generate interpretation from evidence pack
    pub fn generate(&self, evidence_pack: &EvidencePack) -> MockResult<Interpretation> {
        let mut citations = Vec::new();
        let mut statements = Vec::new();

        // Generate statements and citations based on evidence
        if !evidence_pack.neighbors.is_empty() {
            let avg_distance: f32 = evidence_pack.neighbors.iter().map(|n| n.distance).sum::<f32>()
                / evidence_pack.neighbors.len() as f32;

            statements.push(format!(
                "This call segment has {} acoustically similar neighbors (avg distance: {:.3}).",
                evidence_pack.neighbors.len(),
                avg_distance
            ));

            for (_i, neighbor) in evidence_pack.neighbors.iter().take(3).enumerate() {
                citations.push(Citation {
                    claim: statements[0].clone(),
                    evidence_type: EvidenceType::Neighbor,
                    evidence_id: neighbor.segment_id.0.to_string(),
                    strength: neighbor.relevance,
                });
            }
        }

        if !evidence_pack.exemplars.is_empty() {
            statements.push(format!(
                "The segment aligns with {} cluster exemplars.",
                evidence_pack.exemplars.len()
            ));

            for exemplar_id in evidence_pack.exemplars.iter().take(2) {
                citations.push(Citation {
                    claim: statements.last().unwrap().clone(),
                    evidence_type: EvidenceType::Exemplar,
                    evidence_id: exemplar_id.0.to_string(),
                    strength: 0.8,
                });
            }
        }

        // Compute overall confidence
        let confidence = if citations.is_empty() {
            0.0
        } else {
            citations.iter().map(|c| c.strength).sum::<f32>() / citations.len() as f32
        };

        Ok(Interpretation {
            id: Uuid::new_v4(),
            evidence_pack_id: evidence_pack.id,
            statements,
            citations,
            confidence,
            created_at: Utc::now(),
        })
    }

    /// Validate that all claims have citations
    pub fn validate_citations(&self, interpretation: &Interpretation) -> bool {
        for statement in &interpretation.statements {
            let has_citation = interpretation
                .citations
                .iter()
                .any(|c| c.claim == *statement);
            if !has_citation {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// API Context Mocks
// ============================================================================

/// Mock HTTP response
#[derive(Debug)]
pub struct MockResponse {
    pub status: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

/// Mock API client for testing
#[derive(Debug, Default)]
pub struct MockApiClient {
    responses: RwLock<Vec<MockResponse>>,
    request_count: RwLock<usize>,
}

impl MockApiClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a response for the next request
    pub fn queue_response(&self, status: u16, body: &str) {
        let mut responses = self.responses.write().unwrap();
        responses.push(MockResponse {
            status,
            body: body.to_string(),
            headers: HashMap::new(),
        });
    }

    /// Simulate GET request
    pub fn get(&self, _path: &str) -> MockResult<MockResponse> {
        let mut count = self.request_count.write().unwrap();
        *count += 1;

        let mut responses = self.responses.write().unwrap();
        if responses.is_empty() {
            Ok(MockResponse {
                status: 200,
                body: "{}".to_string(),
                headers: HashMap::new(),
            })
        } else {
            Ok(responses.remove(0))
        }
    }

    /// Simulate POST request
    pub fn post(&self, _path: &str, _body: &str) -> MockResult<MockResponse> {
        let mut count = self.request_count.write().unwrap();
        *count += 1;

        let mut responses = self.responses.write().unwrap();
        if responses.is_empty() {
            Ok(MockResponse {
                status: 201,
                body: "{}".to_string(),
                headers: HashMap::new(),
            })
        } else {
            Ok(responses.remove(0))
        }
    }

    /// Get request count
    pub fn request_count(&self) -> usize {
        *self.request_count.read().unwrap()
    }
}

/// Mock rate limiter
#[derive(Debug)]
pub struct MockRateLimiter {
    requests_per_second: usize,
    request_times: RwLock<Vec<std::time::Instant>>,
}

impl MockRateLimiter {
    pub fn new(requests_per_second: usize) -> Self {
        Self {
            requests_per_second,
            request_times: RwLock::new(Vec::new()),
        }
    }

    /// Check if request is allowed
    pub fn check(&self) -> bool {
        let now = std::time::Instant::now();
        let mut times = self.request_times.write().unwrap();

        // Remove old entries (older than 1 second)
        times.retain(|t| now.duration_since(*t).as_secs() < 1);

        if times.len() < self.requests_per_second {
            times.push(now);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_recording_repository() {
        let repo = MockRecordingRepository::new();
        let recording = create_test_recording();
        let id = recording.id;

        repo.save(recording.clone()).unwrap();
        assert_eq!(repo.count(), 1);

        let found = repo.find_by_id(&id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);
    }

    #[test]
    fn test_mock_embedding_model() {
        let model = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        let embedding = model.embed(&audio).unwrap();
        assert_eq!(embedding.len(), 1536);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_mock_vector_index() {
        let index = MockVectorIndex::new();

        // Insert vectors
        for i in 0..10 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        assert_eq!(index.count(), 10);

        // Search
        let query = create_deterministic_vector(1536, 0);
        let results = index.search(&query, 5).unwrap();

        assert_eq!(results.len(), 5);
        assert!(results[0].distance < results[4].distance);
    }

    #[test]
    fn test_mock_clustering_service() {
        let service = MockClusteringService::new();

        // Create embeddings with two distinct groups
        let mut embeddings = Vec::new();

        // Group 1: similar vectors
        let base1 = create_deterministic_vector(1536, 0);
        for _ in 0..10 {
            let noisy: Vec<f32> = base1.iter().map(|v| v + 0.01).collect();
            embeddings.push(create_test_embedding_with_vector(l2_normalize(&noisy)));
        }

        // Group 2: different vectors
        let base2 = create_deterministic_vector(1536, 100);
        for _ in 0..10 {
            let noisy: Vec<f32> = base2.iter().map(|v| v + 0.01).collect();
            embeddings.push(create_test_embedding_with_vector(l2_normalize(&noisy)));
        }

        let clusters = service.cluster_hdbscan(&embeddings).unwrap();
        assert!(!clusters.is_empty());
    }

    #[test]
    fn test_mock_interpretation_generator() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        assert!(!interpretation.statements.is_empty());
        assert!(!interpretation.citations.is_empty());
        assert!(generator.validate_citations(&interpretation));
    }
}
