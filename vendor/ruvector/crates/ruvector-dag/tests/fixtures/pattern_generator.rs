//! Pattern Generator for testing

use rand::Rng;

#[derive(Debug, Clone)]
pub struct GeneratedPattern {
    pub vector: Vec<f32>,
    pub quality_score: f64,
    pub category: PatternCategory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternCategory {
    Scan,
    Join,
    Aggregate,
    Sort,
    Vector,
}

pub struct PatternGenerator {
    dim: usize,
    rng: rand::rngs::ThreadRng,
}

impl PatternGenerator {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            rng: rand::thread_rng(),
        }
    }

    pub fn generate(&mut self, category: PatternCategory) -> GeneratedPattern {
        let base = self.category_base_vector(category);
        let vector = self.add_noise(&base, 0.1);
        let quality_score = 0.5 + self.rng.gen::<f64>() * 0.5;

        GeneratedPattern {
            vector,
            quality_score,
            category,
        }
    }

    pub fn generate_batch(&mut self, count: usize) -> Vec<GeneratedPattern> {
        let categories = [
            PatternCategory::Scan,
            PatternCategory::Join,
            PatternCategory::Aggregate,
            PatternCategory::Sort,
            PatternCategory::Vector,
        ];

        (0..count)
            .map(|i| {
                let cat = categories[i % categories.len()];
                self.generate(cat)
            })
            .collect()
    }

    fn category_base_vector(&mut self, category: PatternCategory) -> Vec<f32> {
        // Each category has a distinct base pattern
        let seed = match category {
            PatternCategory::Scan => 1.0,
            PatternCategory::Join => 2.0,
            PatternCategory::Aggregate => 3.0,
            PatternCategory::Sort => 4.0,
            PatternCategory::Vector => 5.0,
        };

        (0..self.dim)
            .map(|i| {
                let x = (i as f32 + seed) / self.dim as f32;
                (x * std::f32::consts::PI * seed).sin()
            })
            .collect()
    }

    fn add_noise(&mut self, base: &[f32], noise_level: f32) -> Vec<f32> {
        base.iter()
            .map(|&v| v + (self.rng.gen::<f32>() - 0.5) * 2.0 * noise_level)
            .collect()
    }
}

impl Default for PatternGenerator {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Generate clustered patterns for testing ReasoningBank
pub fn generate_clustered_patterns(
    clusters: usize,
    patterns_per_cluster: usize,
    dim: usize,
) -> Vec<GeneratedPattern> {
    let mut gen = PatternGenerator::new(dim);
    let mut patterns = Vec::new();

    let categories = [
        PatternCategory::Scan,
        PatternCategory::Join,
        PatternCategory::Aggregate,
        PatternCategory::Sort,
        PatternCategory::Vector,
    ];

    for c in 0..clusters {
        let category = categories[c % categories.len()];
        for _ in 0..patterns_per_cluster {
            patterns.push(gen.generate(category));
        }
    }

    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pattern() {
        let mut gen = PatternGenerator::new(128);
        let pattern = gen.generate(PatternCategory::Scan);

        assert_eq!(pattern.vector.len(), 128);
        assert!(pattern.quality_score >= 0.5 && pattern.quality_score <= 1.0);
        assert_eq!(pattern.category, PatternCategory::Scan);
    }

    #[test]
    fn test_generate_batch() {
        let mut gen = PatternGenerator::new(64);
        let patterns = gen.generate_batch(10);

        assert_eq!(patterns.len(), 10);
        assert!(patterns.iter().all(|p| p.vector.len() == 64));
    }

    #[test]
    fn test_clustered_patterns() {
        let patterns = generate_clustered_patterns(3, 5, 128);
        assert_eq!(patterns.len(), 15);

        // Check that patterns are distributed across categories
        let scan_count = patterns.iter().filter(|p| p.category == PatternCategory::Scan).count();
        assert!(scan_count > 0);
    }

    #[test]
    fn test_category_distinctness() {
        let mut gen = PatternGenerator::new(64);

        let scan = gen.generate(PatternCategory::Scan);
        let join = gen.generate(PatternCategory::Join);

        // Vectors should be different (cosine similarity should be < 1.0)
        let dot: f32 = scan.vector.iter().zip(&join.vector).map(|(a, b)| a * b).sum();
        assert!(dot.abs() < 0.99);
    }
}
