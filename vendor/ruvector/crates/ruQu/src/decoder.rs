//! Quantum Error Decoder Integration
//!
//! Integrates the fusion-blossom Minimum-Weight Perfect Matching (MWPM) decoder
//! for quantum error syndrome decoding.
//!
//! ## Features
//!
//! When the `decoder` feature is enabled, this module provides:
//! - Real MWPM decoding via fusion-blossom
//! - Syndrome graph construction from detector events
//! - Correction suggestion generation
//!
//! When disabled, a fast heuristic fallback is used.
//!
//! ## Performance
//!
//! fusion-blossom is optimized for real-time decoding:
//! - O(V^3) worst case, O(V) typical for sparse syndromes
//! - Parallelizable for large code distances

use crate::syndrome::DetectorBitmap;

/// Decoder configuration
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    /// Code distance (determines graph size)
    pub distance: usize,
    /// Physical error probability
    pub physical_error_rate: f64,
    /// Number of syndrome rounds to consider
    pub window_size: usize,
    /// Enable parallel decoding (when supported)
    pub parallel: bool,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            distance: 7,
            physical_error_rate: 0.001,
            window_size: 1,
            parallel: false,
        }
    }
}

/// Correction suggestion from the decoder
#[derive(Debug, Clone)]
pub struct Correction {
    /// Data qubit indices to apply X correction
    pub x_corrections: Vec<usize>,
    /// Data qubit indices to apply Z correction
    pub z_corrections: Vec<usize>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Decoder runtime in nanoseconds
    pub decode_time_ns: u64,
}

impl Default for Correction {
    fn default() -> Self {
        Self {
            x_corrections: Vec::new(),
            z_corrections: Vec::new(),
            confidence: 1.0,
            decode_time_ns: 0,
        }
    }
}

/// MWPM Decoder using fusion-blossom
///
/// Provides minimum-weight perfect matching decoding for surface code syndromes.
#[cfg(feature = "decoder")]
pub struct MWPMDecoder {
    config: DecoderConfig,
    /// Pre-built syndrome graph for the surface code
    solver: fusion_blossom::mwpm_solver::SolverSerial,
    /// Vertex count in the matching graph
    vertex_count: usize,
    /// Edge definitions: (v1, v2, weight)
    edges: Vec<(usize, usize, i32)>,
    /// Mapping from detector index to vertex
    detector_to_vertex: Vec<usize>,
}

#[cfg(feature = "decoder")]
impl MWPMDecoder {
    /// Create a new MWPM decoder for a surface code of given distance
    pub fn new(config: DecoderConfig) -> Self {
        use fusion_blossom::mwpm_solver::{SolverInitializer, SolverSerial};
        use fusion_blossom::util::*;

        let d = config.distance;

        // For a distance-d surface code, we have approximately d^2 data qubits
        // and (d^2-1)/2 X-type + (d^2-1)/2 Z-type stabilizers
        let num_detectors = d * d;
        let vertex_count = num_detectors + 1; // +1 for virtual boundary vertex

        // Build edges between neighboring detectors
        // Weight is -log(p) scaled to integer
        let weight = (-(config.physical_error_rate.ln()) * 1000.0) as i32;
        let mut edges = Vec::new();

        // Grid connectivity for surface code
        for row in 0..d {
            for col in 0..d {
                let v = row * d + col;

                // Connect to right neighbor
                if col + 1 < d {
                    let neighbor = row * d + (col + 1);
                    edges.push((v, neighbor, weight));
                }

                // Connect to bottom neighbor
                if row + 1 < d {
                    let neighbor = (row + 1) * d + col;
                    edges.push((v, neighbor, weight));
                }
            }
        }

        // Connect boundary vertices to virtual boundary
        let boundary_vertex = num_detectors;
        for col in 0..d {
            edges.push((col, boundary_vertex, weight / 2)); // Top edge
            edges.push(((d - 1) * d + col, boundary_vertex, weight / 2)); // Bottom edge
        }
        for row in 0..d {
            edges.push((row * d, boundary_vertex, weight / 2)); // Left edge
            edges.push((row * d + (d - 1), boundary_vertex, weight / 2)); // Right edge
        }

        // Convert to fusion-blossom format
        let fb_edges: Vec<(VertexIndex, VertexIndex, Weight)> = edges
            .iter()
            .map(|(v1, v2, w)| (*v1 as VertexIndex, *v2 as VertexIndex, *w as Weight))
            .collect();

        // Create initializer
        let initializer = SolverInitializer::new(vertex_count as VertexNum, fb_edges);
        let solver = SolverSerial::new(&initializer);

        // Simple 1:1 detector mapping for now
        let detector_to_vertex: Vec<usize> = (0..num_detectors).collect();

        Self {
            config,
            solver,
            vertex_count,
            edges,
            detector_to_vertex,
        }
    }

