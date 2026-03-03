/**
 * MCP Tools for GOAP Search and Planning Operations
 * Provides the main interface for Claude to interact with the GOAP planner
 */

import dotenv from 'dotenv';

// Load environment variables at the very beginning
dotenv.config();

console.log('[DEBUG] MCP Tools environment check:', {
  hasPerplexityKey: !!process.env.PERPLEXITY_API_KEY,
  perplexityKeyLength: process.env.PERPLEXITY_API_KEY?.length || 0,
  envKeys: Object.keys(process.env).filter(k => k.includes('PERPLEXITY')),
  totalEnvKeys: Object.keys(process.env).length
});

import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { GoapPlanner } from '../goap/planner.js';
import { PluginRegistry } from '../core/plugin-system.js';
import { createPluginRegistry } from '../plugins/plugin-registry.js';
import { AdvancedReasoningEngine } from '../core/advanced-reasoning-engine.js';
import { perplexityActions } from '../actions/perplexity-actions.js';
import { OutputManager } from '../utils/output-manager.js';
import { Ed25519Verifier, AntiHallucinationVerifier } from '../core/ed25519-verifier.js';
import {
  WorldState,
  GoapGoal,
  GoapAction,
  PlanningContext,
  SearchRequest,
  SearchResult
} from '../core/types.js';

export class GoapMCPTools {
  private planner: GoapPlanner;
  private pluginRegistry: PluginRegistry;
  private reasoningEngine: AdvancedReasoningEngine;
  private outputManager: OutputManager;
  private availableActions: GoapAction[];
  private ed25519Verifier: Ed25519Verifier;
  private antiHallucinationVerifier: AntiHallucinationVerifier;

  constructor() {
    this.planner = new GoapPlanner();
    this.pluginRegistry = createPluginRegistry();  // Use the configured registry with all plugins
    this.reasoningEngine = new AdvancedReasoningEngine();
    this.outputManager = new OutputManager();
    this.availableActions = perplexityActions;
    this.ed25519Verifier = new Ed25519Verifier();
    this.antiHallucinationVerifier = new AntiHallucinationVerifier(this.ed25519Verifier);
  }

  async initialize(): Promise<void> {
    await this.pluginRegistry.initialize();
    await this.reasoningEngine.initialize();

    // Plugins are registered through the plugin registry factory

    // Initialize trusted keys for known AI providers (optional)
    // These would be real public keys from OpenAI, Anthropic, etc.
    this.initializeTrustedKeys();
  }

  private initializeTrustedKeys(): void {
    // Register known AI provider public keys
    // In production, these would be fetched from trusted sources
    // Example placeholder keys (not real):
    this.ed25519Verifier.registerTrustedKey('perplexity-ai', 'PERPLEXITY_PUBLIC_KEY_BASE64');
    this.ed25519Verifier.registerTrustedKey('openai', 'OPENAI_PUBLIC_KEY_BASE64');
    this.ed25519Verifier.registerTrustedKey('anthropic', 'ANTHROPIC_PUBLIC_KEY_BASE64');
  }

