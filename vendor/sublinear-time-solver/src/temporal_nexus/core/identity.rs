//! Identity Continuity Tracking for Temporal Consciousness
//!
//! This module tracks and preserves identity continuity across temporal boundaries
//! to ensure consciousness coherence. It monitors identity state, detects breaks
//! in continuity, and provides mechanisms for identity preservation.

use std::collections::{HashMap, VecDeque};
use super::{TemporalResult, TemporalError, TscTimestamp};

/// Metrics for identity continuity analysis
#[derive(Debug, Clone, Default)]
pub struct ContinuityMetrics {
    pub continuity_score: f64,
    pub identity_stability: f64,
    pub continuity_breaks: u64,
    pub average_gap_duration_ns: f64,
    pub max_gap_duration_ns: u64,
    pub identity_coherence: f64,
    pub temporal_consistency: f64,
    pub preservation_efficiency: f64,
}

/// Identity state snapshot at a specific time
#[derive(Debug, Clone)]
struct IdentitySnapshot {
    timestamp: TscTimestamp,
    state_hash: u64,
    feature_vector: Vec<f64>,
    coherence_score: f64,
    stability_metric: f64,
    memory_fingerprint: Vec<u8>,
}

impl IdentitySnapshot {
    /// Create a new identity snapshot
    fn new(timestamp: TscTimestamp, state: &[u8]) -> Self {
        let feature_vector = Self::extract_features(state);
        let state_hash = Self::compute_hash(state);
        let coherence_score = Self::calculate_coherence(&feature_vector);
        
        Self {
            timestamp,
            state_hash,
            feature_vector,
            coherence_score,
            stability_metric: 1.0, // Will be updated during tracking
            memory_fingerprint: state.to_vec(),
        }
    }
    
    /// Extract feature vector from state data
    fn extract_features(state: &[u8]) -> Vec<f64> {
        if state.is_empty() {
            return vec![0.0; 16]; // Default feature size
        }
        
        let mut features = Vec::with_capacity(16);
        
        // Statistical features
        let mean = state.iter().map(|&x| x as f64).sum::<f64>() / state.len() as f64;
        features.push(mean);
        
        let variance = state.iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>() / state.len() as f64;
        features.push(variance.sqrt());
        
        // Entropy-like measure
        let mut byte_counts = [0u32; 256];
        for &byte in state {
            byte_counts[byte as usize] += 1;
        }
        
        let entropy = byte_counts.iter()
            .filter(|&&count| count > 0)
            .map(|&count| {
                let p = count as f64 / state.len() as f64;
                -p * p.ln()
            })
            .sum::<f64>();
        features.push(entropy);
        
        // Spectral features (simple FFT-like)
        for i in 0..8 {
            let freq_component = state.iter()
                .enumerate()
                .map(|(j, &x)| {
                    let phase = 2.0 * std::f64::consts::PI * (i + 1) as f64 * j as f64 / state.len() as f64;
                    x as f64 * phase.cos()
                })
                .sum::<f64>();
            features.push(freq_component / state.len() as f64);
        }
        
        // Compression ratio estimate
        let complexity = Self::estimate_complexity(state);
        features.push(complexity);
        
        // Pattern density
        let pattern_density = Self::calculate_pattern_density(state);
        features.push(pattern_density);
        
        // Autocorrelation at lag 1
        let autocorr = Self::calculate_autocorrelation(state, 1);
        features.push(autocorr);
        
        // Trend measure
        let trend = Self::calculate_trend(state);
        features.push(trend);
        
        // Normalize features
        for feature in &mut features {
            *feature = feature.tanh(); // Bound between -1 and 1
        }
        
        features
    }
    
