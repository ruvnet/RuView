use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use planner::{Planner, State, Goal, Action, AStarPlanner};
use std::collections::{HashMap, HashSet};
use rand::prelude::*;

fn generate_test_state(complexity: usize) -> State {
    let mut rng = rand::thread_rng();
    let mut properties = HashMap::new();

    // Generate properties based on complexity
    for i in 0..complexity {
        let key = format!("property_{}", i);
        let value = match rng.gen_range(0..4) {
            0 => serde_json::Value::Bool(rng.gen()),
            1 => serde_json::Value::Number(serde_json::Number::from(rng.gen_range(0..100))),
            2 => serde_json::Value::String(format!("value_{}", rng.gen_range(0..10))),
            _ => serde_json::Value::Array(vec![
                serde_json::Value::Number(serde_json::Number::from(rng.gen_range(0..10))),
                serde_json::Value::Number(serde_json::Number::from(rng.gen_range(0..10))),
            ]),
        };
        properties.insert(key, value);
    }

    State::new(properties)
}

fn generate_test_goal(target_properties: usize) -> Goal {
    let mut rng = rand::thread_rng();
    let mut conditions = HashMap::new();

    for i in 0..target_properties {
        let key = format!("property_{}", i);
        let value = serde_json::Value::Bool(true);
        conditions.insert(key, value);
    }

    Goal::new("test_goal", conditions)
}

fn generate_test_actions(count: usize, state_complexity: usize) -> Vec<Action> {
    let mut rng = rand::thread_rng();
    let mut actions = Vec::new();

    for i in 0..count {
        let mut preconditions = HashMap::new();
        let mut effects = HashMap::new();

        // Generate random preconditions
        let precond_count = rng.gen_range(1..std::cmp::min(5, state_complexity + 1));
        for j in 0..precond_count {
            let key = format!("property_{}", rng.gen_range(0..state_complexity));
            preconditions.insert(key, serde_json::Value::Bool(rng.gen()));
        }

        // Generate random effects
        let effect_count = rng.gen_range(1..std::cmp::min(3, state_complexity + 1));
        for _ in 0..effect_count {
            let key = format!("property_{}", rng.gen_range(0..state_complexity));
            effects.insert(key, serde_json::Value::Bool(rng.gen()));
        }

        let action = Action::new(
            &format!("action_{}", i),
            preconditions,
            effects,
            rng.gen_range(1..10) as f64, // Random cost
        );

        actions.push(action);
    }

    actions
}

fn bench_state_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_operations");

    for &complexity in [10, 50, 200, 1000].iter() {
        let state = generate_test_state(complexity);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("state_creation", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    let state = generate_test_state(black_box(complexity));
                    black_box(state);
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("state_clone", complexity),
            &state,
            |b, state| {
                b.iter(|| {
                    let cloned = state.clone();
                    black_box(cloned);
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("state_comparison", complexity),
            &state,
            |b, state| {
                let other_state = generate_test_state(complexity);
                b.iter(|| {
                    let result = state == &other_state;
                    black_box(result);
                });
            }
        );
    }

    group.finish();
}

fn bench_action_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("action_application");

    for &state_complexity in [50, 200, 500].iter() {
        for &action_count in [10, 50, 100].iter() {
            let state = generate_test_state(state_complexity);
            let actions = generate_test_actions(action_count, state_complexity);

            group.throughput(Throughput::Elements(action_count as u64));
            group.bench_with_input(
                BenchmarkId::new("apply_actions", format!("{}_{}", state_complexity, action_count)),
                &(state, actions),
                |b, (state, actions)| {
                    b.iter(|| {
                        let mut current_state = state.clone();
                        for action in actions {
                            if action.can_apply(&current_state) {
                                current_state = action.apply(&current_state);
                            }
                        }
                        black_box(current_state);
                    });
                }
            );
        }
    }

    group.finish();
}

fn bench_goal_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("goal_checking");

    for &complexity in [10, 50, 200, 500].iter() {
        let state = generate_test_state(complexity);
        let goal = generate_test_goal(complexity / 4);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("goal_satisfaction", complexity),
            &(state, goal),
            |b, (state, goal)| {
                b.iter(|| {
                    let satisfied = goal.is_satisfied(black_box(state));
                    black_box(satisfied);
                });
            }
        );
    }

    group.finish();
}

