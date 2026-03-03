//! Data loading utilities for temporal neural networks

use crate::{
    data::{TimeSeriesData, DataMetadata},
    error::{Result, TemporalNeuralError},
};
use nalgebra::DMatrix;
use std::path::Path;

/// Trait for data loaders
pub trait DataLoader {
    /// Load data from the specified path
    fn load<P: AsRef<Path>>(path: P) -> Result<TimeSeriesData>;
}

/// CSV data loader
pub struct CsvLoader;

impl DataLoader for CsvLoader {
    fn load<P: AsRef<Path>>(path: P) -> Result<TimeSeriesData> {
        let path = path.as_ref();
        let mut reader = csv::Reader::from_path(path)?;

        // Get headers
        let headers = reader.headers()?.clone();
        let feature_names: Vec<String> = headers.iter().map(|h| h.to_string()).collect();

        // Read all records
        let mut records = Vec::new();
        for result in reader.records() {
            let record = result?;
            let values: Result<Vec<f64>, _> = record.iter()
                .map(|field| field.parse::<f64>())
                .collect();

            match values {
                Ok(vals) => records.push(vals),
                Err(e) => {
                    return Err(TemporalNeuralError::DataError {
                        message: format!("Failed to parse CSV record: {}", e),
                        context: Some(path.to_string_lossy().to_string()),
                    });
                }
            }
        }

        if records.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "No data records found in CSV file".to_string(),
                context: Some(path.to_string_lossy().to_string()),
            });
        }

        let num_features = records[0].len();
        let num_samples = records.len();

        // Create matrix (features x samples)
        let mut features = DMatrix::zeros(num_features, num_samples);
        for (sample_idx, record) in records.iter().enumerate() {
            if record.len() != num_features {
                return Err(TemporalNeuralError::DataError {
                    message: format!(
                        "Inconsistent number of features at sample {}: expected {}, got {}",
                        sample_idx, num_features, record.len()
                    ),
                    context: Some(path.to_string_lossy().to_string()),
                });
            }

            for (feature_idx, &value) in record.iter().enumerate() {
                features[(feature_idx, sample_idx)] = value;
            }
        }

        // Assume 1kHz sample rate by default (can be overridden)
        let sample_rate = 1000.0;

        let metadata = DataMetadata {
            name: path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
            num_samples,
            num_features,
            duration_seconds: num_samples as f64 / sample_rate,
            source: path.to_string_lossy().to_string(),
            created_at: chrono::Utc::now(),
            preprocessing_history: Vec::new(),
        };

        Ok(TimeSeriesData {
            features,
            feature_names,
            timestamps: None,
            sample_rate,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_csv_loading() {
        // Create temporary CSV file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "x,y,vx,vy").unwrap();
        writeln!(temp_file, "1.0,2.0,0.1,0.2").unwrap();
        writeln!(temp_file, "1.1,2.1,0.15,0.25").unwrap();
        writeln!(temp_file, "1.2,2.2,0.2,0.3").unwrap();

        let data = CsvLoader::load(temp_file.path()).unwrap();

        assert_eq!(data.features.nrows(), 4); // 4 features
        assert_eq!(data.features.ncols(), 3); // 3 samples
        assert_eq!(data.feature_names.len(), 4);
        assert_eq!(data.feature_names[0], "x");
        assert_eq!(data.features[(0, 0)], 1.0);
        assert_eq!(data.features[(1, 1)], 2.1);
    }

    #[test]
    fn test_csv_loading_errors() {
        // Test with non-existent file
        let result = CsvLoader::load("non_existent_file.csv");
        assert!(result.is_err());

        // Test with malformed CSV
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "x,y").unwrap();
        writeln!(temp_file, "1.0,invalid_number").unwrap();

        let result = CsvLoader::load(temp_file.path());
        assert!(result.is_err());
    }
}