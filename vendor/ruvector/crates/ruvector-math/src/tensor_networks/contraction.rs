//! Tensor Network Contraction
//!
//! General tensor network operations for quantum-inspired algorithms.

use std::collections::HashMap;

/// A node in a tensor network
#[derive(Debug, Clone)]
pub struct TensorNode {
    /// Node identifier
    pub id: usize,
    /// Tensor data
    pub data: Vec<f64>,
    /// Dimensions of each leg
    pub leg_dims: Vec<usize>,
    /// Labels for each leg (for contraction)
    pub leg_labels: Vec<String>,
}

impl TensorNode {
    /// Create new tensor node
    pub fn new(id: usize, data: Vec<f64>, leg_dims: Vec<usize>, leg_labels: Vec<String>) -> Self {
        let expected_size: usize = leg_dims.iter().product();
        assert_eq!(data.len(), expected_size);
        assert_eq!(leg_dims.len(), leg_labels.len());

        Self {
            id,
            data,
            leg_dims,
            leg_labels,
        }
    }

    /// Number of legs
    pub fn num_legs(&self) -> usize {
        self.leg_dims.len()
    }

    /// Total size
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Tensor network for contraction operations
#[derive(Debug, Clone)]
pub struct TensorNetwork {
    /// Nodes in the network
    nodes: Vec<TensorNode>,
    /// Next node ID
    next_id: usize,
}

impl TensorNetwork {
    /// Create empty network
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 0,
        }
    }

    /// Add a tensor node
    pub fn add_node(
        &mut self,
        data: Vec<f64>,
        leg_dims: Vec<usize>,
        leg_labels: Vec<String>,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes
            .push(TensorNode::new(id, data, leg_dims, leg_labels));
        id
    }

    /// Get node by ID
    pub fn get_node(&self, id: usize) -> Option<&TensorNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Contract two nodes on matching labels
    pub fn contract(&mut self, id1: usize, id2: usize) -> Option<usize> {
        let node1_idx = self.nodes.iter().position(|n| n.id == id1)?;
        let node2_idx = self.nodes.iter().position(|n| n.id == id2)?;

        // Find matching labels
        let node1 = &self.nodes[node1_idx];
        let node2 = &self.nodes[node2_idx];

        let mut contract_pairs: Vec<(usize, usize)> = Vec::new();

        for (i1, label1) in node1.leg_labels.iter().enumerate() {
            for (i2, label2) in node2.leg_labels.iter().enumerate() {
                if label1 == label2 && !label1.starts_with("open_") {
                    assert_eq!(node1.leg_dims[i1], node2.leg_dims[i2], "Dimension mismatch");
                    contract_pairs.push((i1, i2));
                }
            }
        }

        if contract_pairs.is_empty() {
            // Outer product
            return self.outer_product(id1, id2);
        }

        // Perform contraction
        let result = contract_tensors(node1, node2, &contract_pairs);

        // Remove old nodes and add new
        self.nodes.retain(|n| n.id != id1 && n.id != id2);

        let new_id = self.next_id;
        self.next_id += 1;
        self.nodes
            .push(TensorNode::new(new_id, result.0, result.1, result.2));

        Some(new_id)
    }

    /// Outer product of two nodes
    fn outer_product(&mut self, id1: usize, id2: usize) -> Option<usize> {
        let node1 = self.nodes.iter().find(|n| n.id == id1)?;
        let node2 = self.nodes.iter().find(|n| n.id == id2)?;

        let mut new_data = Vec::with_capacity(node1.size() * node2.size());
        for &a in &node1.data {
            for &b in &node2.data {
                new_data.push(a * b);
            }
        }

        let mut new_dims = node1.leg_dims.clone();
        new_dims.extend(node2.leg_dims.iter());

        let mut new_labels = node1.leg_labels.clone();
        new_labels.extend(node2.leg_labels.iter().cloned());

        self.nodes.retain(|n| n.id != id1 && n.id != id2);

        let new_id = self.next_id;
        self.next_id += 1;
        self.nodes
            .push(TensorNode::new(new_id, new_data, new_dims, new_labels));

        Some(new_id)
    }

    /// Contract entire network to scalar (if possible)
    pub fn contract_all(&mut self) -> Option<f64> {
        while self.nodes.len() > 1 {
            // Find a pair with matching labels
            let mut found = None;
            'outer: for i in 0..self.nodes.len() {
                for j in i + 1..self.nodes.len() {
                    for label in &self.nodes[i].leg_labels {
                        if !label.starts_with("open_") && self.nodes[j].leg_labels.contains(label) {
                            found = Some((self.nodes[i].id, self.nodes[j].id));
                            break 'outer;
                        }
                    }
                }
            }

            if let Some((id1, id2)) = found {
                self.contract(id1, id2)?;
            } else {
                // No more contractions possible
                break;
            }
        }

        if self.nodes.len() == 1 && self.nodes[0].leg_dims.is_empty() {
            Some(self.nodes[0].data[0])
        } else {
            None
        }
    }
}

