import {
  PlannerSystemWasm,
  PlannerSystemInstance,
  WorldState,
  StateValue,
  Action,
  Goal,
  PlanningResult,
  Plan,
  ExecutionResult,
  WasmExecutionError,
  InvalidInputError
} from '../types/index
import { SimpleWasmLoader } from '../wasm/wasm-loader-simple
import { WasmMemoryManager } from '../wasm/memory-manager
import {
  validateInput,
  schemas,
  WorldStateType,
  ActionType,
  GoalType,
  PlanningRequestType,
  StateUpdateType,
  PlanExecutionRequestType
} from '../schemas/index

export class PlannerWrapper {
  private wasmModule: PlannerSystemWasm | null = null;
  private memoryManager: WasmMemoryManager;
  private loader: SimpleWasmLoader;
  private initialized = false;

  constructor() {
    this.memoryManager = WasmMemoryManager.getInstance();
    this.loader = SimpleWasmLoader.getInstance();
  }

  /**
   * Initialize the WASM module
   */
  public async initialize(wasmPath: string): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      this.wasmModule = await this.loader.loadPlannerSystem({
        wasmPath,
        initTimeoutMs: 30000,
        memoryInitialPages: 256,
        memoryMaximumPages: 1024
      });
      this.initialized = true;
    } catch (error) {
      throw new WasmExecutionError('Planner system initialization', {
        wasmPath,
        originalError: error
      });
    }
  }

  /**
   * Create a new planner instance
   */
  public createInstance(instanceId?: string): string {
    this.ensureInitialized();
    
    const result = this.memoryManager.createInstance(
      () => new this.wasmModule!.PlannerSystem(),
      'planner_system',
      instanceId
    );
    
    return result.id;
  }

  /**
   * Get an existing instance
   */
  private getInstance(instanceId: string): PlannerSystemInstance {
    const instance = this.memoryManager.getInstance<PlannerSystemInstance>(instanceId);
    if (!instance) {
      throw new InvalidInputError(
        'instanceId',
        'valid instance ID',
        instanceId
      );
    }
    return instance;
  }

  /**
   * Update world state
   */
  public setState(
    instanceId: string,
    update: StateUpdateType
  ): boolean {
    validateInput(schemas.StateUpdate, update);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const valueJson = JSON.stringify(update.value);
      return instance.set_state(update.key, valueJson);
    } catch (error) {
      throw new WasmExecutionError('Setting state', {
        update,
        originalError: error
      });
    }
  }

  /**
   * Get world state value
   */
  public getState(instanceId: string, key: string): StateValue {
    const instance = this.getInstance(instanceId);
    
    try {
      const valueStr = instance.get_state(key);
      if (valueStr === 'null') {
        return null;
      }
      return JSON.parse(valueStr);
    } catch (error) {
      throw new WasmExecutionError('Getting state', {
        key,
        originalError: error
      });
    }
  }

  /**
   * Get complete world state
   */
  public getWorldState(instanceId: string): WorldState {
    const instance = this.getInstance(instanceId);
    
    try {
      const stateStr = instance.get_world_state();
      return JSON.parse(stateStr);
    } catch (error) {
      throw new WasmExecutionError('Getting world state', {
        instanceId,
        originalError: error
      });
    }
  }

  /**
   * Set multiple state values
   */
  public setWorldState(
    instanceId: string,
    worldState: WorldStateType
  ): boolean {
    validateInput(schemas.WorldState, worldState);
    
    let allSuccessful = true;
    
    for (const [key, value] of Object.entries(worldState.states)) {
      const success = this.setState(instanceId, { key, value });
      if (!success) {
        allSuccessful = false;
      }
    }
    
    return allSuccessful;
  }

  /**
   * Add an action to the planner
   */
  public addAction(
    instanceId: string,
    action: ActionType
  ): boolean {
    validateInput(schemas.Action, action);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const actionJson = JSON.stringify(action);
      return instance.add_action(actionJson);
    } catch (error) {
      throw new WasmExecutionError('Adding action', {
        action,
        originalError: error
      });
    }
  }

  /**
   * Add multiple actions
   */
  public addActionsBulk(
    instanceId: string,
    actions: ActionType[]
  ): { successful: number; failed: number; errors: string[] } {
    const errors: string[] = [];
    let successful = 0;
    
    for (const action of actions) {
      try {
        if (this.addAction(instanceId, action)) {
          successful++;
        } else {
          errors.push(`Failed to add action: ${action.id}`);
        }
      } catch (error) {
        errors.push(`Error adding action ${action.id}: ${error.message}`);
      }
    }
    
    return {
      successful,
      failed: actions.length - successful,
      errors
    };
  }

  /**
   * Add a goal to the planner
   */
  public addGoal(
    instanceId: string,
    goal: GoalType
  ): boolean {
    validateInput(schemas.Goal, goal);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const goalJson = JSON.stringify(goal);
      return instance.add_goal(goalJson);
    } catch (error) {
      throw new WasmExecutionError('Adding goal', {
        goal,
        originalError: error
      });
    }
  }

  /**
   * Create a plan for a specific goal
   */
  public planForGoal(
    instanceId: string,
    goalId: string,
    options: {
      maxDepth?: number;
      timeoutMs?: number;
      heuristic?: 'astar' | 'dijkstra' | 'greedy';
      allowPartialPlans?: boolean;
    } = {}
  ): PlanningResult {
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.plan(goalId);
      
      let result: PlanningResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing planning result', {
          goalId,
          resultStr,
          originalError: parseError
        });
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Planning for goal', {
        goalId,
        originalError: error
      });
    }
  }

  /**
   * Create a plan to reach a target state
   */
  public planToState(
    instanceId: string,
    request: PlanningRequestType
  ): PlanningResult {
    validateInput(schemas.PlanningRequest, request);
    
    if (!request.target_state) {
      throw new InvalidInputError(
        'target_state',
        'WorldState object',
        request.target_state
      );
    }
    
    const instance = this.getInstance(instanceId);
    
    try {
      const targetStateJson = JSON.stringify(request.target_state);
      const resultStr = instance.plan_to_state(targetStateJson);
      
      let result: PlanningResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing planning result', {
          targetState: request.target_state,
          resultStr,
          originalError: parseError
        });
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Planning to state', {
        targetState: request.target_state,
        originalError: error
      });
    }
  }

  /**
   * Execute a plan
   */
  public executePlan(
    instanceId: string,
    request: PlanExecutionRequestType
  ): ExecutionResult {
    validateInput(schemas.PlanExecutionRequest, request);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.execute_plan(request.plan_json);
      
      let result: ExecutionResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing execution result', {
          planJson: request.plan_json.substring(0, 200) + '...',
          resultStr,
          originalError: parseError
        });
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Executing plan', {
        originalError: error
      });
    }
  }

  /**
   * Execute a plan with validation
   */
  public executePlanSafely(
    instanceId: string,
    plan: Plan,
    options: {
      dryRun?: boolean;
      stopOnFailure?: boolean;
      validatePreconditions?: boolean;
    } = {}
  ): ExecutionResult {
    const {
      dryRun = false,
      stopOnFailure = true,
      validatePreconditions = true
    } = options;
    
    // Validate plan structure
    if (!plan.steps || plan.steps.length === 0) {
      throw new InvalidInputError(
        'plan.steps',
        'non-empty array',
        plan.steps
      );
    }
    
    if (dryRun) {
      // Simulate execution without actually changing state
      return {
        success: true,
        executed_steps: plan.steps,
        final_state: this.getWorldState(instanceId),
        total_cost: plan.total_cost || 0,
        error: undefined
      };
    }
    
    const planJson = JSON.stringify(plan);
    return this.executePlan(instanceId, {
      plan_json: planJson,
      options: {
        dry_run: dryRun,
        stop_on_failure: stopOnFailure,
        validate_preconditions: validatePreconditions
      }
    });
  }

  /**
   * Get available actions for the current state
   */
  public getAvailableActions(instanceId: string): Action[] {
    const instance = this.getInstance(instanceId);
    
    try {
      const actionsStr = instance.get_available_actions();
      return JSON.parse(actionsStr);
    } catch (error) {
      throw new WasmExecutionError('Getting available actions', {
        instanceId,
        originalError: error
      });
    }
  }

  /**
   * Add a decision rule
   */
  public addRule(
    instanceId: string,
    rule: any
  ): boolean {
    const instance = this.getInstance(instanceId);
    
    try {
      const ruleJson = JSON.stringify(rule);
      return instance.add_rule(ruleJson);
    } catch (error) {
      throw new WasmExecutionError('Adding rule', {
        rule,
        originalError: error
      });
    }
  }

  /**
   * Evaluate decision rules
   */
  public evaluateRules(instanceId: string): any[] {
    const instance = this.getInstance(instanceId);
    
    try {
      const decisionsStr = instance.evaluate_rules();
      return JSON.parse(decisionsStr);
    } catch (error) {
      throw new WasmExecutionError('Evaluating rules', {
        instanceId,
        originalError: error
      });
    }
  }

  /**
   * Create a complex planning scenario with multiple goals
   */
  public planMultipleGoals(
    instanceId: string,
    goalIds: string[],
    options: {
      prioritize?: boolean;
      allowParallel?: boolean;
      maxTotalCost?: number;
    } = {}
  ): {
    plans: { goalId: string; plan: PlanningResult }[];
    totalCost: number;
    conflicts: string[];
    recommendations: string[];
  } {
    const plans: { goalId: string; plan: PlanningResult }[] = [];
    const conflicts: string[] = [];
    const recommendations: string[] = [];
    let totalCost = 0;
    
    for (const goalId of goalIds) {
      try {
        const plan = this.planForGoal(instanceId, goalId);
        plans.push({ goalId, plan });
        
        if (plan.success && plan.plan) {
          totalCost += plan.plan.total_cost;
        }
      } catch (error) {
        conflicts.push(`Failed to plan for goal ${goalId}: ${error.message}`);
      }
    }
    
    // Check for cost constraints
    if (options.maxTotalCost && totalCost > options.maxTotalCost) {
      recommendations.push(
        `Total cost ${totalCost} exceeds maximum ${options.maxTotalCost}`
      );
    }
    
    return {
      plans,
      totalCost,
      conflicts,
      recommendations
    };
  }

  /**
   * Reset planner to initial state
   */
  public reset(instanceId: string): void {
    // Since WASM doesn't expose a reset method, we recreate the instance
    this.removeInstance(instanceId);
    this.createInstance(instanceId);
  }

  /**
   * Remove an instance
   */
  public removeInstance(instanceId: string): boolean {
    return this.memoryManager.removeInstance(instanceId);
  }

  /**
   * Get all active instance IDs
   */
  public getActiveInstances(): string[] {
    const instances = this.memoryManager.getInstancesByType('planner_system');
    return Array.from(instances.keys());
  }

  /**
   * Validate that the module is initialized
   */
  private ensureInitialized(): void {
    if (!this.initialized || !this.wasmModule) {
      throw new WasmExecutionError('Module not initialized', {
        hint: 'Call initialize() first'
      });
    }
  }

  /**
   * Get memory usage for this wrapper
   */
  public getMemoryStats(): any {
    const managerStats = this.memoryManager.getMemoryStats();
    const plannerInstances = this.memoryManager.getInstancesByType('planner_system');
    
    return {
      ...managerStats,
      plannerInstances: plannerInstances.size,
      initialized: this.initialized
    };
  }

  /**
   * Cleanup all instances
   */
  public cleanup(): void {
    this.memoryManager.removeInstancesByType('planner_system');
  }
}