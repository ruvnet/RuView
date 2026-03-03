use ndarray::{Array2, Array1};
use rand::{thread_rng, Rng};

pub struct Mlp {
    w1: Array2<f32>,
    b1: Array1<f32>,
    w2: Array2<f32>,
    b2: Array1<f32>,
    output_dim: usize,
}

fn relu(x: &mut Array1<f32>) {
    x.iter_mut().for_each(|v| if *v < 0.0 { *v = 0.0 });
}

impl Mlp {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        let mut rng = thread_rng();
        let scale = (2.0 / input as f32).sqrt();

        let w1 = Array2::from_shape_fn((hidden, input), |_| rng.gen::<f32>() * scale - scale/2.0);
        let b1 = Array1::zeros(hidden);
        let w2 = Array2::from_shape_fn((output, hidden), |_| rng.gen::<f32>() * scale - scale/2.0);
        let b2 = Array1::zeros(output);

        Self { w1, b1, w2, b2, output_dim: output }
    }

    fn forward(&self, x: &Array1<f32>) -> Array1<f32> {
        let mut h = self.w1.dot(x) + &self.b1;
        relu(&mut h);
        self.w2.dot(&h) + &self.b2
    }

    pub fn train_regression(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>, epochs: usize, lr: f32) {
        // Simplified SGD without full backprop for now
        for _ in 0..epochs {
            for (xi, yi) in x.iter().zip(y) {
                let x_arr = Array1::from_vec(xi.clone());
                let output = self.forward(&x_arr);

                // Only train if we have 1D output for regression
                if self.output_dim == 1 {
                    let err = output[0] - yi;

                    // Numerical gradient approximation for simplicity
                    let _eps = 0.0001;

                    // Update w2 and b2
                    let mut h = self.w1.dot(&x_arr) + &self.b1;
                    relu(&mut h);

                    for i in 0..self.w2.nrows() {
                        for j in 0..self.w2.ncols() {
                            self.w2[[i, j]] -= lr * err * h[j] * 0.1;
                        }
                        self.b2[i] -= lr * err * 0.1;
                    }

                    // Update w1 and b1 with smaller learning rate
                    for i in 0..self.w1.nrows() {
                        for j in 0..self.w1.ncols() {
                            self.w1[[i, j]] -= lr * err * x_arr[j] * 0.01;
                        }
                        self.b1[i] -= lr * err * 0.01;
                    }
                }
            }
        }
    }

    pub fn predict_reg(&self, x: &[Vec<f32>]) -> Vec<f32> {
        x.iter().map(|xi| {
            let out = self.forward(&Array1::from_vec(xi.clone()));
            if self.output_dim == 1 {
                out[0]
            } else {
                // For multi-output, return first element as regression
                out[0]
            }
        }).collect()
    }

    pub fn predict_cls3(&self, x: &[Vec<f32>]) -> Vec<usize> {
        x.iter().map(|xi| {
            let out = self.forward(&Array1::from_vec(xi.clone()));

            if self.output_dim >= 3 {
                // Find argmax for 3-class classification
                let mut best = 0;
                let mut best_val = out[0];
                for i in 1..3 {
                    if out[i] > best_val {
                        best_val = out[i];
                        best = i;
                    }
                }
                best
            } else {
                // Fall back to threshold-based classification
                let val = out[0];
                if val < -0.25 { 0 }
                else if val > 0.25 { 2 }
                else { 1 }
            }
        }).collect()
    }
}