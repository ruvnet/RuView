//! Example demonstrating the Ruvector GNN layer usage

use ruvector_gnn::{RuvectorLayer, Linear, MultiHeadAttention, GRUCell, LayerNorm};

fn main() {
    println!("=== Ruvector GNN Layer Example ===\n");

    // Create a GNN layer
    // Parameters: input_dim=128, hidden_dim=256, heads=4, dropout=0.1
    let gnn_layer = RuvectorLayer::new(128, 256, 4, 0.1);

    // Simulate a node embedding (128 dimensions)
    let node_embedding = vec![0.5; 128];

    // Simulate 3 neighbor embeddings
    let neighbor_embeddings = vec![
        vec![0.3; 128],
        vec![0.7; 128],
        vec![0.5; 128],
    ];

    // Edge weights (e.g., inverse distances)
    let edge_weights = vec![0.8, 0.6, 0.4];

    // Forward pass through the GNN layer
    let updated_embedding = gnn_layer.forward(&node_embedding, &neighbor_embeddings, &edge_weights);

    println!("Input dimension: {}", node_embedding.len());
    println!("Output dimension: {}", updated_embedding.len());
    println!("Number of neighbors: {}", neighbor_embeddings.len());
    println!("\n✓ GNN layer forward pass successful!");

    // Demonstrate individual components
    println!("\n=== Individual Components ===\n");

    // 1. Linear layer
    let linear = Linear::new(128, 64);
    let linear_output = linear.forward(&node_embedding);
    println!("Linear layer: 128 -> {}", linear_output.len());

    // 2. Layer normalization
    let layer_norm = LayerNorm::new(128, 1e-5);
    let normalized = layer_norm.forward(&node_embedding);
    println!("LayerNorm output dimension: {}", normalized.len());

    // 3. Multi-head attention
    let attention = MultiHeadAttention::new(128, 4);
    let keys = neighbor_embeddings.clone();
    let values = neighbor_embeddings.clone();
    let attention_output = attention.forward(&node_embedding, &keys, &values);
    println!("Multi-head attention output: {}", attention_output.len());

    // 4. GRU cell
    let gru = GRUCell::new(128, 256);
    let hidden_state = vec![0.0; 256];
    let new_hidden = gru.forward(&node_embedding, &hidden_state);
    println!("GRU cell output dimension: {}", new_hidden.len());

    println!("\n✓ All components working correctly!");
}
