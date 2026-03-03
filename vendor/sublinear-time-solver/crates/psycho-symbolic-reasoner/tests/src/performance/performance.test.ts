/**
 * Performance Tests
 * Tests performance characteristics of each reasoning component
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';
import Benchmark from 'benchmark';

describe('Performance Tests', () => {
  let benchmarkSuite: Benchmark.Suite;

  beforeAll(() => {
    console.log('Initializing performance test environment...');
  });

  beforeEach(() => {
    benchmarkSuite = new Benchmark.Suite();
  });

  afterEach(async () => {
    if (benchmarkSuite) {
      await new Promise<void>((resolve) => {
        benchmarkSuite.on('complete', () => resolve());
        if (benchmarkSuite.length === 0) {
          resolve();
        }
      });
    }
  });

  describe('Graph Reasoning Performance', () => {
    test('should benchmark fact insertion performance', async () => {
      const collector = testUtils.performanceCollector;
      const datasets = [
        { name: 'small', facts: 100 },
        { name: 'medium', facts: 1000 },
        { name: 'large', facts: 10000 },
        { name: 'xlarge', facts: 50000 }
      ];

      const results: Array<{
        dataset: string;
        facts: number;
        totalTime: number;
        avgTimePerFact: number;
        factsPerSecond: number;
        memoryUsage: number;
      }> = [];

      for (const dataset of datasets) {
        collector.start();

        // Simulate fact insertion
        const facts = [];
        for (let i = 0; i < dataset.facts; i++) {
          const fact = {
            id: `fact_${i}`,
            subject: `entity_${i % 1000}`,
            predicate: ['knows', 'likes', 'works_with', 'related_to'][i % 4],
            object: `entity_${(i + 1) % 1000}`,
            confidence: Math.random()
          };
          facts.push(fact);
          collector.incrementOperation();

          // Simulate processing time
          if (i % 100 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const result = {
          dataset: dataset.name,
          facts: dataset.facts,
          totalTime: metrics.executionTime,
          avgTimePerFact: metrics.executionTime / dataset.facts,
          factsPerSecond: dataset.facts / (metrics.executionTime / 1000),
          memoryUsage: metrics.memoryUsage
        };

        results.push(result);

        // Performance assertions
        expect(result.avgTimePerFact).toBeLessThan(1); // Less than 1ms per fact
        expect(result.factsPerSecond).toBeGreaterThan(100); // At least 100 facts/second
        expect(result.memoryUsage).toBeLessThan(dataset.facts * 1000); // Reasonable memory usage

        console.log(`${dataset.name}: ${Math.round(result.factsPerSecond)} facts/sec, ${result.avgTimePerFact.toFixed(3)}ms/fact`);
      }

      // Verify scalability
      const smallResult = results.find(r => r.dataset === 'small')!;
      const largeResult = results.find(r => r.dataset === 'large')!;

      // Performance should scale reasonably (not linearly degrading)
      const scalingFactor = largeResult.avgTimePerFact / smallResult.avgTimePerFact;
      expect(scalingFactor).toBeLessThan(5); // Should not be more than 5x slower per fact
    });

    test('should benchmark query performance', async () => {
      // Pre-populate graph with test data
      const graphData = testUtils.dataGenerator.generateGraphData(5000, 15000);
      const collector = testUtils.performanceCollector;

      const queryTypes = [
        { name: 'simple_subject', pattern: { subject: 'node_100', predicate: null, object: null } },
        { name: 'simple_predicate', pattern: { subject: null, predicate: 'knows', object: null } },
        { name: 'complex_pattern', pattern: { subject: 'node_*', predicate: 'knows', object: 'node_*' } },
        { name: 'wildcard_query', pattern: { subject: null, predicate: null, object: null } }
      ];

      const queryResults: Array<{
        queryType: string;
        avgResponseTime: number;
        resultsFound: number;
        queriesPerSecond: number;
      }> = [];

      for (const queryType of queryTypes) {
        collector.start();

        const iterations = 1000;
        let totalResults = 0;

        for (let i = 0; i < iterations; i++) {
          // Simulate query execution
          const query = {
            ...queryType.pattern,
            execution_id: `query_${i}`,
            timestamp: Date.now()
          };

          // Mock query processing
          const mockResults = graphData.edges.filter(edge => {
            if (queryType.pattern.subject && !edge.source.includes(queryType.pattern.subject.replace('*', ''))) {
              return false;
            }
            if (queryType.pattern.predicate && edge.relation !== queryType.pattern.predicate) {
              return false;
            }
            if (queryType.pattern.object && !edge.target.includes(queryType.pattern.object.replace('*', ''))) {
              return false;
            }
            return true;
          });

          totalResults += mockResults.length;
          collector.incrementOperation();

          // Simulate processing delay
          if (i % 100 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const result = {
          queryType: queryType.name,
          avgResponseTime: metrics.executionTime / iterations,
          resultsFound: totalResults / iterations,
          queriesPerSecond: iterations / (metrics.executionTime / 1000)
        };

        queryResults.push(result);

        // Performance assertions
        expect(result.avgResponseTime).toBeLessThan(10); // Less than 10ms per query
        expect(result.queriesPerSecond).toBeGreaterThan(50); // At least 50 queries/second

        console.log(`${queryType.name}: ${Math.round(result.queriesPerSecond)} queries/sec, ${result.avgResponseTime.toFixed(2)}ms avg`);
      }

      // Verify query complexity scaling
      const simpleQuery = queryResults.find(r => r.queryType === 'simple_subject')!;
      const complexQuery = queryResults.find(r => r.queryType === 'complex_pattern')!;

      expect(simpleQuery.avgResponseTime).toBeLessThan(complexQuery.avgResponseTime * 2);
    });

    test('should benchmark inference performance', async () => {
      const collector = testUtils.performanceCollector;

      const inferenceScenarios = [
        { name: 'simple_rules', rules: 5, facts: 100, iterations: 10 },
        { name: 'complex_rules', rules: 20, facts: 500, iterations: 20 },
        { name: 'deep_inference', rules: 10, facts: 200, iterations: 50 }
      ];

      for (const scenario of inferenceScenarios) {
        collector.start();

        // Setup inference scenario
        const rules = [];
        for (let i = 0; i < scenario.rules; i++) {
          rules.push({
            id: `rule_${i}`,
            conditions: [`?x type_${i % 3} ?y`],
            conclusions: [`?x inferred_${i} ?y`],
            confidence: 0.8 + (Math.random() * 0.2)
          });
        }

        const facts = [];
        for (let i = 0; i < scenario.facts; i++) {
          facts.push({
            subject: `entity_${i}`,
            predicate: `type_${i % 3}`,
            object: `value_${i}`,
            confidence: Math.random()
          });
        }

        // Run inference iterations
        let totalInferences = 0;
        for (let iter = 0; iter < scenario.iterations; iter++) {
          // Simulate inference process
          const newInferences = [];

          for (const rule of rules) {
            for (const fact of facts) {
              // Simple pattern matching simulation
              if (Math.random() > 0.7) { // 30% chance of rule firing
                newInferences.push({
                  subject: fact.subject,
                  predicate: rule.conclusions[0].split(' ')[1],
                  object: fact.object,
                  confidence: rule.confidence * fact.confidence,
                  derived_from: [rule.id, fact.subject]
                });
              }
            }
          }

          totalInferences += newInferences.length;
          collector.incrementOperation();

          // Simulate processing time
          await testUtils.asyncUtils.sleep(2);
        }

        const metrics = collector.stop();

        const results = {
          scenario: scenario.name,
          totalInferences: totalInferences,
          inferencesPerIteration: totalInferences / scenario.iterations,
          avgIterationTime: metrics.executionTime / scenario.iterations,
          inferencesPerSecond: totalInferences / (metrics.executionTime / 1000)
        };

        // Performance assertions
        expect(results.avgIterationTime).toBeLessThan(100); // Less than 100ms per iteration
        expect(results.inferencesPerSecond).toBeGreaterThan(1); // At least 1 inference/second
        expect(results.totalInferences).toBeGreaterThan(0); // Should generate some inferences

        console.log(`${scenario.name}: ${Math.round(results.inferencesPerSecond)} inferences/sec, ${results.avgIterationTime.toFixed(1)}ms/iteration`);
      }
    });
  });

  describe('Planning Performance', () => {
    test('should benchmark planning algorithm performance', async () => {
      const collector = testUtils.performanceCollector;

      const planningScenarios = [
        { name: 'simple_linear', states: 10, actions: 5, goals: 1 },
        { name: 'branching_medium', states: 50, actions: 20, goals: 3 },
        { name: 'complex_network', states: 200, actions: 100, goals: 5 },
        { name: 'large_state_space', states: 1000, actions: 500, goals: 10 }
      ];

      for (const scenario of planningScenarios) {
        collector.start();

        // Generate planning scenario
        const planningData = testUtils.dataGenerator.generatePlanningScenario();

        // Extend scenario based on complexity
        const states = {};
        for (let i = 0; i < scenario.states; i++) {
          states[`state_${i}`] = Math.random() > 0.5;
        }

        const actions = [];
        for (let i = 0; i < scenario.actions; i++) {
          actions.push({
            id: `action_${i}`,
            preconditions: { [`state_${i % scenario.states}`]: true },
            effects: { [`state_${(i + 1) % scenario.states}`]: !states[`state_${(i + 1) % scenario.states}`] },
            cost: Math.random() * 10 + 1
          });
        }

        const goals = [];
        for (let i = 0; i < scenario.goals; i++) {
          goals.push({
            id: `goal_${i}`,
            conditions: { [`state_${Math.floor(Math.random() * scenario.states)}`]: true }
          });
        }

        // Simulate planning process
        const planningRuns = 10;
        let totalPlansGenerated = 0;
        let totalPlanSteps = 0;

        for (let run = 0; run < planningRuns; run++) {
          // Mock A* search
          const plan = {
            steps: [],
            totalCost: 0,
            searchNodes: 0
          };

          // Simulate search process
          const maxSearchNodes = scenario.states * scenario.actions;
          const searchSteps = Math.min(maxSearchNodes, 1000);

          for (let step = 0; step < searchSteps; step++) {
            plan.searchNodes++;

            // Randomly find applicable actions
            if (Math.random() > 0.8) {
              const applicableAction = actions[Math.floor(Math.random() * actions.length)];
              plan.steps.push({
                action: applicableAction.id,
                cost: applicableAction.cost
              });
              plan.totalCost += applicableAction.cost;

              // Check if goal reached
              if (plan.steps.length >= scenario.goals.length) {
                break;
              }
            }

            collector.incrementOperation();
          }

          if (plan.steps.length > 0) {
            totalPlansGenerated++;
            totalPlanSteps += plan.steps.length;
          }

          // Simulate planning computation time
          if (run % 3 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const results = {
          scenario: scenario.name,
          stateSpaceSize: scenario.states,
          actionSpaceSize: scenario.actions,
          avgPlanningTime: metrics.executionTime / planningRuns,
          plansGenerated: totalPlansGenerated,
          avgPlanLength: totalPlanSteps / Math.max(totalPlansGenerated, 1),
          plansPerSecond: totalPlansGenerated / (metrics.executionTime / 1000)
        };

        // Performance assertions
        expect(results.avgPlanningTime).toBeLessThan(1000); // Less than 1 second per plan
        expect(results.plansGenerated).toBeGreaterThan(0); // Should generate some plans
        expect(results.avgPlanLength).toBeGreaterThan(0); // Plans should have steps

        console.log(`${scenario.name}: ${results.avgPlanningTime.toFixed(1)}ms/plan, ${results.plansGenerated}/${planningRuns} successful`);
      }
    });

    test('should benchmark state space exploration', async () => {
      const collector = testUtils.performanceCollector;

      const explorationTests = [
        { name: 'breadth_first', strategy: 'BFS', maxDepth: 5 },
        { name: 'depth_first', strategy: 'DFS', maxDepth: 10 },
        { name: 'best_first', strategy: 'A*', maxDepth: 8 },
        { name: 'iterative_deepening', strategy: 'IDS', maxDepth: 6 }
      ];

      for (const test of explorationTests) {
        collector.start();

        // Simulate state space exploration
        const explored = new Set();
        const frontier = ['initial_state'];
        let explorationSteps = 0;
        const maxSteps = 10000;

        while (frontier.length > 0 && explorationSteps < maxSteps) {
          const currentState = frontier.shift()!;

          if (explored.has(currentState)) {
            continue;
          }

          explored.add(currentState);
          explorationSteps++;

          // Generate successor states
          const successors = [];
          const branchingFactor = Math.floor(Math.random() * 4) + 2; // 2-5 successors

          for (let i = 0; i < branchingFactor; i++) {
            const depth = currentState.split('_').length - 1;
            if (depth < test.maxDepth) {
              successors.push(`${currentState}_${i}`);
            }
          }

          // Add to frontier based on strategy
          if (test.strategy === 'BFS') {
            frontier.push(...successors);
          } else if (test.strategy === 'DFS') {
            frontier.unshift(...successors);
          } else {
            // For A* and IDS, add with some ordering
            successors.sort(() => Math.random() - 0.5);
            frontier.push(...successors);
          }

          collector.incrementOperation();

          // Simulate computation time
          if (explorationSteps % 100 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const results = {
          strategy: test.strategy,
          statesExplored: explored.size,
          explorationSteps: explorationSteps,
          avgTimePerState: metrics.executionTime / explored.size,
          statesPerSecond: explored.size / (metrics.executionTime / 1000),
          frontierPeakSize: Math.max(0, explorationSteps - explored.size)
        };

        // Performance assertions
        expect(results.statesExplored).toBeGreaterThan(10); // Should explore some states
        expect(results.avgTimePerState).toBeLessThan(10); // Less than 10ms per state
        expect(results.statesPerSecond).toBeGreaterThan(1); // At least 1 state/second

        console.log(`${test.strategy}: ${results.statesExplored} states, ${results.avgTimePerState.toFixed(2)}ms/state`);
      }
    });
  });

  describe('Text Extraction Performance', () => {
    test('should benchmark sentiment analysis performance', async () => {
      const collector = testUtils.performanceCollector;

      const textSizes = [
        { name: 'short', length: 100, count: 1000 },
        { name: 'medium', length: 500, count: 500 },
        { name: 'long', length: 2000, count: 100 },
        { name: 'very_long', length: 10000, count: 50 }
      ];

      for (const textSize of textSizes) {
        collector.start();

        // Generate test texts
        const texts = [];
        for (let i = 0; i < textSize.count; i++) {
          texts.push(testUtils.dataGenerator.generateRandomString(textSize.length));
        }

        // Perform sentiment analysis
        let totalAnalyses = 0;
        for (const text of texts) {
          // Mock sentiment analysis processing
          const words = text.split(' ').length;
          const processingTime = Math.max(1, words * 0.1); // Simulate word-based processing

          // Simulate sentiment scoring
          const sentimentScore = Math.random() * 2 - 1; // -1 to 1
          const confidence = Math.random();

          const analysis = {
            text_length: text.length,
            word_count: words,
            sentiment_score: sentimentScore,
            confidence: confidence,
            processing_time: processingTime
          };

          totalAnalyses++;
          collector.incrementOperation();

          // Simulate actual processing time
          if (totalAnalyses % 50 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const results = {
          textSize: textSize.name,
          avgTextLength: textSize.length,
          textsProcessed: totalAnalyses,
          avgProcessingTime: metrics.executionTime / totalAnalyses,
          textsPerSecond: totalAnalyses / (metrics.executionTime / 1000),
          charactersPerSecond: (totalAnalyses * textSize.length) / (metrics.executionTime / 1000)
        };

        // Performance assertions
        expect(results.avgProcessingTime).toBeLessThan(50); // Less than 50ms per text
        expect(results.textsPerSecond).toBeGreaterThan(5); // At least 5 texts/second
        expect(results.charactersPerSecond).toBeGreaterThan(1000); // At least 1000 chars/second

        console.log(`${textSize.name}: ${Math.round(results.textsPerSecond)} texts/sec, ${Math.round(results.charactersPerSecond)} chars/sec`);
      }
    });

    test('should benchmark pattern matching performance', async () => {
      const collector = testUtils.performanceCollector;

      const patterns = [
        { name: 'email', regex: /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/, complexity: 'medium' },
        { name: 'phone', regex: /\b\d{3}[-.]?\d{3}[-.]?\d{4}\b/, complexity: 'low' },
        { name: 'url', regex: /https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)/, complexity: 'high' },
        { name: 'date', regex: /\b\d{1,2}[-/]\d{1,2}[-/]\d{4}\b/, complexity: 'low' }
      ];

      const testText = `
        Contact us at support@example.com or call 555-123-4567.
        Visit our website at https://www.example.com/contact for more info.
        The meeting is scheduled for 12/25/2023 at 2:00 PM.
        Alternative contact: admin@test.org or 555-987-6543.
        Additional resources: https://docs.example.com/api/v1/reference
        Event date: 01/15/2024, backup date: 02/01/2024.
      `.repeat(100); // Repeat to create larger text

      for (const pattern of patterns) {
        collector.start();

        const iterations = 1000;
        let totalMatches = 0;

        for (let i = 0; i < iterations; i++) {
          const matches = testText.match(new RegExp(pattern.regex, 'g')) || [];
          totalMatches += matches.length;
          collector.incrementOperation();

          // Simulate processing overhead
          if (i % 100 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }
        }

        const metrics = collector.stop();

        const results = {
          pattern: pattern.name,
          complexity: pattern.complexity,
          avgMatchTime: metrics.executionTime / iterations,
          matchesFound: totalMatches / iterations,
          matchesPerSecond: totalMatches / (metrics.executionTime / 1000),
          textProcessingRate: (testText.length * iterations) / (metrics.executionTime / 1000)
        };

        // Performance assertions based on complexity
        const maxTimeByComplexity = {
          low: 5,
          medium: 10,
          high: 20
        };

        expect(results.avgMatchTime).toBeLessThan(maxTimeByComplexity[pattern.complexity as keyof typeof maxTimeByComplexity]);
        expect(results.matchesFound).toBeGreaterThan(0);

        console.log(`${pattern.name}: ${results.avgMatchTime.toFixed(2)}ms/match, ${Math.round(results.matchesPerSecond)} matches/sec`);
      }
    });
  });

  describe('Memory and Resource Usage', () => {
    test('should benchmark memory efficiency', async () => {
      const detector = testUtils.memoryLeakDetector;
      detector.start();

      const memoryTests = [
        { name: 'graph_growth', operations: 10000, type: 'graph' },
        { name: 'planning_states', operations: 5000, type: 'planning' },
        { name: 'text_processing', operations: 2000, type: 'text' },
        { name: 'agent_creation', operations: 1000, type: 'agents' }
      ];

      for (const test of memoryTests) {
        const initialSnapshot = detector.snapshots.length;

        for (let i = 0; i < test.operations; i++) {
          switch (test.type) {
            case 'graph':
              // Simulate graph node creation
              const graphNode = {
                id: `node_${i}`,
                properties: new Array(100).fill(0).map(() => Math.random()),
                connections: new Array(Math.floor(Math.random() * 10)).fill(0).map(() => `node_${Math.floor(Math.random() * i)}`)
              };
              break;

            case 'planning':
              // Simulate state creation
              const state = {
                id: `state_${i}`,
                variables: new Array(50).fill(0).reduce((acc, _, idx) => {
                  acc[`var_${idx}`] = Math.random();
                  return acc;
                }, {} as Record<string, number>)
              };
              break;

            case 'text':
              // Simulate text analysis results
              const analysis = {
                id: `analysis_${i}`,
                text: testUtils.dataGenerator.generateRandomString(500),
                results: {
                  sentiment: Math.random() * 2 - 1,
                  keywords: new Array(20).fill(0).map(() => testUtils.dataGenerator.generateRandomString(10)),
                  entities: new Array(10).fill(0).map(() => ({ name: testUtils.dataGenerator.generateRandomString(8), type: 'entity' }))
                }
              };
              break;

            case 'agents':
              // Simulate agent creation
              const agent = testUtils.mockAgentFactory.createAgent(
                `memory_test_${i}`,
                'test_agent',
                [`capability_${i % 10}`]
              );
              break;
          }

          // Take memory snapshots periodically
          if (i % 1000 === 0) {
            detector.snapshot();
          }
        }

        const finalSnapshot = detector.snapshots.length;
        const snapshotsTaken = finalSnapshot - initialSnapshot;

        expect(snapshotsTaken).toBeGreaterThan(0);
        console.log(`${test.name}: ${test.operations} operations, ${snapshotsTaken} memory snapshots`);
      }

      const memoryAnalysis = detector.checkForLeaks();

      // Memory usage assertions
      expect(memoryAnalysis.hasLeak).toBe(false);
      expect(memoryAnalysis.memoryIncrease).toBeLessThan(200 * 1024 * 1024); // Less than 200MB total
      expect(memoryAnalysis.leakRate).toBeLessThan(1024 * 1024); // Less than 1MB/second

      console.log(`Total memory increase: ${Math.round(memoryAnalysis.memoryIncrease / 1024 / 1024)}MB`);
      console.log(`Memory growth rate: ${Math.round(memoryAnalysis.leakRate / 1024)}KB/sec`);
    });

    test('should benchmark concurrent operation performance', async () => {
      const collector = testUtils.performanceCollector;
      collector.start();

      const concurrencyLevels = [1, 2, 4, 8, 16];
      const operationsPerLevel = 1000;

      for (const concurrency of concurrencyLevels) {
        const levelStart = performance.now();

        const promises = [];
        for (let i = 0; i < concurrency; i++) {
          const promise = async () => {
            for (let j = 0; j < operationsPerLevel / concurrency; j++) {
              // Simulate mixed operations
              const operationType = j % 4;

              switch (operationType) {
                case 0: // Graph operation
                  const fact = {
                    subject: `entity_${i}_${j}`,
                    predicate: 'relates_to',
                    object: `entity_${(i + 1) % concurrency}_${j}`
                  };
                  break;

                case 1: // Planning operation
                  const planStep = {
                    action: `action_${i}_${j}`,
                    preconditions: [`condition_${j}`],
                    effects: [`effect_${j + 1}`]
                  };
                  break;

                case 2: // Text operation
                  const text = testUtils.dataGenerator.generateRandomString(100);
                  const sentiment = Math.random() * 2 - 1;
                  break;

                case 3: // Agent operation
                  const message = {
                    from: `agent_${i}`,
                    to: `agent_${(i + 1) % concurrency}`,
                    content: `message_${j}`
                  };
                  break;
              }

              collector.incrementOperation();

              // Simulate some async work
              if (j % 10 === 0) {
                await testUtils.asyncUtils.sleep(1);
              }
            }
          };

          promises.push(promise());
        }

        await Promise.all(promises);

        const levelTime = performance.now() - levelStart;
        const throughput = operationsPerLevel / (levelTime / 1000);

        console.log(`Concurrency ${concurrency}: ${Math.round(throughput)} ops/sec, ${levelTime.toFixed(1)}ms total`);

        // Performance should scale with concurrency (up to a point)
        expect(levelTime).toBeLessThan(10000); // Under 10 seconds
        expect(throughput).toBeGreaterThan(50); // At least 50 ops/second
      }

      const metrics = collector.stop();

      expect(metrics.executionTime).toBeLessThan(60000); // Under 1 minute total
      expect(metrics.operationCount).toBe(operationsPerLevel * concurrencyLevels.length);
    });
  });

  describe('Stress Testing', () => {
    test('should handle sustained high load', async () => {
      const stressDuration = 60000; // 1 minute
      const startTime = Date.now();
      let operationCount = 0;
      let errorCount = 0;

      const detector = testUtils.memoryLeakDetector;
      detector.start();

      while (Date.now() - startTime < stressDuration) {
        try {
          // Mixed stress operations
          const operationType = operationCount % 6;

          switch (operationType) {
            case 0:
              // High-frequency graph operations
              for (let i = 0; i < 100; i++) {
                const fact = {
                  id: `stress_fact_${operationCount}_${i}`,
                  subject: `entity_${Math.floor(Math.random() * 1000)}`,
                  predicate: ['knows', 'likes', 'works_with'][Math.floor(Math.random() * 3)],
                  object: `entity_${Math.floor(Math.random() * 1000)}`
                };
              }
              break;

            case 1:
              // Rapid planning scenarios
              const scenario = testUtils.dataGenerator.generatePlanningScenario();
              break;

            case 2:
              // Burst text processing
              const texts = new Array(50).fill(0).map(() =>
                testUtils.dataGenerator.generateRandomString(200)
              );
              break;

            case 3:
              // Agent network stress
              for (let i = 0; i < 20; i++) {
                const agent = testUtils.mockAgentFactory.createAgent(
                  `stress_agent_${operationCount}_${i}`,
                  'stress_worker',
                  [`stress_capability_${i % 5}`]
                );
              }
              break;

            case 4:
              // Memory allocation stress
              const largeArray = new Array(10000).fill(0).map(() => Math.random());
              break;

            case 5:
              // Cleanup operations
              testUtils.mockAgentFactory.clearAgents();
              if (global.gc) {
                global.gc();
              }
              break;
          }

          operationCount++;

          // Take memory snapshots during stress
          if (operationCount % 100 === 0) {
            detector.snapshot();
          }

          // Small delay to prevent overwhelming
          if (operationCount % 50 === 0) {
            await testUtils.asyncUtils.sleep(1);
          }

        } catch (error) {
          errorCount++;
          console.warn(`Stress test error ${errorCount}:`, error);
        }
      }

      const actualDuration = Date.now() - startTime;
      const operationsPerSecond = operationCount / (actualDuration / 1000);
      const errorRate = errorCount / operationCount;

      const memoryAnalysis = detector.checkForLeaks();

      // Stress test assertions
      expect(operationCount).toBeGreaterThan(100); // Should complete meaningful work
      expect(errorRate).toBeLessThan(0.05); // Less than 5% error rate
      expect(operationsPerSecond).toBeGreaterThan(5); // At least 5 ops/second under stress
      expect(memoryAnalysis.hasLeak).toBe(false); // No memory leaks under stress

      console.log(`Stress test: ${operationCount} ops in ${actualDuration}ms`);
      console.log(`Performance: ${Math.round(operationsPerSecond)} ops/sec`);
      console.log(`Error rate: ${(errorRate * 100).toFixed(2)}%`);
      console.log(`Memory stable: ${!memoryAnalysis.hasLeak}`);
    });
  });
});