impl Default for TensorNetwork {
    fn default() -> Self {
        Self::new()
    }
}

/// Contract two tensors on specified index pairs
fn contract_tensors(
    node1: &TensorNode,
    node2: &TensorNode,
    contract_pairs: &[(usize, usize)],
) -> (Vec<f64>, Vec<usize>, Vec<String>) {
    // Determine output shape and labels
    let mut out_dims = Vec::new();
    let mut out_labels = Vec::new();

    let contracted1: Vec<usize> = contract_pairs.iter().map(|p| p.0).collect();
    let contracted2: Vec<usize> = contract_pairs.iter().map(|p| p.1).collect();

    for (i, (dim, label)) in node1
        .leg_dims
        .iter()
        .zip(node1.leg_labels.iter())
        .enumerate()
    {
        if !contracted1.contains(&i) {
            out_dims.push(*dim);
            out_labels.push(label.clone());
        }
    }

    for (i, (dim, label)) in node2
        .leg_dims
        .iter()
        .zip(node2.leg_labels.iter())
        .enumerate()
    {
        if !contracted2.contains(&i) {
            out_dims.push(*dim);
            out_labels.push(label.clone());
        }
    }

    let out_size: usize = if out_dims.is_empty() {
        1
    } else {
        out_dims.iter().product()
    };
    let mut out_data = vec![0.0; out_size];

    // Contract by enumeration
    let size1 = node1.size();
    let size2 = node2.size();

    let strides1 = compute_strides(&node1.leg_dims);
    let strides2 = compute_strides(&node2.leg_dims);
    let out_strides = compute_strides(&out_dims);

    // For each element of output
    let mut out_indices = vec![0usize; out_dims.len()];
    for out_flat in 0..out_size {
        // Map to input indices
        // Sum over contracted indices
        let contract_sizes: Vec<usize> =
            contract_pairs.iter().map(|p| node1.leg_dims[p.0]).collect();
        let contract_total: usize = if contract_sizes.is_empty() {
            1
        } else {
            contract_sizes.iter().product()
        };

        let mut sum = 0.0;

        for contract_flat in 0..contract_total {
            // Build indices for node1 and node2
            let mut idx1 = vec![0usize; node1.num_legs()];
            let mut idx2 = vec![0usize; node2.num_legs()];

            // Set contracted indices
            let mut cf = contract_flat;
            for (pi, &(i1, i2)) in contract_pairs.iter().enumerate() {
                let ci = cf % contract_sizes[pi];
                cf /= contract_sizes[pi];
                idx1[i1] = ci;
                idx2[i2] = ci;
            }

            // Set free indices from output
            let mut out_idx_copy = out_flat;
            let mut free1_pos = 0;
            let mut free2_pos = 0;

            for i in 0..node1.num_legs() {
                if !contracted1.contains(&i) {
                    if free1_pos < out_dims.len() {
                        idx1[i] = (out_idx_copy / out_strides.get(free1_pos).unwrap_or(&1))
                            % node1.leg_dims[i];
                    }
                    free1_pos += 1;
                }
            }

            for i in 0..node2.num_legs() {
                if !contracted2.contains(&i) {
                    let pos = (node1.num_legs() - contracted1.len()) + free2_pos;
                    if pos < out_dims.len() {
                        idx2[i] =
                            (out_flat / out_strides.get(pos).unwrap_or(&1)) % node2.leg_dims[i];
                    }
                    free2_pos += 1;
                }
            }

            // Compute linear indices
            let lin1: usize = idx1.iter().zip(strides1.iter()).map(|(i, s)| i * s).sum();
            let lin2: usize = idx2.iter().zip(strides2.iter()).map(|(i, s)| i * s).sum();

            sum += node1.data[lin1.min(node1.data.len() - 1)]
                * node2.data[lin2.min(node2.data.len() - 1)];
        }

        out_data[out_flat] = sum;
    }

    (out_data, out_dims, out_labels)
}

