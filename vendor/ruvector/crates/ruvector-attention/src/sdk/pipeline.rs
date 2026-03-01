//! Pipeline API for chaining attention operations.

use crate::{error::AttentionResult, traits::Attention};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormType {
    LayerNorm,
    RMSNorm,
    BatchNorm,
}

pub enum PipelineStage {
    Attention(Box<dyn Attention + Send + Sync>),
    Normalize(NormType),
}

pub struct AttentionPipeline {
    stages: Vec<PipelineStage>,
}

impl AttentionPipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn add_attention(mut self, attn: Box<dyn Attention + Send + Sync>) -> Self {
        self.stages.push(PipelineStage::Attention(attn));
        self
    }

    pub fn add_norm(mut self, norm: NormType) -> Self {
        self.stages.push(PipelineStage::Normalize(norm));
        self
    }

    pub fn add_dropout(self, _p: f32) -> Self {
        self
    }
    pub fn add_residual(self) -> Self {
        self
    }

    pub fn run(
        &self,
        query: &[f32],
        _keys: &[&[f32]],
        _values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        Ok(query.to_vec())
    }
}

impl Default for AttentionPipeline {
    fn default() -> Self {
        Self::new()
    }
}
