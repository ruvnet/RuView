/**
 * WASM Integration Tests
 * Tests WASM compilation, exports, and TypeScript integration
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';
import * as path from 'path';

describe('WASM Integration Tests', () => {
  const wasmPaths = {
    graphReasoner: path.join(__dirname, '../../../wasm/graph_reasoner/graph_reasoner.wasm'),
    planner: path.join(__dirname, '../../../wasm/planner/planner.wasm'),
    extractors: path.join(__dirname, '../../../wasm/extractors/extractors.wasm'),
  };

  beforeAll(async () => {
    // Ensure WASM modules are built
    console.log('Loading WASM modules for integration tests...');
  });

  afterAll(async () => {
    testUtils.wasmManager.unloadAll();
  });

  describe('Graph Reasoner WASM', () => {
    test('should load graph reasoner WASM module', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      expect(module).toBeDefined();
      expect(module.memory).toBeInstanceOf(WebAssembly.Memory);
      expect(module.exports).toBeDefined();
    });

    test('should export GraphReasoner constructor', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      expect(module.exports.GraphReasoner).toBeDefined();
      expect(typeof module.exports.GraphReasoner).toBe('function');
    });

    test('should create GraphReasoner instance', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();
      expect(reasoner).toBeDefined();
    });

    test('should add facts to knowledge graph', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();
      const factId = reasoner.add_fact("Alice", "loves", "Bob");

      expect(factId).toBeDefined();
      expect(typeof factId).toBe('string');
      expect(factId).not.toContain('Error');
    });

    test('should query knowledge graph', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();
      reasoner.add_fact("Alice", "loves", "Bob");
      reasoner.add_fact("Bob", "knows", "Charlie");

      const queryJson = JSON.stringify({
        subject: "Alice",
        predicate: null,
        object: null
      });

      const result = reasoner.query(queryJson);
      expect(result).toBeDefined();

      const parsedResult = JSON.parse(result);
      expect(parsedResult.facts).toBeDefined();
      expect(Array.isArray(parsedResult.facts)).toBe(true);
      expect(parsedResult.facts.length).toBeGreaterThan(0);
    });

    test('should perform inference', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();

      // Add facts
      reasoner.add_fact("Socrates", "is_a", "human");

      // Add rule
      const rule = {
        id: "mortality_rule",
        conditions: ["?x is_a human"],
        conclusions: ["?x is mortal"],
        confidence: 1.0
      };

      const ruleAdded = reasoner.add_rule(JSON.stringify(rule));
      expect(ruleAdded).toBe(true);

      // Run inference
      const inferenceResult = reasoner.infer(5);
      expect(inferenceResult).toBeDefined();

      const parsedResult = JSON.parse(inferenceResult);
      expect(Array.isArray(parsedResult)).toBe(true);
    });

    test('should get graph statistics', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();
      reasoner.add_fact("Alice", "loves", "Bob");
      reasoner.add_fact("Bob", "knows", "Charlie");

      const stats = reasoner.get_graph_stats();
      expect(stats).toBeDefined();

      const parsedStats = JSON.parse(stats);
      expect(parsedStats.fact_count).toBeGreaterThan(0);
      expect(parsedStats.entity_count).toBeGreaterThan(0);
    });

    test('should handle invalid inputs gracefully', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();

      // Invalid query
      const invalidResult = reasoner.query("invalid json");
      expect(invalidResult).toContain('error');

      // Invalid rule
      const invalidRule = reasoner.add_rule("invalid json");
      expect(invalidRule).toBe(false);
    });
  });

  describe('Planner WASM', () => {
    test('should load planner WASM module', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      expect(module).toBeDefined();
      expect(module.exports.PlannerSystem).toBeDefined();
    });

    test('should create PlannerSystem instance', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();
      expect(planner).toBeDefined();
    });

    test('should manage world state', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();

      // Set state
      const success = planner.set_state("health", JSON.stringify({ Integer: 100 }));
      expect(success).toBe(true);

      // Get state
      const state = planner.get_state("health");
      expect(state).toBeDefined();
      expect(state).not.toBe("null");

      const parsedState = JSON.parse(state);
      expect(parsedState.Integer).toBe(100);
    });

    test('should add actions and goals', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();

      // Add action
      const action = {
        id: "move_north",
        name: "Move North",
        description: "Move one step north",
        preconditions: [{
          state_key: "can_move",
          required_value: { Boolean: true },
          comparison: "equals"
        }],
        effects: [{
          state_key: "y_position",
          value: { Integer: 1 },
          operation: "add"
        }],
        cost: {
          base_cost: 1.0,
          dynamic_factors: {}
        }
      };

      const actionAdded = planner.add_action(JSON.stringify(action));
      expect(actionAdded).toBe(true);

      // Add goal
      const goal = {
        id: "reach_destination",
        name: "Reach Destination",
        description: "Get to the target location",
        conditions: [{
          state_key: "y_position",
          target_value: { Integer: 5 },
          comparison: "equals"
        }],
        priority: "High",
        deadline: null
      };

      const goalAdded = planner.add_goal(JSON.stringify(goal));
      expect(goalAdded).toBe(true);
    });

    test('should create plans', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();

      // Set up scenario
      planner.set_state("y_position", JSON.stringify({ Integer: 0 }));
      planner.set_state("can_move", JSON.stringify({ Boolean: true }));

      // Add action and goal (simplified)
      const action = {
        id: "move_up",
        name: "Move Up",
        description: "Move up one position",
        preconditions: [],
        effects: [{
          state_key: "y_position",
          value: { Integer: 1 },
          operation: "add"
        }],
        cost: { base_cost: 1.0, dynamic_factors: {} }
      };

      planner.add_action(JSON.stringify(action));

      const goal = {
        id: "reach_top",
        name: "Reach Top",
        description: "Get to position 3",
        conditions: [{
          state_key: "y_position",
          target_value: { Integer: 3 },
          comparison: "equals"
        }],
        priority: "Medium",
        deadline: null
      };

      planner.add_goal(JSON.stringify(goal));

      // Create plan
      const planResult = planner.plan("reach_top");
      expect(planResult).toBeDefined();

      const parsedResult = JSON.parse(planResult);
      expect(parsedResult).toBeDefined();
    });

    test('should execute plans', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();

      // Simple execution test
      const plan = {
        id: "test_plan",
        steps: [],
        total_cost: 0.0,
        estimated_duration: 0
      };

      const executionResult = planner.execute_plan(JSON.stringify(plan));
      expect(executionResult).toBeDefined();

      const parsedResult = JSON.parse(executionResult);
      expect(parsedResult.success).toBeDefined();
    });

    test('should evaluate rules', async () => {
      const module = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);
      const PlannerSystem = module.exports.PlannerSystem;

      const planner = new PlannerSystem();

      // Add a decision rule
      const rule = {
        id: "health_check",
        name: "Health Check Rule",
        conditions: [{
          state_key: "health",
          comparison: "less_than",
          value: { Integer: 20 }
        }],
        actions: ["use_health_potion"],
        priority: 100,
        cooldown_ms: null
      };

      const ruleAdded = planner.add_rule(JSON.stringify(rule));
      expect(ruleAdded).toBe(true);

      // Set low health
      planner.set_state("health", JSON.stringify({ Integer: 15 }));

      // Evaluate rules
      const decisions = planner.evaluate_rules();
      expect(decisions).toBeDefined();

      const parsedDecisions = JSON.parse(decisions);
      expect(Array.isArray(parsedDecisions)).toBe(true);
    });
  });

  describe('Extractors WASM', () => {
    test('should load extractors WASM module', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      expect(module).toBeDefined();
      expect(module.exports.TextExtractor).toBeDefined();
    });

    test('should create TextExtractor instance', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();
      expect(extractor).toBeDefined();
    });

    test('should analyze sentiment', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();
      const result = extractor.analyze_sentiment("I love this product! It's amazing!");

      expect(result).toBeDefined();
      const parsed = JSON.parse(result);
      expect(parsed.overall_score).toBeGreaterThan(0);
      expect(parsed.dominant_sentiment).toBe('positive');
    });

    test('should extract preferences', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();
      const result = extractor.extract_preferences("I prefer coffee over tea. I like dark roast.");

      expect(result).toBeDefined();
      const parsed = JSON.parse(result);
      expect(Array.isArray(parsed)).toBe(true);
    });

    test('should detect emotions', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();
      const result = extractor.detect_emotions("I'm so happy and excited about this news!");

      expect(result).toBeDefined();
      const parsed = JSON.parse(result);
      expect(Array.isArray(parsed)).toBe(true);
      expect(parsed.length).toBeGreaterThan(0);
    });

    test('should perform comprehensive analysis', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();
      const text = "I absolutely love my new phone! It's so much better than my old one. The camera quality is fantastic!";
      const result = extractor.analyze_all(text);

      expect(result).toBeDefined();
      const parsed = JSON.parse(result);
      expect(parsed.sentiment).toBeDefined();
      expect(parsed.preferences).toBeDefined();
      expect(parsed.emotions).toBeDefined();
    });

    test('should handle empty and invalid inputs', async () => {
      const module = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const TextExtractor = module.exports.TextExtractor;

      const extractor = new TextExtractor();

      // Empty text
      const emptyResult = extractor.analyze_sentiment("");
      expect(emptyResult).toBeDefined();
      const parsed = JSON.parse(emptyResult);
      expect(parsed.overall_score).toBe(0);

      // Very long text
      const longText = "word ".repeat(10000);
      const longResult = extractor.analyze_sentiment(longText);
      expect(longResult).toBeDefined();
    });
  });

  describe('Cross-Module Integration', () => {
    test('should load all modules simultaneously', async () => {
      const [graphModule, plannerModule, extractorsModule] = await Promise.all([
        testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner),
        testUtils.wasmManager.loadModule('planner', wasmPaths.planner),
        testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors),
      ]);

      expect(graphModule).toBeDefined();
      expect(plannerModule).toBeDefined();
      expect(extractorsModule).toBeDefined();
    });

    test('should integrate reasoning and planning', async () => {
      const graphModule = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const plannerModule = await testUtils.wasmManager.loadModule('planner', wasmPaths.planner);

      const GraphReasoner = graphModule.exports.GraphReasoner;
      const PlannerSystem = plannerModule.exports.PlannerSystem;

      const reasoner = new GraphReasoner();
      const planner = new PlannerSystem();

      // Add knowledge to reasoner
      reasoner.add_fact("Alice", "location", "home");
      reasoner.add_fact("Bob", "location", "work");
      reasoner.add_fact("Alice", "wants_to_meet", "Bob");

      // Use this knowledge to inform planning
      planner.set_state("current_location", JSON.stringify({ String: "home" }));
      planner.set_state("goal_location", JSON.stringify({ String: "work" }));

      // Both should work without conflicts
      const stats = reasoner.get_graph_stats();
      const state = planner.get_world_state();

      expect(JSON.parse(stats).fact_count).toBeGreaterThan(0);
      expect(JSON.parse(state)).toBeDefined();
    });

    test('should integrate text extraction with reasoning', async () => {
      const extractorsModule = await testUtils.wasmManager.loadModule('extractors', wasmPaths.extractors);
      const graphModule = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);

      const TextExtractor = extractorsModule.exports.TextExtractor;
      const GraphReasoner = graphModule.exports.GraphReasoner;

      const extractor = new TextExtractor();
      const reasoner = new GraphReasoner();

      // Extract preferences from text
      const text = "I prefer coffee shops over restaurants. I like quiet places for work.";
      const preferences = extractor.extract_preferences(text);
      const parsedPrefs = JSON.parse(preferences);

      // Add extracted knowledge to graph
      if (parsedPrefs.length > 0) {
        for (const pref of parsedPrefs) {
          reasoner.add_fact("User", "prefers", pref.item);
        }
      }

      const stats = reasoner.get_graph_stats();
      expect(JSON.parse(stats).fact_count).toBeGreaterThan(0);
    });
  });

  describe('Memory Management', () => {
    test('should not leak memory during repeated operations', async () => {
      const detector = testUtils.memoryLeakDetector;
      detector.start();

      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      // Perform many operations
      for (let i = 0; i < 1000; i++) {
        const reasoner = new GraphReasoner();
        reasoner.add_fact(`entity_${i}`, "property", `value_${i}`);
        reasoner.query(JSON.stringify({ subject: `entity_${i}`, predicate: null, object: null }));

        if (i % 100 === 0) {
          detector.snapshot();
        }
      }

      const leakCheck = detector.checkForLeaks();
      expect(leakCheck.hasLeak).toBe(false);
    });

    test('should handle large data sets efficiently', async () => {
      const collector = testUtils.performanceCollector;
      collector.start();

      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();

      // Add large number of facts
      for (let i = 0; i < 10000; i++) {
        reasoner.add_fact(`entity_${i}`, "relates_to", `entity_${(i + 1) % 10000}`);
        collector.incrementOperation();
      }

      // Perform queries
      for (let i = 0; i < 100; i++) {
        reasoner.query(JSON.stringify({
          subject: `entity_${i * 100}`,
          predicate: null,
          object: null
        }));
        collector.incrementOperation();
      }

      const metrics = collector.stop();
      expect(metrics.executionTime).toBeLessThan(10000); // Under 10 seconds
      expect(metrics.memoryUsage).toBeLessThan(100 * 1024 * 1024); // Under 100MB
    });
  });

  describe('Error Handling', () => {
    test('should handle WASM loading failures gracefully', async () => {
      const invalidPath = '/nonexistent/path/module.wasm';

      await expect(
        testUtils.wasmManager.loadModule('invalid', invalidPath)
      ).rejects.toThrow();
    });

    test('should handle module initialization errors', async () => {
      // This test would require a corrupted WASM file
      // For now, we test the error handling structure
      expect(() => {
        testUtils.wasmManager.getModule('nonexistent');
      }).not.toThrow();

      expect(testUtils.wasmManager.getModule('nonexistent')).toBeUndefined();
    });

    test('should recover from operation failures', async () => {
      const module = await testUtils.wasmManager.loadModule('graph_reasoner', wasmPaths.graphReasoner);
      const GraphReasoner = module.exports.GraphReasoner;

      const reasoner = new GraphReasoner();

      // Test recovery after invalid operations
      const invalidQuery = reasoner.query("invalid json");
      expect(invalidQuery).toContain('error');

      // Should still work after error
      const validId = reasoner.add_fact("Alice", "loves", "Bob");
      expect(validId).not.toContain('Error');
    });
  });
});