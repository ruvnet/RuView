//! Composable behavior trees for robot task execution.
//!
//! Provides a declarative way to build complex robot behaviors from simple
//! building blocks: actions, conditions, sequences, selectors, decorators,
//! and parallel nodes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Status & decorator types
// ---------------------------------------------------------------------------

/// Result of ticking a behavior tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

/// Decorator modifiers that wrap a single child node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecoratorType {
    /// Inverts Success <-> Failure; Running stays Running.
    Inverter,
    /// Repeats the child a fixed number of times.
    Repeat(usize),
    /// Keeps ticking the child until it returns Failure.
    UntilFail,
    /// Fails the child if it does not finish within `ms` milliseconds (tick count proxy).
    Timeout(u64),
}

// ---------------------------------------------------------------------------
// Node enum
// ---------------------------------------------------------------------------

/// A single node in the behavior tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BehaviorNode {
    /// Leaf that executes a named action. Result is looked up in the context.
    Action(String),
    /// Leaf that checks a named boolean condition in the context.
    Condition(String),
    /// Runs children left-to-right; stops on first non-Success.
    Sequence(Vec<BehaviorNode>),
    /// Runs children left-to-right; stops on first Success.
    Selector(Vec<BehaviorNode>),
    /// Applies a [`DecoratorType`] to a single child.
    Decorator(DecoratorType, Box<BehaviorNode>),
    /// Runs all children concurrently; succeeds when `threshold` children succeed.
    Parallel(usize, Vec<BehaviorNode>),
}

// ---------------------------------------------------------------------------
// Context (blackboard)
// ---------------------------------------------------------------------------

/// Shared context passed through the tree during evaluation.
#[derive(Debug, Clone, Default)]
pub struct BehaviorContext {
    /// General-purpose string key-value store.
    pub blackboard: HashMap<String, String>,
    /// Monotonically increasing tick counter.
    pub tick_count: u64,
    /// Boolean conditions used by `Condition` nodes.
    pub conditions: HashMap<String, bool>,
    /// Pre-set results for `Action` nodes.
    pub action_results: HashMap<String, BehaviorStatus>,
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// A behavior tree with a root node and shared context.
#[derive(Debug, Clone)]
pub struct BehaviorTree {
    root: BehaviorNode,
    context: BehaviorContext,
}

impl BehaviorTree {
    /// Create a new tree with the given root node.
    pub fn new(root: BehaviorNode) -> Self {
        Self {
            root,
            context: BehaviorContext::default(),
        }
    }

    /// Tick the tree once, returning the root status.
    pub fn tick(&mut self) -> BehaviorStatus {
        self.context.tick_count += 1;
        // Split borrow: `root` is borrowed immutably, `context` mutably.
        // Rust allows this because they are disjoint struct fields.
        let root = &self.root;
        let ctx = &mut self.context;
        eval(root, ctx)
    }

    /// Reset the context (tick count, blackboard, etc.).
    pub fn reset(&mut self) {
        self.context = BehaviorContext::default();
    }

    /// Set a named boolean condition.
    pub fn set_condition(&mut self, name: &str, value: bool) {
        self.context.conditions.insert(name.to_string(), value);
    }

    /// Set the result that a named action should return.
    pub fn set_action_result(&mut self, name: &str, status: BehaviorStatus) {
        self.context
            .action_results
            .insert(name.to_string(), status);
    }

    /// Read-only access to the context.
    pub fn context(&self) -> &BehaviorContext {
        &self.context
    }

