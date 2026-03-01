//! Authority and resource budget enforcement runtime for AGI containers (ADR-036).
//!
//! - [`AuthorityGuard`]: enforces per-execution authority levels and per-action-class
//!   overrides, ensuring container actions never exceed their granted privileges.
//! - [`BudgetTracker`]: tracks resource consumption against a [`ResourceBudget`] and
//!   returns `BudgetExhausted` errors when any resource is at its limit.

use rvf_types::agi_container::*;

const ACTION_CLASS_COUNT: usize = 10;

/// Classification of actions that a container execution may perform.
///
/// Each class can be independently granted a different [`AuthorityLevel`]
/// via [`AuthorityGuard::grant_action_class`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ActionClass {
    ReadMemory = 0,
    WriteMemory = 1,
    ReadFile = 2,
    WriteFile = 3,
    RunTest = 4,
    RunCommand = 5,
    GitPush = 6,
    CreatePR = 7,
    SendMessage = 8,
    ModifyInfra = 9,
}

impl ActionClass {
    /// All variants in discriminant order.
    pub const ALL: [ActionClass; ACTION_CLASS_COUNT] = [
        Self::ReadMemory,
        Self::WriteMemory,
        Self::ReadFile,
        Self::WriteFile,
        Self::RunTest,
        Self::RunCommand,
        Self::GitPush,
        Self::CreatePR,
        Self::SendMessage,
        Self::ModifyInfra,
    ];
}

/// Runtime guard enforcing authority levels for container execution.
///
/// Holds a global maximum authority (from execution mode or explicit) and an
/// optional per-action-class override table as a fixed-size array.
#[derive(Clone, Debug)]
pub struct AuthorityGuard {
    max_authority: AuthorityLevel,
    mode: ExecutionMode,
    class_overrides: [Option<AuthorityLevel>; ACTION_CLASS_COUNT],
}

impl AuthorityGuard {
    /// Create a guard using the default authority for the given execution mode.
    pub fn new(mode: ExecutionMode) -> Self {
        Self {
            max_authority: AuthorityLevel::default_for_mode(mode),
            mode,
            class_overrides: [None; ACTION_CLASS_COUNT],
        }
    }

    /// Create a guard with an explicit maximum authority level.
    pub fn with_max_authority(mode: ExecutionMode, max: AuthorityLevel) -> Self {
        Self {
            max_authority: max,
            mode,
            class_overrides: [None; ACTION_CLASS_COUNT],
        }
    }

    /// The execution mode this guard was created for.
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    /// The global maximum authority level.
    pub fn max_authority(&self) -> AuthorityLevel {
        self.max_authority
    }

    /// Check whether the guard permits the `required` level.
    pub fn check(&self, required: AuthorityLevel) -> Result<(), ContainerError> {
        if self.max_authority.permits(required) {
            Ok(())
        } else {
            Err(ContainerError::InsufficientAuthority {
                required: required as u8,
                granted: self.max_authority as u8,
            })
        }
    }

    /// Check authority for a specific action class.
    ///
    /// Per-class overrides are capped by the global maximum to prevent escalation.
    pub fn check_action_class(
        &self,
        class: ActionClass,
        required: AuthorityLevel,
    ) -> Result<(), ContainerError> {
        let effective = match self.class_overrides[class as usize] {
            Some(o) if (o as u8) <= (self.max_authority as u8) => o,
            Some(_) => self.max_authority,
            None => self.max_authority,
        };
        if effective.permits(required) {
            Ok(())
        } else {
            Err(ContainerError::InsufficientAuthority {
                required: required as u8,
                granted: effective as u8,
            })
        }
    }

    /// Grant authority for a specific action class (capped by global max at check time).
    pub fn grant_action_class(&mut self, class: ActionClass, level: AuthorityLevel) {
        self.class_overrides[class as usize] = Some(level);
    }
}

/// Percentage utilization for each resource dimension (0.0..=100.0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BudgetUtilization {
    pub time_pct: f32,
    pub tokens_pct: f32,
    pub cost_pct: f32,
    pub tool_calls_pct: f32,
    pub external_writes_pct: f32,
}

/// Point-in-time snapshot of budget state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetSnapshot {
    pub budget: ResourceBudget,
    pub used_time_secs: u32,
    pub used_tokens: u32,
    pub used_cost_microdollars: u32,
    pub used_tool_calls: u16,
    pub used_external_writes: u16,
}

/// Tracks resource consumption against a [`ResourceBudget`].
///
/// Each `charge_*` method adds to the running total and returns
/// [`ContainerError::BudgetExhausted`] if the total would exceed the budget.
#[derive(Clone, Debug)]
pub struct BudgetTracker {
    budget: ResourceBudget,
    used_time_secs: u32,
    used_tokens: u32,
    used_cost_microdollars: u32,
    used_tool_calls: u16,
    used_external_writes: u16,
}

