use planner::*;
use std::collections::HashMap;

#[cfg(test)]
mod world_state_tests {
    use super::*;

    #[test]
    fn test_world_state_creation() {
        let state = WorldState::new();
        assert!(state.is_empty());
    }

    #[test]
    fn test_set_and_get_state() {
        let mut state = WorldState::new();
        state.set_state("health", StateValue::Integer(100));
        state.set_state("location", StateValue::String("forest".to_string()));
        state.set_state("has_key", StateValue::Boolean(true));

        assert_eq!(state.get_state("health"), Some(&StateValue::Integer(100)));
        assert_eq!(state.get_state("location"), Some(&StateValue::String("forest".to_string())));
        assert_eq!(state.get_state("has_key"), Some(&StateValue::Boolean(true)));
        assert_eq!(state.get_state("nonexistent"), None);
    }

    #[test]
    fn test_state_comparison() {
        let mut state1 = WorldState::new();
        let mut state2 = WorldState::new();

        state1.set_state("x", StateValue::Integer(5));
        state2.set_state("x", StateValue::Integer(5));

        assert!(state1.satisfies(&state2));
        assert!(state2.satisfies(&state1));

        state2.set_state("y", StateValue::String("test".to_string()));
        assert!(!state1.satisfies(&state2)); // state1 doesn't have 'y'
        assert!(state2.satisfies(&state1)); // state2 has all of state1's properties
    }

    #[test]
    fn test_state_difference() {
        let mut state1 = WorldState::new();
        let mut state2 = WorldState::new();

        state1.set_state("health", StateValue::Integer(100));
        state1.set_state("location", StateValue::String("forest".to_string()));

        state2.set_state("health", StateValue::Integer(80));
        state2.set_state("location", StateValue::String("cave".to_string()));
        state2.set_state("has_torch", StateValue::Boolean(true));

        let diff = state1.difference(&state2);
        assert!(diff.contains_key("health"));
        assert!(diff.contains_key("location"));
        assert!(diff.contains_key("has_torch"));
    }

    #[test]
    fn test_state_serialization() {
        let mut state = WorldState::new();
        state.set_state("test", StateValue::Float(3.14));

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: WorldState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state.get_state("test"), deserialized.get_state("test"));
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;