  /**
   * Main GOAP search tool - plans and executes search with synthesis
   */
  getGoapSearchTool(): Tool {
    return {
      name: 'goap.search',
      description: 'Execute intelligent search using GOAP planning with Perplexity integration and Advanced Reasoning Engine',
      inputSchema: {
        type: 'object',
        properties: {
          query: {
            type: 'string',
            description: 'The search query or research question'
          },
          domains: {
            type: 'array',
            items: { type: 'string' },
            description: 'Optional domain restrictions (e.g., ["edu", "gov"])'
          },
          recency: {
            type: 'string',
            enum: ['hour', 'day', 'week', 'month', 'year'],
            description: 'Recency filter for search results'
          },
          mode: {
            type: 'string',
            enum: ['web', 'academic'],
            description: 'Search mode - web for general search, academic for scholarly sources'
          },
          maxResults: {
            type: 'number',
            description: 'Maximum number of search results to process',
            minimum: 1,
            maximum: 20,
            default: 10
          },
          model: {
            type: 'string',
            description: 'Perplexity model to use for synthesis',
            enum: ['sonar', 'sonar-pro', 'sonar-deep-research'],
            default: 'sonar-pro'
          },
          enableReasoning: {
            type: 'boolean',
            description: 'Enable Advanced Reasoning Engine enhanced reasoning',
            default: true
          },
          planningTimeout: {
            type: 'number',
            description: 'Maximum planning time in seconds',
            default: 30
          },
          outputToFile: {
            type: 'boolean',
            description: 'Save results to file (default: .research/ directory)',
            default: false
          },
          outputFormat: {
            type: 'string',
            enum: ['json', 'markdown', 'both'],
            description: 'Output format when saving to file',
            default: 'markdown'
          },
          outputPath: {
            type: 'string',
            description: 'Custom output directory (default: .research/)',
            default: '.research'
          },
          useQuerySubfolder: {
            type: 'boolean',
            description: 'Create subfolder based on query',
            default: true
          },
          pagination: {
            type: 'object',
            properties: {
              page: { type: 'number', minimum: 1, default: 1 },
              pageSize: { type: 'number', minimum: 5, maximum: 50, default: 10 }
            },
            description: 'Pagination options for large results'
          },
          ed25519Verification: {
            type: 'object',
            properties: {
              enabled: {
                type: 'boolean',
                description: 'Enable Ed25519 signature verification for citations',
                default: false
              },
              requireSignatures: {
                type: 'boolean',
                description: 'Require all citations to be signed (strict mode)',
                default: false
              },
              signResult: {
                type: 'boolean',
                description: 'Sign the search result with Ed25519',
                default: false
              },
              privateKey: {
                type: 'string',
                description: 'Base64 encoded Ed25519 private key for signing (optional)'
              },
              keyId: {
                type: 'string',
                description: 'Key identifier for signing (optional)'
              },
              certId: {
                type: 'string',
                description: 'Certificate ID for mandate certificate chain (optional)'
              },
              trustedIssuers: {
                type: 'array',
                items: { type: 'string' },
                description: 'List of trusted certificate issuers',
                default: ['perplexity-ai', 'openai', 'anthropic']
              }
            },
            description: 'Optional Ed25519 cryptographic verification for anti-hallucination'
          }
        },
        required: ['query']
      }
    };
  }

