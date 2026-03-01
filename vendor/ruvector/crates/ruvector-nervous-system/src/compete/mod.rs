//! Winner-Take-All Competition Module
//!
//! Implements neural competition mechanisms for sparse activation and fast routing.
//! Based on cortical competition principles with lateral inhibition.
//!
//! # Components
//!
//! - `WTALayer`: Single winner competition with lateral inhibition
//! - `KWTALayer`: K-winners variant for sparse distributed coding
//! - `LateralInhibition`: Inhibitory connection model
//!
//! # Performance
//!
//! - Single winner: <1μs for 1000 neurons
//! - K-winners: <10μs for 1000 neurons, k=50
//!
//! # Use Cases
//!
//! 1. Fast routing in HNSW graph traversal
//! 2. Sparse activation patterns for efficiency
//! 3. Attention head selection in transformers

mod inhibition;
mod kwta;
mod wta;

pub use inhibition::LateralInhibition;
pub use kwta::KWTALayer;
pub use wta::WTALayer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let _wta = WTALayer::new(10, 0.5, 0.8);
        let _kwta = KWTALayer::new(10, 3);
        let _inhibition = LateralInhibition::new(10, 0.1, 0.9);
    }
}
