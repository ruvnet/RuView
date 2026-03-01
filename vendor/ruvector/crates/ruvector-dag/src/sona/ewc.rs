//! EWC++: Elastic Weight Consolidation to prevent forgetting

use ndarray::Array1;

#[derive(Debug, Clone)]
pub struct EwcConfig {
    pub lambda: f32,  // Importance weight (2000-15000)
    pub decay: f32,   // Fisher decay rate
    pub online: bool, // Use online EWC
}

impl Default for EwcConfig {
    fn default() -> Self {
        Self {
            lambda: 5000.0,
            decay: 0.99,
            online: true,
        }
    }
}

pub struct EwcPlusPlus {
    config: EwcConfig,
    fisher_diag: Option<Array1<f32>>,
    optimal_params: Option<Array1<f32>>,
    task_count: usize,
}

impl EwcPlusPlus {
    pub fn new(config: EwcConfig) -> Self {
        Self {
            config,
            fisher_diag: None,
            optimal_params: None,
            task_count: 0,
        }
    }

    /// Consolidate current parameters after training
    pub fn consolidate(&mut self, params: &Array1<f32>, fisher: &Array1<f32>) {
        if self.config.online && self.fisher_diag.is_some() {
            // Online EWC: accumulate Fisher information
            let current_fisher = self.fisher_diag.as_ref().unwrap();
            self.fisher_diag =
                Some(current_fisher * self.config.decay + fisher * (1.0 - self.config.decay));
        } else {
            self.fisher_diag = Some(fisher.clone());
        }

        self.optimal_params = Some(params.clone());
        self.task_count += 1;
    }

    /// Compute EWC penalty for given parameters
    pub fn penalty(&self, params: &Array1<f32>) -> f32 {
        match (&self.fisher_diag, &self.optimal_params) {
            (Some(fisher), Some(optimal)) => {
                let diff = params - optimal;
                let weighted = &diff * &diff * fisher;
                0.5 * self.config.lambda * weighted.sum()
            }
            _ => 0.0,
        }
    }

    /// Compute gradient of EWC penalty
    pub fn penalty_gradient(&self, params: &Array1<f32>) -> Option<Array1<f32>> {
        match (&self.fisher_diag, &self.optimal_params) {
            (Some(fisher), Some(optimal)) => {
                let diff = params - optimal;
                Some(self.config.lambda * fisher * &diff)
            }
            _ => None,
        }
    }

    /// Compute Fisher information from gradients
    pub fn compute_fisher(gradients: &[Array1<f32>]) -> Array1<f32> {
        if gradients.is_empty() {
            return Array1::zeros(0);
        }

        let dim = gradients[0].len();
        let mut fisher = Array1::zeros(dim);

        for grad in gradients {
            fisher = fisher + grad.mapv(|x| x * x);
        }

        fisher / gradients.len() as f32
    }

    pub fn has_prior(&self) -> bool {
        self.fisher_diag.is_some()
    }

    pub fn task_count(&self) -> usize {
        self.task_count
    }
}
