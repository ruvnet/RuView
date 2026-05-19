//! Inference engine — currently a stub that loads the trained safetensors
//! from `wifi-densepose-train` and runs the pose head. The Hailo HEF path
//! is gated behind the `hailo` feature and stubbed out until ADR-101 P6
//! (Hailo SDK on a self-hosted runner) lands.

use std::error::Error;

/// 56 subcarriers × 20 frames per CSI window — matches the format
/// produced by `scripts/align-ground-truth.js` after #641.
pub const INPUT_SUBCARRIERS: usize = 56;
pub const INPUT_TIMESTEPS: usize = 20;
pub const OUTPUT_KEYPOINTS: usize = 17;

#[derive(Debug, Clone)]
pub struct CsiWindow {
    pub data: Vec<f32>, // length INPUT_SUBCARRIERS * INPUT_TIMESTEPS
}

#[derive(Debug, Clone)]
pub struct PoseOutput {
    /// Flat `[OUTPUT_KEYPOINTS * 2]` keypoints in `[0, 1]` normalised
    /// image coords, ordered (x0, y0, x1, y1, …).
    pub keypoints: Vec<f32>,
    pub confidence: f32,
}

impl PoseOutput {
    pub fn is_finite(&self) -> bool {
        self.keypoints.iter().all(|v| v.is_finite()) && self.confidence.is_finite()
    }
}

pub struct InferenceEngine {
    // Placeholder. Real engine will hold the loaded encoder + pose head.
    _initialized: bool,
}

impl InferenceEngine {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Health-check path doesn't need a real model. The `run` path
        // will replace this with a libtorch / ONNX / Hailo loader.
        Ok(Self { _initialized: true })
    }

    pub fn infer(&self, window: &CsiWindow) -> Result<PoseOutput, Box<dyn Error>> {
        if window.data.len() != INPUT_SUBCARRIERS * INPUT_TIMESTEPS {
            return Err(format!(
                "expected {} input values, got {}",
                INPUT_SUBCARRIERS * INPUT_TIMESTEPS,
                window.data.len()
            )
            .into());
        }
        // Stub: produce a centred-skeleton baseline at (0.5, 0.5) so the
        // health-check verifies the I/O surface end-to-end. Replaced once
        // the trained safetensors are wired in.
        let keypoints = vec![0.5f32; OUTPUT_KEYPOINTS * 2];
        Ok(PoseOutput {
            keypoints,
            confidence: 0.0, // honest: no model yet → zero confidence
        })
    }
}

/// Synthetic CSI window for the `health` subcommand. Just zeros — the
/// stub engine ignores the values; we only exercise the I/O surface.
pub struct SyntheticInput;

impl Default for SyntheticInput {
    fn default() -> Self {
        Self
    }
}

impl SyntheticInput {
    pub fn as_window(&self) -> CsiWindow {
        CsiWindow {
            data: vec![0.0; INPUT_SUBCARRIERS * INPUT_TIMESTEPS],
        }
    }
}
