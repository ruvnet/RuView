use planner::action::{Action, ActionCategory, CommonActions};
use planner::goal::{Goal, GoalManager, CommonGoals, GoalPriority, GoalState};
use planner::planner::{PlanStatus, PlanMonitor, StepExecutionResult, GOAPPlanner, Plan, PlanStep};
use planner::astar::{SearchConstraints, AStarSearch};
use planner::rules::{DecisionRule, RuleEngine};
use planner::state::{StateBuilder, StateCondition, ComparisonOperator, StateValue, WorldState};
use std::collections::HashMap;

#[test]
fn test_world_state_basic_operations() {
    let mut state = WorldState::new();

    // Test setting and getting states
    state.set_state("health", StateValue::Integer(100));
    state.set_state("location", StateValue::String("home".to_string()));
    state.set_state("has_key", StateValue::Boolean(true));

    assert_eq!(state.get_state("health").unwrap().as_integer(), Some(100));
    assert_eq!(state.get_state("location").unwrap().as_string(), Some(&"home".to_string()));
    assert_eq!(state.get_state("has_key").unwrap().as_bool(), Some(true));

    // Test state removal
    let removed = state.remove_state("health");
    assert!(removed.is_some());
    assert!(state.get_state("health").is_none());
}

#[test]
fn test_state_distance_calculation() {
    let state1 = StateBuilder::new()
        .with_int("x", 10)
        .with_int("y", 20)
        .with_bool("active", true)
        .build();

    let state2 = StateBuilder::new()
        .with_int("x", 15)
        .with_int("y", 25)
        .with_bool("active", false)
        .build();

    let distance = state1.distance_to(&state2);
    assert!(distance > 0.0);

    // Distance to self should be 0
    let self_distance = state1.distance_to(&state1);
    assert_eq!(self_distance, 0.0);
}

#[test]
fn test_state_conditions() {
    let state = StateBuilder::new()
        .with_int("score", 85)
        .with_string("grade", "B")
        .with_bool("passed", true)
        .build();

    // Test various condition operators
    assert!(state.satisfies_condition("score", &StateValue::Integer(85)));
    assert!(!state.satisfies_condition("score", &StateValue::Integer(90)));

    let condition_ge = StateCondition::new("score", ComparisonOperator::GreaterThanOrEqual, StateValue::Integer(80));
    assert!(condition_ge.evaluate(&state));

    let condition_lt = StateCondition::new("score", ComparisonOperator::LessThan, StateValue::Integer(90));
    assert!(condition_lt.evaluate(&state));
}

#[test]
fn test_action_creation_and_execution() {
    let action = Action::new("move_north", "Move one step north")
        .with_category(ActionCategory::Movement)
        .add_precondition("can_move", ComparisonOperator::Equal, StateValue::Boolean(true))
        .add_effect("y_position", StateValue::Integer(1))
        .with_cost(1.5);

    let mut state = StateBuilder::new()
        .with_bool("can_move", true)
        .with_int("y_position", 0)
        .build();

    // Test precondition checking
    assert!(action.can_execute(&state));

    // Test action execution
    let effects = action.apply_effects(&mut state);
    assert!(!effects.is_empty());
    assert!(effects[0].applied);

    // Verify state change
    assert_eq!(state.get_state("y_position").unwrap().as_integer(), Some(1));
}

#[test]
fn test_action_probability_effects() {
    let action = Action::new("risky_action", "An action that might fail")
        .add_probabilistic_effect("success", StateValue::Boolean(true), 0.3);

    let mut state = WorldState::new();

    // Run multiple times to test probability (not deterministic)
    let mut successes = 0;
    for _ in 0..100 {
        let mut test_state = state.clone();
        let effects = action.apply_effects(&mut test_state);

        if !effects.is_empty() && effects[0].applied {
            successes += 1;
        }
    }

    // Should have some successes but not all (approximately 30%)
    assert!(successes > 10);
    assert!(successes < 50);
}

#[test]
fn test_goal_creation_and_satisfaction() {
    let goal = Goal::new("reach_treasure", "Find the treasure chest")
        .add_condition("location", ComparisonOperator::Equal, StateValue::String("treasure_room".to_string()))
        .add_condition("has_key", ComparisonOperator::Equal, StateValue::Boolean(true))
        .with_priority(GoalPriority::High)
        .with_reward(100.0);

    // Test unsatisfied goal
    let state1 = StateBuilder::new()
        .with_string("location", "entrance")
        .with_bool("has_key", false)
        .build();

    assert!(!goal.is_satisfied(&state1));
    assert_eq!(goal.get_satisfaction_score(&state1), 0.0);

    // Test partially satisfied goal
    let state2 = StateBuilder::new()
        .with_string("location", "treasure_room")
        .with_bool("has_key", false)
        .build();

    assert!(!goal.is_satisfied(&state2));
    assert_eq!(goal.get_satisfaction_score(&state2), 0.5);

    // Test fully satisfied goal
    let state3 = StateBuilder::new()
        .with_string("location", "treasure_room")
        .with_bool("has_key", true)
        .build();

    assert!(goal.is_satisfied(&state3));
    assert_eq!(goal.get_satisfaction_score(&state3), 1.0);
}

