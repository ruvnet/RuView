use ndarray::{Array2, Array1, Axis};
use rand::{thread_rng, Rng};
use rayon::prelude::*;

pub struct OptimizedMlp {
    w1: Array2<f32>,
    b1: Array1<f32>,
    w2: Array2<f32>,
    b2: Array1<f32>,
    // Momentum terms
    vw1: Array2<f32>,
    vb1: Array1<f32>,
    vw2: Array2<f32>,
    vb2: Array1<f32>,
    // Adam optimizer state
    mw1: Array2<f32>,
    mb1: Array1<f32>,
    mw2: Array2<f32>,
    mb2: Array1<f32>,
    sw1: Array2<f32>,
    sb1: Array1<f32>,
    sw2: Array2<f32>,
    sb2: Array1<f32>,
    t: f32,
    output_dim: usize,
    use_adam: bool,
}

impl OptimizedMlp {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        let mut rng = thread_rng();

        // He initialization for ReLU
        let scale1 = (2.0 / input as f32).sqrt();
        let scale2 = (2.0 / hidden as f32).sqrt();

        let w1 = Array2::from_shape_fn((hidden, input), |_|
            rng.gen::<f32>() * scale1 - scale1/2.0);
        let b1 = Array1::zeros(hidden);
        let w2 = Array2::from_shape_fn((output, hidden), |_|
            rng.gen::<f32>() * scale2 - scale2/2.0);
        let b2 = Array1::zeros(output);

        // Initialize momentum and Adam states
        let vw1 = Array2::zeros((hidden, input));
        let vb1 = Array1::zeros(hidden);
        let vw2 = Array2::zeros((output, hidden));
        let vb2 = Array1::zeros(output);

        let mw1 = Array2::zeros((hidden, input));
        let mb1 = Array1::zeros(hidden);
        let mw2 = Array2::zeros((output, hidden));
        let mb2 = Array1::zeros(output);

        let sw1 = Array2::zeros((hidden, input));
        let sb1 = Array1::zeros(hidden);
        let sw2 = Array2::zeros((output, hidden));
        let sb2 = Array1::zeros(output);

