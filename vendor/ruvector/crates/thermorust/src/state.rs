//! System state: continuous activations or binary spins with dissipation bookkeeping.

/// State of a thermodynamic motif.
///
/// Activations are stored as `f32` in `[-1.0, 1.0]` (or `{-1.0, +1.0}` for
/// discrete Ising spins). `dissipated_j` accumulates the total Joules of heat
/// shed over all accepted irreversible transitions.
#[derive(Clone, Debug)]
pub struct State {
    /// Spin / activation vector.
    pub x: Vec<f32>,
    /// Cumulative heat dissipated (Joules).
    pub dissipated_j: f64,
}

impl State {
    /// Construct a new state with all spins set to `+1`.
    pub fn ones(n: usize) -> Self {
        Self {
            x: vec![1.0; n],
            dissipated_j: 0.0,
        }
    }

    /// Construct a new state with all spins set to `-1`.
    pub fn neg_ones(n: usize) -> Self {
        Self {
            x: vec![-1.0; n],
            dissipated_j: 0.0,
        }
    }

    /// Construct a state from an explicit activation vector.
    pub fn from_vec(x: Vec<f32>) -> Self {
        Self {
            x,
            dissipated_j: 0.0,
        }
    }

    /// Number of units in the motif.
    #[inline]
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// True if the motif has no units.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Clamp all activations to `[-1.0, 1.0]`.
    pub fn clamp(&mut self) {
        for xi in &mut self.x {
            *xi = xi.clamp(-1.0, 1.0);
        }
    }
}
