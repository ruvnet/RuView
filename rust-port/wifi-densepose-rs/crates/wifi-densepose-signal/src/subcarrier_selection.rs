//! Subcarrier Sensitivity Selection
//!
//! Ranks subcarriers by their response to human motion using variance ratio
//! (motion variance / static variance) and selects the top-K most sensitive
//! ones. This improves SNR by 6-10 dB compared to using all subcarriers.
//!
//! # References
//! - WiDance (MobiCom 2017)
//! - WiGest: Using WiFi Gestures for Device-Free Sensing (SenSys 2015)

use ndarray::Array2;
use ruvector_mincut::MinCutBuilder;

/// Configuration for subcarrier selection.
#[derive(Debug, Clone)]
pub struct SubcarrierSelectionConfig {
    /// Number of top subcarriers to select
    pub top_k: usize,
    /// Minimum sensitivity ratio to include a subcarrier
    pub min_sensitivity: f64,
}

impl Default for SubcarrierSelectionConfig {
    fn default() -> Self {
        Self {
            top_k: 20,
            min_sensitivity: 1.5,
        }
    }
}

/// Result of subcarrier selection.
#[derive(Debug, Clone)]
pub struct SubcarrierSelection {
    /// Selected subcarrier indices (sorted by sensitivity, descending)
    pub selected_indices: Vec<usize>,
    /// Sensitivity scores for ALL subcarriers (variance ratio)
    pub sensitivity_scores: Vec<f64>,
    /// The filtered data matrix containing only selected subcarrier columns
    pub selected_data: Option<Array2<f64>>,
}

/// Select the most motion-sensitive subcarriers using variance ratio.
///
/// `motion_data`: (num_samples × num_subcarriers) CSI amplitude during motion
/// `static_data`: (num_samples × num_subcarriers) CSI amplitude during static period
///
/// Sensitivity = var(motion[k]) / (var(static[k]) + ε)
pub fn select_sensitive_subcarriers(
    motion_data: &Array2<f64>,
    static_data: &Array2<f64>,
    config: &SubcarrierSelectionConfig,
) -> Result<SubcarrierSelection, SelectionError> {
    let (_, n_sc_motion) = motion_data.dim();
    let (_, n_sc_static) = static_data.dim();

    if n_sc_motion != n_sc_static {
        return Err(SelectionError::SubcarrierCountMismatch {
            motion: n_sc_motion,
            statik: n_sc_static,
        });
    }
    if n_sc_motion == 0 {
        return Err(SelectionError::NoSubcarriers);
    }

    let n_sc = n_sc_motion;
    let mut scores = Vec::with_capacity(n_sc);

    for k in 0..n_sc {
        let motion_var = column_variance(motion_data, k);
        let static_var = column_variance(static_data, k);
        let sensitivity = motion_var / (static_var + 1e-12);
        scores.push(sensitivity);
    }

    // Rank by sensitivity (descending)
    let mut ranked: Vec<(usize, f64)> = scores.iter().copied().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Select top-K above minimum threshold
    let selected: Vec<usize> = ranked
        .iter()
        .filter(|(_, score)| *score >= config.min_sensitivity)
        .take(config.top_k)
        .map(|(idx, _)| *idx)
        .collect();

    Ok(SubcarrierSelection {
        selected_indices: selected,
        sensitivity_scores: scores,
        selected_data: None,
    })
}

/// Select and extract data for sensitive subcarriers from a temporal matrix.
///
/// `data`: (num_samples × num_subcarriers) - the full CSI matrix to filter
/// `selection`: previously computed subcarrier selection
///
/// Returns a new matrix with only the selected columns.
pub fn extract_selected(
    data: &Array2<f64>,
    selection: &SubcarrierSelection,
) -> Result<Array2<f64>, SelectionError> {
    let (n_samples, n_sc) = data.dim();

    for &idx in &selection.selected_indices {
        if idx >= n_sc {
            return Err(SelectionError::IndexOutOfBounds { index: idx, max: n_sc });
        }
    }

    if selection.selected_indices.is_empty() {
        return Err(SelectionError::NoSubcarriersSelected);
    }

    let n_selected = selection.selected_indices.len();
    let mut result = Array2::zeros((n_samples, n_selected));

    for (col, &sc_idx) in selection.selected_indices.iter().enumerate() {
        for row in 0..n_samples {
            result[[row, col]] = data[[row, sc_idx]];
        }
    }

    Ok(result)
}

