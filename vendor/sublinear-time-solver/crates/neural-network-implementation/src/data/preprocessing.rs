//! Data preprocessing utilities

use crate::{data::TimeSeriesData, error::Result};
use nalgebra::DMatrix;

/// Normalization strategies
#[derive(Debug, Clone)]
pub enum NormalizationStrategy {
    ZScore,
    MinMax,
    Robust,
}

/// Data preprocessor
pub struct Preprocessor {
    strategy: NormalizationStrategy,
}

impl Preprocessor {
    pub fn new(strategy: NormalizationStrategy) -> Self {
        Self { strategy }
    }

    pub fn fit_transform(&self, data: &mut TimeSeriesData) -> Result<()> {
        match self.strategy {
            NormalizationStrategy::ZScore => self.z_score_normalize(&mut data.features),
            NormalizationStrategy::MinMax => self.min_max_normalize(&mut data.features),
            NormalizationStrategy::Robust => self.robust_normalize(&mut data.features),
        }
    }

    fn z_score_normalize(&self, features: &mut DMatrix<f64>) -> Result<()> {
        for i in 0..features.nrows() {
            let row_data: Vec<f64> = features.row(i).iter().cloned().collect();
            let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
            let std_dev = (row_data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / row_data.len() as f64).sqrt();

            if std_dev > 1e-8 {
                for j in 0..features.ncols() {
                    features[(i, j)] = (features[(i, j)] - mean) / std_dev;
                }
            }
        }
        Ok(())
    }

    fn min_max_normalize(&self, features: &mut DMatrix<f64>) -> Result<()> {
        for i in 0..features.nrows() {
            let row_data: Vec<f64> = features.row(i).iter().cloned().collect();
            let min_val = row_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_val = row_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            if (max_val - min_val).abs() > 1e-8 {
                for j in 0..features.ncols() {
                    features[(i, j)] = (features[(i, j)] - min_val) / (max_val - min_val);
                }
            }
        }
        Ok(())
    }

    fn robust_normalize(&self, _features: &mut DMatrix<f64>) -> Result<()> {
        // Placeholder for robust normalization (median-based)
        Ok(())
    }
}