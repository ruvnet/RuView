import {
  CallToolRequestSchema,
  CallToolResult,
  Tool
} from '@modelcontextprotocol/sdk/types
import { z } from 'zod';
import { GraphReasonerWrapper } from '../wrappers/graph-reasoner';
import { TextExtractorWrapper } from '../wrappers/text-extractor';
import { PlannerWrapper } from '../wrappers/planner';
import { WasmMemoryManager } from '../wasm/memory-manager';
import {
  validateInput,
  schemas,
  QueryType,
  FactType,
  RuleType,
  SentimentAnalysisRequestType,
  PreferenceExtractionRequestType,
  EmotionDetectionRequestType,
  TextAnalysisRequestType,
  PlanningRequestType,
  StateUpdateType
} from '../schemas/index';
import {
  PsychoSymbolicError,
  WasmExecutionError,
  InvalidInputError
} from '../types/index';

export class PsychoSymbolicMcpTools {
  private graphReasoner: GraphReasonerWrapper;
  private textExtractor: TextExtractorWrapper;
  private planner: PlannerWrapper;
  private memoryManager: WasmMemoryManager;
  private initialized = false;
  private defaultInstanceIds = {
    graphReasoner: 'default-graph-reasoner',
    textExtractor: 'default-text-extractor',
    planner: 'default-planner'
  };

  constructor() {
    this.graphReasoner = new GraphReasonerWrapper();
    this.textExtractor = new TextExtractorWrapper();
    this.planner = new PlannerWrapper();
    this.memoryManager = WasmMemoryManager.getInstance();
  }

  /**
   * Initialize all WASM modules
   */
  public async initialize(config: {
    graphReasonerWasmPath: string;
    textExtractorWasmPath: string;
    plannerWasmPath: string;
  }): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      // Initialize all modules in parallel
      await Promise.all([
        this.graphReasoner.initialize(config.graphReasonerWasmPath),
        this.textExtractor.initialize(config.textExtractorWasmPath),
        this.planner.initialize(config.plannerWasmPath)
      ]);

      // Create default instances
      this.graphReasoner.createInstance(this.defaultInstanceIds.graphReasoner);
      this.textExtractor.createInstance(this.defaultInstanceIds.textExtractor);
      this.planner.createInstance(this.defaultInstanceIds.planner);