/// Online subcarrier selection using only variance (no separate static period).
///
/// Ranks by absolute variance — high-variance subcarriers carry more
/// information about environmental changes.
pub fn select_by_variance(
    data: &Array2<f64>,
    config: &SubcarrierSelectionConfig,
) -> SubcarrierSelection {
    let (_, n_sc) = data.dim();
    let mut scores = Vec::with_capacity(n_sc);

    for k in 0..n_sc {
        scores.push(column_variance(data, k));
    }

    let mut ranked: Vec<(usize, f64)> = scores.iter().copied().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let selected: Vec<usize> = ranked
        .iter()
        .take(config.top_k)
        .map(|(idx, _)| *idx)
        .collect();

    SubcarrierSelection {
        selected_indices: selected,
        sensitivity_scores: scores,
        selected_data: None,
    }
}

/// Compute variance of a single column in a 2D array.
fn column_variance(data: &Array2<f64>, col: usize) -> f64 {
    let n = data.nrows() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let col_data = data.column(col);
    let mean: f64 = col_data.sum() / n;
    col_data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
}

/// Partition subcarriers into (sensitive, insensitive) groups via DynamicMinCut.
///
/// Builds a similarity graph: subcarriers are vertices, edges encode inverse
/// variance-ratio distance. The min-cut separates high-sensitivity from
/// low-sensitivity subcarriers in O(n^1.5 log n) amortized time.
///
/// # Arguments
/// * `sensitivity` - Per-subcarrier sensitivity score (variance_motion / variance_static)
///
/// # Returns
/// (sensitive_indices, insensitive_indices) — indices into the input slice
pub fn mincut_subcarrier_partition(sensitivity: &[f32]) -> (Vec<usize>, Vec<usize>) {
    let n = sensitivity.len();
    if n < 4 {
        // Too small for meaningful cut — put all in sensitive
        return ((0..n).collect(), Vec::new());
    }

    // Build similarity graph: edge weight = 1 / |sensitivity_i - sensitivity_j|
    // Only include edges where weight > min_weight (prune very weak similarities)
    let min_weight = 0.5_f64;
    let mut edges: Vec<(u64, u64, f64)> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let diff = (sensitivity[i] - sensitivity[j]).abs() as f64;
            let weight = if diff > 1e-9 { 1.0 / diff } else { 1e6_f64 };
            if weight > min_weight {
                edges.push((i as u64, j as u64, weight));
            }
        }
    }

    if edges.is_empty() {
        // All subcarriers equally sensitive — split by median
        let median_idx = n / 2;
        return ((0..median_idx).collect(), (median_idx..n).collect());
    }

    let mc = MinCutBuilder::new().exact().with_edges(edges).build();
    let (side_a, side_b) = mc.partition();

    // The side with higher mean sensitivity is the "sensitive" group
    let mean_a: f32 = if side_a.is_empty() {
        0.0
    } else {
        side_a.iter().map(|&i| sensitivity[i as usize]).sum::<f32>() / side_a.len() as f32
    };
    let mean_b: f32 = if side_b.is_empty() {
        0.0
    } else {
        side_b.iter().map(|&i| sensitivity[i as usize]).sum::<f32>() / side_b.len() as f32
    };

    if mean_a >= mean_b {
        (
            side_a.into_iter().map(|x| x as usize).collect(),
            side_b.into_iter().map(|x| x as usize).collect(),
        )
    } else {
        (
            side_b.into_iter().map(|x| x as usize).collect(),
            side_a.into_iter().map(|x| x as usize).collect(),
        )
    }
}

