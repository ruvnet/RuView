use thiserror::Error;

/// Errors that can occur during cognitive container operations.
#[derive(Error, Debug)]
pub enum ContainerError {
    #[error("Memory allocation failed: requested {requested} bytes, available {available}")]
    AllocationFailed { requested: usize, available: usize },

    #[error("Epoch budget exhausted: used {used} of {budget} ticks")]
    EpochExhausted { used: u64, budget: u64 },

    #[error("Witness chain broken at epoch {epoch}")]
    BrokenChain { epoch: u64 },

    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    #[error("Container not initialized")]
    NotInitialized,

    #[error("Slab overflow: component {component} exceeded budget")]
    SlabOverflow { component: String },
}

/// Convenience alias for container results.
pub type Result<T> = std::result::Result<T, ContainerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ContainerError::AllocationFailed {
            requested: 1024,
            available: 512,
        };
        assert!(err.to_string().contains("1024"));
        assert!(err.to_string().contains("512"));
    }

    #[test]
    fn test_error_variants() {
        let err = ContainerError::EpochExhausted {
            used: 100,
            budget: 50,
        };
        assert!(err.to_string().contains("100"));

        let err = ContainerError::BrokenChain { epoch: 7 };
        assert!(err.to_string().contains("7"));

        let err = ContainerError::InvalidConfig {
            reason: "bad value".into(),
        };
        assert!(err.to_string().contains("bad value"));

        let err = ContainerError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));

        let err = ContainerError::SlabOverflow {
            component: "graph".into(),
        };
        assert!(err.to_string().contains("graph"));
    }
}
