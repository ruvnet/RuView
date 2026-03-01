//! Fluent builder API for constructing attention mechanisms.

use crate::{error::AttentionResult, traits::Attention};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttentionType {
    ScaledDot,
    MultiHead,
    Flash,
    Linear,
    LocalGlobal,
    Hyperbolic,
    MoE,
}

pub struct AttentionBuilder {
    dim: usize,
    attention_type: AttentionType,
}

impl AttentionBuilder {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            attention_type: AttentionType::ScaledDot,
        }
    }

    pub fn multi_head(mut self, _heads: usize) -> Self {
        self.attention_type = AttentionType::MultiHead;
        self
    }

    pub fn flash(mut self, _block: usize) -> Self {
        self.attention_type = AttentionType::Flash;
        self
    }

    pub fn dropout(self, _p: f32) -> Self {
        self
    }
    pub fn causal(self, _c: bool) -> Self {
        self
    }

    pub fn build(self) -> AttentionResult<Box<dyn Attention + Send + Sync>> {
        Ok(Box::new(crate::attention::ScaledDotProductAttention::new(
            self.dim,
        )))
    }
}

pub fn scaled_dot(dim: usize) -> AttentionBuilder {
    AttentionBuilder::new(dim)
}
pub fn multi_head(dim: usize, heads: usize) -> AttentionBuilder {
    AttentionBuilder::new(dim).multi_head(heads)
}
pub fn flash(dim: usize, block: usize) -> AttentionBuilder {
    AttentionBuilder::new(dim).flash(block)
}
