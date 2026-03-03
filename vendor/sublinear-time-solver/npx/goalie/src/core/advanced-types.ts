/**
 * Advanced Types for Extended Plugin System
 * Provides additional context and hooks for advanced reasoning plugins
 */

import { GoapPlugin, PluginHooks, PlanningContext } from './types.js';

/**
 * Extended plugin context for advanced reasoning
 */
export interface PluginContext extends Partial<PlanningContext> {
  // Query information
  query?: string;
  searchResults?: any;

  // Metadata storage
  metadata?: Record<string, any>;

  // Search parameters
  searchParams?: {
    return_citations?: boolean;
    citation_quality?: string;
    domains?: string[];
    mode?: string;
    [key: string]: any;
  };

  // Synthesis parameters
  synthesisParams?: {
    instruction?: string;
    requireCitations?: boolean;
    uncertaintyThreshold?: number;
    [key: string]: any;
  };

  // Control flags
  requiresAdditionalVerification?: boolean;
  skipSearch?: boolean;
  cachedResult?: any;
}

/**
 * Extended plugin hooks for advanced reasoning
 */
export interface AdvancedPluginHooks extends PluginHooks {
  // Advanced reasoning hooks
  beforeSynthesize?: (context: PluginContext) => Promise<void> | void;
  afterSynthesize?: (result: any, context: PluginContext) => Promise<any> | any;
  verify?: (result: any, context: PluginContext) => Promise<VerificationResult> | VerificationResult;
}

/**
 * Verification result from plugins
 */
export interface VerificationResult {
  valid: boolean;
  confidence: number;
  method: string;
  details?: any;
}

/**
 * Advanced reasoning plugin interface
 */
export interface AdvancedGoapPlugin extends GoapPlugin {
  hooks: AdvancedPluginHooks;
}

/**
 * Adapter to convert advanced plugins to standard GOAP plugins
 */
export class AdvancedPluginAdapter implements GoapPlugin {
  name: string;
  version: string;
  description?: string;
  hooks: PluginHooks;
  execute?: (params: any) => Promise<any>;

  constructor(private advancedPlugin: any) {
    this.name = advancedPlugin.name;
    this.version = advancedPlugin.version;
    this.description = advancedPlugin.description;

    // Adapt hooks to standard interface
    this.hooks = this.createCompatibleHooks(advancedPlugin.hooks);

    // Add execute method that calls the appropriate hook
    this.execute = async (params: any) => {
      // First check if the plugin itself has an execute method
      if (this.advancedPlugin.execute) {
        return this.advancedPlugin.execute(params);
      }

      // Then check if the hooks have an execute method
      if (this.advancedPlugin.hooks?.execute) {
        return this.advancedPlugin.hooks.execute(params);
      }

      // Fallback to processing through hooks
      const context: PluginContext = {
        query: params.query || '',
        metadata: {},
        searchParams: params
      };

      if (this.advancedPlugin.hooks?.processReasoning) {
        return this.advancedPlugin.hooks.processReasoning(context);
      }

      // Default response
      return {
        success: true,
        plugin: this.name,
        params,
        message: `Plugin ${this.name} executed successfully`,
        result: `Processed query: ${params.query || 'N/A'}`
      };
    };
  }

  private createCompatibleHooks(advancedHooks: any): PluginHooks {
    const hooks: PluginHooks = {};

    // Map advanced hooks to standard hooks where possible
    if (advancedHooks.beforeSearch) {
      hooks.beforeSearch = async (context: PlanningContext) => {
        // Create extended context
        const extendedContext: PluginContext = {
          ...context,
          query: (context as any).query,
          metadata: {},
          searchParams: {}
        };

        await advancedHooks.beforeSearch(extendedContext);

        // Copy back any modifications
        Object.assign(context, extendedContext);
      };
    }

    if (advancedHooks.afterSearch) {
      hooks.afterSearch = async (plan: any, context: PlanningContext) => {
        // Create extended context
        const extendedContext: PluginContext = {
          ...context,
          query: (context as any).query,
          searchResults: plan
        };

        const result = await advancedHooks.afterSearch(plan, extendedContext);

        // Store verification results if any
        if (advancedHooks.verify) {
          (context as any).verificationPending = true;
        }

        return result;
      };
    }

    // Map synthesis hooks to plan execution hooks
    if (advancedHooks.beforeSynthesize) {
      hooks.beforeExecute = async (step: any, state: any) => {
        const extendedContext: PluginContext = {
          query: (step as any).query,
          metadata: (step as any).metadata || {},
          synthesisParams: {}
        };

        await advancedHooks.beforeSynthesize(extendedContext);
      };
    }

    if (advancedHooks.afterSynthesize) {
      hooks.afterExecute = async (step: any, result: any, state: any) => {
        const extendedContext: PluginContext = {
          query: (step as any).query,
          metadata: (step as any).metadata || {}
        };

        return await advancedHooks.afterSynthesize(result, extendedContext);
      };
    }

    // Add verification as error handler
    if (advancedHooks.verify) {
      hooks.onPlanComplete = async (result: any) => {
        const extendedContext: PluginContext = {
          metadata: (result as any).metadata || {}
        };

        const verification = await advancedHooks.verify(result, extendedContext);

        if (!verification.valid) {
          console.log(`⚠️ [${this.name}] Verification failed: ${verification.method} (${(verification.confidence * 100).toFixed(1)}% confidence)`);
        }

        (result as any).verification = verification;
      };
    }

    return hooks;
  }

  async initialize?(): Promise<void> {
    if (this.advancedPlugin.initialize) {
      await this.advancedPlugin.initialize();
    }
  }

  async cleanup?(): Promise<void> {
    if (this.advancedPlugin.cleanup) {
      await this.advancedPlugin.cleanup();
    }
  }
}

