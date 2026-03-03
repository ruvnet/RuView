use ndarray::{Array2, Array1, Axis};
use rand::{thread_rng, Rng};

/// Specialized classifier with proper softmax, dropout, and batch norm
pub struct ClassifierMlp {
    // Weights
    w1: Array2<f32>,
    b1: Array1<f32>,
    w2: Array2<f32>,
    b2: Array1<f32>,
    w3: Array2<f32>, // Extra layer for better classification
    b3: Array1<f32>,

    // Batch normalization parameters
    bn1_gamma: Array1<f32>,
    bn1_beta: Array1<f32>,
    bn1_running_mean: Array1<f32>,
    bn1_running_var: Array1<f32>,

    bn2_gamma: Array1<f32>,
    bn2_beta: Array1<f32>,
    bn2_running_mean: Array1<f32>,
    bn2_running_var: Array1<f32>,

    // Adam optimizer state
    mw1: Array2<f32>, mb1: Array1<f32>,
    mw2: Array2<f32>, mb2: Array1<f32>,
    mw3: Array2<f32>, mb3: Array1<f32>,
    sw1: Array2<f32>, sb1: Array1<f32>,
    sw2: Array2<f32>, sb2: Array1<f32>,
    sw3: Array2<f32>, sb3: Array1<f32>,
    t: f32,

    // Architecture
    input_dim: usize,
    hidden1_dim: usize,
    hidden2_dim: usize,
    output_dim: usize,

    // Training settings
    dropout_rate: f32,
    is_training: bool,
    lr_schedule: LRSchedule,
}

pub enum LRSchedule {
    Constant(f32),
    Cosine { initial: f32, min: f32, period: usize },
    StepDecay { initial: f32, decay: f32, step_size: usize },
}

impl ClassifierMlp {
    pub fn new(input: usize, output: usize) -> Self {
        let mut rng = thread_rng();

        // Deeper network for better classification
        let hidden1 = 128;
        let hidden2 = 64;

        // Xavier initialization
        let scale1 = (2.0 / input as f32).sqrt();
        let scale2 = (2.0 / hidden1 as f32).sqrt();
        let scale3 = (2.0 / hidden2 as f32).sqrt();

        let w1 = Array2::from_shape_fn((hidden1, input), |_|
            rng.gen::<f32>() * scale1 - scale1/2.0);
        let b1 = Array1::zeros(hidden1);

        let w2 = Array2::from_shape_fn((hidden2, hidden1), |_|
            rng.gen::<f32>() * scale2 - scale2/2.0);
        let b2 = Array1::zeros(hidden2);

        let w3 = Array2::from_shape_fn((output, hidden2), |_|
            rng.gen::<f32>() * scale3 - scale3/2.0);
        let b3 = Array1::zeros(output);

        // Batch norm parameters
        let bn1_gamma = Array1::ones(hidden1);
        let bn1_beta = Array1::zeros(hidden1);
        let bn1_running_mean = Array1::zeros(hidden1);
        let bn1_running_var = Array1::ones(hidden1);

        let bn2_gamma = Array1::ones(hidden2);
        let bn2_beta = Array1::zeros(hidden2);
        let bn2_running_mean = Array1::zeros(hidden2);
        let bn2_running_var = Array1::ones(hidden2);

        // Adam states
        let mw1 = Array2::zeros((hidden1, input));
        let mb1 = Array1::zeros(hidden1);
        let mw2 = Array2::zeros((hidden2, hidden1));
        let mb2 = Array1::zeros(hidden2);
        let mw3 = Array2::zeros((output, hidden2));
        let mb3 = Array1::zeros(output);

        let sw1 = Array2::zeros((hidden1, input));
        let sb1 = Array1::zeros(hidden1);
        let sw2 = Array2::zeros((hidden2, hidden1));
        let sb2 = Array1::zeros(hidden2);
        let sw3 = Array2::zeros((output, hidden2));
        let sb3 = Array1::zeros(output);

        Self {
            w1, b1, w2, b2, w3, b3,
            bn1_gamma, bn1_beta, bn1_running_mean, bn1_running_var,
            bn2_gamma, bn2_beta, bn2_running_mean, bn2_running_var,
            mw1, mb1, mw2, mb2, mw3, mb3,
            sw1, sb1, sw2, sb2, sw3, sb3,
            t: 0.0,
            input_dim: input,
            hidden1_dim: hidden1,
            hidden2_dim: hidden2,
            output_dim: output,
            dropout_rate: 0.3,
            is_training: true,
            lr_schedule: LRSchedule::Cosine {
                initial: 0.001,
                min: 0.00001,
                period: 1000
            },
        }
    }