  /**
   * Execute GOAP search
   */
  async executeGoapSearch(params: SearchRequest): Promise<SearchResult> {
    const startTime = Date.now();

    try {
      // Define initial world state
      const initialState: WorldState = {
        user_query: params.query,
        queries_composed: false,
        information_searched: false,
        results_synthesized: false,
        answer_verified: false
      };

      // Define goal
      const goal: GoapGoal = {
        name: 'complete_research',
        conditions: [
          { key: 'answer_verified', value: true, operator: 'equals' }
        ],
        priority: 1,
        timeout: (params.planningTimeout || 30) * 1000
      };

      // Create planning context
      const context: PlanningContext = {
        currentState: initialState,
        goal,
        availableActions: this.availableActions,
        maxDepth: 10,
        maxCost: 50
      };

      // Execute planning hooks
      await this.pluginRegistry.executeOnPlanStart(context);

      // Enhanced reasoning if enabled
      let reasoningInsights;
      if (params.enableReasoning) {
        reasoningInsights = await this.reasoningEngine.analyze(initialState, goal);
      }

      // Create plan
      await this.pluginRegistry.executeBeforeSearch(context);
      let plan = await this.planner.createPlan(context);
      await this.pluginRegistry.executeAfterSearch(plan, context);

      if (!plan) {
        throw new Error('No viable plan found for the given query');
      }

      // Enhance plan with Strange Loop reasoning
      if (params.enableReasoning) {
        plan = await this.reasoningEngine.enhance(plan);
      }

      // Execute plan with dynamic re-planning
      const executionParams = {
        domains: params.domains,
        recency: params.recency,
        mode: params.mode,
        maxResults: params.maxResults,
        model: params.model
      };

      const result = await this.planner.executePlan(
        plan,
        this.availableActions,
        (newPlan) => {
          console.log(`🔄 Replanned: ${newPlan.id}`);
        }
      );

      // Execute completion hooks
      await this.pluginRegistry.executeOnPlanComplete(result);

      if (!result.success) {
        throw new Error(result.error || 'Plan execution failed');
      }

      // Extract results with size limits for deep research
      const isDeepModel = params.model === 'sonar-deep-research';
      const maxAnswerLength = isDeepModel ? 5000 : 50000;  // Limit answer size for deep model

      let answer = result.finalState.final_answer as string || 'No answer generated';
      if (answer.length > maxAnswerLength) {
        answer = answer.substring(0, maxAnswerLength) + '\n\n[Answer truncated for size. Full answer available in saved files.]';
      }

      const citations = result.finalState.citations as any[] || [];
      const usage = result.finalState.usage as any || { tokens: 0, cost: 0 };
      const verificationNotes = result.finalState.verification_notes as string[] || [];

      // Apply Ed25519 verification if enabled
      let ed25519Result = undefined;
      if (params.ed25519Verification?.enabled) {
        const { requireSignatures, signResult, privateKey, keyId, certId, trustedIssuers } = params.ed25519Verification;

        // Verify citations if required
        if (citations.length > 0) {
          const citationVerification = await this.antiHallucinationVerifier.verifyCitations(
            citations
          );

          console.log(`🔐 Ed25519 Citation Verification: ${citationVerification.verified}/${citationVerification.total} verified`);

          if (citationVerification.untrusted.length > 0) {
            console.log(`⚠️ Untrusted sources: ${citationVerification.untrusted.join(', ')}`);
          }

          ed25519Result = citationVerification;
        }

        // Sign the result if requested
        if (signResult && privateKey && keyId) {
          const signedContent = await this.antiHallucinationVerifier.signSearchResult(
            { answer, citations, metadata: { planId: plan.id } }
          );

          console.log(`✅ Result signed with Ed25519 (Key: ${keyId})`);

          // Add signature to metadata
          result.finalState.ed25519Signature = signedContent.signature;
          result.finalState.ed25519KeyId = keyId;
        }
      }

      // Generate plan log
      const planLog = this.generatePlanLog(result, reasoningInsights);

      // Create full result object
      let fullResult: SearchResult = {
        answer,
        citations,
        planLog,
        usage,
        reasoning: reasoningInsights,
        metadata: {
          planId: plan.id,
          executionTime: Date.now() - startTime,
          replanned: result.replanned || false,
          ...(ed25519Result && { ed25519Verification: ed25519Result }),
          ...(result.finalState.ed25519Signature && {
            ed25519Signature: result.finalState.ed25519Signature,
            ed25519KeyId: result.finalState.ed25519KeyId
          })
        }
      };

      // Apply pagination by default to avoid token limits
      // Use provided pagination, or default based on model type
      const effectivePagination = params.pagination || {
        page: 1,
        pageSize: isDeepModel ? 2 : 5  // Smaller pages for deep research, moderate for regular
      };

      // Always apply pagination to prevent token limit errors
      if (effectivePagination) {
        const paginated = this.outputManager.paginateResults(fullResult, effectivePagination);
        fullResult = {
          ...paginated.data,
          paginationInfo: paginated.pagination
        } as SearchResult;
      }

      // Save to file if requested
      if (params.outputToFile) {
        const savedFiles = await this.outputManager.saveToFile(
          fullResult,
          params.query,
          params.outputFormat || 'markdown',
          {
            outputPath: params.outputPath,
            useQuerySubfolder: params.useQuerySubfolder
          }
        );

        // Add saved files to metadata
        fullResult.metadata = {
          ...fullResult.metadata,
          savedFiles
        };

        console.log(`📁 Results saved to: ${savedFiles.join(', ')}`);
      }

      return fullResult;

    } catch (error) {
      await this.pluginRegistry.executeOnError(
        error instanceof Error ? error : new Error('Unknown error'),
        { params, startTime }
      );

      throw error;
    }
  }

