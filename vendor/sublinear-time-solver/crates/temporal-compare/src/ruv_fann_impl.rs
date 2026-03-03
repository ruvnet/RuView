#[cfg(feature = "ruv-fann")]
use ruv_fann_dep::{Network, Activation};

#[cfg(feature = "ruv-fann")]
pub struct RuvFannModel {
    network: Network,
    input_dim: usize,
    output_dim: usize,
}

#[cfg(feature = "ruv-fann")]
impl RuvFannModel {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        // Create a 3-layer network: input -> hidden -> output
        let network = Network::new(&[input, hidden, output])
            .with_activation(Activation::Sigmoid)
            .with_learning_rate(0.7);

        Self {
            network,
            input_dim: input,
            output_dim: output,
        }
    }

    pub fn train_regression(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>, epochs: usize, lr: f32) {
        // Get mutable access to network
        let network = Arc::get_mut(&mut self.network)
            .expect("Cannot get mutable reference to network");

        network.set_learning_rate(lr);

        // Prepare training data in FANN format
        let mut inputs: Vec<Vec<f32>> = Vec::new();
        let mut outputs: Vec<Vec<f32>> = Vec::new();

        for (xi, yi) in x.iter().zip(y) {
            inputs.push(xi.clone());
            if self.output_dim == 1 {
                outputs.push(vec![*yi]);
            } else {
                // For classification, create one-hot encoding
                let class = if *yi < -0.25 { 0 }
                           else if *yi > 0.25 { 2 }
                           else { 1 };
                let mut one_hot = vec![0.0; self.output_dim];
                if class < self.output_dim {
                    one_hot[class] = 1.0;
                }
                outputs.push(one_hot);
            }
        }

        // Train using batch training
        for _ in 0..epochs {
            for (input, output) in inputs.iter().zip(&outputs) {
                network.train(input, output);
            }
        }
    }

    pub fn predict_reg(&self, x: &[Vec<f32>]) -> Vec<f32> {
        x.iter().map(|xi| {
            let output = self.network.run(xi).expect("Failed to run network");
            if self.output_dim == 1 {
                output[0]
            } else {
                // Return first output for regression
                output[0]
            }
        }).collect()
    }

    pub fn predict_cls3(&self, x: &[Vec<f32>]) -> Vec<usize> {
        x.iter().map(|xi| {
            let output = self.network.run(xi).expect("Failed to run network");

            if self.output_dim >= 3 {
                // Find argmax for classification
                let mut best = 0;
                let mut best_val = output[0];
                for i in 1..3.min(output.len()) {
                    if output[i] > best_val {
                        best_val = output[i];
                        best = i;
                    }
                }
                best
            } else {
                // Threshold-based classification for single output
                let val = output[0];
                if val < -0.25 { 0 }
                else if val > 0.25 { 2 }
                else { 1 }
            }
        }).collect()
    }
}

#[cfg(not(feature = "ruv-fann"))]
pub struct RuvFannModel;

#[cfg(not(feature = "ruv-fann"))]
impl RuvFannModel {
    pub fn new(_: usize, _: usize, _: usize) -> Self { Self }
    pub fn train_regression(&mut self, _: &Vec<Vec<f32>>, _: &Vec<f32>, _: usize, _: f32) {}
    pub fn predict_reg(&self, _: &[Vec<f32>]) -> Vec<f32> { Vec::new() }
    pub fn predict_cls3(&self, _: &[Vec<f32>]) -> Vec<usize> { Vec::new() }
}