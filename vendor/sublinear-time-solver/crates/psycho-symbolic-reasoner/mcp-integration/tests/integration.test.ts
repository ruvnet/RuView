import { describe, test, expect, beforeAll, afterAll, beforeEach } from '@jest/globals';
import { PsychoSymbolicMcpTools } from '../src/tools/index.js';
import { GraphReasonerWrapper } from '../src/wrappers/graph-reasoner.js';
import { TextExtractorWrapper } from '../src/wrappers/text-extractor.js';
import { PlannerWrapper } from '../src/wrappers/planner.js';
import { WasmMemoryManager } from '../src/wasm/memory-manager.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { existsSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Mock WASM paths for testing
const MOCK_WASM_DIR = join(__dirname, '..', 'mock-wasm');
const MOCK_CONFIG = {
  graphReasonerWasmPath: join(MOCK_WASM_DIR, 'graph_reasoner.wasm'),
  textExtractorWasmPath: join(MOCK_WASM_DIR, 'extractors.wasm'),
  plannerWasmPath: join(MOCK_WASM_DIR, 'planner.wasm')
};

// Mock WASM modules for testing
const createMockWasmModule = (moduleName: string) => {
  switch (moduleName) {
    case 'graph_reasoner':
      return {
        GraphReasoner: class MockGraphReasoner {
          add_fact(subject: string, predicate: string, object: string): string {
            return `fact_${Date.now()}`;
          }
          add_rule(rule_json: string): boolean {
            return true;
          }
          query(query_json: string): string {
            return JSON.stringify({
              success: true,
              results: [{ subject: 'test', predicate: 'is', object: 'working' }],
              count: 1,
              execution_time_ms: 10
            });
          }
          infer(max_iterations?: number): string {
            return JSON.stringify({
              new_facts: [{ subject: 'inferred', predicate: 'fact', object: 'test' }],
              applied_rules: ['rule_1'],
              inference_steps: [],
              confidence_scores: {}
            });
          }
          get_graph_stats(): string {
            return JSON.stringify({
              fact_count: 10,
              rule_count: 5,
              entity_count: 15
            });
          }
          free(): void {}
        },
        memory: new WebAssembly.Memory({ initial: 256 }),
        __wbindgen_malloc: (size: number) => 0,
        __wbindgen_free: (ptr: number, size: number) => {},
        __wbindgen_realloc: (ptr: number, oldSize: number, newSize: number) => 0
      };
      
    case 'text_extractor':
      return {
        TextExtractor: class MockTextExtractor {
          analyze_sentiment(text: string): string {
            return JSON.stringify({
              overall_sentiment: 'positive',
              confidence: 0.85,
              scores: { positive: 0.85, negative: 0.1, neutral: 0.05 }
            });
          }
          extract_preferences(text: string): string {
            return JSON.stringify({
              preferences: [
                { category: 'food', preference: 'pizza', strength: 'strong', confidence: 0.9 }
              ],
              confidence: 0.8,
              categories: ['food']
            });
          }
          detect_emotions(text: string): string {
            return JSON.stringify({
              primary_emotion: 'joy',
              emotions: [{ emotion: 'joy', score: 0.8, confidence: 0.9 }],
              intensity: 0.7,
              confidence: 0.85
            });
          }
          analyze_all(text: string): string {
            return JSON.stringify({
              sentiment: { overall_sentiment: 'positive', confidence: 0.85 },
              preferences: { preferences: [], confidence: 0.5 },
              emotions: { primary_emotion: 'joy', confidence: 0.8 }
            });
          }
          free(): void {}
        },
        memory: new WebAssembly.Memory({ initial: 256 }),
        __wbindgen_malloc: (size: number) => 0,
        __wbindgen_free: (ptr: number, size: number) => {},
        __wbindgen_realloc: (ptr: number, oldSize: number, newSize: number) => 0
      };
      
    case 'planner_system':
      return {
        PlannerSystem: class MockPlannerSystem {
          set_state(key: string, value: string): boolean {
            return true;
          }
          get_state(key: string): string {
            return JSON.stringify('test_value');
          }
          add_action(action_json: string): boolean {
            return true;
          }
          add_goal(goal_json: string): boolean {
            return true;
          }
          plan(goal_id: string): string {
            return JSON.stringify({
              success: true,
              plan: {
                id: 'plan_1',
                goal_id: goal_id,
                steps: [
                  {
                    step_number: 1,
                    action_id: 'action_1',
                    description: 'Test action',
                    estimated_cost: 10,
                    estimated_duration: 5
                  }
                ],
                total_cost: 10,
                estimated_duration: 5,
                created_at: new Date().toISOString()
              }
            });
          }
          plan_to_state(target_state_json: string): string {
            return this.plan('temp_goal');
          }
          execute_plan(plan_json: string): string {
            return JSON.stringify({
              success: true,
              executed_steps: [],
              final_state: { states: {} },
              total_cost: 0
            });
          }
          add_rule(rule_json: string): boolean {
            return true;
          }
          evaluate_rules(): string {
            return JSON.stringify([]);
          }
          get_world_state(): string {
            return JSON.stringify({ states: { test_key: 'test_value' } });
          }
          get_available_actions(): string {
            return JSON.stringify([]);
          }
          free(): void {}
        },
        memory: new WebAssembly.Memory({ initial: 256 }),
        __wbindgen_malloc: (size: number) => 0,
        __wbindgen_free: (ptr: number, size: number) => {},
        __wbindgen_realloc: (ptr: number, oldSize: number, newSize: number) => 0
      };
      
    default:
      throw new Error(`Unknown module: ${moduleName}`);
  }
};

