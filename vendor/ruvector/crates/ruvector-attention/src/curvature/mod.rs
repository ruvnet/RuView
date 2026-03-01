//! Mixed Curvature Attention
//!
//! Attention in product spaces: E^e × H^h × S^s
//!
//! ## Key Optimizations
//!
//! 1. **Tangent Space Mapping**: Map hyperbolic to tangent space at origin
//! 2. **Fused Dot Kernel**: Single vectorized loop for all three similarities
//! 3. **Per-Head Mixing**: Low-rank learned weights per head
//! 4. **Quantization-Friendly**: Different precision for each component

mod component_quantizer;
mod fused_attention;
mod tangent_space;

pub use component_quantizer::{ComponentQuantizer, QuantizationConfig, QuantizedVector};
pub use fused_attention::{
    FusedCurvatureConfig, MixedCurvatureCache, MixedCurvatureFusedAttention,
};
pub use tangent_space::{TangentSpaceConfig, TangentSpaceMapper};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        assert!(true);
    }
}
