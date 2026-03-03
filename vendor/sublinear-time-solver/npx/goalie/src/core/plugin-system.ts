/**
 * Plugin System with Lifecycle Hooks
 * Allows extensible functionality through plugin architecture
 */

import {
  GoapPlugin,
  PluginHooks,
  PlanningContext,
  GoapPlan,
  PlanStep,
  WorldState,
  PlanExecutionResult
} from './types.js';

export class PluginRegistry {
  private plugins: Map<string, GoapPlugin> = new Map();
  private enabledPlugins: Set<string> = new Set();
  private initialized = false;

  /**
   * Register a plugin
   */
  register(plugin: GoapPlugin): void {
    if (this.plugins.has(plugin.name)) {
      throw new Error(`Plugin ${plugin.name} is already registered`);
    }

    this.plugins.set(plugin.name, plugin);
    this.enabledPlugins.add(plugin.name); // Enable by default
    console.log(`Registered plugin: ${plugin.name} v${plugin.version}`);
  }

  /**
   * Unregister a plugin
   */
  unregister(pluginName: string): void {
    const plugin = this.plugins.get(pluginName);
    if (plugin && plugin.cleanup) {
      plugin.cleanup();
    }
    this.plugins.delete(pluginName);
  }

  /**
   * Initialize all plugins
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    for (const plugin of this.plugins.values()) {
      if (plugin.initialize) {
        try {
          await plugin.initialize();
          console.log(`Initialized plugin: ${plugin.name}`);
        } catch (error) {
          console.error(`Failed to initialize plugin ${plugin.name}:`, error);
        }
      }
    }

    this.initialized = true;
  }

  /**
   * Execute onPlanStart hooks
   */
  async executeOnPlanStart(context: PlanningContext): Promise<void> {
    await this.executeHook('onPlanStart', context);
  }

  /**
   * Execute beforeSearch hooks
   */
  async executeBeforeSearch(context: PlanningContext): Promise<void> {
    await this.executeHook('beforeSearch', context);
  }

  /**
   * Execute afterSearch hooks
   */
  async executeAfterSearch(plan: GoapPlan | null, context: PlanningContext): Promise<void> {
    await this.executeHook('afterSearch', plan, context);
  }

  /**
   * Execute beforeExecute hooks
   */
  async executeBeforeExecute(step: PlanStep, state: WorldState): Promise<void> {
    await this.executeHook('beforeExecute', step, state);
  }

  /**
   * Execute afterExecute hooks
   */
  async executeAfterExecute(step: PlanStep, result: any, state: WorldState): Promise<void> {
    await this.executeHook('afterExecute', step, result, state);
  }

  /**
   * Execute onReplan hooks
   */
  async executeOnReplan(failedStep: PlanStep, state: WorldState): Promise<void> {
    await this.executeHook('onReplan', failedStep, state);
  }

  /**
   * Execute onPlanComplete hooks
   */
  async executeOnPlanComplete(result: PlanExecutionResult): Promise<void> {
    await this.executeHook('onPlanComplete', result);
  }

  /**
   * Execute onError hooks
   */
  async executeOnError(error: Error, context: any): Promise<void> {
    await this.executeHook('onError', error, context);
  }

  /**
   * Get list of registered plugins
   */
  getPlugins(): GoapPlugin[] {
    return Array.from(this.plugins.values());
  }

  /**
   * Get plugin by name
   */
  getPlugin(name: string): GoapPlugin | undefined {
    return this.plugins.get(name);
  }

  /**
   * List all registered plugins
   */
  listPlugins(): { name: string; version: string; description?: string; enabled: boolean }[] {
    return Array.from(this.plugins.values()).map(plugin => ({
      name: plugin.name,
      version: plugin.version,
      description: plugin.description,
      enabled: this.enabledPlugins.has(plugin.name)
    }));
  }

  /**
   * Enable a plugin by name
   */
  enablePlugin(name: string): { success: boolean; message: string } {
    const plugin = this.plugins.get(name);
    if (!plugin) {
      return { success: false, message: `Plugin ${name} not found` };
    }
    this.enabledPlugins.add(name);
    return { success: true, message: `Plugin ${name} enabled` };
  }

  /**
   * Disable a plugin by name
   */
  disablePlugin(name: string): { success: boolean; message: string } {
    const plugin = this.plugins.get(name);
    if (!plugin) {
      return { success: false, message: `Plugin ${name} not found` };
    }
    this.enabledPlugins.delete(name);
    return { success: true, message: `Plugin ${name} disabled` };
  }

  /**
   * Get detailed plugin information
   */
  getPluginInfo(name: string): any {
    const plugin = this.plugins.get(name);
    if (!plugin) {
      return { error: `Plugin ${name} not found` };
    }
    return {
      name: plugin.name,
      version: plugin.version,
      description: plugin.description,
      enabled: this.enabledPlugins.has(name),
      hooks: Object.keys(plugin.hooks)
    };
  }

  /**
   * Generic hook execution
   */
  private async executeHook(hookName: keyof PluginHooks, ...args: any[]): Promise<void> {
    for (const plugin of this.plugins.values()) {
      const hook = plugin.hooks[hookName] as any;
      if (hook) {
        try {
          await hook(...args);
        } catch (error) {
          console.error(`Error in plugin ${plugin.name} hook ${hookName}:`, error);
          // Continue executing other plugins even if one fails
        }
      }
    }
  }
}

/**
 * Plugin loader for external plugins
 */
export class PluginLoader {
  static async loadFromFile(filePath: string): Promise<GoapPlugin> {
    try {
      const pluginModule = await import(filePath);
      const plugin = pluginModule.default || pluginModule;

      if (!this.isValidPlugin(plugin)) {
        throw new Error(`Invalid plugin structure in ${filePath}`);
      }

      return plugin;
    } catch (error) {
      throw new Error(`Failed to load plugin from ${filePath}: ${error}`);
    }
  }