impl BudgetTracker {
    /// Create a tracker with the given budget (clamped to hard maximums).
    pub fn new(budget: ResourceBudget) -> Self {
        Self {
            budget: budget.clamped(),
            used_time_secs: 0,
            used_tokens: 0,
            used_cost_microdollars: 0,
            used_tool_calls: 0,
            used_external_writes: 0,
        }
    }

    /// The clamped budget this tracker enforces.
    pub fn budget(&self) -> &ResourceBudget {
        &self.budget
    }

    /// Charge token usage.
    pub fn charge_tokens(&mut self, tokens: u32) -> Result<(), ContainerError> {
        let t = self.used_tokens.saturating_add(tokens);
        if t > self.budget.max_tokens {
            return Err(ContainerError::BudgetExhausted("tokens"));
        }
        self.used_tokens = t;
        Ok(())
    }

    /// Charge cost in microdollars.
    pub fn charge_cost(&mut self, microdollars: u32) -> Result<(), ContainerError> {
        let t = self.used_cost_microdollars.saturating_add(microdollars);
        if t > self.budget.max_cost_microdollars {
            return Err(ContainerError::BudgetExhausted("cost"));
        }
        self.used_cost_microdollars = t;
        Ok(())
    }

    /// Charge one tool call.
    pub fn charge_tool_call(&mut self) -> Result<(), ContainerError> {
        let t = self.used_tool_calls.saturating_add(1);
        if t > self.budget.max_tool_calls {
            return Err(ContainerError::BudgetExhausted("tool_calls"));
        }
        self.used_tool_calls = t;
        Ok(())
    }

    /// Charge one external write.
    pub fn charge_external_write(&mut self) -> Result<(), ContainerError> {
        let t = self.used_external_writes.saturating_add(1);
        if t > self.budget.max_external_writes {
            return Err(ContainerError::BudgetExhausted("external_writes"));
        }
        self.used_external_writes = t;
        Ok(())
    }

    /// Charge wall-clock time in seconds.
    pub fn charge_time(&mut self, secs: u32) -> Result<(), ContainerError> {
        let t = self.used_time_secs.saturating_add(secs);
        if t > self.budget.max_time_secs {
            return Err(ContainerError::BudgetExhausted("time"));
        }
        self.used_time_secs = t;
        Ok(())
    }

    /// Remaining tokens before exhaustion.
    pub fn remaining_tokens(&self) -> u32 {
        self.budget.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Remaining cost budget in microdollars.
    pub fn remaining_cost(&self) -> u32 {
        self.budget
            .max_cost_microdollars
            .saturating_sub(self.used_cost_microdollars)
    }

    /// Remaining wall-clock time in seconds.
    pub fn remaining_time(&self) -> u32 {
        self.budget
            .max_time_secs
            .saturating_sub(self.used_time_secs)
    }

    /// Compute utilization percentages for each resource dimension.
    pub fn utilization(&self) -> BudgetUtilization {
        BudgetUtilization {
            time_pct: pct(self.used_time_secs as f32, self.budget.max_time_secs as f32),
            tokens_pct: pct(self.used_tokens as f32, self.budget.max_tokens as f32),
            cost_pct: pct(
                self.used_cost_microdollars as f32,
                self.budget.max_cost_microdollars as f32,
            ),
            tool_calls_pct: pct(
                self.used_tool_calls as f32,
                self.budget.max_tool_calls as f32,
            ),
            external_writes_pct: pct(
                self.used_external_writes as f32,
                self.budget.max_external_writes as f32,
            ),
        }
    }

    /// Returns `true` if ANY resource dimension with a non-zero budget has reached
    /// its limit. A zero-max dimension is disabled, not exhausted.
    pub fn is_exhausted(&self) -> bool {
        (self.budget.max_time_secs > 0 && self.used_time_secs >= self.budget.max_time_secs)
            || (self.budget.max_tokens > 0 && self.used_tokens >= self.budget.max_tokens)
            || (self.budget.max_cost_microdollars > 0
                && self.used_cost_microdollars >= self.budget.max_cost_microdollars)
            || (self.budget.max_tool_calls > 0
                && self.used_tool_calls >= self.budget.max_tool_calls)
            || (self.budget.max_external_writes > 0
                && self.used_external_writes >= self.budget.max_external_writes)
    }

    /// Capture a point-in-time snapshot of the tracker state.
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            budget: self.budget,
            used_time_secs: self.used_time_secs,
            used_tokens: self.used_tokens,
            used_cost_microdollars: self.used_cost_microdollars,
            used_tool_calls: self.used_tool_calls,
            used_external_writes: self.used_external_writes,
        }
    }
}