    /// Compute hash of state data
    fn compute_hash(state: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Calculate coherence score from feature vector
    fn calculate_coherence(features: &[f64]) -> f64 {
        if features.is_empty() {
            return 0.0;
        }
        
        // Coherence based on feature consistency
        let mean = features.iter().sum::<f64>() / features.len() as f64;
        let variance = features.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / features.len() as f64;
        
        // Lower variance = higher coherence
        (-variance).exp().min(1.0)
    }
    
    /// Calculate similarity with another snapshot
    fn calculate_similarity(&self, other: &IdentitySnapshot) -> f64 {
        if self.feature_vector.len() != other.feature_vector.len() {
            return 0.0;
        }
        
        // Cosine similarity between feature vectors
        let dot_product: f64 = self.feature_vector.iter()
            .zip(other.feature_vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        
        let magnitude_self: f64 = self.feature_vector.iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt();
        
        let magnitude_other: f64 = other.feature_vector.iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt();
        
        if magnitude_self > 0.0 && magnitude_other > 0.0 {
            dot_product / (magnitude_self * magnitude_other)
        } else {
            0.0
        }
    }
    
    // Helper methods for feature extraction
    
    fn estimate_complexity(data: &[u8]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }
        
        // Simple complexity estimate based on run lengths
        let mut runs = 0;
        let mut current_byte = data[0];
        
        for &byte in &data[1..] {
            if byte != current_byte {
                runs += 1;
                current_byte = byte;
            }
        }
        
        runs as f64 / data.len() as f64
    }
    
    fn calculate_pattern_density(data: &[u8]) -> f64 {
        if data.len() < 4 {
            return 0.0;
        }
        
        let mut patterns = HashMap::new();
        
        // Count 2-byte patterns
        for window in data.windows(2) {
            *patterns.entry((window[0], window[1])).or_insert(0) += 1;
        }
        
        patterns.len() as f64 / (data.len() - 1) as f64
    }
    
    fn calculate_autocorrelation(data: &[u8], lag: usize) -> f64 {
        if data.len() <= lag {
            return 0.0;
        }
        
        let mean = data.iter().map(|&x| x as f64).sum::<f64>() / data.len() as f64;
        
        let numerator: f64 = data.iter()
            .take(data.len() - lag)
            .zip(data.iter().skip(lag))
            .map(|(&x, &y)| (x as f64 - mean) * (y as f64 - mean))
            .sum();
        
        let denominator: f64 = data.iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum();
        
        if denominator > 0.0 {
            numerator / denominator
        } else {
            0.0
        }
    }
    
    fn calculate_trend(data: &[u8]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }
        
        let n = data.len() as f64;
        let sum_x = (0..data.len()).sum::<usize>() as f64;
        let sum_y = data.iter().map(|&x| x as f64).sum::<f64>();
        let sum_xy = data.iter()
            .enumerate()
            .map(|(i, &y)| i as f64 * y as f64)
            .sum::<f64>();
        let sum_x2 = (0..data.len())
            .map(|i| (i as f64).powi(2))
            .sum::<f64>();
        
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator > 0.0 {
            (n * sum_xy - sum_x * sum_y) / denominator
        } else {
            0.0
        }
    }
}

/// Identity continuity tracker
pub struct IdentityContinuityTracker {
    snapshots: VecDeque<IdentitySnapshot>,
    metrics: ContinuityMetrics,
    max_snapshots: usize,
    continuity_threshold: f64,
    gap_tolerance_ns: u64,
    identity_baseline: Option<IdentitySnapshot>,
    last_validation_time: Option<TscTimestamp>,
}