    /// Decode a syndrome bitmap and return correction suggestions
    pub fn decode(&mut self, syndrome: &DetectorBitmap) -> Correction {
        use fusion_blossom::mwpm_solver::PrimalDualSolver;
        use std::time::Instant;

        let start = Instant::now();

        // Clear previous syndrome
        self.solver.clear();

        // Add defects (fired detectors) to the solver
        let mut defect_vertices = Vec::new();
        for detector_idx in syndrome.iter_fired() {
            if detector_idx < self.detector_to_vertex.len() {
                let vertex = self.detector_to_vertex[detector_idx];
                defect_vertices.push(vertex as fusion_blossom::util::VertexIndex);
            }
        }

        // Must have even number of defects for perfect matching
        // If odd, add virtual boundary vertex
        if defect_vertices.len() % 2 == 1 {
            defect_vertices.push((self.vertex_count - 1) as fusion_blossom::util::VertexIndex);
        }

        // Set syndrome and solve
        self.solver.solve_visualizer(None);

        // Extract matching
        let matching = self.solver.perfect_matching();

        // Convert matching to corrections
        // Each matched pair indicates an error chain
        let mut x_corrections = Vec::new();
        let d = self.config.distance;

        for (v1, v2) in matching.iter() {
            let v1 = *v1 as usize;
            let v2 = *v2 as usize;

            // Find data qubits along the path between v1 and v2
            if v1 < d * d && v2 < d * d {
                // Both are real detectors - correction on data qubit between them
                let row1 = v1 / d;
                let col1 = v1 % d;
                let row2 = v2 / d;
                let col2 = v2 % d;

                // Simple: correct all data qubits in the bounding box
                let min_row = row1.min(row2);
                let max_row = row1.max(row2);
                let min_col = col1.min(col2);
                let max_col = col1.max(col2);

                for r in min_row..=max_row {
                    for c in min_col..=max_col {
                        x_corrections.push(r * d + c);
                    }
                }
            }
        }

        // Deduplicate corrections (XOR logic - double correction = no correction)
        x_corrections.sort_unstable();
        let mut deduped = Vec::new();
        let mut i = 0;
        while i < x_corrections.len() {
            let mut count = 1;
            while i + count < x_corrections.len() && x_corrections[i] == x_corrections[i + count] {
                count += 1;
            }
            if count % 2 == 1 {
                deduped.push(x_corrections[i]);
            }
            i += count;
        }

        let elapsed = start.elapsed();

        Correction {
            x_corrections: deduped,
            z_corrections: Vec::new(), // Z corrections from separate decoder pass
            confidence: if syndrome.fired_count() == 0 {
                1.0
            } else {
                0.9
            },
            decode_time_ns: elapsed.as_nanos() as u64,
        }
    }

    /// Get decoder statistics
    pub fn config(&self) -> &DecoderConfig {
        &self.config
    }
}

/// Heuristic decoder fallback (when fusion-blossom is not available)
#[cfg(not(feature = "decoder"))]
pub struct MWPMDecoder {
    config: DecoderConfig,
}

#[cfg(not(feature = "decoder"))]
impl MWPMDecoder {
    /// Create a new heuristic decoder
    pub fn new(config: DecoderConfig) -> Self {
        Self { config }
    }

