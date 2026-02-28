//! WiFi-DensePose model definition and construction.
//!
//! This module will be implemented by the trainer agent. It currently provides
//! the public interface stubs so that the crate compiles as a whole.

/// Placeholder for the compiled model handle.
///
/// The real implementation wraps a `tch::CModule` or a custom `nn::Module`.
pub struct DensePoseModel;

impl DensePoseModel {
    /// Construct a new model from the given number of subcarriers and keypoints.
    pub fn new(_num_subcarriers: usize, _num_keypoints: usize) -> Self {
        DensePoseModel
    }
}