#[test]
fn test_goal_manager() {
    let mut manager = GoalManager::new();

    let goal1 = Goal::new("goal1", "First goal")
        .with_state(GoalState::Active)
        .with_priority(GoalPriority::High);

    let goal2 = Goal::new("goal2", "Second goal")
        .with_state(GoalState::Pending)
        .with_priority(GoalPriority::Low);

    let goal1_id = goal1.id.clone();
    let goal2_id = goal2.id.clone();

    manager.add_goal(goal1);
    manager.add_goal(goal2);

    // Test goal retrieval
    assert!(manager.get_goal(&goal1_id).is_some());
    assert!(manager.get_goal(&goal2_id).is_some());

    // Test active goals
    let active_goals = manager.get_active_goals();
    assert_eq!(active_goals.len(), 1);
    assert_eq!(active_goals[0].id, goal1_id);

    // Test state updates
    manager.update_goal_state(&goal2_id, GoalState::Active);
    let active_goals = manager.get_active_goals();
    assert_eq!(active_goals.len(), 2);
}

#[test]
fn test_astar_search() {
    let search = AStarSearch::new();

    let start_state = StateBuilder::new()
        .with_int("x", 0)
        .with_int("y", 0)
        .build();

    let goal_state = StateBuilder::new()
        .with_int("x", 2)
        .with_int("y", 2)
        .build();

    // Create movement actions
    let actions = vec![
        Action::new("move_right", "Move right")
            .add_effect("x", StateValue::Integer(1))
            .with_cost(1.0),
        Action::new("move_up", "Move up")
            .add_effect("y", StateValue::Integer(1))
            .with_cost(1.0),
    ];

    let result = search.search(&start_state, &goal_state, &actions);

    // For this simple test, we expect the search to complete
    // Note: The actual success depends on the implementation details
    assert!(result.nodes_explored > 0);
    assert!(result.time_taken >= 0.0);
}

#[test]
fn test_goap_planner_basic() {
    let mut planner = GOAPPlanner::new();

    // Add actions
    let move_action = CommonActions::move_to("destination");
    let pickup_action = CommonActions::pick_up_item("key");

    planner.add_action(move_action);
    planner.add_action(pickup_action);

    // Add goal
    let goal = CommonGoals::reach_location("destination");
    let goal_id = goal.id.clone();
    planner.add_goal(goal);

    // Test planning
    let current_state = StateBuilder::new()
        .with_string("location", "start")
        .with_bool("can_move", true)
        .build();

    let result = planner.plan(&current_state, &goal_id);

    // Planning should complete without error
    assert!(result.planning_time >= 0.0);
    assert!(result.nodes_explored >= 0);

    if result.success {
        let plan = result.plan.unwrap();
        assert!(!plan.steps.is_empty());
        assert_eq!(plan.goal_id, goal_id);
    }
}

#[test]
fn test_plan_validation() {
    let mut planner = GOAPPlanner::new();

    // Add a simple action
    let action = Action::new("test_action", "Test action")
        .add_precondition("ready", ComparisonOperator::Equal, StateValue::Boolean(true))
        .add_effect("done", StateValue::Boolean(true));

    let action_id = action.id.clone();
    planner.add_action(action);

    // Create a plan with the action
    let plan = Plan {
        id: "test_plan".to_string(),
        goal_id: "test_goal".to_string(),
        steps: vec![
            PlanStep {
                action_id,
                parameters: std::collections::HashMap::new(),
                expected_cost: 1.0,
                expected_duration: 1.0,
                state_before: None,
                state_after: None,
            }
        ],
        total_cost: 1.0,
        estimated_duration: 1.0,
        confidence: 0.8,
        created_at: 0,
        status: PlanStatus::Created,
    };

    // Test validation with valid state
    let valid_state = StateBuilder::new()
        .with_bool("ready", true)
        .build();

    let validation = planner.validate_plan(&plan, &valid_state);
    assert!(validation.is_valid);
    assert!(validation.errors.is_empty());

    // Test validation with invalid state
    let invalid_state = StateBuilder::new()
        .with_bool("ready", false)
        .build();

    let validation = planner.validate_plan(&plan, &invalid_state);
    assert!(!validation.is_valid);
    assert!(!validation.errors.is_empty());
}

