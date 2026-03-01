use serde::{Deserialize, Serialize};

/// Per-phase tick budgets for a single container epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerEpochBudget {
    /// Maximum total ticks for the entire epoch.
    pub total: u64,
    /// Ticks allocated to the ingest phase.
    pub ingest: u64,
    /// Ticks allocated to the min-cut phase.
    pub mincut: u64,
    /// Ticks allocated to the spectral analysis phase.
    pub spectral: u64,
    /// Ticks allocated to the evidence accumulation phase.
    pub evidence: u64,
    /// Ticks allocated to the witness receipt phase.
    pub witness: u64,
}

impl Default for ContainerEpochBudget {
    fn default() -> Self {
        Self {
            total: 10_000,
            ingest: 2_000,
            mincut: 3_000,
            spectral: 2_000,
            evidence: 2_000,
            witness: 1_000,
        }
    }
}

/// Processing phases within a single epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ingest,
    MinCut,
    Spectral,
    Evidence,
    Witness,
}

/// Controls compute-tick budgeting across phases within an epoch.
pub struct EpochController {
    budget: ContainerEpochBudget,
    ticks_used: u64,
    phase_used: [u64; 5],
    current_phase: Phase,
}

impl EpochController {
    /// Create a new controller with the given budget.
    pub fn new(budget: ContainerEpochBudget) -> Self {
        Self {
            budget,
            ticks_used: 0,
            phase_used: [0; 5],
            current_phase: Phase::Ingest,
        }
    }

    /// Check whether `phase` still has budget remaining.
    /// If yes, sets the current phase and returns `true`.
    pub fn try_budget(&mut self, phase: Phase) -> bool {
        let idx = Self::phase_index(phase);
        let limit = self.phase_budget(phase);
        if self.phase_used[idx] < limit && self.ticks_used < self.budget.total {
            self.current_phase = phase;
            true
        } else {
            false
        }
    }

    /// Consume `ticks` from both the total budget and the current phase budget.
    pub fn consume(&mut self, ticks: u64) {
        let idx = Self::phase_index(self.current_phase);
        self.ticks_used += ticks;
        self.phase_used[idx] += ticks;
    }

    /// Ticks remaining in the total epoch budget.
    pub fn remaining(&self) -> u64 {
        self.budget.total.saturating_sub(self.ticks_used)
    }

    /// Reset the controller for a new epoch.
    pub fn reset(&mut self) {
        self.ticks_used = 0;
        self.phase_used = [0; 5];
        self.current_phase = Phase::Ingest;
    }

    /// Total tick budget allocated to `phase`.
    pub fn phase_budget(&self, phase: Phase) -> u64 {
        match phase {
            Phase::Ingest => self.budget.ingest,
            Phase::MinCut => self.budget.mincut,
            Phase::Spectral => self.budget.spectral,
            Phase::Evidence => self.budget.evidence,
            Phase::Witness => self.budget.witness,
        }
    }

    /// Ticks consumed so far by `phase`.
    pub fn phase_used(&self, phase: Phase) -> u64 {
        self.phase_used[Self::phase_index(phase)]
    }

    fn phase_index(phase: Phase) -> usize {
        match phase {
            Phase::Ingest => 0,
            Phase::MinCut => 1,
            Phase::Spectral => 2,
            Phase::Evidence => 3,
            Phase::Witness => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_budgeting() {
        let budget = ContainerEpochBudget {
            total: 100,
            ingest: 30,
            mincut: 25,
            spectral: 20,
            evidence: 15,
            witness: 10,
        };
        let mut ctl = EpochController::new(budget);

        assert!(ctl.try_budget(Phase::Ingest));
        ctl.consume(30);
        assert_eq!(ctl.phase_used(Phase::Ingest), 30);
        // Phase is now exhausted.
        assert!(!ctl.try_budget(Phase::Ingest));
        assert_eq!(ctl.remaining(), 70);

        assert!(ctl.try_budget(Phase::MinCut));
        ctl.consume(25);
        assert!(!ctl.try_budget(Phase::MinCut));
        assert_eq!(ctl.remaining(), 45);

        assert!(ctl.try_budget(Phase::Spectral));
        ctl.consume(20);
        assert!(ctl.try_budget(Phase::Evidence));
        ctl.consume(15);
        assert!(ctl.try_budget(Phase::Witness));
        ctl.consume(10);

        assert_eq!(ctl.remaining(), 0);
    }

    #[test]
    fn test_epoch_reset() {
        let mut ctl = EpochController::new(ContainerEpochBudget::default());
        assert!(ctl.try_budget(Phase::Ingest));
        ctl.consume(500);
        assert_eq!(ctl.phase_used(Phase::Ingest), 500);

        ctl.reset();
        assert_eq!(ctl.phase_used(Phase::Ingest), 0);
        assert_eq!(ctl.remaining(), 10_000);
    }

    #[test]
    fn test_total_budget_caps_phase() {
        let budget = ContainerEpochBudget {
            total: 10,
            ingest: 100,
            mincut: 100,
            spectral: 100,
            evidence: 100,
            witness: 100,
        };
        let mut ctl = EpochController::new(budget);
        assert!(ctl.try_budget(Phase::Ingest));
        ctl.consume(10);
        // Total is exhausted even though phase still has room.
        assert!(!ctl.try_budget(Phase::MinCut));
    }
}