  static async loadFromFiles(filePaths: string[]): Promise<GoapPlugin[]> {
    const plugins: GoapPlugin[] = [];

    for (const filePath of filePaths) {
      try {
        const plugin = await this.loadFromFile(filePath);
        plugins.push(plugin);
      } catch (error) {
        console.error(`Failed to load plugin from ${filePath}:`, error);
      }
    }

    return plugins;
  }

  private static isValidPlugin(obj: any): obj is GoapPlugin {
    return (
      obj &&
      typeof obj.name === 'string' &&
      typeof obj.version === 'string' &&
      typeof obj.hooks === 'object'
    );
  }
}

/**
 * Built-in plugins
 */

// Cost tracking plugin
export const costTrackingPlugin: GoapPlugin = {
  name: 'cost-tracker',
  version: '1.0.0',
  description: 'Tracks execution costs and provides cost analytics',
  hooks: {
    onPlanStart: (context: PlanningContext) => {
      (context as any).startTime = Date.now();
      (context as any).costs = [];
    },
    afterExecute: (step: PlanStep, result: any, state: WorldState) => {
      const costs = (state as any).costs || [];
      costs.push({
        action: step.action.name,
        cost: step.estimatedCost,
        timestamp: Date.now()
      });
      (state as any).costs = costs;
    },
    onPlanComplete: (result: PlanExecutionResult) => {
      const totalCost = (result.finalState as any).costs?.reduce(
        (sum: number, item: any) => sum + item.cost, 0
      ) || 0;
      console.log(`Total execution cost: ${totalCost}`);
    }
  }
};

// Performance monitoring plugin
export const performanceMonitoringPlugin: GoapPlugin = {
  name: 'performance-monitor',
  version: '1.0.0',
  description: 'Monitors execution performance and timing',
  hooks: {
    onPlanStart: (context: PlanningContext) => {
      (context as any).performanceMetrics = {
        startTime: Date.now(),
        stepTimes: []
      };
    },
    beforeExecute: (step: PlanStep, state: WorldState) => {
      (state as any).stepStartTime = Date.now();
    },
    afterExecute: (step: PlanStep, result: any, state: WorldState) => {
      const stepTime = Date.now() - (state as any).stepStartTime;
      const metrics = (state as any).performanceMetrics || { stepTimes: [] };
      metrics.stepTimes.push({
        action: step.action.name,
        duration: stepTime,
        success: result.success
      });
      (state as any).performanceMetrics = metrics;
    },
    onPlanComplete: (result: PlanExecutionResult) => {
      const metrics = (result.finalState as any).performanceMetrics;
      if (metrics) {
        const totalTime = Date.now() - metrics.startTime;
        const avgStepTime = metrics.stepTimes.reduce(
          (sum: number, step: any) => sum + step.duration, 0
        ) / Math.max(metrics.stepTimes.length, 1);

        console.log(`Plan execution completed in ${totalTime}ms`);
        console.log(`Average step time: ${avgStepTime.toFixed(2)}ms`);
      }
    }
  }
};

// Logging plugin
export const loggingPlugin: GoapPlugin = {
  name: 'logger',
  version: '1.0.0',
  description: 'Comprehensive logging of plan execution',
  hooks: {
    onPlanStart: (context: PlanningContext) => {
      console.log(`🎯 Starting plan for goal: ${context.goal.name}`);
      console.log(`📊 Available actions: ${context.availableActions.length}`);
    },
    beforeSearch: (context: PlanningContext) => {
      console.log(`🔍 Searching for plan...`);
    },
    afterSearch: (plan: GoapPlan | null, context: PlanningContext) => {
      if (plan) {
        console.log(`✅ Plan found with ${plan.steps.length} steps, cost: ${plan.totalCost}`);
      } else {
        console.log(`❌ No plan found for goal: ${context.goal.name}`);
      }
    },
    beforeExecute: (step: PlanStep, state: WorldState) => {
      console.log(`⚡ Executing: ${step.action.name}`);
    },
    afterExecute: (step: PlanStep, result: any, state: WorldState) => {
      const status = result.success ? '✅' : '❌';
      console.log(`${status} ${step.action.name}: ${result.success ? 'success' : result.error}`);
    },
    onReplan: (failedStep: PlanStep, state: WorldState) => {
      console.log(`🔄 Replanning after failed step: ${failedStep.action.name}`);
    },
    onPlanComplete: (result: PlanExecutionResult) => {
      const status = result.success ? '🎉' : '💥';
      console.log(`${status} Plan ${result.success ? 'completed' : 'failed'} after ${result.executedSteps} steps`);
      if (result.replanned) {
        console.log(`🔄 Plan was replanned ${result.planHistory.length - 1} times`);
      }
    },
    onError: (error: Error, context: any) => {
      console.error(`💥 Plugin system error:`, error.message);
    }
  }
};

// Query diversification plugin (for search enhancement)
export const queryDiversificationPlugin: GoapPlugin = {
  name: 'query-diversifier',
  version: '1.0.0',
  description: 'Diversifies search queries for better coverage',
  hooks: {
    beforeExecute: (step: PlanStep, state: WorldState) => {
      if (step.action.name === 'compose_queries') {
        // Add query variants
        const baseQuery = (step.params as any)?.query || '';
        const variants = [
          `${baseQuery} site:edu`,
          `${baseQuery} site:gov`,
          `${baseQuery} filetype:pdf`,
          `${baseQuery} latest`,
          `"${baseQuery}" research`
        ];

        (step.params as any).queryVariants = variants;
        console.log(`🎲 Added ${variants.length} query variants`);
      }
    }
  }
};