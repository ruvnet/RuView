//! Reasoning Bank: K-means++ clustering for pattern storage

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DagPattern {
    pub id: u64,
    pub vector: Vec<f32>,
    pub quality_score: f32,
    pub usage_count: usize,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ReasoningBankConfig {
    pub num_clusters: usize,
    pub pattern_dim: usize,
    pub max_patterns: usize,
    pub similarity_threshold: f32,
}

impl Default for ReasoningBankConfig {
    fn default() -> Self {
        Self {
            num_clusters: 100,
            pattern_dim: 256,
            max_patterns: 10000,
            similarity_threshold: 0.7,
        }
    }
}

pub struct DagReasoningBank {
    config: ReasoningBankConfig,
    patterns: Vec<DagPattern>,
    centroids: Vec<Vec<f32>>,
    cluster_assignments: Vec<usize>,
    next_id: u64,
}

impl DagReasoningBank {
    pub fn new(config: ReasoningBankConfig) -> Self {
        Self {
            config,
            patterns: Vec::new(),
            centroids: Vec::new(),
            cluster_assignments: Vec::new(),
            next_id: 0,
        }
    }

    /// Store a new pattern
    pub fn store_pattern(&mut self, vector: Vec<f32>, quality: f32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let pattern = DagPattern {
            id,
            vector,
            quality_score: quality,
            usage_count: 0,
            metadata: HashMap::new(),
        };

        self.patterns.push(pattern);

        // Evict if over capacity
        if self.patterns.len() > self.config.max_patterns {
            self.evict_lowest_quality();
        }

        id
    }

    /// Query similar patterns using cosine similarity
    pub fn query_similar(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        let mut similarities: Vec<(u64, f32)> = self
            .patterns
            .iter()
            .map(|p| (p.id, cosine_similarity(&p.vector, query)))
            .filter(|(_, sim)| *sim >= self.config.similarity_threshold)
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        similarities.truncate(k);
        similarities
    }

    /// Run K-means++ clustering
    pub fn recompute_clusters(&mut self) {
        if self.patterns.is_empty() {
            return;
        }

        let k = self.config.num_clusters.min(self.patterns.len());

        // K-means++ initialization
        self.centroids = kmeans_pp_init(&self.patterns, k);

        // K-means iterations
        for _ in 0..10 {
            // Assign points to clusters
            self.cluster_assignments = self
                .patterns
                .iter()
                .map(|p| self.nearest_centroid(&p.vector))
                .collect();

            // Update centroids
            self.update_centroids();
        }
    }

    fn nearest_centroid(&self, point: &[f32]) -> usize {
        self.centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, euclidean_distance(point, c)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn update_centroids(&mut self) {
        let k = self.centroids.len();
        let dim = if !self.centroids.is_empty() {
            self.centroids[0].len()
        } else {
            return;
        };

        // Initialize new centroids
        let mut new_centroids = vec![vec![0.0; dim]; k];
        let mut counts = vec![0usize; k];

        // Sum points in each cluster
        for (pattern, &cluster) in self.patterns.iter().zip(self.cluster_assignments.iter()) {
            if cluster < k {
                for (i, &val) in pattern.vector.iter().enumerate() {
                    new_centroids[cluster][i] += val;
                }
                counts[cluster] += 1;
            }
        }

        // Average to get centroids
        for (centroid, count) in new_centroids.iter_mut().zip(counts.iter()) {
            if *count > 0 {
                for val in centroid.iter_mut() {
                    *val /= *count as f32;
                }
            }
        }

        self.centroids = new_centroids;
    }

    fn evict_lowest_quality(&mut self) {
        // Remove pattern with lowest quality * usage score
        if let Some(min_idx) = self
            .patterns
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let score_a = a.quality_score * (a.usage_count as f32 + 1.0).ln();
                let score_b = b.quality_score * (b.usage_count as f32 + 1.0).ln();
                score_a.partial_cmp(&score_b).unwrap()
            })
            .map(|(i, _)| i)
        {
            self.patterns.remove(min_idx);
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    pub fn cluster_count(&self) -> usize {
        self.centroids.len()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn kmeans_pp_init(patterns: &[DagPattern], k: usize) -> Vec<Vec<f32>> {
    use rand::Rng;

    if patterns.is_empty() || k == 0 {
        return Vec::new();
    }

    let mut rng = rand::thread_rng();
    let mut centroids = Vec::with_capacity(k);
    let _dim = patterns[0].vector.len();

    // Choose first centroid randomly
    let first_idx = rng.gen_range(0..patterns.len());
    centroids.push(patterns[first_idx].vector.clone());

    // Choose remaining centroids using D^2 weighting
    for _ in 1..k {
        let mut distances = Vec::with_capacity(patterns.len());
        let mut total_distance = 0.0f32;

        // Compute minimum distance to existing centroids for each point
        for pattern in patterns {
            let min_dist = centroids
                .iter()
                .map(|c| euclidean_distance(&pattern.vector, c))
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            let squared = min_dist * min_dist;
            distances.push(squared);
            total_distance += squared;
        }

        // Select next centroid with probability proportional to D^2
        if total_distance > 0.0 {
            let mut threshold = rng.gen::<f32>() * total_distance;
            for (idx, &dist) in distances.iter().enumerate() {
                threshold -= dist;
                if threshold <= 0.0 {
                    centroids.push(patterns[idx].vector.clone());
                    break;
                }
            }
        } else {
            // Fallback: choose random point
            let idx = rng.gen_range(0..patterns.len());
            centroids.push(patterns[idx].vector.clone());
        }

        if centroids.len() >= k {
            break;
        }
    }

    centroids
}
