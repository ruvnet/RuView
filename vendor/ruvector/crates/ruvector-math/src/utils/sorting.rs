//! Sorting utilities for optimal transport

/// Argsort: returns indices that would sort the array
pub fn argsort(data: &[f64]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..data.len()).collect();
    indices.sort_by(|&a, &b| {
        data[a]
            .partial_cmp(&data[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    indices
}

/// Sort with indices: returns (sorted_data, original_indices)
pub fn sort_with_indices(data: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let indices = argsort(data);
    let sorted: Vec<f64> = indices.iter().map(|&i| data[i]).collect();
    (sorted, indices)
}

/// Quantile of sorted data (0.0 to 1.0)
pub fn quantile_sorted(sorted_data: &[f64], q: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let q = q.clamp(0.0, 1.0);
    let n = sorted_data.len();

    if n == 1 {
        return sorted_data[0];
    }

    let idx_f = q * (n - 1) as f64;
    let idx_low = idx_f.floor() as usize;
    let idx_high = (idx_low + 1).min(n - 1);
    let frac = idx_f - idx_low as f64;

    sorted_data[idx_low] * (1.0 - frac) + sorted_data[idx_high] * frac
}

/// Compute cumulative distribution function values
pub fn compute_cdf(weights: &[f64]) -> Vec<f64> {
    let total: f64 = weights.iter().sum();
    let mut cdf = Vec::with_capacity(weights.len());
    let mut cumsum = 0.0;

    for &w in weights {
        cumsum += w / total;
        cdf.push(cumsum);
    }

    cdf
}

/// Weighted quantile
pub fn weighted_quantile(values: &[f64], weights: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let indices = argsort(values);
    let sorted_values: Vec<f64> = indices.iter().map(|&i| values[i]).collect();
    let sorted_weights: Vec<f64> = indices.iter().map(|&i| weights[i]).collect();

    let cdf = compute_cdf(&sorted_weights);
    let q = q.clamp(0.0, 1.0);

    // Find the value at quantile q
    for (i, &c) in cdf.iter().enumerate() {
        if c >= q {
            return sorted_values[i];
        }
    }

    sorted_values[sorted_values.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argsort() {
        let data = vec![3.0, 1.0, 2.0];
        let indices = argsort(&data);
        assert_eq!(indices, vec![1, 2, 0]);
    }

    #[test]
    fn test_quantile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert!((quantile_sorted(&data, 0.0) - 1.0).abs() < 1e-10);
        assert!((quantile_sorted(&data, 0.5) - 3.0).abs() < 1e-10);
        assert!((quantile_sorted(&data, 1.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_cdf() {
        let weights = vec![0.25, 0.25, 0.25, 0.25];
        let cdf = compute_cdf(&weights);

        assert!((cdf[0] - 0.25).abs() < 1e-10);
        assert!((cdf[1] - 0.50).abs() < 1e-10);
        assert!((cdf[2] - 0.75).abs() < 1e-10);
        assert!((cdf[3] - 1.00).abs() < 1e-10);
    }
}