    /// Read-only reference to the root node.
    pub fn root(&self) -> &BehaviorNode {
        &self.root
    }
}

/// Maximum iterations for `UntilFail` before returning `Running` as a safety
/// guard against infinite loops when the child always succeeds.
const UNTIL_FAIL_MAX_ITERATIONS: usize = 10_000;

// Free functions that borrow the tree and context independently, avoiding the
// need to clone the tree on every tick.

fn eval(node: &BehaviorNode, ctx: &mut BehaviorContext) -> BehaviorStatus {
    match node {
        BehaviorNode::Action(name) => ctx
            .action_results
            .get(name)
            .copied()
            .unwrap_or(BehaviorStatus::Failure),

        BehaviorNode::Condition(name) => {
            if ctx.conditions.get(name).copied().unwrap_or(false) {
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        }

        BehaviorNode::Sequence(children) => {
            for child in children {
                match eval(child, ctx) {
                    BehaviorStatus::Success => continue,
                    other => return other,
                }
            }
            BehaviorStatus::Success
        }

        BehaviorNode::Selector(children) => {
            for child in children {
                match eval(child, ctx) {
                    BehaviorStatus::Failure => continue,
                    other => return other,
                }
            }
            BehaviorStatus::Failure
        }

        BehaviorNode::Decorator(dtype, child) => eval_decorator(dtype, child, ctx),

        BehaviorNode::Parallel(threshold, children) => {
            let mut success_count = 0usize;
            let mut any_running = false;
            for child in children {
                match eval(child, ctx) {
                    BehaviorStatus::Success => success_count += 1,
                    BehaviorStatus::Running => any_running = true,
                    BehaviorStatus::Failure => {}
                }
            }
            if success_count >= *threshold {
                BehaviorStatus::Success
            } else if any_running {
                BehaviorStatus::Running
            } else {
                BehaviorStatus::Failure
            }
        }
    }
}

fn eval_decorator(
    dtype: &DecoratorType,
    child: &BehaviorNode,
    ctx: &mut BehaviorContext,
) -> BehaviorStatus {
    match dtype {
        DecoratorType::Inverter => match eval(child, ctx) {
            BehaviorStatus::Success => BehaviorStatus::Failure,
            BehaviorStatus::Failure => BehaviorStatus::Success,
            BehaviorStatus::Running => BehaviorStatus::Running,
        },
        DecoratorType::Repeat(n) => {
            for _ in 0..*n {
                match eval(child, ctx) {
                    BehaviorStatus::Failure => return BehaviorStatus::Failure,
                    BehaviorStatus::Running => return BehaviorStatus::Running,
                    BehaviorStatus::Success => {}
                }
            }
            BehaviorStatus::Success
        }
        DecoratorType::UntilFail => {
            for _ in 0..UNTIL_FAIL_MAX_ITERATIONS {
                match eval(child, ctx) {
                    BehaviorStatus::Failure => return BehaviorStatus::Success,
                    BehaviorStatus::Running => return BehaviorStatus::Running,
                    BehaviorStatus::Success => continue,
                }
            }
            // Safety: child never failed within the iteration budget.
            BehaviorStatus::Running
        }
        DecoratorType::Timeout(max_ticks) => {
            if ctx.tick_count > *max_ticks {
                return BehaviorStatus::Failure;
            }
            eval(child, ctx)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_success() {
        let mut tree = BehaviorTree::new(BehaviorNode::Action("move".into()));
        tree.set_action_result("move", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_action_default_failure() {
        let mut tree = BehaviorTree::new(BehaviorNode::Action("unknown".into()));
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_condition_true() {
        let mut tree = BehaviorTree::new(BehaviorNode::Condition("has_target".into()));
        tree.set_condition("has_target", true);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_condition_false() {
        let mut tree = BehaviorTree::new(BehaviorNode::Condition("has_target".into()));
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_sequence_all_success() {
        let seq = BehaviorNode::Sequence(vec![
            BehaviorNode::Action("a".into()),
            BehaviorNode::Action("b".into()),
        ]);
        let mut tree = BehaviorTree::new(seq);
        tree.set_action_result("a", BehaviorStatus::Success);
        tree.set_action_result("b", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_sequence_early_failure() {
        let seq = BehaviorNode::Sequence(vec![
            BehaviorNode::Action("a".into()),
            BehaviorNode::Action("b".into()),
        ]);
        let mut tree = BehaviorTree::new(seq);
        tree.set_action_result("a", BehaviorStatus::Failure);
        tree.set_action_result("b", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_selector_first_success() {
        let sel = BehaviorNode::Selector(vec![
            BehaviorNode::Action("a".into()),
            BehaviorNode::Action("b".into()),
        ]);
        let mut tree = BehaviorTree::new(sel);
        tree.set_action_result("a", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_selector_fallback() {
        let sel = BehaviorNode::Selector(vec![
            BehaviorNode::Action("a".into()),
            BehaviorNode::Action("b".into()),
        ]);
        let mut tree = BehaviorTree::new(sel);
        tree.set_action_result("a", BehaviorStatus::Failure);
        tree.set_action_result("b", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_selector_all_fail() {
        let sel = BehaviorNode::Selector(vec![
            BehaviorNode::Action("a".into()),
            BehaviorNode::Action("b".into()),
        ]);
        let mut tree = BehaviorTree::new(sel);
        tree.set_action_result("a", BehaviorStatus::Failure);
        tree.set_action_result("b", BehaviorStatus::Failure);
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_inverter_decorator() {
        let node = BehaviorNode::Decorator(
            DecoratorType::Inverter,
            Box::new(BehaviorNode::Action("a".into())),
        );
        let mut tree = BehaviorTree::new(node);
        tree.set_action_result("a", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_repeat_decorator() {
        let node = BehaviorNode::Decorator(
            DecoratorType::Repeat(3),
            Box::new(BehaviorNode::Action("a".into())),
        );
        let mut tree = BehaviorTree::new(node);
        tree.set_action_result("a", BehaviorStatus::Success);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_repeat_decorator_failure() {
        let node = BehaviorNode::Decorator(
            DecoratorType::Repeat(3),
            Box::new(BehaviorNode::Action("a".into())),
        );
        let mut tree = BehaviorTree::new(node);
        tree.set_action_result("a", BehaviorStatus::Failure);
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_parallel_threshold() {
        let par = BehaviorNode::Parallel(
            2,
            vec![
                BehaviorNode::Action("a".into()),
                BehaviorNode::Action("b".into()),
                BehaviorNode::Action("c".into()),
            ],
        );
        let mut tree = BehaviorTree::new(par);
        tree.set_action_result("a", BehaviorStatus::Success);
        tree.set_action_result("b", BehaviorStatus::Success);
        tree.set_action_result("c", BehaviorStatus::Failure);
        assert_eq!(tree.tick(), BehaviorStatus::Success);
    }

    #[test]
    fn test_parallel_running() {
        let par = BehaviorNode::Parallel(
            2,
            vec![
                BehaviorNode::Action("a".into()),
                BehaviorNode::Action("b".into()),
            ],
        );
        let mut tree = BehaviorTree::new(par);
        tree.set_action_result("a", BehaviorStatus::Success);
        tree.set_action_result("b", BehaviorStatus::Running);
        assert_eq!(tree.tick(), BehaviorStatus::Running);
    }

    #[test]
    fn test_timeout_decorator() {
        let node = BehaviorNode::Decorator(
            DecoratorType::Timeout(2),
            Box::new(BehaviorNode::Action("a".into())),
        );
        let mut tree = BehaviorTree::new(node);
        tree.set_action_result("a", BehaviorStatus::Running);
        // tick 1 => within timeout
        assert_eq!(tree.tick(), BehaviorStatus::Running);
        // tick 2 => within timeout
        assert_eq!(tree.tick(), BehaviorStatus::Running);
        // tick 3 => exceeds timeout
        assert_eq!(tree.tick(), BehaviorStatus::Failure);
    }

    #[test]
    fn test_reset() {
        let mut tree = BehaviorTree::new(BehaviorNode::Action("a".into()));
        tree.set_action_result("a", BehaviorStatus::Success);
        tree.set_condition("flag", true);
        tree.tick();
        assert_eq!(tree.context().tick_count, 1);
        tree.reset();
        assert_eq!(tree.context().tick_count, 0);
        assert!(tree.context().conditions.is_empty());
    }

    #[test]
    fn test_blackboard() {
        let mut tree = BehaviorTree::new(BehaviorNode::Action("a".into()));
        tree.set_action_result("a", BehaviorStatus::Success);
        tree.context
            .blackboard
            .insert("target".into(), "object_1".into());
        assert_eq!(tree.context().blackboard.get("target").unwrap(), "object_1");
    }
}
