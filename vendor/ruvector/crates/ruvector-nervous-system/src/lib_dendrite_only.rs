//! Temporary lib file to test dendrite module independently

pub mod dendrite;

pub use dendrite::{Compartment, Dendrite, DendriticTree, PlateauPotential};

#[derive(Debug, thiserror::Error)]
pub enum NervousSystemError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Compartment index out of bounds: {0}")]
    CompartmentOutOfBounds(usize),

    #[error("Synapse index out of bounds: {0}")]
    SynapseOutOfBounds(usize),
}

pub type Result<T> = std::result::Result<T, NervousSystemError>;