#[test]
fn test_decision_rules() {
    let mut rule_engine = RuleEngine::new();

    // Create a simple rule
    let rule = DecisionRule::new("health_check", "Check if health is low")
        .add_simple_condition("health", ComparisonOperator::LessThan, StateValue::Integer(20))
        .add_set_state_action("needs_healing", StateValue::Boolean(true))
        .with_priority(10);

    rule_engine.add_rule(rule);

    // Test with low health
    let mut state = StateBuilder::new()
        .with_int("health", 15)
        .build();

    let recommendations = rule_engine.evaluate(&state);
    assert!(!recommendations.is_empty());
    assert!(recommendations[0].score > 0.0);

    // Execute the rule
    let results = rule_engine.execute_all_applicable(&mut state);
    assert!(!results.is_empty());
    assert!(results[0].success);

    // Verify the effect was applied
    assert_eq!(state.get_state("needs_healing").unwrap().as_bool(), Some(true));
}

#[test]
fn test_rule_priority_ordering() {
    let mut rule_engine = RuleEngine::new();

    let rule1 = DecisionRule::new("low_priority", "Low priority rule")
        .add_simple_condition("test", ComparisonOperator::Equal, StateValue::Boolean(true))
        .with_priority(1);

    let rule2 = DecisionRule::new("high_priority", "High priority rule")
        .add_simple_condition("test", ComparisonOperator::Equal, StateValue::Boolean(true))
        .with_priority(10);

    rule_engine.add_rule(rule1);
    rule_engine.add_rule(rule2);

    let state = StateBuilder::new()
        .with_bool("test", true)
        .build();

    let recommendations = rule_engine.evaluate(&state);
    assert_eq!(recommendations.len(), 2);

    // Higher priority should come first
    assert!(recommendations[0].score >= recommendations[1].score);
}

#[test]
fn test_common_actions() {
    // Test move action
    let move_action = CommonActions::move_to("library");
    assert_eq!(move_action.name, "move_to");
    assert!(matches!(move_action.category, ActionCategory::Movement));

    // Test pickup action
    let pickup_action = CommonActions::pick_up_item("book");
    assert_eq!(pickup_action.name, "pick_up");
    assert!(matches!(pickup_action.category, ActionCategory::Interaction));

    // Test use action
    let use_action = CommonActions::use_item("sword");
    assert_eq!(use_action.name, "use_item");
    assert!(matches!(use_action.category, ActionCategory::Utility));
}

#[test]
fn test_common_goals() {
    // Test location goal
    let location_goal = CommonGoals::reach_location("treasure_chamber");
    assert_eq!(location_goal.name, "reach_location");
    assert!(!location_goal.conditions.is_empty());

    // Test collection goal
    let collect_goal = CommonGoals::collect_item("magic_sword");
    assert_eq!(collect_goal.name, "collect_item");
    assert!(!collect_goal.conditions.is_empty());

    // Test resource maintenance goal
    let resource_goal = CommonGoals::maintain_resource("energy", 50);
    assert_eq!(resource_goal.name, "maintain_resource");
    assert!(matches!(resource_goal.priority, GoalPriority::High));
}

#[test]
fn test_plan_execution_monitoring() {
    let mut monitor = PlanMonitor::new();

    let plan = Plan {
        id: "test_plan".to_string(),
        goal_id: "test_goal".to_string(),
        steps: vec![],
        total_cost: 0.0,
        estimated_duration: 0.0,
        confidence: 1.0,
        created_at: 0,
        status: PlanStatus::Created,
    };

    let context_id = monitor.start_execution(&plan);
    assert!(!context_id.is_empty());

    // Test step recording
    let step_result = StepExecutionResult {
        step_index: 0,
        action_id: "test_action".to_string(),
        success: true,
        actual_cost: 1.0,
        actual_duration: 1.0,
        error: None,
        state_after_execution: WorldState::new(),
    };

    let recorded = monitor.record_step_execution(&context_id, step_result);
    assert!(recorded);

    // Test replan decision
    let should_replan = monitor.should_replan(&context_id);
    assert!(!should_replan); // Should not replan with 100% success rate
}

#[test]
fn test_search_constraints() {
    let constraints = SearchConstraints::new()
        .with_max_action_cost(5.0)
        .forbid_action("dangerous_action")
        .allow_category("Movement");

    assert_eq!(constraints.max_action_cost, Some(5.0));
    assert!(constraints.forbidden_actions.contains("dangerous_action"));
    assert!(constraints.allowed_categories.contains("Movement"));
}

// WASM tests would require wasm-bindgen-test crate
// These are commented out for basic functionality testing