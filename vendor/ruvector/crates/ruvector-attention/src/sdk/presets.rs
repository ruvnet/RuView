//! Pre-configured attention presets for common use cases.

use crate::sdk::builder::AttentionBuilder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttentionPreset {
    Bert,
    Gpt,
    Longformer,
    Performer,
    FlashOptimized,
    SwitchTransformer,
    HyperbolicTree,
    T5,
    ViT,
    SparseTransformer,
}

impl AttentionPreset {
    pub fn builder(self, dim: usize) -> AttentionBuilder {
        match self {
            AttentionPreset::Bert => AttentionBuilder::new(dim).multi_head(12).dropout(0.1),
            AttentionPreset::Gpt => AttentionBuilder::new(dim)
                .multi_head(12)
                .causal(true)
                .dropout(0.1),
            _ => AttentionBuilder::new(dim),
        }
    }
}

pub fn for_sequences(dim: usize, _max_len: usize) -> AttentionBuilder {
    AttentionBuilder::new(dim)
}

pub fn for_graphs(dim: usize, _hierarchical: bool) -> AttentionBuilder {
    AttentionBuilder::new(dim)
}

pub fn for_large_scale(dim: usize) -> AttentionBuilder {
    AttentionBuilder::new(dim).flash(128)
}
