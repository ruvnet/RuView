#[cfg(feature = "ruv-fann")]
pub mod ruv_fann_backend {
    // Implement this against ruv-fann's API.
    // Expected surface:
    // - new(input, hidden, output) -> Self
    // - train_regression(x, y, epochs, lr)
    // - predict_reg(x) -> Vec<f32>
    // - predict_cls3(x) -> Vec<usize>
    // Keep identical signatures to mlp.rs so the runner can swap backends.
    // Example skeleton:
    pub struct RuvFannModel { /* fields from ruv-fann */ }

    impl RuvFannModel {
        pub fn new(_input: usize, _hidden: usize, _output: usize) -> Self { Self { } }
        pub fn train_regression(&mut self, _x: &Vec<Vec<f32>>, _y: &Vec<f32>, _epochs: usize, _lr: f32) { /* ... */ }
        pub fn predict_reg(&self, _x: &[Vec<f32>]) -> Vec<f32> { vec![] }
        pub fn predict_cls3(&self, _x: &[Vec<f32>]) -> Vec<usize> { vec![] }
    }
}

#[cfg(not(feature = "ruv-fann"))]
pub mod ruv_fann_backend {
    // No-op stub so the crate compiles without the feature.
    pub struct RuvFannModel;
    impl RuvFannModel {
        pub fn new(_: usize, _: usize, _: usize) -> Self { Self }
        pub fn train_regression(&mut self, _: &Vec<Vec<f32>>, _: &Vec<f32>, _: usize, _: f32) {}
        pub fn predict_reg(&self, _: &[Vec<f32>]) -> Vec<f32> { Vec::new() }
        pub fn predict_cls3(&self, _: &[Vec<f32>]) -> Vec<usize> { Vec::new() }
    }
}