fn bench_astar_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("astar_planning");

    for &complexity in [20, 50, 100].iter() {
        let initial_state = generate_test_state(complexity);
        let goal = generate_test_goal(complexity / 4);
        let actions = generate_test_actions(complexity * 2, complexity);

        let planner = AStarPlanner::new();

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("plan_generation", complexity),
            &(initial_state, goal, actions),
            |b, (initial_state, goal, actions)| {
                b.iter(|| {
                    let plan = planner.plan(
                        black_box(initial_state),
                        black_box(goal),
                        black_box(actions),
                        Some(1000) // Max iterations
                    );
                    black_box(plan);
                });
            }
        );
    }

    group.finish();
}

fn bench_heuristic_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("heuristic_computation");

    for &complexity in [50, 200, 500, 1000].iter() {
        let state = generate_test_state(complexity);
        let goal = generate_test_goal(complexity / 4);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("heuristic_calculation", complexity),
            &(state, goal),
            |b, (state, goal)| {
                b.iter(|| {
                    // Simple heuristic: count unsatisfied goal conditions
                    let mut unsatisfied = 0;
                    for (key, target_value) in goal.conditions() {
                        if let Some(current_value) = state.get_property(key) {
                            if current_value != target_value {
                                unsatisfied += 1;
                            }
                        } else {
                            unsatisfied += 1;
                        }
                    }
                    black_box(unsatisfied);
                });
            }
        );
    }

    group.finish();
}

fn bench_plan_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("plan_validation");

    for &complexity in [50, 100, 200].iter() {
        let initial_state = generate_test_state(complexity);
        let goal = generate_test_goal(complexity / 4);
        let actions = generate_test_actions(complexity, complexity);

        // Generate a random plan
        let mut rng = rand::thread_rng();
        let plan_length = rng.gen_range(5..15);
        let plan: Vec<_> = (0..plan_length)
            .map(|_| actions.choose(&mut rng).unwrap().clone())
            .collect();

        group.throughput(Throughput::Elements(plan_length as u64));
        group.bench_with_input(
            BenchmarkId::new("validate_plan", complexity),
            &(initial_state, goal, plan),
            |b, (initial_state, goal, plan)| {
                b.iter(|| {
                    let mut current_state = initial_state.clone();
                    let mut valid = true;

                    for action in plan {
                        if !action.can_apply(&current_state) {
                            valid = false;
                            break;
                        }
                        current_state = action.apply(&current_state);
                    }

                    let goal_achieved = goal.is_satisfied(&current_state);
                    black_box((valid, goal_achieved));
                });
            }
        );
    }

    group.finish();
}

fn bench_planning_with_different_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("planning_strategies");

    let complexity = 100;
    let initial_state = generate_test_state(complexity);
    let goal = generate_test_goal(complexity / 4);
    let actions = generate_test_actions(complexity, complexity);

    // Different heuristics can be simulated by different planner configurations
    let strategies = ["greedy", "balanced", "thorough"];

    for strategy in strategies.iter() {
        let max_iterations = match *strategy {
            "greedy" => 100,
            "balanced" => 500,
            "thorough" => 1000,
            _ => 500,
        };

        let planner = AStarPlanner::new();

        group.bench_with_input(
            BenchmarkId::new("strategy", strategy),
            &(initial_state.clone(), goal.clone(), actions.clone()),
            |b, (initial_state, goal, actions)| {
                b.iter(|| {
                    let plan = planner.plan(
                        black_box(initial_state),
                        black_box(goal),
                        black_box(actions),
                        Some(black_box(max_iterations))
                    );
                    black_box(plan);
                });
            }
        );
    }

    group.finish();
}

fn bench_memory_usage_during_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage_planning");

    for &complexity in [50, 100, 200].iter() {
        let initial_state = generate_test_state(complexity);
        let goal = generate_test_goal(complexity / 4);
        let actions = generate_test_actions(complexity * 2, complexity);

        group.bench_with_input(
            BenchmarkId::new("memory_intensive_planning", complexity),
            &(initial_state, goal, actions),
            |b, (initial_state, goal, actions)| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        let planner = AStarPlanner::new();
                        let plan = planner.plan(
                            initial_state,
                            goal,
                            actions,
                            Some(500)
                        );

                        // Force memory allocation by storing intermediate results
                        let mut all_states = Vec::new();
                        if let Ok(plan) = plan {
                            let mut current_state = initial_state.clone();
                            for action in &plan {
                                current_state = action.apply(&current_state);
                                all_states.push(current_state.clone());
                            }
                        }

                        black_box((plan, all_states));
                    }

                    start.elapsed()
                });
            }
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_state_operations,
    bench_action_application,
    bench_goal_checking,
    bench_astar_planning,
    bench_heuristic_computation,
    bench_plan_validation,
    bench_planning_with_different_strategies,
    bench_memory_usage_during_planning
);

criterion_main!(benches);