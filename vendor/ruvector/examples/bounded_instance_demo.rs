//! Demonstration of BoundedInstance using DeterministicLocalKCut
//!
//! This example shows how to use the production BoundedInstance
//! implementation with the LocalKCut oracle.

use ruvector_mincut::prelude::*;

fn main() {
    println!("BoundedInstance Demo");
    println!("===================\n");

    // Create a dynamic graph
    let graph = DynamicGraph::new();

    // Create a bounded instance for range [1, 5]
    let mut instance = BoundedInstance::init(&graph, 1, 5);

    println!("Created BoundedInstance with bounds: {:?}", instance.bounds());

    // Add a simple path graph: 0 -- 1 -- 2
    println!("\nAdding path graph: 0 -- 1 -- 2");
    instance.apply_inserts(&[
        (0, 0, 1),
        (1, 1, 2),
    ]);

    // Query the minimum cut
    match instance.query() {
        InstanceResult::ValueInRange { value, witness } => {
            println!("Found cut with value: {}", value);
            println!("Witness seed: {}", witness.seed());
            println!("Witness cardinality: {}", witness.cardinality());
        }
        InstanceResult::AboveRange => {
            println!("Cut value is above range");
        }
    }

    // Add edge to form a cycle: 0 -- 1 -- 2 -- 0
    println!("\nAdding edge to form cycle: 2 -- 0");
    instance.apply_inserts(&[(2, 2, 0)]);

    // Query again
    match instance.query() {
        InstanceResult::ValueInRange { value, witness } => {
            println!("Found cut with value: {}", value);
            println!("Witness seed: {}", witness.seed());
            println!("Witness cardinality: {}", witness.cardinality());
        }
        InstanceResult::AboveRange => {
            println!("Cut value is above range");
        }
    }

    // Delete an edge to break the cycle
    println!("\nDeleting edge: 1 -- 2");
    instance.apply_deletes(&[(1, 1, 2)]);

    // Query final state
    match instance.query() {
        InstanceResult::ValueInRange { value, witness } => {
            println!("Found cut with value: {}", value);
            println!("Witness seed: {}", witness.seed());
        }
        InstanceResult::AboveRange => {
            println!("Cut value is above range");
        }
    }

    // Get certificate
    let cert = instance.certificate();
    println!("\nCertificate has {} LocalKCut responses", cert.localkcut_responses.len());
}