  /**
   * Plan explanation tool
   */
  getPlanExplainTool(): Tool {
    return {
      name: 'goap.plan.explain',
      description: 'Explain how GOAP planning works for a given query without executing',
      inputSchema: {
        type: 'object',
        properties: {
          query: {
            type: 'string',
            description: 'The query to create a plan for'
          },
          showSteps: {
            type: 'boolean',
            description: 'Include detailed step-by-step breakdown',
            default: true
          },
          showReasoning: {
            type: 'boolean',
            description: 'Include Strange Loop reasoning analysis',
            default: true
          }
        },
        required: ['query']
      }
    };
  }

  /**
   * Execute plan explanation
   */
  async executePlanExplain(params: any): Promise<any> {
    const initialState: WorldState = {
      user_query: params.query,
      queries_composed: false,
      information_searched: false,
      results_synthesized: false,
      answer_verified: false
    };

    const goal: GoapGoal = {
      name: 'complete_research',
      conditions: [
        { key: 'answer_verified', value: true, operator: 'equals' }
      ],
      priority: 1
    };

    const context: PlanningContext = {
      currentState: initialState,
      goal,
      availableActions: this.availableActions
    };

    // Create plan (don't execute)
    const plan = await this.planner.createPlan(context);

    if (!plan) {
      return {
        explanation: 'No viable plan could be created for this query.',
        reason: 'The goal conditions cannot be satisfied with available actions.'
      };
    }

    let reasoning;
    if (params.showReasoning) {
      reasoning = await this.reasoningEngine.analyze(initialState, goal);
    }

    const explanation = {
      query: params.query,
      planId: plan.id,
      totalCost: plan.totalCost,
      stepCount: plan.steps.length,
      reasoning,
      workflow: this.explainWorkflow(),
      steps: params.showSteps ? plan.steps.map(step => ({
        action: step.action.name,
        cost: step.estimatedCost,
        preconditions: step.action.preconditions,
        effects: step.action.effects,
        description: this.getActionDescription(step.action.name)
      })) : undefined
    };

    return explanation;
  }

  /**
   * Raw search tool (bypass GOAP planning)
   */
  getRawSearchTool(): Tool {
    return {
      name: 'search.raw',
      description: 'Direct Perplexity search without GOAP planning - for simple queries',
      inputSchema: {
        type: 'object',
        properties: {
          query: {
            type: 'array',
            items: { type: 'string' },
            description: 'Search queries (can be multiple)'
          },
          mode: {
            type: 'string',
            enum: ['web', 'academic'],
            default: 'web'
          },
          recency: {
            type: 'string',
            enum: ['hour', 'day', 'week', 'month', 'year']
          },
          domains: {
            type: 'array',
            items: { type: 'string' }
          },
          maxResults: {
            type: 'number',
            minimum: 1,
            maximum: 20,
            default: 10
          }
        },
        required: ['query']
      }
    };
  }

  /**
   * Execute raw search
   */
  async executeRawSearch(params: any): Promise<any> {
    console.log('[DEBUG] executeRawSearch called with params:', {
      queryType: Array.isArray(params.query) ? 'array' : typeof params.query,
      queryLength: Array.isArray(params.query) ? params.query.length : 1,
      mode: params.mode,
      hasApiKey: !!process.env.PERPLEXITY_API_KEY
    });

    const searchAction = this.availableActions.find(a => a.name === 'search_information');
    console.log('[DEBUG] Search action found:', !!searchAction);
    if (!searchAction) {
      throw new Error('Search action not available');
    }

    const state: WorldState = {
      queries_composed: true,
      search_queries: Array.isArray(params.query) ? params.query : [params.query]
    };

    console.log('[DEBUG] Calling searchAction.execute with state:', state);
    const result = await searchAction.execute(state, {
      mode: params.mode,
      recency: params.recency,
      domains: params.domains,
      maxResults: params.maxResults
    });
    console.log('[DEBUG] searchAction.execute completed, result keys:', Object.keys(result));

    return result;
  }