    fn batch_norm(&self, x: &Array1<f32>, gamma: &Array1<f32>, beta: &Array1<f32>,
                  running_mean: &Array1<f32>, running_var: &Array1<f32>) -> Array1<f32> {
        if self.is_training {
            let mean = x.mean().unwrap();
            let var = x.var(0.0);
            let x_norm = (x - mean) / (var + 1e-5).sqrt();
            gamma * &x_norm + beta
        } else {
            let x_norm = (x - running_mean) / (running_var + 1e-5).mapv(f32::sqrt);
            gamma * &x_norm + beta
        }
    }

    fn dropout(&self, x: &Array1<f32>) -> Array1<f32> {
        if self.is_training && self.dropout_rate > 0.0 {
            let mut rng = thread_rng();
            x.mapv(|v| if rng.gen::<f32>() > self.dropout_rate {
                v / (1.0 - self.dropout_rate)
            } else {
                0.0
            })
        } else {
            x.clone()
        }
    }

    fn leaky_relu(x: &Array1<f32>) -> Array1<f32> {
        x.mapv(|v| if v > 0.0 { v } else { 0.01 * v })
    }

    pub fn forward(&self, x: &Array1<f32>) -> (Array1<f32>, Array1<f32>, Array1<f32>) {
        // Layer 1: Linear + BatchNorm + LeakyReLU + Dropout
        let z1 = self.w1.dot(x) + &self.b1;
        let bn1 = self.batch_norm(&z1, &self.bn1_gamma, &self.bn1_beta,
                                  &self.bn1_running_mean, &self.bn1_running_var);
        let h1 = Self::leaky_relu(&bn1);
        let h1_drop = self.dropout(&h1);

        // Layer 2: Linear + BatchNorm + LeakyReLU + Dropout
        let z2 = self.w2.dot(&h1_drop) + &self.b2;
        let bn2 = self.batch_norm(&z2, &self.bn2_gamma, &self.bn2_beta,
                                  &self.bn2_running_mean, &self.bn2_running_var);
        let h2 = Self::leaky_relu(&bn2);
        let h2_drop = self.dropout(&h2);

        // Output layer: Linear (no activation, will apply softmax in loss)
        let logits = self.w3.dot(&h2_drop) + &self.b3;

        (logits, h1_drop, h2_drop)
    }

    fn softmax(logits: &Array1<f32>) -> Array1<f32> {
        let max = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp_vals = logits.mapv(|v| (v - max).exp());
        let sum = exp_vals.sum();
        exp_vals / sum
    }

    fn get_lr(&self, epoch: usize) -> f32 {
        match &self.lr_schedule {
            LRSchedule::Constant(lr) => *lr,
            LRSchedule::Cosine { initial, min, period } => {
                let progress = (epoch % period) as f32 / *period as f32;
                min + (initial - min) * (1.0 + (std::f32::consts::PI * progress).cos()) / 2.0
            }
            LRSchedule::StepDecay { initial, decay, step_size } => {
                initial * decay.powi((epoch / step_size) as i32)
            }
        }
    }

    pub fn backward(&mut self, x: &Array1<f32>, y_class: usize, epoch: usize) {
        let (logits, h1, h2) = self.forward(x);
        let probs = Self::softmax(&logits);

        // Cross-entropy gradient
        let mut grad_logits = probs;
        if y_class < self.output_dim {
            grad_logits[y_class] -= 1.0;
        }

        // Gradient w.r.t W3, b3
        let grad_w3 = grad_logits.clone().insert_axis(Axis(1)) * h2.clone().insert_axis(Axis(0));
        let grad_b3 = grad_logits.clone();

        // Backprop to h2
        let grad_h2 = self.w3.t().dot(&grad_logits);
        let grad_h2_relu = grad_h2 * h2.mapv(|v| if v > 0.0 { 1.0 } else { 0.01 });

        // Gradient w.r.t W2, b2
        let grad_w2 = grad_h2_relu.clone().insert_axis(Axis(1)) * h1.clone().insert_axis(Axis(0));
        let grad_b2 = grad_h2_relu.clone();

        // Backprop to h1
        let grad_h1 = self.w2.t().dot(&grad_h2_relu);
        let grad_h1_relu = grad_h1 * h1.mapv(|v| if v > 0.0 { 1.0 } else { 0.01 });

        // Gradient w.r.t W1, b1
        let grad_w1 = grad_h1_relu.clone().insert_axis(Axis(1)) * x.clone().insert_axis(Axis(0));
        let grad_b1 = grad_h1_relu;

        // Adam update with scheduled learning rate
        let lr = self.get_lr(epoch);
        self.adam_update(
            grad_w1, grad_b1,
            grad_w2.into_shape(self.w2.dim()).unwrap(), grad_b2,
            grad_w3.into_shape(self.w3.dim()).unwrap(), grad_b3,
            lr
        );
    }