fn pct(used: f32, max: f32) -> f32 {
    if max == 0.0 {
        if used > 0.0 {
            100.0
        } else {
            0.0
        }
    } else {
        (used / max * 100.0).min(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_authority_per_mode() {
        let r = AuthorityGuard::new(ExecutionMode::Replay);
        assert_eq!(r.max_authority(), AuthorityLevel::ReadOnly);
        assert_eq!(r.mode(), ExecutionMode::Replay);
        assert_eq!(
            AuthorityGuard::new(ExecutionMode::Verify).max_authority(),
            AuthorityLevel::ExecuteTools
        );
        assert_eq!(
            AuthorityGuard::new(ExecutionMode::Live).max_authority(),
            AuthorityLevel::WriteMemory
        );
    }

    #[test]
    fn check_pass_and_fail() {
        let g = AuthorityGuard::new(ExecutionMode::Verify);
        assert!(g.check(AuthorityLevel::ReadOnly).is_ok());
        assert!(g.check(AuthorityLevel::ExecuteTools).is_ok());
        assert_eq!(
            g.check(AuthorityLevel::WriteExternal).unwrap_err(),
            ContainerError::InsufficientAuthority {
                required: 3,
                granted: 2
            }
        );
        let ro = AuthorityGuard::new(ExecutionMode::Replay);
        assert_eq!(
            ro.check(AuthorityLevel::WriteMemory).unwrap_err(),
            ContainerError::InsufficientAuthority {
                required: 1,
                granted: 0
            }
        );
    }

    #[test]
    fn with_max_authority_overrides_default() {
        let g = AuthorityGuard::with_max_authority(
            ExecutionMode::Replay,
            AuthorityLevel::WriteExternal,
        );
        assert_eq!(g.mode(), ExecutionMode::Replay);
        assert!(g.check(AuthorityLevel::WriteExternal).is_ok());
    }

    #[test]
    fn action_class_grant_restrict_and_inherit() {
        let mut g =
            AuthorityGuard::with_max_authority(ExecutionMode::Live, AuthorityLevel::WriteExternal);
        assert!(g
            .check_action_class(ActionClass::GitPush, AuthorityLevel::WriteExternal)
            .is_ok());
        g.grant_action_class(ActionClass::GitPush, AuthorityLevel::ReadOnly);
        assert_eq!(
            g.check_action_class(ActionClass::GitPush, AuthorityLevel::WriteMemory)
                .unwrap_err(),
            ContainerError::InsufficientAuthority {
                required: 1,
                granted: 0
            }
        );
        assert!(g
            .check_action_class(ActionClass::ReadMemory, AuthorityLevel::WriteExternal)
            .is_ok());
    }

    #[test]
    fn action_class_override_capped_by_global() {
        let mut g = AuthorityGuard::new(ExecutionMode::Replay);
        g.grant_action_class(ActionClass::RunCommand, AuthorityLevel::WriteExternal);
        assert!(g
            .check_action_class(ActionClass::RunCommand, AuthorityLevel::WriteMemory)
            .is_err());
    }

    #[test]
    fn action_class_override_within_global() {
        let mut g =
            AuthorityGuard::with_max_authority(ExecutionMode::Live, AuthorityLevel::ExecuteTools);
        g.grant_action_class(ActionClass::WriteFile, AuthorityLevel::WriteMemory);
        assert!(g
            .check_action_class(ActionClass::WriteFile, AuthorityLevel::WriteMemory)
            .is_ok());
        assert!(g
            .check_action_class(ActionClass::WriteFile, AuthorityLevel::ExecuteTools)
            .is_err());
    }

    #[test]
    fn action_class_all_variants() {
        assert_eq!(ActionClass::ALL.len(), ACTION_CLASS_COUNT);
        for (i, c) in ActionClass::ALL.iter().enumerate() {
            assert_eq!(*c as usize, i);
        }
    }

    #[test]
    fn tracker_zero_usage() {
        let t = BudgetTracker::new(ResourceBudget::DEFAULT);
        assert_eq!(t.remaining_tokens(), 200_000);
        assert_eq!(t.remaining_cost(), 1_000_000);
        assert_eq!(t.remaining_time(), 300);
        assert!(!t.is_exhausted());
    }

    #[test]
    fn charge_and_exhaust_each_resource() {
        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        assert!(t.charge_tokens(200_000).is_ok());
        assert_eq!(
            t.charge_tokens(1),
            Err(ContainerError::BudgetExhausted("tokens"))
        );

        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        assert!(t.charge_cost(1_000_000).is_ok());
        assert_eq!(
            t.charge_cost(1),
            Err(ContainerError::BudgetExhausted("cost"))
        );

        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        for _ in 0..50 {
            t.charge_tool_call().unwrap();
        }
        assert!(t.charge_tool_call().is_err());

        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        assert!(t.charge_time(300).is_ok());
        assert!(t.charge_time(1).is_err());

        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        assert!(t.charge_external_write().is_err()); // zero budget

        let mut t = BudgetTracker::new(ResourceBudget::EXTENDED);
        for _ in 0..10 {
            t.charge_external_write().unwrap();
        }
        assert!(t.charge_external_write().is_err());
    }

    #[test]
    fn is_exhausted_semantics() {
        assert!(!BudgetTracker::new(ResourceBudget::DEFAULT).is_exhausted());
        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        t.charge_tokens(200_000).unwrap();
        assert!(t.is_exhausted());
    }

    #[test]
    fn utilization_calculation() {
        let mut t = BudgetTracker::new(ResourceBudget::DEFAULT);
        t.charge_tokens(100_000).unwrap();
        t.charge_time(150).unwrap();
        t.charge_cost(500_000).unwrap();
        let u = t.utilization();
        assert!((u.tokens_pct - 50.0).abs() < 0.01);
        assert!((u.time_pct - 50.0).abs() < 0.01);
        assert!((u.cost_pct - 50.0).abs() < 0.01);

        t.charge_tokens(100_000).unwrap();
        let u2 = t.utilization();
        assert!((u2.tokens_pct - 100.0).abs() < 0.01);

        let z = BudgetTracker::new(ResourceBudget {
            max_time_secs: 0,
            max_tokens: 0,
            max_cost_microdollars: 0,
            max_tool_calls: 0,
            max_external_writes: 0,
        });
        assert!((z.utilization().time_pct).abs() < 0.01);
    }

    #[test]
    fn snapshot_captures_state() {
        let mut t = BudgetTracker::new(ResourceBudget::EXTENDED);
        t.charge_tokens(5_000).unwrap();
        t.charge_time(60).unwrap();
        t.charge_cost(100_000).unwrap();
        t.charge_tool_call().unwrap();
        t.charge_external_write().unwrap();
        let s = t.snapshot();
        assert_eq!(s.budget, ResourceBudget::EXTENDED);
        assert_eq!(s.used_tokens, 5_000);
        assert_eq!(s.used_time_secs, 60);
        assert_eq!(s.used_cost_microdollars, 100_000);
        assert_eq!(s.used_tool_calls, 1);
        assert_eq!(s.used_external_writes, 1);
    }

    #[test]
    fn budget_clamped_on_creation() {
        let t = BudgetTracker::new(ResourceBudget {
            max_time_secs: 999_999,
            max_tokens: 999_999_999,
            max_cost_microdollars: 999_999_999,
            max_tool_calls: 60_000,
            max_external_writes: 60_000,
        });
        let b = t.budget();
        assert_eq!(b.max_time_secs, ResourceBudget::MAX.max_time_secs);
        assert_eq!(b.max_tokens, ResourceBudget::MAX.max_tokens);
        assert_eq!(
            b.max_external_writes,
            ResourceBudget::MAX.max_external_writes
        );
    }

    #[test]
    fn charge_exactly_at_limit() {
        let mut t = BudgetTracker::new(ResourceBudget {
            max_time_secs: 10,
            max_tokens: 100,
            max_cost_microdollars: 500,
            max_tool_calls: 3,
            max_external_writes: 2,
        });
        assert!(t.charge_tokens(100).is_ok());
        assert!(t.charge_time(10).is_ok());
        assert!(t.charge_cost(500).is_ok());
        for _ in 0..3 {
            t.charge_tool_call().unwrap();
        }
        for _ in 0..2 {
            t.charge_external_write().unwrap();
        }
        assert_eq!(t.remaining_tokens(), 0);
        assert_eq!(t.remaining_cost(), 0);
        assert_eq!(t.remaining_time(), 0);
        assert!(t.is_exhausted());
        assert!(t.charge_tokens(1).is_err());
        assert!(t.charge_time(1).is_err());
        assert!(t.charge_cost(1).is_err());
        assert!(t.charge_tool_call().is_err());
        assert!(t.charge_external_write().is_err());
    }

    #[test]
    fn max_budget_allows_high_usage() {
        let mut t = BudgetTracker::new(ResourceBudget::MAX);
        assert!(t.charge_tokens(1_000_000).is_ok());
        assert!(t.charge_time(3600).is_ok());
        assert!(t.charge_cost(10_000_000).is_ok());
        for _ in 0..500 {
            t.charge_tool_call().unwrap();
        }
        for _ in 0..50 {
            t.charge_external_write().unwrap();
        }
        assert!(t.is_exhausted());
    }
}