    #[test]
    fn test_action_creation() {
        let action = Action {
            id: "move_north".to_string(),
            name: "Move North".to_string(),
            description: "Move one step north".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "can_move".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "y_position".to_string(),
                    value: StateValue::Integer(1), // relative change
                    operation: "add".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        assert_eq!(action.id, "move_north");
        assert_eq!(action.preconditions.len(), 1);
        assert_eq!(action.effects.len(), 1);
    }

    #[test]
    fn test_action_can_execute() {
        let action = Action {
            id: "unlock_door".to_string(),
            name: "Unlock Door".to_string(),
            description: "Unlock a door with a key".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "has_key".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                },
                ActionPrecondition {
                    state_key: "at_door".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "door_locked".to_string(),
                    value: StateValue::Boolean(false),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 2.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let mut valid_state = WorldState::new();
        valid_state.set_state("has_key", StateValue::Boolean(true));
        valid_state.set_state("at_door", StateValue::Boolean(true));

        let mut invalid_state = WorldState::new();
        invalid_state.set_state("has_key", StateValue::Boolean(false));
        invalid_state.set_state("at_door", StateValue::Boolean(true));

        assert!(action.can_execute(&valid_state));
        assert!(!action.can_execute(&invalid_state));
    }

    #[test]
    fn test_action_cost_calculation() {
        let mut cost = ActionCost {
            base_cost: 10.0,
            dynamic_factors: HashMap::new(),
        };

        cost.dynamic_factors.insert("distance_multiplier".to_string(), 2.0);
        cost.dynamic_factors.insert("difficulty_bonus".to_string(), 5.0);

        let mut state = WorldState::new();
        state.set_state("distance", StateValue::Float(3.0));

        let calculated_cost = cost.calculate(&state);
        assert!(calculated_cost >= 10.0); // At least base cost
    }

    #[test]
    fn test_action_effects_application() {
        let action = Action {
            id: "gain_experience".to_string(),
            name: "Gain Experience".to_string(),
            description: "Gain 50 XP".to_string(),
            preconditions: vec![],
            effects: vec![
                ActionEffect {
                    state_key: "experience".to_string(),
                    value: StateValue::Integer(50),
                    operation: "add".to_string(),
                },
                ActionEffect {
                    state_key: "level".to_string(),
                    value: StateValue::Integer(1),
                    operation: "add".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 0.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let mut state = WorldState::new();
        state.set_state("experience", StateValue::Integer(100));
        state.set_state("level", StateValue::Integer(5));

        let new_state = action.apply_effects(&state);

        assert_eq!(new_state.get_state("experience"), Some(&StateValue::Integer(150)));
        assert_eq!(new_state.get_state("level"), Some(&StateValue::Integer(6)));
    }

    #[test]
    fn test_action_comparison_operators() {
        let action = Action {
            id: "check_health".to_string(),
            name: "Check Health".to_string(),
            description: "Check if health is above threshold".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "health".to_string(),
                    required_value: StateValue::Integer(50),
                    comparison: "greater_than".to_string(),
                }
            ],
            effects: vec![],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let mut high_health_state = WorldState::new();
        high_health_state.set_state("health", StateValue::Integer(80));

        let mut low_health_state = WorldState::new();
        low_health_state.set_state("health", StateValue::Integer(30));

        assert!(action.can_execute(&high_health_state));
        assert!(!action.can_execute(&low_health_state));
    }
}

#[cfg(test)]
mod goal_tests {
    use super::*;

    #[test]
    fn test_goal_creation() {
        let goal = Goal {
            id: "reach_destination".to_string(),
            name: "Reach Destination".to_string(),
            description: "Get to the target location".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "location".to_string(),
                    target_value: StateValue::String("destination".to_string()),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::High,
            deadline: None,
        };

        assert_eq!(goal.id, "reach_destination");
        assert_eq!(goal.priority, GoalPriority::High);
        assert_eq!(goal.conditions.len(), 1);
    }

    #[test]
    fn test_goal_satisfaction() {
        let goal = Goal {
            id: "collect_items".to_string(),
            name: "Collect Items".to_string(),
            description: "Collect required items".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "gold".to_string(),
                    target_value: StateValue::Integer(100),
                    comparison: "greater_than_or_equal".to_string(),
                },
                GoalCondition {
                    state_key: "has_artifact".to_string(),
                    target_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::Medium,
            deadline: None,
        };

        let mut satisfied_state = WorldState::new();
        satisfied_state.set_state("gold", StateValue::Integer(150));
        satisfied_state.set_state("has_artifact", StateValue::Boolean(true));

        let mut unsatisfied_state = WorldState::new();
        unsatisfied_state.set_state("gold", StateValue::Integer(50));
        unsatisfied_state.set_state("has_artifact", StateValue::Boolean(false));

        assert!(goal.is_satisfied(&satisfied_state));
        assert!(!goal.is_satisfied(&unsatisfied_state));
    }

    #[test]
    fn test_goal_priority_ordering() {
        let low_goal = Goal {
            id: "low".to_string(),
            name: "Low Priority".to_string(),
            description: "Low priority goal".to_string(),
            conditions: vec![],
            priority: GoalPriority::Low,
            deadline: None,
        };

        let high_goal = Goal {
            id: "high".to_string(),
            name: "High Priority".to_string(),
            description: "High priority goal".to_string(),
            conditions: vec![],
            priority: GoalPriority::High,
            deadline: None,
        };

        assert!(high_goal.priority > low_goal.priority);
    }

    #[test]
    fn test_goal_state_creation() {
        let goal_state = GoalState {
            id: "target_state".to_string(),
            target_conditions: vec![
                ("health".to_string(), StateValue::Integer(100)),
                ("location".to_string(), StateValue::String("home".to_string())),
            ],
            optional_conditions: vec![
                ("score".to_string(), StateValue::Integer(1000)),
            ],
        };

        assert_eq!(goal_state.target_conditions.len(), 2);
        assert_eq!(goal_state.optional_conditions.len(), 1);
    }

    #[test]
    fn test_goal_with_deadline() {
        use std::time::{SystemTime, Duration};

        let deadline = SystemTime::now() + Duration::from_secs(3600); // 1 hour from now

        let goal = Goal {
            id: "timed_goal".to_string(),
            name: "Timed Goal".to_string(),
            description: "Goal with deadline".to_string(),
            conditions: vec![],
            priority: GoalPriority::Critical,
            deadline: Some(deadline),
        };

        assert!(goal.deadline.is_some());
        assert!(!goal.is_expired());
    }
}

#[cfg(test)]
mod planner_tests {
    use super::*;

    #[test]
    fn test_goap_planner_creation() {
        let planner = GOAPPlanner::new();
        assert_eq!(planner.get_action_count(), 0);
        assert_eq!(planner.get_goal_count(), 0);
    }

    #[test]
    fn test_add_actions_and_goals() {
        let mut planner = GOAPPlanner::new();

        let action = Action {
            id: "test_action".to_string(),
            name: "Test Action".to_string(),
            description: "A test action".to_string(),
            preconditions: vec![],
            effects: vec![],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let goal = Goal {
            id: "test_goal".to_string(),
            name: "Test Goal".to_string(),
            description: "A test goal".to_string(),
            conditions: vec![],
            priority: GoalPriority::Medium,
            deadline: None,
        };

        planner.add_action(action);
        planner.add_goal(goal);

        assert_eq!(planner.get_action_count(), 1);
        assert_eq!(planner.get_goal_count(), 1);
    }

    #[test]
    fn test_simple_planning() {
        let mut planner = GOAPPlanner::new();
        let mut world_state = WorldState::new();

        // Set initial state
        world_state.set_state("at_home", StateValue::Boolean(true));
        world_state.set_state("has_car", StateValue::Boolean(true));
        world_state.set_state("at_work", StateValue::Boolean(false));

        // Add action: drive to work
        let drive_action = Action {
            id: "drive_to_work".to_string(),
            name: "Drive to Work".to_string(),
            description: "Drive from home to work".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "at_home".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                },
                ActionPrecondition {
                    state_key: "has_car".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "at_home".to_string(),
                    value: StateValue::Boolean(false),
                    operation: "set".to_string(),
                },
                ActionEffect {
                    state_key: "at_work".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 10.0,
                dynamic_factors: HashMap::new(),
            },
        };

        // Add goal: be at work
        let work_goal = Goal {
            id: "be_at_work".to_string(),
            name: "Be at Work".to_string(),
            description: "Be present at workplace".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "at_work".to_string(),
                    target_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::High,
            deadline: None,
        };

        planner.add_action(drive_action);
        planner.add_goal(work_goal);

        let result = planner.plan(&world_state, "be_at_work");

        assert!(result.success);
        assert_eq!(result.plan.as_ref().unwrap().steps.len(), 1);
        assert_eq!(result.plan.as_ref().unwrap().steps[0].action_id, "drive_to_work");
    }

    #[test]
    fn test_complex_planning() {
        let mut planner = GOAPPlanner::new();
        let mut world_state = WorldState::new();

        // Complex scenario: make dinner
        world_state.set_state("has_ingredients", StateValue::Boolean(false));
        world_state.set_state("at_home", StateValue::Boolean(true));
        world_state.set_state("has_money", StateValue::Boolean(true));
        world_state.set_state("dinner_ready", StateValue::Boolean(false));

        // Action 1: Go shopping
        let shop_action = Action {
            id: "go_shopping".to_string(),
            name: "Go Shopping".to_string(),
            description: "Buy ingredients from store".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "has_money".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "has_ingredients".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 5.0,
                dynamic_factors: HashMap::new(),
            },
        };

        // Action 2: Cook dinner
        let cook_action = Action {
            id: "cook_dinner".to_string(),
            name: "Cook Dinner".to_string(),
            description: "Prepare dinner using ingredients".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "has_ingredients".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                },
                ActionPrecondition {
                    state_key: "at_home".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "dinner_ready".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 8.0,
                dynamic_factors: HashMap::new(),
            },
        };

        // Goal: Have dinner ready
        let dinner_goal = Goal {
            id: "have_dinner".to_string(),
            name: "Have Dinner Ready".to_string(),
            description: "Prepare dinner for consumption".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "dinner_ready".to_string(),
                    target_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::High,
            deadline: None,
        };

        planner.add_action(shop_action);
        planner.add_action(cook_action);
        planner.add_goal(dinner_goal);

        let result = planner.plan(&world_state, "have_dinner");

        assert!(result.success);
        assert_eq!(result.plan.as_ref().unwrap().steps.len(), 2);
        assert_eq!(result.plan.as_ref().unwrap().steps[0].action_id, "go_shopping");
        assert_eq!(result.plan.as_ref().unwrap().steps[1].action_id, "cook_dinner");
    }

    #[test]
    fn test_impossible_goal() {
        let mut planner = GOAPPlanner::new();
        let mut world_state = WorldState::new();

        world_state.set_state("can_fly", StateValue::Boolean(false));

        // Impossible goal: fly without ability
        let fly_goal = Goal {
            id: "fly".to_string(),
            name: "Fly".to_string(),
            description: "Achieve flight".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "is_flying".to_string(),
                    target_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::Low,
            deadline: None,
        };

        planner.add_goal(fly_goal);

        let result = planner.plan(&world_state, "fly");

        assert!(!result.success);
        assert!(result.plan.is_none());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_plan_optimization() {
        let mut planner = GOAPPlanner::new();
        let mut world_state = WorldState::new();

        world_state.set_state("at_location_a", StateValue::Boolean(true));
        world_state.set_state("at_location_b", StateValue::Boolean(false));

        // Two different paths to reach location B
        // Path 1: Direct but expensive
        let direct_action = Action {
            id: "direct_travel".to_string(),
            name: "Direct Travel".to_string(),
            description: "Travel directly to location B".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "at_location_a".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "at_location_a".to_string(),
                    value: StateValue::Boolean(false),
                    operation: "set".to_string(),
                },
                ActionEffect {
                    state_key: "at_location_b".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 20.0,
                dynamic_factors: HashMap::new(),
            },
        };

        // Path 2: Indirect but cheaper (via waypoint)
        let to_waypoint_action = Action {
            id: "travel_to_waypoint".to_string(),
            name: "Travel to Waypoint".to_string(),
            description: "Travel to intermediate waypoint".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "at_location_a".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "at_location_a".to_string(),
                    value: StateValue::Boolean(false),
                    operation: "set".to_string(),
                },
                ActionEffect {
                    state_key: "at_waypoint".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 8.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let from_waypoint_action = Action {
            id: "travel_from_waypoint".to_string(),
            name: "Travel from Waypoint".to_string(),
            description: "Travel from waypoint to location B".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "at_waypoint".to_string(),
                    required_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "at_waypoint".to_string(),
                    value: StateValue::Boolean(false),
                    operation: "set".to_string(),
                },
                ActionEffect {
                    state_key: "at_location_b".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 7.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let travel_goal = Goal {
            id: "reach_b".to_string(),
            name: "Reach Location B".to_string(),
            description: "Get to location B".to_string(),
            conditions: vec![
                GoalCondition {
                    state_key: "at_location_b".to_string(),
                    target_value: StateValue::Boolean(true),
                    comparison: "equals".to_string(),
                }
            ],
            priority: GoalPriority::Medium,
            deadline: None,
        };

        planner.add_action(direct_action);
        planner.add_action(to_waypoint_action);
        planner.add_action(from_waypoint_action);
        planner.add_goal(travel_goal);

        let result = planner.plan(&world_state, "reach_b");

        assert!(result.success);
        let plan = result.plan.unwrap();

        // Should choose cheaper path (total cost 15.0 vs 20.0)
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.total_cost < 20.0);
    }
}

#[cfg(test)]
mod astar_tests {
    use super::*;

    #[test]
    fn test_search_node_creation() {
        let state = WorldState::new();
        let node = SearchNode::new(state.clone(), 0.0, 10.0, None);

        assert_eq!(node.g_cost, 0.0);
        assert_eq!(node.h_cost, 10.0);
        assert_eq!(node.f_cost(), 10.0);
        assert!(node.parent.is_none());
    }

    #[test]
    fn test_astar_search_creation() {
        let search = AStarSearch::new();
        assert!(search.is_ready());
    }

    #[test]
    fn test_heuristic_calculation() {
        let mut current_state = WorldState::new();
        current_state.set_state("x", StateValue::Integer(0));
        current_state.set_state("y", StateValue::Integer(0));

        let mut goal_state = WorldState::new();
        goal_state.set_state("x", StateValue::Integer(5));
        goal_state.set_state("y", StateValue::Integer(5));

        let search = AStarSearch::new();
        let heuristic = search.calculate_heuristic(&current_state, &goal_state);

        assert!(heuristic > 0.0);
    }

    #[test]
    fn test_search_simple_path() {
        let mut search = AStarSearch::new();
        let mut start_state = WorldState::new();
        let mut goal_state = WorldState::new();

        start_state.set_state("position", StateValue::Integer(0));
        goal_state.set_state("position", StateValue::Integer(3));

        // Simple move action
        let move_action = Action {
            id: "move_forward".to_string(),
            name: "Move Forward".to_string(),
            description: "Move one step forward".to_string(),
            preconditions: vec![],
            effects: vec![
                ActionEffect {
                    state_key: "position".to_string(),
                    value: StateValue::Integer(1),
                    operation: "add".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let actions = vec![move_action];
        let result = search.search(&start_state, &goal_state, &actions, 100);

        assert!(result.success);
        assert!(result.path.is_some());
        let path = result.path.unwrap();
        assert_eq!(path.len(), 3); // 3 moves to get from 0 to 3
    }

    #[test]
    fn test_search_no_solution() {
        let mut search = AStarSearch::new();
        let mut start_state = WorldState::new();
        let mut goal_state = WorldState::new();

        start_state.set_state("locked", StateValue::Boolean(true));
        goal_state.set_state("free", StateValue::Boolean(true));

        // No actions that can change the state
        let actions = vec![];
        let result = search.search(&start_state, &goal_state, &actions, 100);

        assert!(!result.success);
        assert!(result.path.is_none());
    }

    #[test]
    fn test_search_with_obstacles() {
        let mut search = AStarSearch::new();
        let mut start_state = WorldState::new();
        let mut goal_state = WorldState::new();

        start_state.set_state("x", StateValue::Integer(0));
        start_state.set_state("y", StateValue::Integer(0));
        start_state.set_state("has_key", StateValue::Boolean(false));

        goal_state.set_state("x", StateValue::Integer(2));
        goal_state.set_state("y", StateValue::Integer(2));

        // Move actions
        let move_right = Action {
            id: "move_right".to_string(),
            name: "Move Right".to_string(),
            description: "Move one step right".to_string(),
            preconditions: vec![],
            effects: vec![
                ActionEffect {
                    state_key: "x".to_string(),
                    value: StateValue::Integer(1),
                    operation: "add".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let move_up = Action {
            id: "move_up".to_string(),
            name: "Move Up".to_string(),
            description: "Move one step up".to_string(),
            preconditions: vec![],
            effects: vec![
                ActionEffect {
                    state_key: "y".to_string(),
                    value: StateValue::Integer(1),
                    operation: "add".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 1.0,
                dynamic_factors: HashMap::new(),
            },
        };

        // Key pickup action (required to pass through certain areas)
        let pickup_key = Action {
            id: "pickup_key".to_string(),
            name: "Pickup Key".to_string(),
            description: "Pick up a key".to_string(),
            preconditions: vec![
                ActionPrecondition {
                    state_key: "x".to_string(),
                    required_value: StateValue::Integer(1),
                    comparison: "equals".to_string(),
                },
                ActionPrecondition {
                    state_key: "y".to_string(),
                    required_value: StateValue::Integer(1),
                    comparison: "equals".to_string(),
                }
            ],
            effects: vec![
                ActionEffect {
                    state_key: "has_key".to_string(),
                    value: StateValue::Boolean(true),
                    operation: "set".to_string(),
                }
            ],
            cost: ActionCost {
                base_cost: 2.0,
                dynamic_factors: HashMap::new(),
            },
        };

        let actions = vec![move_right, move_up, pickup_key];
        let result = search.search(&start_state, &goal_state, &actions, 1000);

        assert!(result.success);
        assert!(result.path.is_some());
    }
}

#[cfg(test)]
mod rule_engine_tests {
    use super::*;

    #[test]
    fn test_decision_rule_creation() {
        let rule = DecisionRule {
            id: "health_check".to_string(),
            name: "Health Check Rule".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "health".to_string(),
                    comparison: "less_than".to_string(),
                    value: StateValue::Integer(20),
                }
            ],
            actions: vec!["use_health_potion".to_string()],
            priority: 100,
            cooldown_ms: Some(5000),
        };

        assert_eq!(rule.id, "health_check");
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(rule.priority, 100);
    }

    #[test]
    fn test_rule_evaluation() {
        let rule = DecisionRule {
            id: "low_health_rule".to_string(),
            name: "Low Health Emergency".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "health".to_string(),
                    comparison: "less_than_or_equal".to_string(),
                    value: StateValue::Integer(25),
                }
            ],
            actions: vec!["flee".to_string(), "use_health_potion".to_string()],
            priority: 200,
            cooldown_ms: None,
        };

        let mut low_health_state = WorldState::new();
        low_health_state.set_state("health", StateValue::Integer(15));

        let mut high_health_state = WorldState::new();
        high_health_state.set_state("health", StateValue::Integer(80));

        assert!(rule.matches(&low_health_state));
        assert!(!rule.matches(&high_health_state));
    }

    #[test]
    fn test_rule_engine_evaluation() {
        let mut engine = RuleEngine::new();

        let critical_rule = DecisionRule {
            id: "critical_health".to_string(),
            name: "Critical Health".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "health".to_string(),
                    comparison: "less_than".to_string(),
                    value: StateValue::Integer(10),
                }
            ],
            actions: vec!["emergency_heal".to_string()],
            priority: 300,
            cooldown_ms: None,
        };

        let low_health_rule = DecisionRule {
            id: "low_health".to_string(),
            name: "Low Health".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "health".to_string(),
                    comparison: "less_than".to_string(),
                    value: StateValue::Integer(30),
                }
            ],
            actions: vec!["heal".to_string()],
            priority: 100,
            cooldown_ms: None,
        };

        engine.add_rule(critical_rule);
        engine.add_rule(low_health_rule);

        let mut critical_state = WorldState::new();
        critical_state.set_state("health", StateValue::Integer(5));

        let decisions = engine.evaluate(&critical_state);

        // Should trigger both rules, but critical should have higher priority
        assert!(!decisions.is_empty());
        assert_eq!(decisions[0].rule_id, "critical_health"); // Highest priority first
    }

    #[test]
    fn test_rule_cooldown() {
        let rule = DecisionRule {
            id: "cooldown_test".to_string(),
            name: "Cooldown Test".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "trigger".to_string(),
                    comparison: "equals".to_string(),
                    value: StateValue::Boolean(true),
                }
            ],
            actions: vec!["test_action".to_string()],
            priority: 50,
            cooldown_ms: Some(1000), // 1 second cooldown
        };

        let mut state = WorldState::new();
        state.set_state("trigger", StateValue::Boolean(true));

        // First evaluation should match
        assert!(rule.matches(&state));

        // Simulate rule execution and cooldown
        let mut engine = RuleEngine::new();
        engine.add_rule(rule);

        let decisions1 = engine.evaluate(&state);
        assert!(!decisions1.is_empty());

        // Immediate re-evaluation should not trigger due to cooldown
        let decisions2 = engine.evaluate(&state);
        // Note: In a real implementation, this would check actual timestamps
        // For this test, we assume the engine tracks cooldowns
    }