    fn adam_update(&mut self, grad_w1: Array2<f32>, grad_b1: Array1<f32>,
                   grad_w2: Array2<f32>, grad_b2: Array1<f32>,
                   grad_w3: Array2<f32>, grad_b3: Array1<f32>, lr: f32) {
        let beta1 = 0.9;
        let beta2 = 0.999;
        let epsilon = 1e-8;

        self.t += 1.0;

        // Update first moments
        self.mw1 = &self.mw1 * beta1 + &grad_w1 * (1.0 - beta1);
        self.mb1 = &self.mb1 * beta1 + &grad_b1 * (1.0 - beta1);
        self.mw2 = &self.mw2 * beta1 + &grad_w2 * (1.0 - beta1);
        self.mb2 = &self.mb2 * beta1 + &grad_b2 * (1.0 - beta1);
        self.mw3 = &self.mw3 * beta1 + &grad_w3 * (1.0 - beta1);
        self.mb3 = &self.mb3 * beta1 + &grad_b3 * (1.0 - beta1);

        // Update second moments
        self.sw1 = &self.sw1 * beta2 + grad_w1.mapv(|x| x * x) * (1.0 - beta2);
        self.sb1 = &self.sb1 * beta2 + grad_b1.mapv(|x| x * x) * (1.0 - beta2);
        self.sw2 = &self.sw2 * beta2 + grad_w2.mapv(|x| x * x) * (1.0 - beta2);
        self.sb2 = &self.sb2 * beta2 + grad_b2.mapv(|x| x * x) * (1.0 - beta2);
        self.sw3 = &self.sw3 * beta2 + grad_w3.mapv(|x| x * x) * (1.0 - beta2);
        self.sb3 = &self.sb3 * beta2 + grad_b3.mapv(|x| x * x) * (1.0 - beta2);

        // Bias correction
        let bias1 = 1.0 - beta1.powf(self.t);
        let bias2 = 1.0 - beta2.powf(self.t);

        // Update weights
        self.w1 = &self.w1 - lr * &self.mw1 / bias1 / ((&self.sw1 / bias2).mapv(f32::sqrt) + epsilon);
        self.b1 = &self.b1 - lr * &self.mb1 / bias1 / ((&self.sb1 / bias2).mapv(f32::sqrt) + epsilon);
        self.w2 = &self.w2 - lr * &self.mw2 / bias1 / ((&self.sw2 / bias2).mapv(f32::sqrt) + epsilon);
        self.b2 = &self.b2 - lr * &self.mb2 / bias1 / ((&self.sb2 / bias2).mapv(f32::sqrt) + epsilon);
        self.w3 = &self.w3 - lr * &self.mw3 / bias1 / ((&self.sw3 / bias2).mapv(f32::sqrt) + epsilon);
        self.b3 = &self.b3 - lr * &self.mb3 / bias1 / ((&self.sb3 / bias2).mapv(f32::sqrt) + epsilon);
    }

    pub fn train_classification(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>,
                               epochs: usize, batch_size: usize) {
        self.is_training = true;

        for epoch in 0..epochs {
            let mut indices: Vec<usize> = (0..x.len()).collect();
            let mut rng = thread_rng();
            use rand::seq::SliceRandom;
            indices.shuffle(&mut rng);

            for batch_start in (0..x.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(x.len());

                for i in batch_start..batch_end {
                    let idx = indices[i];
                    let x_arr = Array1::from_vec(x[idx].clone());

                    // Convert continuous y to class
                    let y_class = if y[idx] < -0.25 { 0 }
                                 else if y[idx] > 0.25 { 2 }
                                 else { 1 };

                    self.backward(&x_arr, y_class, epoch);
                }

                // Update batch norm running stats
                self.bn1_running_mean = &self.bn1_running_mean * 0.9 + &self.bn1_beta * 0.1;
                self.bn1_running_var = &self.bn1_running_var * 0.9 + &self.bn1_gamma * 0.1;
                self.bn2_running_mean = &self.bn2_running_mean * 0.9 + &self.bn2_beta * 0.1;
                self.bn2_running_var = &self.bn2_running_var * 0.9 + &self.bn2_gamma * 0.1;
            }
        }
    }

    pub fn predict_cls3(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        self.is_training = false;

        x.iter().map(|xi| {
            let x_arr = Array1::from_vec(xi.clone());
            let (logits, _, _) = self.forward(&x_arr);
            let probs = Self::softmax(&logits);

            // Return argmax
            let mut best = 0;
            for i in 1..self.output_dim.min(3) {
                if probs[i] > probs[best] {
                    best = i;
                }
            }
            best
        }).collect()
    }
}