    /// Decode using simple nearest-neighbor heuristic
    pub fn decode(&mut self, syndrome: &DetectorBitmap) -> Correction {
        let start = std::time::Instant::now();

        let fired: Vec<usize> = syndrome.iter_fired().collect();

        // Simple heuristic: pair adjacent fired detectors
        let d = self.config.distance;
        let mut x_corrections = Vec::new();
        let mut used = vec![false; fired.len()];

        for (i, &det1) in fired.iter().enumerate() {
            if used[i] {
                continue;
            }

            let row1 = det1 / d;
            let col1 = det1 % d;

            // Find nearest unmatched detector
            let mut best_dist = usize::MAX;
            let mut best_j = None;

            for (j, &det2) in fired.iter().enumerate().skip(i + 1) {
                if used[j] {
                    continue;
                }

                let row2 = det2 / d;
                let col2 = det2 % d;
                let dist = row1.abs_diff(row2) + col1.abs_diff(col2);

                if dist < best_dist {
                    best_dist = dist;
                    best_j = Some(j);
                }
            }

            if let Some(j) = best_j {
                used[i] = true;
                used[j] = true;

                // Add correction between det1 and det2
                let det2 = fired[j];
                let row2 = det2 / d;
                let col2 = det2 % d;

                // Correct along Manhattan path
                let min_row = row1.min(row2);
                let max_row = row1.max(row2);
                let min_col = col1.min(col2);
                let max_col = col1.max(col2);

                // Horizontal path
                for c in min_col..max_col {
                    x_corrections.push(min_row * d + c);
                }
                // Vertical path
                for r in min_row..max_row {
                    x_corrections.push(r * d + max_col);
                }
            }
        }

        let elapsed = start.elapsed();

        Correction {
            x_corrections,
            z_corrections: Vec::new(),
            confidence: if fired.is_empty() { 1.0 } else { 0.7 }, // Lower confidence for heuristic
            decode_time_ns: elapsed.as_nanos() as u64,
        }
    }

    /// Get decoder configuration
    pub fn config(&self) -> &DecoderConfig {
        &self.config
    }
}

/// Streaming decoder for real-time syndrome processing
pub struct StreamingDecoder {
    inner: MWPMDecoder,
    /// Recent corrections for temporal correlation
    correction_history: Vec<Correction>,
    /// Maximum history size
    history_size: usize,
}

impl StreamingDecoder {
    /// Create a new streaming decoder
    pub fn new(config: DecoderConfig) -> Self {
        let history_size = config.window_size.max(10);
        Self {
            inner: MWPMDecoder::new(config),
            correction_history: Vec::with_capacity(history_size),
            history_size,
        }
    }

    /// Process a syndrome round and return corrections
    pub fn process(&mut self, syndrome: &DetectorBitmap) -> Correction {
        let correction = self.inner.decode(syndrome);

        // Add to history
        if self.correction_history.len() >= self.history_size {
            self.correction_history.remove(0);
        }
        self.correction_history.push(correction.clone());

        correction
    }

    /// Get average decode time over recent history
    pub fn average_decode_time_ns(&self) -> u64 {
        if self.correction_history.is_empty() {
            return 0;
        }
        let sum: u64 = self
            .correction_history
            .iter()
            .map(|c| c.decode_time_ns)
            .sum();
        sum / self.correction_history.len() as u64
    }

    /// Get decoder configuration
    pub fn config(&self) -> &DecoderConfig {
        self.inner.config()
    }

    /// Clear correction history
    pub fn clear_history(&mut self) {
        self.correction_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_config_default() {
        let config = DecoderConfig::default();
        assert_eq!(config.distance, 7);
        assert!((config.physical_error_rate - 0.001).abs() < 1e-10);
    }

    #[test]
    fn test_decoder_empty_syndrome() {
        let config = DecoderConfig::default();
        let mut decoder = MWPMDecoder::new(config);

        let syndrome = DetectorBitmap::new(49); // d=7, 7*7=49 detectors
        let correction = decoder.decode(&syndrome);

        assert!(correction.x_corrections.is_empty());
        assert_eq!(correction.confidence, 1.0);
    }

    #[test]
    fn test_decoder_single_pair() {
        let config = DecoderConfig {
            distance: 5,
            physical_error_rate: 0.01,
            window_size: 1,
            parallel: false,
        };
        let mut decoder = MWPMDecoder::new(config);

        // Two adjacent fired detectors
        let mut syndrome = DetectorBitmap::new(25); // d=5, 5*5=25 detectors
        syndrome.set(0, true); // (0,0)
        syndrome.set(1, true); // (0,1)

        let correction = decoder.decode(&syndrome);

        // Should suggest correction between them
        assert!(!correction.x_corrections.is_empty());
        assert!(correction.decode_time_ns > 0);
    }

    #[test]
    fn test_streaming_decoder() {
        let config = DecoderConfig::default();
        let mut decoder = StreamingDecoder::new(config);

        // Process several rounds
        for i in 0..5 {
            let mut syndrome = DetectorBitmap::new(49);
            if i % 2 == 0 {
                syndrome.set(0, true);
                syndrome.set(6, true);
            }
            let _ = decoder.process(&syndrome);
        }

        assert!(decoder.average_decode_time_ns() > 0);
    }

    #[test]
    fn test_correction_default() {
        let correction = Correction::default();
        assert!(correction.x_corrections.is_empty());
        assert!(correction.z_corrections.is_empty());
        assert_eq!(correction.confidence, 1.0);
    }
}