// Mock the WASM loader
jest.mock('../src/wasm/loader.js', () => {
  return {
    WasmLoader: {
      getInstance: () => ({
        loadGraphReasoner: async () => createMockWasmModule('graph_reasoner'),
        loadTextExtractor: async () => createMockWasmModule('text_extractor'),
        loadPlannerSystem: async () => createMockWasmModule('planner_system'),
        isModuleLoaded: () => true,
        getModule: (name: string) => createMockWasmModule(name),
        unloadModule: () => true,
        unloadAllModules: () => {},
        getMemoryStats: () => ({ loadedModules: 3, loadingModules: 0, moduleNames: ['graph_reasoner', 'text_extractor', 'planner_system'] })
      })
    }
  };
});

describe('Psycho-Symbolic Reasoner MCP Integration', () => {
  let mcpTools: PsychoSymbolicMcpTools;
  let memoryManager: WasmMemoryManager;
  
  beforeAll(async () => {
    // Initialize with mock configuration
    mcpTools = new PsychoSymbolicMcpTools();
    memoryManager = WasmMemoryManager.getInstance();
    
    await mcpTools.initialize(MOCK_CONFIG);
  });
  
  afterAll(() => {
    mcpTools.cleanup();
  });
  
  beforeEach(() => {
    // Clean up instances between tests
    memoryManager.cleanup();
  });
  
  describe('MCP Tools Initialization', () => {
    test('should initialize successfully', () => {
      expect(mcpTools.isInitialized()).toBe(true);
    });
    
    test('should return health status', () => {
      const health = mcpTools.getHealthStatus();
      expect(health.initialized).toBe(true);
      expect(health.memoryStats).toBeDefined();
      expect(health.activeInstances).toBeDefined();
    });
    
    test('should list available tools', () => {
      const tools = mcpTools.getTools();
      expect(tools).toBeInstanceOf(Array);
      expect(tools.length).toBeGreaterThan(0);
      
      // Check for key tools
      const toolNames = tools.map(t => t.name);
      expect(toolNames).toContain('queryGraph');
      expect(toolNames).toContain('extractAffect');
      expect(toolNames).toContain('extractPreferences');
      expect(toolNames).toContain('extractEmotions');
      expect(toolNames).toContain('planAction');
    });
  });
  
  describe('Graph Reasoner Tools', () => {
    test('should add facts successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'addFact',
        arguments: {
          fact: {
            subject: 'John',
            predicate: 'likes',
            object: 'pizza'
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
      expect(response.factId).toBeDefined();
    });
    
    test('should query graph successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'queryGraph',
        arguments: {
          query: {
            pattern: {
              subject: 'John',
              predicate: 'likes'
            }
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
      expect(response.results).toBeInstanceOf(Array);
    });
    
    test('should add rules successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'addRule',
        arguments: {
          rule: {
            id: 'test_rule',
            name: 'Test Rule',
            conditions: [
              {
                type: 'fact',
                pattern: { subject: '?x', predicate: 'likes', object: 'pizza' }
              }
            ],
            conclusions: [
              {
                type: 'fact',
                content: { subject: '?x', predicate: 'is', object: 'pizza_lover' }
              }
            ]
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
    });
    
    test('should run inference successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'runInference',
        arguments: {
          options: {
            maxIterations: 5
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.new_facts).toBeInstanceOf(Array);
      expect(response.applied_rules).toBeInstanceOf(Array);
    });
  });
  
  describe('Text Extractor Tools', () => {
    test('should extract sentiment successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'extractAffect',
        arguments: {
          text: 'I love this amazing product! It makes me so happy.'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.overall_sentiment).toBeDefined();
      expect(response.confidence).toBeDefined();
      expect(response.scores).toBeDefined();
    });
    
    test('should extract preferences successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'extractPreferences',
        arguments: {
          text: 'I really enjoy Italian food, especially pizza and pasta. I also love watching movies.'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.preferences).toBeInstanceOf(Array);
      expect(response.confidence).toBeDefined();
    });
    
    test('should detect emotions successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'extractEmotions',
        arguments: {
          text: 'I am so excited about this new opportunity! It fills me with joy and anticipation.'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.primary_emotion).toBeDefined();
      expect(response.emotions).toBeInstanceOf(Array);
      expect(response.confidence).toBeDefined();
    });
    
    test('should analyze text comprehensively', async () => {
      const result = await mcpTools.callTool({
        name: 'analyzeText',
        arguments: {
          text: 'I absolutely love pizza! It\'s my favorite food and always makes me happy.',
          includeSentiment: true,
          includePreferences: true,
          includeEmotions: true
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.sentiment).toBeDefined();
      expect(response.preferences).toBeDefined();
      expect(response.emotions).toBeDefined();
    });
  });
  
  describe('Planner Tools', () => {
    test('should set state successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'setState',
        arguments: {
          key: 'player_health',
          value: 100
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
    });
    
    test('should get state successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'getState',
        arguments: {
          key: 'player_health'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.key).toBe('player_health');
      expect(response.value).toBeDefined();
    });
    
    test('should add actions successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'addAction',
        arguments: {
          action: {
            id: 'move_forward',
            name: 'Move Forward',
            description: 'Move the player forward one step',
            preconditions: [
              {
                state_key: 'can_move',
                required_value: true
              }
            ],
            effects: [
              {
                state_key: 'position_x',
                value: 1
              }
            ],
            cost: {
              base_cost: 1
            }
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
    });
    
    test('should add goals successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'addGoal',
        arguments: {
          goal: {
            id: 'reach_destination',
            name: 'Reach Destination',
            description: 'Get to the target location',
            conditions: [
              {
                state_key: 'position_x',
                target_value: 10
              },
              {
                state_key: 'position_y',
                target_value: 5
              }
            ],
            priority: 'high'
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
    });
    
    test('should create action plan successfully', async () => {
      const result = await mcpTools.callTool({
        name: 'planAction',
        arguments: {
          goalId: 'reach_destination',
          options: {
            maxDepth: 10,
            heuristic: 'astar'
          }
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
      if (response.plan) {
        expect(response.plan.steps).toBeInstanceOf(Array);
        expect(response.plan.total_cost).toBeDefined();
      }
    });
  });
  
  describe('Utility Tools', () => {
    test('should get memory stats', async () => {
      const result = await mcpTools.callTool({
        name: 'getMemoryStats',
        arguments: {}
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.totalInstances).toBeDefined();
      expect(response.instancesByType).toBeDefined();
    });
    
    test('should create instances', async () => {
      const result = await mcpTools.callTool({
        name: 'createInstance',
        arguments: {
          type: 'graph_reasoner',
          instanceId: 'test_graph_instance'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
      expect(response.instanceId).toBe('test_graph_instance');
      expect(response.type).toBe('graph_reasoner');
    });
    
    test('should remove instances', async () => {
      // First create an instance
      await mcpTools.callTool({
        name: 'createInstance',
        arguments: {
          type: 'text_extractor',
          instanceId: 'test_remove_instance'
        }
      });
      
      // Then remove it
      const result = await mcpTools.callTool({
        name: 'removeInstance',
        arguments: {
          instanceId: 'test_remove_instance'
        }
      });
      
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.success).toBe(true);
      expect(response.instanceId).toBe('test_remove_instance');
    });
  });
  
  describe('Error Handling', () => {
    test('should handle invalid tool names', async () => {
      const result = await mcpTools.callTool({
        name: 'nonexistentTool',
        arguments: {}
      });
      
      expect(result.isError).toBe(true);
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.error).toBeDefined();
      expect(response.code).toBe('INVALID_INPUT_ERROR');
    });
    
    test('should handle invalid arguments', async () => {
      const result = await mcpTools.callTool({
        name: 'addFact',
        arguments: {
          fact: {
            // Missing required fields
            subject: 'John'
            // predicate and object are missing
          }
        }
      });
      
      expect(result.isError).toBe(true);
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.error).toBeDefined();
    });
    
    test('should handle missing instance ID gracefully', async () => {
      const result = await mcpTools.callTool({
        name: 'queryGraph',
        arguments: {
          query: {
            pattern: {
              subject: 'test'
            }
          },
          instanceId: 'nonexistent_instance'
        }
      });
      
      expect(result.isError).toBe(true);
      expect(result.content[0].type).toBe('text');
      const response = JSON.parse(result.content[0].text);
      expect(response.error).toBeDefined();
    });
  });
  
  describe('Integration Scenarios', () => {
    test('should handle complex workflow', async () => {
      // 1. Analyze text to extract preferences and emotions
      const textAnalysis = await mcpTools.callTool({
        name: 'analyzeText',
        arguments: {
          text: 'I love Italian food and it makes me feel happy and excited!'
        }
      });
      
      expect(textAnalysis.content[0].type).toBe('text');
      const analysis = JSON.parse(textAnalysis.content[0].text);
      
      // 2. Add facts based on analysis to knowledge graph
      if (analysis.preferences && analysis.preferences.preferences.length > 0) {
        const addFactResult = await mcpTools.callTool({
          name: 'addFact',
          arguments: {
            fact: {
              subject: 'user',
              predicate: 'likes',
              object: analysis.preferences.preferences[0].preference
            }
          }
        });
        
        expect(addFactResult.content[0].type).toBe('text');
        const factResponse = JSON.parse(addFactResult.content[0].text);
        expect(factResponse.success).toBe(true);
      }
      
      // 3. Set planner state based on emotional state
      if (analysis.emotions && analysis.emotions.primary_emotion) {
        const setStateResult = await mcpTools.callTool({
          name: 'setState',
          arguments: {
            key: 'user_mood',
            value: analysis.emotions.primary_emotion
          }
        });
        
        expect(setStateResult.content[0].type).toBe('text');
        const stateResponse = JSON.parse(setStateResult.content[0].text);
        expect(stateResponse.success).toBe(true);
      }
      
      // 4. Query knowledge graph for related information
      const queryResult = await mcpTools.callTool({
        name: 'queryGraph',
        arguments: {
          query: {
            pattern: {
              subject: 'user',
              predicate: 'likes'
            }
          }
        }
      });
      
      expect(queryResult.content[0].type).toBe('text');
      const queryResponse = JSON.parse(queryResult.content[0].text);
      expect(queryResponse.success).toBe(true);
    });
  });
});