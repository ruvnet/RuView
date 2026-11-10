/// Combine activity-score-derived count with per-node count estimates.
///
/// Prevents activity saturation from holding count at 1 when nodes detect multiple persons.
pub fn aggregate_person_count<I>(activity_count: usize, fresh_node_counts: I) -> usize
where
    I: IntoIterator<Item = usize>,
{
    let node_max = fresh_node_counts.into_iter().max().unwrap_or(0);
    activity_count.max(node_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_nodes_fall_back_to_activity_count() {
        let counts: Vec<usize> = vec![];
        assert_eq!(aggregate_person_count(1, counts.clone()), 1);
        assert_eq!(aggregate_person_count(0, counts), 0);
    }

    #[test]
    fn node_estimate_raises_saturated_activity_count() {
        let counts = vec![2];
        assert_eq!(aggregate_person_count(1, counts), 2);
    }
}

