/**
 * GOAP Planner Tests
 * Comprehensive testing for the GOAP planning system
 */

import { GoapPlanner } from '../src/goap/planner';
import { GoapAction, WorldState, GoapGoal, PlanningContext } from '../src/core/types';

describe('GoapPlanner', () => {
  let planner: GoapPlanner;

  beforeEach(() => {
    planner = new GoapPlanner();
  });

  // Test actions for planning
  const testActions: GoapAction[] = [
    {
      name: 'get_key',
      cost: 1,
      preconditions: [],
      effects: [{ key: 'has_key', value: true, operation: 'set' }],
      async execute(state: WorldState) {
        return {
          success: true,
          newState: { ...state, has_key: true }
        };
      }
    },
    {
      name: 'unlock_door',
      cost: 2,
      preconditions: [{ key: 'has_key', value: true, operator: 'equals' }],
      effects: [{ key: 'door_unlocked', value: true, operation: 'set' }],
      async execute(state: WorldState) {
        if (!state.has_key) {
          return {
            success: false,
            newState: state,
            error: 'No key available'
          };
        }
        return {
          success: true,
          newState: { ...state, door_unlocked: true }
        };
      }
    },
    {
      name: 'enter_room',
      cost: 1,
      preconditions: [{ key: 'door_unlocked', value: true, operator: 'equals' }],
      effects: [{ key: 'in_room', value: true, operation: 'set' }],
      async execute(state: WorldState) {
        if (!state.door_unlocked) {
          return {
            success: false,
            newState: state,
            error: 'Door is locked'
          };
        }
        return {
          success: true,
          newState: { ...state, in_room: true }
        };
      }
    }
  ];

  describe('Plan Creation', () => {
    test('should create a plan to achieve goal', async () => {
      const initialState: WorldState = {};
      const goal: GoapGoal = {
        name: 'enter_room',
        conditions: [{ key: 'in_room', value: true, operator: 'equals' }],
        priority: 1
      };

      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: testActions
      };

      const plan = await planner.createPlan(context);

      expect(plan).toBeTruthy();
      expect(plan!.steps).toHaveLength(3);
      expect(plan!.steps[0].action.name).toBe('get_key');
      expect(plan!.steps[1].action.name).toBe('unlock_door');
      expect(plan!.steps[2].action.name).toBe('enter_room');
      expect(plan!.totalCost).toBe(4); // 1 + 2 + 1
    });

    test('should return null when no plan exists', async () => {
      const initialState: WorldState = {};
      const goal: GoapGoal = {
        name: 'impossible_goal',
        conditions: [{ key: 'impossible', value: true, operator: 'equals' }],
        priority: 1
      };

      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: testActions
      };

      const plan = await planner.createPlan(context);
      expect(plan).toBeNull();
    });

    test('should return empty plan when goal already satisfied', async () => {
      const initialState: WorldState = { in_room: true };
      const goal: GoapGoal = {
        name: 'enter_room',
        conditions: [{ key: 'in_room', value: true, operator: 'equals' }],
        priority: 1
      };

      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: testActions
      };

      const plan = await planner.createPlan(context);

      expect(plan).toBeTruthy();
      expect(plan!.steps).toHaveLength(0);
      expect(plan!.totalCost).toBe(0);
    });
  });

  describe('Plan Execution', () => {
    test('should execute plan successfully', async () => {
      const initialState: WorldState = {};
      const goal: GoapGoal = {
        name: 'enter_room',
        conditions: [{ key: 'in_room', value: true, operator: 'equals' }],
        priority: 1
      };

      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: testActions
      };

      const plan = await planner.createPlan(context);
      expect(plan).toBeTruthy();

      const result = await planner.executePlan(plan!, testActions);

      expect(result.success).toBe(true);
      expect(result.executedSteps).toBe(3);
      expect(result.finalState.in_room).toBe(true);
      expect(result.finalState.has_key).toBe(true);
      expect(result.finalState.door_unlocked).toBe(true);
    });

    test('should handle action failures with replanning', async () => {
      // Create a failing action
      const failingActions: GoapAction[] = [
        {
          name: 'get_key',
          cost: 1,
          preconditions: [],
          effects: [{ key: 'has_key', value: true, operation: 'set' }],
          async execute(state: WorldState) {
            return {
              success: false,
              newState: state,
              error: 'Key not found'
            };
          }
        },
        ...testActions.slice(1) // Keep other actions
      ];

      const initialState: WorldState = {};
      const goal: GoapGoal = {
        name: 'enter_room',
        conditions: [{ key: 'in_room', value: true, operator: 'equals' }],
        priority: 1
      };

      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: failingActions
      };

      const plan = await planner.createPlan(context);
      expect(plan).toBeTruthy();

      const result = await planner.executePlan(plan!, failingActions);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Key not found');
    });
  });

  describe('Precondition Evaluation', () => {
    test('should evaluate different precondition operators', () => {
      const state: WorldState = {
        count: 5,
        items: ['a', 'b', 'c'],
        flag: true,
        missing: undefined
      };

      // Test equals
      expect(planner['evaluatePrecondition']({ key: 'flag', value: true, operator: 'equals' }, state)).toBe(true);
      expect(planner['evaluatePrecondition']({ key: 'flag', value: false, operator: 'equals' }, state)).toBe(false);

      // Test exists
      expect(planner['evaluatePrecondition']({ key: 'count', value: null, operator: 'exists' }, state)).toBe(true);
      expect(planner['evaluatePrecondition']({ key: 'missing', value: null, operator: 'exists' }, state)).toBe(false);

      // Test not_exists
      expect(planner['evaluatePrecondition']({ key: 'missing', value: null, operator: 'not_exists' }, state)).toBe(true);
      expect(planner['evaluatePrecondition']({ key: 'count', value: null, operator: 'not_exists' }, state)).toBe(false);

      // Test greater/less
      expect(planner['evaluatePrecondition']({ key: 'count', value: 3, operator: 'greater' }, state)).toBe(true);
      expect(planner['evaluatePrecondition']({ key: 'count', value: 10, operator: 'greater' }, state)).toBe(false);
      expect(planner['evaluatePrecondition']({ key: 'count', value: 10, operator: 'less' }, state)).toBe(true);

      // Test contains
      expect(planner['evaluatePrecondition']({ key: 'items', value: 'b', operator: 'contains' }, state)).toBe(true);
      expect(planner['evaluatePrecondition']({ key: 'items', value: 'x', operator: 'contains' }, state)).toBe(false);
    });
  });

  describe('Effect Application', () => {
    test('should apply different effect operations', () => {
      const state: WorldState = {
        count: 5,
        items: ['a', 'b'],
        flag: false
      };

      // Test set
      planner['applyEffect']({ key: 'flag', value: true, operation: 'set' }, state);
      expect(state.flag).toBe(true);

      // Test add
      planner['applyEffect']({ key: 'items', value: 'c', operation: 'add' }, state);
      expect(state.items).toEqual(['a', 'b', 'c']);

      // Test remove
      planner['applyEffect']({ key: 'items', value: 'b', operation: 'remove' }, state);
      expect(state.items).toEqual(['a', 'c']);

      // Test increment
      planner['applyEffect']({ key: 'count', value: 2, operation: 'increment' }, state);
      expect(state.count).toBe(7);

      // Test decrement
      planner['applyEffect']({ key: 'count', value: 3, operation: 'decrement' }, state);
      expect(state.count).toBe(4);
    });
  });

  describe('Heuristic Calculation', () => {
    test('should calculate heuristic distance to goal', () => {
      const state: WorldState = {
        has_key: true,
        door_unlocked: false,
        in_room: false
      };

      const goal: GoapGoal = {
        name: 'test_goal',
        conditions: [
          { key: 'door_unlocked', value: true, operator: 'equals' },
          { key: 'in_room', value: true, operator: 'equals' }
        ],
        priority: 1
      };

      const heuristic = planner['calculateHeuristic'](state, goal);
      expect(heuristic).toBe(2); // Two unsatisfied conditions
    });
  });
});