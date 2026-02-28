//! Cognitive architecture for autonomous robot behavior.
//!
//! This module provides a layered cognitive system comprising:
//! - **Behavior trees** for composable task execution
//! - **Cognitive core** implementing a perceive-think-act-learn loop
//! - **Decision engine** for multi-criteria utility-based action selection
//! - **Memory system** with working, episodic, and semantic tiers
//! - **Skill learning** for acquiring and refining motor skills
//! - **Swarm intelligence** for multi-robot coordination
//! - **World model** for internal environment representation

pub mod behavior_tree;
pub mod cognitive_core;
pub mod decision_engine;
pub mod memory_system;
pub mod skill_learning;
pub mod swarm_intelligence;
pub mod world_model;

pub use behavior_tree::{
    BehaviorContext, BehaviorNode, BehaviorStatus, BehaviorTree, DecoratorType,
};
pub use cognitive_core::{
    ActionCommand, ActionType, CognitiveConfig, CognitiveCore, CognitiveMode, CognitiveState,
    Decision, Outcome, Percept,
};
pub use decision_engine::{ActionOption, DecisionConfig, DecisionEngine};
pub use memory_system::{Episode, EpisodicMemory, MemoryItem, SemanticMemory, WorkingMemory};
pub use skill_learning::{Demonstration, Skill, SkillLibrary};
pub use swarm_intelligence::{
    ConsensusResult, Formation, FormationType, RobotCapabilities, SwarmConfig, SwarmCoordinator,
    SwarmTask, TaskAssignment,
};
pub use world_model::{PredictedState, TrackedObject, WorldModel};