      this.initialized = true;
    } catch (error) {
      throw new WasmExecutionError('MCP tools initialization', {
        config,
        originalError: error
      });
    }
  }

  /**
   * Get all available MCP tools
   */
  public getTools(): Tool[] {
    return [
      // Graph Reasoner Tools
      {
        name: 'queryGraph',
        description: 'Query the knowledge graph using symbolic reasoning',
        inputSchema: {
          type: 'object',
          properties: {
            query: {
              type: 'object',
              properties: {
                pattern: {
                  type: 'object',
                  properties: {
                    subject: { type: 'string' },
                    predicate: { type: 'string' },
                    object: { type: 'string' },
                    variables: {
                      type: 'array',
                      items: { type: 'string' }
                    }
                  }
                },
                constraints: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      field: { type: 'string' },
                      operator: {
                        type: 'string',
                        enum: ['eq', 'ne', 'gt', 'lt', 'gte', 'lte', 'contains', 'regex']
                      },
                      value: {}
                    },
                    required: ['field', 'operator', 'value']
                  }
                },
                options: {
                  type: 'object',
                  properties: {
                    limit: { type: 'number', minimum: 1 },
                    offset: { type: 'number', minimum: 0 },
                    order_by: { type: 'string' },
                    include_inferred: { type: 'boolean' }
                  }
                }
              },
              required: ['pattern']
            },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                format: {
                  type: 'string',
                  enum: ['json', 'turtle', 'csv']
                },
                includeMetadata: { type: 'boolean' }
              }
            }
          },
          required: ['query']
        }
      },
      {
        name: 'addFact',
        description: 'Add a fact to the knowledge graph',
        inputSchema: {
          type: 'object',
          properties: {
            fact: {
              type: 'object',
              properties: {
                subject: { type: 'string', minLength: 1 },
                predicate: { type: 'string', minLength: 1 },
                object: { type: 'string', minLength: 1 },
                confidence: { type: 'number', minimum: 0, maximum: 1 },
                timestamp: { type: 'string' }
              },
              required: ['subject', 'predicate', 'object']
            },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                validate: { type: 'boolean' },
                autoInfer: { type: 'boolean' }
              }
            }
          },
          required: ['fact']
        }
      },
      {
        name: 'addRule',
        description: 'Add an inference rule to the knowledge graph',
        inputSchema: {
          type: 'object',
          properties: {
            rule: {
              type: 'object',
              properties: {
                id: { type: 'string', minLength: 1 },
                name: { type: 'string', minLength: 1 },
                description: { type: 'string' },
                conditions: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      type: {
                        type: 'string',
                        enum: ['fact', 'state', 'function']
                      },
                      pattern: {},
                      negated: { type: 'boolean' }
                    },
                    required: ['type', 'pattern']
                  },
                  minItems: 1
                },
                conclusions: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      type: {
                        type: 'string',
                        enum: ['fact', 'action', 'state_change']
                      },
                      content: {},
                      confidence: { type: 'number', minimum: 0, maximum: 1 }
                    },
                    required: ['type', 'content']
                  },
                  minItems: 1
                },
                confidence: { type: 'number', minimum: 0, maximum: 1 },
                priority: { type: 'number', minimum: 0 }
              },
              required: ['id', 'name', 'conditions', 'conclusions']
            },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                validate: { type: 'boolean' },
                replaceExisting: { type: 'boolean' }
              }
            }
          },
          required: ['rule']
        }
      },
      {
        name: 'runInference',
        description: 'Run inference on the knowledge graph to derive new facts',
        inputSchema: {
          type: 'object',
          properties: {
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                maxIterations: { type: 'number', minimum: 1, maximum: 100 },
                confidenceThreshold: { type: 'number', minimum: 0, maximum: 1 },
                maxNewFacts: { type: 'number', minimum: 1 },
                timeoutMs: { type: 'number', minimum: 1000 }
              }
            }
          }
        }
      },
      
      // Text Extractor Tools
      {
        name: 'extractAffect',
        description: 'Extract sentiment and emotional affect from text',
        inputSchema: {
          type: 'object',
          properties: {
            text: { type: 'string', minLength: 1 },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                includeAspects: { type: 'boolean' },
                language: { type: 'string' },
                confidenceThreshold: { type: 'number', minimum: 0, maximum: 1 }
              }
            }
          },
          required: ['text']
        }
      },
      {
        name: 'extractPreferences',
        description: 'Extract user preferences and patterns from text',
        inputSchema: {
          type: 'object',
          properties: {
            text: { type: 'string', minLength: 1 },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                categories: {
                  type: 'array',
                  items: { type: 'string' }
                },
                minConfidence: { type: 'number', minimum: 0, maximum: 1 },
                maxPreferences: { type: 'number', minimum: 1 }
              }
            }
          },
          required: ['text']
        }
      },
      {
        name: 'extractEmotions',
        description: 'Detect and analyze emotions in text',
        inputSchema: {
          type: 'object',
          properties: {
            text: { type: 'string', minLength: 1 },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                emotionModel: {
                  type: 'string',
                  enum: ['plutchik', 'ekman', 'custom']
                },
                intensityThreshold: { type: 'number', minimum: 0, maximum: 1 },
                includeSecondary: { type: 'boolean' }
              }
            }
          },
          required: ['text']
        }
      },
      {
        name: 'analyzeText',
        description: 'Comprehensive text analysis including sentiment, preferences, and emotions',
        inputSchema: {
          type: 'object',
          properties: {
            text: { type: 'string', minLength: 1 },
            instanceId: { type: 'string' },
            includeSentiment: { type: 'boolean' },
            includePreferences: { type: 'boolean' },
            includeEmotions: { type: 'boolean' },
            options: {
              type: 'object',
              properties: {
                sentiment: {},
                preferences: {},
                emotions: {}
              }
            }
          },
          required: ['text']
        }
      },
      
      // Planner Tools
      {
        name: 'planAction',
        description: 'Create an action plan using GOAP (Goal-Oriented Action Planning)',
        inputSchema: {
          type: 'object',
          properties: {
            goalId: { type: 'string' },
            targetState: {
              type: 'object',
              properties: {
                states: {
                  type: 'object',
                  additionalProperties: {}
                },
                timestamp: { type: 'string' }
              }
            },
            instanceId: { type: 'string' },
            options: {
              type: 'object',
              properties: {
                maxDepth: { type: 'number', minimum: 1 },
                timeoutMs: { type: 'number', minimum: 1000 },
                heuristic: {
                  type: 'string',
                  enum: ['astar', 'dijkstra', 'greedy']
                },
                allowPartialPlans: { type: 'boolean' }
              }
            }
          }
        }
      },
      {
        name: 'addAction',
        description: 'Add an action to the planner\'s action library',
        inputSchema: {
          type: 'object',
          properties: {
            action: {
              type: 'object',
              properties: {
                id: { type: 'string', minLength: 1 },
                name: { type: 'string', minLength: 1 },
                description: { type: 'string' },
                preconditions: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      state_key: { type: 'string', minLength: 1 },
                      required_value: {},
                      operator: {
                        type: 'string',
                        enum: ['eq', 'ne', 'gt', 'lt', 'gte', 'lte']
                      }
                    },
                    required: ['state_key', 'required_value']
                  }
                },
                effects: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      state_key: { type: 'string', minLength: 1 },
                      value: {},
                      probability: { type: 'number', minimum: 0, maximum: 1 }
                    },
                    required: ['state_key', 'value']
                  },
                  minItems: 1
                },
                cost: {
                  type: 'object',
                  properties: {
                    base_cost: { type: 'number', minimum: 0 },
                    variable_costs: {
                      type: 'object',
                      additionalProperties: { type: 'number' }
                    }
                  },
                  required: ['base_cost']
                }
              },
              required: ['id', 'name', 'effects', 'cost']
            },
            instanceId: { type: 'string' }
          },
          required: ['action']
        }
      },
      {
        name: 'addGoal',
        description: 'Add a goal to the planner',
        inputSchema: {
          type: 'object',
          properties: {
            goal: {
              type: 'object',
              properties: {
                id: { type: 'string', minLength: 1 },
                name: { type: 'string', minLength: 1 },
                description: { type: 'string' },
                conditions: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      state_key: { type: 'string', minLength: 1 },
                      target_value: {},
                      operator: {
                        type: 'string',
                        enum: ['eq', 'ne', 'gt', 'lt', 'gte', 'lte']
                      },
                      weight: { type: 'number', minimum: 0 }
                    },
                    required: ['state_key', 'target_value']
                  },
                  minItems: 1
                },
                priority: {
                  type: 'string',
                  enum: ['low', 'medium', 'high', 'critical']
                },
                deadline: { type: 'string' }
              },
              required: ['id', 'name', 'conditions']
            },
            instanceId: { type: 'string' }
          },
          required: ['goal']
        }
      },
      {
        name: 'setState',
        description: 'Update the world state in the planner',
        inputSchema: {
          type: 'object',
          properties: {
            key: { type: 'string', minLength: 1 },
            value: {},
            instanceId: { type: 'string' }
          },
          required: ['key', 'value']
        }
      },
      {
        name: 'getState',
        description: 'Get the current world state from the planner',
        inputSchema: {
          type: 'object',
          properties: {
            key: { type: 'string' },
            instanceId: { type: 'string' }
          }
        }
      },
      
      // Utility Tools
      {
        name: 'getMemoryStats',
        description: 'Get memory usage statistics for all WASM instances',
        inputSchema: {
          type: 'object',
          properties: {}
        }
      },
      {
        name: 'createInstance',
        description: 'Create a new WASM instance of a specific type',
        inputSchema: {
          type: 'object',
          properties: {
            type: {
              type: 'string',
              enum: ['graph_reasoner', 'text_extractor', 'planner']
            },
            instanceId: { type: 'string' }
          },
          required: ['type']
        }
      },
      {
        name: 'removeInstance',
        description: 'Remove a WASM instance and free its memory',
        inputSchema: {
          type: 'object',
          properties: {
            instanceId: { type: 'string', minLength: 1 }
          },
          required: ['instanceId']
        }
      }
    ];
  }

  /**
   * Handle MCP tool calls
   */
  public async callTool(request: {
    name: string;
    arguments?: any;
  }): Promise<CallToolResult> {
    if (!this.initialized) {
      return {
        content: [{
          type: 'text',
          text: JSON.stringify({
            error: 'Tools not initialized. Call initialize() first.',
            code: 'NOT_INITIALIZED'
          })
        }]
      };
    }

    try {
      const result = await this.executeToolCall(request.name, request.arguments || {});
      
      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    } catch (error) {
      const errorResult = {
        error: (error as Error).message,
        code: error instanceof PsychoSymbolicError ? error.code : 'UNKNOWN_ERROR',
        details: error instanceof PsychoSymbolicError ? error.details : undefined
      };
      
      return {
        content: [{
          type: 'text',
          text: JSON.stringify(errorResult, null, 2)
        }],
        isError: true
      };
    }
  }

  /**
   * Execute specific tool calls
   */
  private async executeToolCall(toolName: string, args: any): Promise<any> {
    const instanceId = args.instanceId || this.getDefaultInstanceId(toolName);
    
    switch (toolName) {
      // Graph Reasoner Tools
      case 'queryGraph':
        return this.graphReasoner.query(
          instanceId,
          args.query,
          args.options
        );
        
      case 'addFact':
        const factId = this.graphReasoner.addFact(
          instanceId,
          args.fact,
          args.options
        );
        return { success: true, factId };
        
      case 'addRule':
        const ruleSuccess = this.graphReasoner.addRule(
          instanceId,
          args.rule,
          args.options
        );
        return { success: ruleSuccess };
        
      case 'runInference':
        return this.graphReasoner.runInference(
          instanceId,
          args.options
        );
        
      // Text Extractor Tools
      case 'extractAffect':
        return this.textExtractor.analyzeSentiment(
          instanceId,
          {
            text: args.text,
            options: args.options || {}
          }
        );
        
      case 'extractPreferences':
        return this.textExtractor.extractPreferences(
          instanceId,
          {
            text: args.text,
            options: args.options || {}
          }
        );
        
      case 'extractEmotions':
        return this.textExtractor.detectEmotions(
          instanceId,
          {
            text: args.text,
            options: args.options || {}
          }
        );
        
      case 'analyzeText':
        return this.textExtractor.analyzeAll(
          instanceId,
          {
            text: args.text,
            include_sentiment: args.includeSentiment !== false,
            include_preferences: args.includePreferences !== false,
            include_emotions: args.includeEmotions !== false,
            options: args.options || {}
          }
        );
        
      // Planner Tools
      case 'planAction':
        if (args.goalId) {
          return this.planner.planForGoal(instanceId, args.goalId, args.options);
        } else if (args.targetState) {
          return this.planner.planToState(instanceId, {
            target_state: args.targetState,
            options: args.options || {}
          });
        } else {
          throw new InvalidInputError(
            'planAction',
            'goalId or targetState',
            { goalId: args.goalId, targetState: args.targetState }
          );
        }
        
      case 'addAction':
        const actionSuccess = this.planner.addAction(instanceId, args.action);
        return { success: actionSuccess };
        
      case 'addGoal':
        const goalSuccess = this.planner.addGoal(instanceId, args.goal);
        return { success: goalSuccess };
        
      case 'setState':
        const stateSuccess = this.planner.setState(instanceId, {
          key: args.key,
          value: args.value
        });
        return { success: stateSuccess };
        
      case 'getState':
        if (args.key) {
          const value = this.planner.getState(instanceId, args.key);
          return { key: args.key, value };
        } else {
          const worldState = this.planner.getWorldState(instanceId);
          return worldState;
        }
        
      // Utility Tools
      case 'getMemoryStats':
        return this.memoryManager.getMemoryStats();
        
      case 'createInstance':
        return this.createInstanceByType(args.type, args.instanceId);
        
      case 'removeInstance':
        const removed = this.memoryManager.removeInstance(args.instanceId);
        return { success: removed, instanceId: args.instanceId };
        
      default:
        throw new InvalidInputError(
          'toolName',
          'valid tool name',
          toolName
        );
    }
  }

  /**
   * Get default instance ID for a tool
   */
  private getDefaultInstanceId(toolName: string): string {
    if (['queryGraph', 'addFact', 'addRule', 'runInference'].includes(toolName)) {
      return this.defaultInstanceIds.graphReasoner;
    }
    if (['extractAffect', 'extractPreferences', 'extractEmotions', 'analyzeText'].includes(toolName)) {
      return this.defaultInstanceIds.textExtractor;
    }
    if (['planAction', 'addAction', 'addGoal', 'setState', 'getState'].includes(toolName)) {
      return this.defaultInstanceIds.planner;
    }
    throw new InvalidInputError('toolName', 'valid tool name', toolName);
  }

  /**
   * Create instance by type
   */
  private createInstanceByType(type: string, instanceId?: string): any {
    switch (type) {
      case 'graph_reasoner':
        const graphId = this.graphReasoner.createInstance(instanceId);
        return { success: true, instanceId: graphId, type };
        
      case 'text_extractor':
        const textId = this.textExtractor.createInstance(instanceId);
        return { success: true, instanceId: textId, type };
        
      case 'planner':
        const plannerId = this.planner.createInstance(instanceId);
        return { success: true, instanceId: plannerId, type };
        
      default:
        throw new InvalidInputError(
          'type',
          'graph_reasoner|text_extractor|planner',
          type
        );
    }
  }

  /**
   * Cleanup all resources
   */
  public cleanup(): void {
    this.graphReasoner.cleanup();
    this.textExtractor.cleanup();
    this.planner.cleanup();
    this.memoryManager.cleanup();
    this.initialized = false;
  }

  /**
   * Get initialization status
   */
  public isInitialized(): boolean {
    return this.initialized;
  }

  /**
   * Get system health status
   */
  public getHealthStatus(): {
    initialized: boolean;
    memoryStats: any;
    activeInstances: {
      graphReasoner: string[];
      textExtractor: string[];
      planner: string[];
    };
  } {
    return {
      initialized: this.initialized,
      memoryStats: this.memoryManager.getMemoryStats(),
      activeInstances: {
        graphReasoner: this.graphReasoner.getActiveInstances(),
        textExtractor: this.textExtractor.getActiveInstances(),
        planner: this.planner.getActiveInstances()
      }
    };
  }
}