  /**
   * Generate comprehensive plan execution log
   */
  private generatePlanLog(result: any, reasoning?: any): string[] {
    const log: string[] = [];

    log.push('🎯 GOAP Planning & Execution Log');
    log.push('================================');

    if (reasoning) {
      log.push('🧠 Strange Loop Reasoning:');
      reasoning.insights.forEach((insight: string) => {
        log.push(`  • ${insight}`);
      });
      log.push(`  • Confidence: ${(reasoning.confidence * 100).toFixed(1)}%`);
      log.push('');
    }

    log.push(`📋 Plan Execution Summary:`);
    log.push(`  • Steps executed: ${result.executedSteps}`);
    log.push(`  • Success: ${result.success ? 'Yes' : 'No'}`);
    log.push(`  • Replanned: ${result.replanned ? 'Yes' : 'No'}`);

    if (result.planHistory.length > 1) {
      log.push(`  • Plan iterations: ${result.planHistory.length}`);
    }

    if (result.error) {
      log.push(`  • Error: ${result.error}`);
    }

    return log;
  }

  /**
   * Explain the GOAP workflow
   */
  private explainWorkflow(): any {
    return {
      description: 'GOAP (Goal-Oriented Action Planning) Workflow',
      phases: [
        {
          name: 'Goal Definition',
          description: 'Define the target state (verified research answer)'
        },
        {
          name: 'State Analysis',
          description: 'Analyze current world state and required conditions'
        },
        {
          name: 'A* Planning',
          description: 'Use A* pathfinding to find optimal action sequence'
        },
        {
          name: 'Plan Enhancement',
          description: 'Enhance plan using Strange Loop reasoning (optional)'
        },
        {
          name: 'Execution',
          description: 'Execute actions with precondition validation'
        },
        {
          name: 'Dynamic Replanning',
          description: 'Replan automatically if actions fail'
        },
        {
          name: 'Verification',
          description: 'Verify final answer quality and citations'
        }
      ],
      advantages: [
        'Optimal path finding with A* algorithm',
        'Dynamic replanning on failure',
        'Enhanced reasoning with Strange Loop WASM',
        'Extensible plugin system',
        'Comprehensive verification',
        'Cost optimization',
        'Multi-step complex planning'
      ]
    };
  }

  /**
   * Get human-readable action descriptions
   */
  private getActionDescription(actionName: string): string {
    const descriptions: { [key: string]: string } = {
      'compose_queries': 'Break down user query into optimized search queries with variants',
      'search_information': 'Execute parallel searches using Perplexity Search API',
      'synthesize_results': 'Use Perplexity Sonar to create comprehensive answer with citations',
      'verify_answer': 'Validate answer quality, citation coverage, and source diversity'
    };

    return descriptions[actionName] || 'Unknown action';
  }