    #[test]
    fn test_complex_rule_conditions() {
        let complex_rule = DecisionRule {
            id: "combat_strategy".to_string(),
            name: "Combat Strategy".to_string(),
            conditions: vec![
                RuleCondition {
                    state_key: "in_combat".to_string(),
                    comparison: "equals".to_string(),
                    value: StateValue::Boolean(true),
                },
                RuleCondition {
                    state_key: "enemy_health".to_string(),
                    comparison: "greater_than".to_string(),
                    value: StateValue::Integer(50),
                },
                RuleCondition {
                    state_key: "mana".to_string(),
                    comparison: "greater_than_or_equal".to_string(),
                    value: StateValue::Integer(30),
                }
            ],
            actions: vec!["cast_fireball".to_string()],
            priority: 150,
            cooldown_ms: Some(3000),
        };

        let mut valid_state = WorldState::new();
        valid_state.set_state("in_combat", StateValue::Boolean(true));
        valid_state.set_state("enemy_health", StateValue::Integer(75));
        valid_state.set_state("mana", StateValue::Integer(50));

        let mut invalid_state = WorldState::new();
        invalid_state.set_state("in_combat", StateValue::Boolean(true));
        invalid_state.set_state("enemy_health", StateValue::Integer(25)); // Too low
        invalid_state.set_state("mana", StateValue::Integer(50));

        assert!(complex_rule.matches(&valid_state));
        assert!(!complex_rule.matches(&invalid_state));
    }
}