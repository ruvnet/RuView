//! Aggregation functions for combining neighbor messages in GNNs

use rayon::prelude::*;

/// Aggregation methods for combining neighbor messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationMethod {
    /// Sum all neighbor messages
    Sum,
    /// Average all neighbor messages
    Mean,
    /// Take maximum of neighbor messages (element-wise)
    Max,
}

impl AggregationMethod {
    /// Parse aggregation method from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sum" => Some(AggregationMethod::Sum),
            "mean" | "avg" => Some(AggregationMethod::Mean),
            "max" => Some(AggregationMethod::Max),
            _ => None,
        }
    }
}

/// Sum aggregation: sum all neighbor messages
///
/// # Arguments
/// * `messages` - Vector of messages from neighbors
///
/// # Returns
/// Sum of all messages
pub fn sum_aggregate(messages: Vec<Vec<f32>>) -> Vec<f32> {
    if messages.is_empty() {
        return vec![];
    }

    let dim = messages[0].len();
    let mut result = vec![0.0; dim];

    for message in messages {
        for (i, &val) in message.iter().enumerate() {
            result[i] += val;
        }
    }

    result
}

/// Mean aggregation: average all neighbor messages
///
/// # Arguments
/// * `messages` - Vector of messages from neighbors
///
/// # Returns
/// Mean of all messages
pub fn mean_aggregate(messages: Vec<Vec<f32>>) -> Vec<f32> {
    if messages.is_empty() {
        return vec![];
    }

    let count = messages.len() as f32;
    let sum = sum_aggregate(messages);

    sum.into_par_iter().map(|x| x / count).collect()
}

/// Max aggregation: element-wise maximum of all neighbor messages
///
/// # Arguments
/// * `messages` - Vector of messages from neighbors
///
/// # Returns
/// Element-wise maximum of all messages
pub fn max_aggregate(messages: Vec<Vec<f32>>) -> Vec<f32> {
    if messages.is_empty() {
        return vec![];
    }

    let dim = messages[0].len();
    let mut result = vec![f32::NEG_INFINITY; dim];

    for message in messages {
        for (i, &val) in message.iter().enumerate() {
            result[i] = result[i].max(val);
        }
    }

    result
}

/// Generic aggregation function that selects the appropriate aggregator
pub fn aggregate(messages: Vec<Vec<f32>>, method: AggregationMethod) -> Vec<f32> {
    match method {
        AggregationMethod::Sum => sum_aggregate(messages),
        AggregationMethod::Mean => mean_aggregate(messages),
        AggregationMethod::Max => max_aggregate(messages),
    }
}

/// Weighted aggregation - multiply each message by its weight before aggregating
pub fn weighted_aggregate(
    messages: Vec<Vec<f32>>,
    weights: &[f32],
    method: AggregationMethod,
) -> Vec<f32> {
    if messages.is_empty() {
        return vec![];
    }

    // Apply weights to messages
    let weighted_messages: Vec<Vec<f32>> = messages
        .into_par_iter()
        .enumerate()
        .map(|(idx, msg)| {
            let weight = if idx < weights.len() {
                weights[idx]
            } else {
                1.0
            };
            msg.iter().map(|&x| x * weight).collect()
        })
        .collect();

    aggregate(weighted_messages, method)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_aggregate() {
        let messages = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

        let result = sum_aggregate(messages);

        assert_eq!(result, vec![9.0, 12.0]);
    }

    #[test]
    fn test_mean_aggregate() {
        let messages = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

        let result = mean_aggregate(messages);

        assert_eq!(result, vec![3.0, 4.0]);
    }

    #[test]
    fn test_max_aggregate() {
        let messages = vec![vec![1.0, 6.0], vec![5.0, 2.0], vec![3.0, 4.0]];

        let result = max_aggregate(messages);

        assert_eq!(result, vec![5.0, 6.0]);
    }

    #[test]
    fn test_empty_messages() {
        let messages: Vec<Vec<f32>> = vec![];
        let empty: Vec<f32> = vec![];

        assert_eq!(sum_aggregate(messages.clone()), empty);
        assert_eq!(mean_aggregate(messages.clone()), empty.clone());
        assert_eq!(max_aggregate(messages), empty);
    }

    #[test]
    fn test_weighted_aggregate() {
        let messages = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let weights = vec![2.0, 0.5];

        let result = weighted_aggregate(messages, &weights, AggregationMethod::Sum);

        // [1*2, 2*2] + [3*0.5, 4*0.5] = [2, 4] + [1.5, 2] = [3.5, 6]
        assert_eq!(result, vec![3.5, 6.0]);
    }

    #[test]
    fn test_aggregation_method_from_str() {
        assert_eq!(
            AggregationMethod::from_str("sum"),
            Some(AggregationMethod::Sum)
        );
        assert_eq!(
            AggregationMethod::from_str("mean"),
            Some(AggregationMethod::Mean)
        );
        assert_eq!(
            AggregationMethod::from_str("max"),
            Some(AggregationMethod::Max)
        );
        assert_eq!(AggregationMethod::from_str("invalid"), None);
    }
}