impl IdentityContinuityTracker {
    /// Create a new identity continuity tracker
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
            metrics: ContinuityMetrics::default(),
            max_snapshots: 1000,
            continuity_threshold: 0.7, // 70% similarity threshold
            gap_tolerance_ns: 1_000_000, // 1ms tolerance
            identity_baseline: None,
            last_validation_time: None,
        }
    }
    
    /// Track identity continuity at a specific timestamp
    pub fn track_continuity(&mut self, timestamp: TscTimestamp, state: &[u8]) -> TemporalResult<()> {
        let snapshot = IdentitySnapshot::new(timestamp, state);
        
        // Establish baseline if this is the first snapshot
        if self.identity_baseline.is_none() {
            self.identity_baseline = Some(snapshot.clone());
        }
        
        // Check for continuity breaks
        let continuity_break_info = if let Some(prev_snapshot) = self.snapshots.back() {
            Some((snapshot.clone(), prev_snapshot.clone()))
        } else {
            None
        };

        if let Some((current, previous)) = continuity_break_info {
            self.check_continuity_break(&current, &previous)?;
        }
        
        // Update stability metrics
        self.update_stability_metrics(&snapshot);
        
        // Store the snapshot
        self.store_snapshot(snapshot);
        
        // Update overall metrics
        self.update_metrics();
        
        self.last_validation_time = Some(timestamp);
        
        Ok(())
    }
    
    /// Validate current identity continuity
    pub fn validate_continuity(&self) -> TemporalResult<()> {
        if self.metrics.continuity_score < self.continuity_threshold {
            return Err(TemporalError::IdentityContinuityBreak {
                gap_ns: self.metrics.max_gap_duration_ns,
            });
        }
        
        Ok(())
    }
    
    /// Get current continuity metrics
    pub fn get_metrics(&self) -> TemporalResult<ContinuityMetrics> {
        Ok(self.metrics.clone())
    }
    
    /// Get identity stability score
    pub fn get_identity_stability(&self) -> f64 {
        self.metrics.identity_stability
    }
    
    /// Get continuity score
    pub fn get_continuity_score(&self) -> f64 {
        self.metrics.continuity_score
    }
    
    /// Reset tracking state
    pub fn reset(&mut self) {
        self.snapshots.clear();
        self.metrics = ContinuityMetrics::default();
        self.identity_baseline = None;
        self.last_validation_time = None;
    }
    
    /// Set continuity threshold
    pub fn set_continuity_threshold(&mut self, threshold: f64) {
        self.continuity_threshold = threshold.clamp(0.0, 1.0);
    }
    
    /// Get recent identity trajectory
    pub fn get_identity_trajectory(&self, window_size: usize) -> Vec<f64> {
        self.snapshots.iter()
            .rev()
            .take(window_size)
            .map(|s| s.coherence_score)
            .collect()
    }
    
    /// Calculate identity drift over time
    pub fn calculate_identity_drift(&self) -> f64 {
        if let Some(baseline) = &self.identity_baseline {
            if let Some(current) = self.snapshots.back() {
                return 1.0 - baseline.calculate_similarity(current);
            }
        }
        0.0
    }
    
    // Private helper methods
    
    fn check_continuity_break(
        &mut self,
        current: &IdentitySnapshot,
        previous: &IdentitySnapshot,
    ) -> TemporalResult<()> {
        // Check temporal gap
        let gap_ns = current.timestamp.nanos_since(previous.timestamp, 3_000_000_000);
        if gap_ns > self.gap_tolerance_ns {
            self.metrics.continuity_breaks += 1;
            self.metrics.max_gap_duration_ns = self.metrics.max_gap_duration_ns.max(gap_ns);
        }
        
        // Check similarity
        let similarity = current.calculate_similarity(previous);
        if similarity < self.continuity_threshold {
            self.metrics.continuity_breaks += 1;
        }
        
        Ok(())
    }
    
    fn update_stability_metrics(&mut self, snapshot: &IdentitySnapshot) {
        if self.snapshots.len() < 2 {
            return;
        }
        
        // Calculate stability based on recent snapshots
        let recent_snapshots: Vec<_> = self.snapshots.iter().rev().take(10).collect();
        
        if recent_snapshots.len() >= 2 {
            let mut similarities = Vec::new();
            
            for i in 0..recent_snapshots.len() - 1 {
                let sim = recent_snapshots[i].calculate_similarity(recent_snapshots[i + 1]);
                similarities.push(sim);
            }
            
            // Current similarity with latest snapshot
            let current_sim = snapshot.calculate_similarity(recent_snapshots[0]);
            similarities.push(current_sim);
            
            // Stability is average similarity over recent window
            let avg_similarity = similarities.iter().sum::<f64>() / similarities.len() as f64;
            
            // Update stability metric with exponential moving average
            let alpha = 0.1;
            self.metrics.identity_stability = (1.0 - alpha) * self.metrics.identity_stability + alpha * avg_similarity;
        }
    }
    
    fn store_snapshot(&mut self, snapshot: IdentitySnapshot) {
        self.snapshots.push_back(snapshot);
        
        // Keep history bounded
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.pop_front();
        }
    }
    
    fn update_metrics(&mut self) {
        if self.snapshots.len() < 2 {
            return;
        }
        
        // Calculate continuity score
        let mut total_similarity = 0.0;
        let mut similarity_count = 0;
        
        for window in self.snapshots.iter().collect::<Vec<_>>().windows(2) {
            let sim = window[1].calculate_similarity(window[0]);
            total_similarity += sim;
            similarity_count += 1;
        }
        
        if similarity_count > 0 {
            self.metrics.continuity_score = total_similarity / similarity_count as f64;
        }
        
        // Calculate coherence
        let coherence_scores: Vec<f64> = self.snapshots.iter()
            .map(|s| s.coherence_score)
            .collect();
        
        if !coherence_scores.is_empty() {
            self.metrics.identity_coherence = coherence_scores.iter().sum::<f64>() / coherence_scores.len() as f64;
        }
        
        // Calculate temporal consistency
        self.calculate_temporal_consistency();
        
        // Calculate preservation efficiency
        self.calculate_preservation_efficiency();
        
        // Update gap metrics
        self.update_gap_metrics();
    }
    
    fn calculate_temporal_consistency(&mut self) {
        if self.snapshots.len() < 3 {
            return;
        }
        
        // Measure how consistently identity features change over time
        let mut consistency_scores = Vec::new();
        
        for i in 2..self.snapshots.len() {
            let s1 = &self.snapshots[i - 2];
            let s2 = &self.snapshots[i - 1];
            let s3 = &self.snapshots[i];
            
            // Calculate velocity vectors
            let vel1: Vec<f64> = s2.feature_vector.iter()
                .zip(s1.feature_vector.iter())
                .map(|(a, b)| a - b)
                .collect();
            
            let vel2: Vec<f64> = s3.feature_vector.iter()
                .zip(s2.feature_vector.iter())
                .map(|(a, b)| a - b)
                .collect();
            
            // Calculate consistency as cosine similarity of velocity vectors
            let dot_product: f64 = vel1.iter().zip(vel2.iter()).map(|(a, b)| a * b).sum();
            let mag1: f64 = vel1.iter().map(|x| x * x).sum::<f64>().sqrt();
            let mag2: f64 = vel2.iter().map(|x| x * x).sum::<f64>().sqrt();
            
            if mag1 > 0.0 && mag2 > 0.0 {
                consistency_scores.push(dot_product / (mag1 * mag2));
            }
        }
        
        if !consistency_scores.is_empty() {
            self.metrics.temporal_consistency = consistency_scores.iter().sum::<f64>() / consistency_scores.len() as f64;
        }
    }
    
    fn calculate_preservation_efficiency(&mut self) {
        if let Some(baseline) = &self.identity_baseline {
            if let Some(current) = self.snapshots.back() {
                // Efficiency based on how well identity is preserved relative to baseline
                let similarity_to_baseline = current.calculate_similarity(baseline);
                let time_factor = 1.0 / (1.0 + self.snapshots.len() as f64 / 1000.0); // Decay over time
                
                self.metrics.preservation_efficiency = similarity_to_baseline * time_factor;
            }
        }
    }
    
    fn update_gap_metrics(&mut self) {
        if self.snapshots.len() < 2 {
            return;
        }
        
        let mut gaps = Vec::new();
        
        for window in self.snapshots.iter().collect::<Vec<_>>().windows(2) {
            let gap_ns = window[1].timestamp.nanos_since(window[0].timestamp, 3_000_000_000);
            gaps.push(gap_ns);
        }
        
        if !gaps.is_empty() {
            self.metrics.average_gap_duration_ns = gaps.iter().sum::<u64>() as f64 / gaps.len() as f64;
            self.metrics.max_gap_duration_ns = *gaps.iter().max().unwrap_or(&0);
        }
    }
}