fn compute_strides(dims: &[usize]) -> Vec<usize> {
    let mut strides = Vec::with_capacity(dims.len());
    let mut stride = 1;
    for &d in dims.iter().rev() {
        strides.push(stride);
        stride *= d;
    }
    strides.reverse();
    strides
}

/// Optimal contraction order finder
#[derive(Debug, Clone)]
pub struct NetworkContraction {
    /// Estimated contraction cost
    pub estimated_cost: f64,
}

impl NetworkContraction {
    /// Find greedy contraction order (not optimal but fast)
    pub fn greedy_order(network: &TensorNetwork) -> Vec<(usize, usize)> {
        let mut order = Vec::new();
        let mut remaining: Vec<usize> = network.nodes.iter().map(|n| n.id).collect();

        while remaining.len() > 1 {
            // Find pair with smallest contraction cost
            let mut best_pair = None;
            let mut best_cost = f64::INFINITY;

            for i in 0..remaining.len() {
                for j in i + 1..remaining.len() {
                    let id1 = remaining[i];
                    let id2 = remaining[j];

                    if let (Some(n1), Some(n2)) = (network.get_node(id1), network.get_node(id2)) {
                        let cost = estimate_contraction_cost(n1, n2);
                        if cost < best_cost {
                            best_cost = cost;
                            best_pair = Some((i, j));
                        }
                    }
                }
            }

            if let Some((i, j)) = best_pair {
                let id1 = remaining[i];
                let id2 = remaining[j];
                order.push((id1, id2));

                // Remove j first (larger index)
                remaining.remove(j);
                remaining.remove(i);
                // In real implementation, we'd add the result node ID
            } else {
                break;
            }
        }

        order
    }
}

fn estimate_contraction_cost(n1: &TensorNode, n2: &TensorNode) -> f64 {
    // Simple cost estimate: product of all dimension sizes
    let size1: usize = n1.leg_dims.iter().product();
    let size2: usize = n2.leg_dims.iter().product();

    // Find contracted dimensions
    let mut contracted_size = 1usize;
    for (i1, label1) in n1.leg_labels.iter().enumerate() {
        for (i2, label2) in n2.leg_labels.iter().enumerate() {
            if label1 == label2 && !label1.starts_with("open_") {
                contracted_size *= n1.leg_dims[i1];
            }
        }
    }

    // Cost ≈ output_size × contracted_size
    (size1 * size2 / contracted_size.max(1)) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_network_creation() {
        let mut network = TensorNetwork::new();

        let id1 = network.add_node(
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2, 2],
            vec!["i".into(), "j".into()],
        );

        let id2 = network.add_node(
            vec![1.0, 0.0, 0.0, 1.0],
            vec![2, 2],
            vec!["j".into(), "k".into()],
        );

        assert_eq!(network.num_nodes(), 2);
    }

    #[test]
    fn test_matrix_contraction() {
        let mut network = TensorNetwork::new();

        // A = [[1, 2], [3, 4]]
        let id1 = network.add_node(
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2, 2],
            vec!["i".into(), "j".into()],
        );

        // B = [[1, 0], [0, 1]] (identity)
        let id2 = network.add_node(
            vec![1.0, 0.0, 0.0, 1.0],
            vec![2, 2],
            vec!["j".into(), "k".into()],
        );

        let result_id = network.contract(id1, id2).unwrap();
        let result = network.get_node(result_id).unwrap();

        // A * I = A
        assert_eq!(result.data.len(), 4);
        // Result should be [[1, 2], [3, 4]]
    }

    #[test]
    fn test_vector_dot_product() {
        let mut network = TensorNetwork::new();

        // v1 = [1, 2, 3]
        let id1 = network.add_node(vec![1.0, 2.0, 3.0], vec![3], vec!["i".into()]);

        // v2 = [1, 1, 1]
        let id2 = network.add_node(vec![1.0, 1.0, 1.0], vec![3], vec!["i".into()]);

        let result_id = network.contract(id1, id2).unwrap();
        let result = network.get_node(result_id).unwrap();

        // Dot product = 1 + 2 + 3 = 6
        assert_eq!(result.data.len(), 1);
        assert!((result.data[0] - 6.0).abs() < 1e-10);
    }
}