/// Errors from subcarrier selection.
#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Subcarrier count mismatch: motion={motion}, static={statik}")]
    SubcarrierCountMismatch { motion: usize, statik: usize },

    #[error("No subcarriers in input")]
    NoSubcarriers,

    #[error("No subcarriers met selection criteria")]
    NoSubcarriersSelected,

    #[error("Subcarrier index {index} out of bounds (max {max})")]
    IndexOutOfBounds { index: usize, max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_subcarriers_ranked() {
        // 3 subcarriers: SC0 has high motion variance, SC1 low, SC2 medium
        let motion = Array2::from_shape_fn((100, 3), |(t, sc)| match sc {
            0 => (t as f64 * 0.1).sin() * 5.0,  // high variance
            1 => (t as f64 * 0.1).sin() * 0.1,  // low variance
            2 => (t as f64 * 0.1).sin() * 2.0,  // medium variance
            _ => 0.0,
        });
        let statik = Array2::from_shape_fn((100, 3), |(_, _)| 0.01);

        let config = SubcarrierSelectionConfig {
            top_k: 3,
            min_sensitivity: 0.0,
        };
        let result = select_sensitive_subcarriers(&motion, &statik, &config).unwrap();

        // SC0 should be ranked first (highest sensitivity)
        assert_eq!(result.selected_indices[0], 0);
        // SC2 should be second
        assert_eq!(result.selected_indices[1], 2);
        // SC1 should be last
        assert_eq!(result.selected_indices[2], 1);
    }

    #[test]
    fn test_top_k_limits_output() {
        let motion = Array2::from_shape_fn((50, 20), |(t, sc)| {
            (t as f64 * 0.05).sin() * (sc as f64 + 1.0)
        });
        let statik = Array2::from_elem((50, 20), 0.01);

        let config = SubcarrierSelectionConfig {
            top_k: 5,
            min_sensitivity: 0.0,
        };
        let result = select_sensitive_subcarriers(&motion, &statik, &config).unwrap();
        assert_eq!(result.selected_indices.len(), 5);
    }

    #[test]
    fn test_min_sensitivity_filter() {
        // All subcarriers have very low sensitivity
        let motion = Array2::from_elem((50, 10), 1.0);
        let statik = Array2::from_elem((50, 10), 1.0);

        let config = SubcarrierSelectionConfig {
            top_k: 10,
            min_sensitivity: 2.0, // None will pass
        };
        let result = select_sensitive_subcarriers(&motion, &statik, &config).unwrap();
        assert!(result.selected_indices.is_empty());
    }

    #[test]
    fn test_extract_selected_columns() {
        let data = Array2::from_shape_fn((10, 5), |(r, c)| (r * 5 + c) as f64);

        let selection = SubcarrierSelection {
            selected_indices: vec![1, 3],
            sensitivity_scores: vec![0.0; 5],
            selected_data: None,
        };

        let extracted = extract_selected(&data, &selection).unwrap();
        assert_eq!(extracted.dim(), (10, 2));

        // Column 0 of extracted should be column 1 of original
        for r in 0..10 {
            assert_eq!(extracted[[r, 0]], data[[r, 1]]);
            assert_eq!(extracted[[r, 1]], data[[r, 3]]);
        }
    }

    #[test]
    fn test_variance_based_selection() {
        let data = Array2::from_shape_fn((100, 5), |(t, sc)| {
            (t as f64 * 0.1).sin() * (sc as f64 + 1.0)
        });

        let config = SubcarrierSelectionConfig {
            top_k: 3,
            min_sensitivity: 0.0,
        };
        let result = select_by_variance(&data, &config);

        assert_eq!(result.selected_indices.len(), 3);
        // SC4 (highest amplitude) should be first
        assert_eq!(result.selected_indices[0], 4);
    }

    #[test]
    fn test_mismatch_error() {
        let motion = Array2::zeros((10, 5));
        let statik = Array2::zeros((10, 3));

        assert!(matches!(
            select_sensitive_subcarriers(&motion, &statik, &SubcarrierSelectionConfig::default()),
            Err(SelectionError::SubcarrierCountMismatch { .. })
        ));
    }
}

#[cfg(test)]
mod mincut_tests {
    use super::*;

    #[test]
    fn mincut_partition_separates_high_low() {
        // High sensitivity: indices 0,1,2; low: 3,4,5
        let sensitivity = vec![0.9_f32, 0.85, 0.92, 0.1, 0.12, 0.08];
        let (sensitive, insensitive) = mincut_subcarrier_partition(&sensitivity);
        // High-sensitivity indices should cluster together
        assert!(!sensitive.is_empty());
        assert!(!insensitive.is_empty());
        let sens_mean: f32 = sensitive.iter().map(|&i| sensitivity[i]).sum::<f32>() / sensitive.len() as f32;
        let insens_mean: f32 = insensitive.iter().map(|&i| sensitivity[i]).sum::<f32>() / insensitive.len() as f32;
        assert!(sens_mean > insens_mean, "sensitive mean {sens_mean} should exceed insensitive mean {insens_mean}");
    }

    #[test]
    fn mincut_partition_small_input() {
        let sensitivity = vec![0.5_f32, 0.8];
        let (sensitive, insensitive) = mincut_subcarrier_partition(&sensitivity);
        assert_eq!(sensitive.len() + insensitive.len(), 2);
    }
}