  /**
   * Get all plugin management tools
   */
  getPluginTools(): Tool[] {
    return [
      {
        name: 'plugin.list',
        description: 'List all available plugins and their status',
        inputSchema: { type: 'object', properties: {} }
      },
      {
        name: 'plugin.enable',
        description: 'Enable a specific plugin by name',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Plugin name to enable' }
          },
          required: ['name']
        }
      },
      {
        name: 'plugin.disable',
        description: 'Disable a specific plugin by name',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Plugin name to disable' }
          },
          required: ['name']
        }
      },
      {
        name: 'plugin.info',
        description: 'Get detailed information about a specific plugin',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Plugin name' }
          },
          required: ['name']
        }
      }
    ];
  }

  /**
   * Get advanced reasoning plugin tools
   */
  getAdvancedReasoningTools(): Tool[] {
    return [
      {
        name: 'reasoning.chain_of_thought',
        description: 'Apply Chain-of-Thought reasoning with Tree-of-Thoughts exploration',
        inputSchema: {
          type: 'object',
          properties: {
            query: { type: 'string', description: 'Query to reason about' },
            depth: { type: 'number', minimum: 1, maximum: 5, default: 3, description: 'Reasoning depth' },
            branches: { type: 'number', minimum: 2, maximum: 10, default: 3, description: 'Number of reasoning branches' }
          },
          required: ['query']
        }
      },
      {
        name: 'reasoning.self_consistency',
        description: 'Check reasoning consistency with majority voting',
        inputSchema: {
          type: 'object',
          properties: {
            query: { type: 'string', description: 'Query to validate' },
            samples: { type: 'number', minimum: 3, maximum: 10, default: 5, description: 'Number of samples for consistency check' }
          },
          required: ['query']
        }
      },
      {
        name: 'reasoning.anti_hallucination',
        description: 'Verify claims with citation grounding',
        inputSchema: {
          type: 'object',
          properties: {
            claims: { type: 'array', items: { type: 'string' }, description: 'Claims to verify' },
            citations: { type: 'array', items: { type: 'string' }, description: 'Available citations for grounding' }
          },
          required: ['claims']
        }
      },
      {
        name: 'reasoning.agentic_research',
        description: 'Orchestrate multiple research agents for comprehensive analysis',
        inputSchema: {
          type: 'object',
          properties: {
            query: { type: 'string', description: 'Research question' },
            agents: {
              type: 'array',
              items: { type: 'string' },
              default: ['researcher', 'fact_checker', 'synthesizer', 'critic', 'summarizer'],
              description: 'Agent types to spawn'
            },
            parallel: { type: 'boolean', default: true, description: 'Execute agents in parallel' }
          },
          required: ['query']
        }
      }
    ];
  }

  /**
   * Get all available tools
   */
  getTools(): Tool[] {
    return [
      // Core GOAP tools
      this.getGoapSearchTool(),
      this.getPlanExplainTool(),
      this.getRawSearchTool(),

      // Plugin management tools
      ...this.getPluginTools(),

      // Advanced reasoning tools
      ...this.getAdvancedReasoningTools()
    ];
  }

  /**
   * Execute a tool by name
   */
  async executeToolByName(toolName: string, params: any): Promise<any> {
    console.log(`[DEBUG] Executing tool: ${toolName}`, {
      params: Object.keys(params),
      hasApiKey: !!process.env.PERPLEXITY_API_KEY,
      apiKeyLength: process.env.PERPLEXITY_API_KEY?.length || 0
    });

    switch (toolName) {
      // Core tools
      case 'goap.search':
        console.log('[DEBUG] Entering goap.search execution');
        return this.executeGoapSearch(params);
      case 'goap.plan.explain':
        console.log('[DEBUG] Entering goap.plan.explain execution');
        return this.executePlanExplain(params);
      case 'search.raw':
        console.log('[DEBUG] Entering search.raw execution');
        return this.executeRawSearch(params);

      // Plugin management
      case 'plugin.list':
        return this.pluginRegistry.listPlugins();
      case 'plugin.enable':
        return this.pluginRegistry.enablePlugin(params.name);
      case 'plugin.disable':
        return this.pluginRegistry.disablePlugin(params.name);
      case 'plugin.info':
        return this.pluginRegistry.getPluginInfo(params.name);

      // Advanced reasoning tools
      case 'reasoning.chain_of_thought': {
        console.log('[DEBUG] Entering reasoning.chain_of_thought execution');
        const chainOfThought = this.pluginRegistry.getPlugin('chain-of-thought');
        console.log('[DEBUG] Chain-of-thought plugin found:', !!chainOfThought, 'has execute:', !!chainOfThought?.execute);
        if (chainOfThought && chainOfThought.execute) {
          console.log('[DEBUG] Calling chain-of-thought execute method');
          return chainOfThought.execute(params);
        }
        throw new Error('Chain-of-Thought plugin not found or does not support execute');
      }
      case 'reasoning.self_consistency': {
        const selfConsistency = this.pluginRegistry.getPlugin('self-consistency');
        if (selfConsistency && selfConsistency.execute) {
          return selfConsistency.execute(params);
        }
        throw new Error('Self-Consistency plugin not found or does not support execute');
      }
      case 'reasoning.anti_hallucination': {
        const antiHallucination = this.pluginRegistry.getPlugin('anti-hallucination');
        if (antiHallucination && antiHallucination.execute) {
          return antiHallucination.execute(params);
        }
        throw new Error('Anti-Hallucination plugin not found or does not support execute');
      }
      case 'reasoning.agentic_research': {
        const agenticResearch = this.pluginRegistry.getPlugin('agentic-research-flow');
        if (agenticResearch && agenticResearch.execute) {
          return agenticResearch.execute(params);
        }
        throw new Error('Agentic Research plugin not found or does not support execute');
      }

      default:
        throw new Error(`Unknown tool: ${toolName}`);
    }
  }
}