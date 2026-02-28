//! RVF examples crate.
//!
//! This crate contains example binaries demonstrating usage of the RVF
//! (RuVector Format) crates. Run individual examples with:
//!
//! ```bash
//! cargo run --example basic_store
//! cargo run --example progressive_index
//! cargo run --example quantization
//! cargo run --example wire_format
//! cargo run --example crypto_signing
//! cargo run --example filtered_search
//! ```
//!
//! Solver integration examples (sublinear solver + RVF):
//!
//! ```bash
//! cargo run --example solver_witness        # convergence witness chains
//! cargo run --example sparse_matrix_store   # CSR sparse matrix storage
//! cargo run --example solver_benchmark      # benchmark result analysis
//! ```