        Self {
            w1, b1, w2, b2,
            vw1, vb1, vw2, vb2,
            mw1, mb1, mw2, mb2,
            sw1, sb1, sw2, sb2,
            t: 0.0,
            output_dim: output,
            use_adam: true,
        }
    }

    pub fn forward(&self, x: &Array1<f32>) -> (Array1<f32>, Array1<f32>) {
        let z1 = self.w1.dot(x) + &self.b1;
        let h = z1.mapv(|v| v.max(0.0)); // ReLU
        let output = self.w2.dot(&h) + &self.b2;
        (output, h)
    }

    pub fn backward(&mut self, x: &Array1<f32>, y_true: f32, lr: f32) {
        let (output, h) = self.forward(x);

        // Compute loss gradient
        let grad_output = if self.output_dim == 1 {
            Array1::from_elem(1, output[0] - y_true)
        } else {
            // Softmax + cross-entropy gradient for classification
            let exp_out = output.mapv(|v| v.exp());
            let sum_exp = exp_out.sum();
            let softmax = &exp_out / sum_exp;

            // Create one-hot target
            let class = if y_true < -0.25 { 0 }
                       else if y_true > 0.25 { 2 }
                       else { 1 };
            let mut target = Array1::zeros(self.output_dim);
            if class < self.output_dim {
                target[class] = 1.0;
            }

            softmax - target
        };

        // Gradient w.r.t w2 and b2
        let grad_w2 = grad_output.clone().insert_axis(Axis(1)) * h.clone().insert_axis(Axis(0));
        let grad_b2 = grad_output.clone();

        // Backprop through hidden layer
        let grad_h = self.w2.t().dot(&grad_output);
        // Gradient of ReLU: 1 if input > 0, else 0
        let z1 = self.w1.dot(x) + &self.b1;
        let grad_z1 = grad_h * z1.mapv(|v| if v > 0.0 { 1.0 } else { 0.0 });

        // Gradient w.r.t w1 and b1
        let grad_w1 = grad_z1.clone().insert_axis(Axis(1)) * x.clone().insert_axis(Axis(0));
        let grad_b1 = grad_z1;

        // Update weights using Adam or momentum
        if self.use_adam {
            self.adam_update(grad_w1, grad_b1, grad_w2.into_shape(self.w2.dim()).unwrap(), grad_b2, lr);
        } else {
            self.momentum_update(grad_w1, grad_b1, grad_w2.into_shape(self.w2.dim()).unwrap(), grad_b2, lr);
        }
    }

    fn adam_update(&mut self, grad_w1: Array2<f32>, grad_b1: Array1<f32>,
                   grad_w2: Array2<f32>, grad_b2: Array1<f32>, lr: f32) {
        let beta1 = 0.9;
        let beta2 = 0.999;
        let epsilon = 1e-8;

        self.t += 1.0;

        // Update biased first moment estimate
        self.mw1 = &self.mw1 * beta1 + &grad_w1 * (1.0 - beta1);
        self.mb1 = &self.mb1 * beta1 + &grad_b1 * (1.0 - beta1);
        self.mw2 = &self.mw2 * beta1 + &grad_w2 * (1.0 - beta1);
        self.mb2 = &self.mb2 * beta1 + &grad_b2 * (1.0 - beta1);

        // Update biased second raw moment estimate
        self.sw1 = &self.sw1 * beta2 + grad_w1.mapv(|x| x * x) * (1.0 - beta2);
        self.sb1 = &self.sb1 * beta2 + grad_b1.mapv(|x| x * x) * (1.0 - beta2);
        self.sw2 = &self.sw2 * beta2 + grad_w2.mapv(|x| x * x) * (1.0 - beta2);
        self.sb2 = &self.sb2 * beta2 + grad_b2.mapv(|x| x * x) * (1.0 - beta2);

        // Compute bias-corrected moments
        let bias_correction1 = 1.0 - beta1.powf(self.t);
        let bias_correction2 = 1.0 - beta2.powf(self.t);

        // Update weights
        self.w1 = &self.w1 - lr * &self.mw1 / bias_correction1 / ((&self.sw1 / bias_correction2).mapv(f32::sqrt) + epsilon);
        self.b1 = &self.b1 - lr * &self.mb1 / bias_correction1 / ((&self.sb1 / bias_correction2).mapv(f32::sqrt) + epsilon);
        self.w2 = &self.w2 - lr * &self.mw2 / bias_correction1 / ((&self.sw2 / bias_correction2).mapv(f32::sqrt) + epsilon);
        self.b2 = &self.b2 - lr * &self.mb2 / bias_correction1 / ((&self.sb2 / bias_correction2).mapv(f32::sqrt) + epsilon);
    }

    fn momentum_update(&mut self, grad_w1: Array2<f32>, grad_b1: Array1<f32>,
                       grad_w2: Array2<f32>, grad_b2: Array1<f32>, lr: f32) {
        let momentum = 0.9;

        // Update velocity
        self.vw1 = &self.vw1 * momentum - &grad_w1 * lr;
        self.vb1 = &self.vb1 * momentum - &grad_b1 * lr;
        self.vw2 = &self.vw2 * momentum - &grad_w2 * lr;
        self.vb2 = &self.vb2 * momentum - &grad_b2 * lr;

        // Update weights
        self.w1 = &self.w1 + &self.vw1;
        self.b1 = &self.b1 + &self.vb1;
        self.w2 = &self.w2 + &self.vw2;
        self.b2 = &self.b2 + &self.vb2;
    }

    pub fn train_regression(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>, epochs: usize, lr: f32) {
        for _ in 0..epochs {
            // Shuffle indices for SGD
            let mut indices: Vec<usize> = (0..x.len()).collect();
            let mut rng = thread_rng();
            use rand::seq::SliceRandom;
            indices.shuffle(&mut rng);

            for &i in &indices {
                let x_arr = Array1::from_vec(x[i].clone());
                self.backward(&x_arr, y[i], lr);
            }
        }
    }

    pub fn train_batch(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>, epochs: usize,
                       lr: f32, batch_size: usize) {
        for _ in 0..epochs {
            let mut indices: Vec<usize> = (0..x.len()).collect();
            let mut rng = thread_rng();
            use rand::seq::SliceRandom;
            indices.shuffle(&mut rng);

            for batch_start in (0..x.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(x.len());

                // Accumulate gradients for batch
                for i in batch_start..batch_end {
                    let idx = indices[i];
                    let x_arr = Array1::from_vec(x[idx].clone());
                    self.backward(&x_arr, y[idx], lr / batch_size as f32);
                }
            }
        }
    }

    pub fn predict_reg(&self, x: &[Vec<f32>]) -> Vec<f32> {
        x.par_iter().map(|xi| {
            let (out, _) = self.forward(&Array1::from_vec(xi.clone()));
            if self.output_dim == 1 {
                out[0]
            } else {
                out[0]
            }
        }).collect()
    }

    pub fn predict_cls3(&self, x: &[Vec<f32>]) -> Vec<usize> {
        x.par_iter().map(|xi| {
            let (out, _) = self.forward(&Array1::from_vec(xi.clone()));

            if self.output_dim >= 3 {
                // Softmax + argmax for classification
                let exp_out = out.mapv(|v| v.exp());
                let sum_exp = exp_out.sum();
                let probs = exp_out / sum_exp;

                let mut best = 0;
                let mut best_val = probs[0];
                for i in 1..3.min(probs.len()) {
                    if probs[i] > best_val {
                        best_val = probs[i];
                        best = i;
                    }
                }
                best
            } else {
                let val = out[0];
                if val < -0.25 { 0 }
                else if val > 0.25 { 2 }
                else { 1 }
            }
        }).collect()
    }
}