impl Default for IdentityContinuityTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identity_snapshot_creation() {
        let timestamp = TscTimestamp::now();
        let state = vec![1, 2, 3, 4, 5];
        
        let snapshot = IdentitySnapshot::new(timestamp, &state);
        assert_eq!(snapshot.timestamp, timestamp);
        assert!(!snapshot.feature_vector.is_empty());
        assert!(snapshot.coherence_score >= 0.0 && snapshot.coherence_score <= 1.0);
    }
    
    #[test]
    fn test_feature_extraction() {
        let state = vec![1, 2, 3, 4, 5, 4, 3, 2, 1];
        let features = IdentitySnapshot::extract_features(&state);
        
        assert!(!features.is_empty());
        // Features should be normalized
        for &feature in &features {
            assert!(feature >= -1.0 && feature <= 1.0);
        }
    }
    
    #[test]
    fn test_similarity_calculation() {
        let timestamp = TscTimestamp::now();
        let state1 = vec![1, 2, 3, 4, 5];
        let state2 = vec![1, 2, 3, 4, 5]; // Identical
        let state3 = vec![5, 4, 3, 2, 1]; // Different
        
        let snapshot1 = IdentitySnapshot::new(timestamp, &state1);
        let snapshot2 = IdentitySnapshot::new(timestamp, &state2);
        let snapshot3 = IdentitySnapshot::new(timestamp, &state3);
        
        let sim12 = snapshot1.calculate_similarity(&snapshot2);
        let sim13 = snapshot1.calculate_similarity(&snapshot3);
        
        assert!(sim12 > sim13); // Identical states should be more similar
        assert!(sim12 >= 0.0 && sim12 <= 1.0);
        assert!(sim13 >= 0.0 && sim13 <= 1.0);
    }
    
    #[test]
    fn test_continuity_tracker() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        let state = vec![1, 2, 3, 4, 5];
        
        tracker.track_continuity(timestamp, &state).unwrap();
        assert_eq!(tracker.snapshots.len(), 1);
        
        // Track another similar state
        let timestamp2 = timestamp.add_nanos(1000, 3_000_000_000);
        let state2 = vec![1, 2, 3, 4, 6]; // Slightly different
        
        tracker.track_continuity(timestamp2, &state2).unwrap();
        assert_eq!(tracker.snapshots.len(), 2);
        
        let metrics = tracker.get_metrics().unwrap();
        assert!(metrics.continuity_score > 0.0);
    }
    
    #[test]
    fn test_continuity_break_detection() {
        let mut tracker = IdentityContinuityTracker::new();
        tracker.set_continuity_threshold(0.9); // High threshold
        
        let timestamp = TscTimestamp::now();
        let state1 = vec![1, 2, 3, 4, 5];
        let state2 = vec![10, 20, 30, 40, 50]; // Very different
        
        tracker.track_continuity(timestamp, &state1).unwrap();
        
        let timestamp2 = timestamp.add_nanos(1000, 3_000_000_000);
        tracker.track_continuity(timestamp2, &state2).unwrap();
        
        // Should detect continuity break
        assert!(tracker.metrics.continuity_breaks > 0);
    }
    
    #[test]
    fn test_identity_drift_calculation() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        
        // Start with baseline
        let baseline_state = vec![1, 2, 3, 4, 5];
        tracker.track_continuity(timestamp, &baseline_state).unwrap();
        
        // Gradual drift
        for i in 1..=10 {
            let timestamp_i = timestamp.add_nanos(i * 1000, 3_000_000_000);
            let drifted_state = vec![1 + i as u8, 2, 3, 4, 5];
            tracker.track_continuity(timestamp_i, &drifted_state).unwrap();
        }
        
        let drift = tracker.calculate_identity_drift();
        assert!(drift > 0.0); // Should detect some drift
        assert!(drift <= 1.0); // Should be bounded
    }
    
    #[test]
    fn test_stability_metrics() {
        let mut tracker = IdentityContinuityTracker::new();
        let timestamp = TscTimestamp::now();
        
        // Track several stable states
        for i in 0..10 {
            let timestamp_i = timestamp.add_nanos(i * 1000, 3_000_000_000);
            let stable_state = vec![1, 2, 3, 4, 5]; // Same state
            tracker.track_continuity(timestamp_i, &stable_state).unwrap();
        }
        
        assert!(tracker.get_identity_stability() > 0.5); // Should be stable
    }
}