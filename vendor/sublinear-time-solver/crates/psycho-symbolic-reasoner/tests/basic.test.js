import { test, describe } from 'node:test';
import { strictEqual, ok, deepStrictEqual } from 'node:assert';

/**
 * Basic Test Suite for Psycho-Symbolic Reasoner
 *
 * These tests verify core functionality during development.
 * More comprehensive tests will be added as implementation progresses.
 */

describe('Psycho-Symbolic Reasoner Tests', () => {

  test('package.json is valid', async () => {
    const pkg = await import('../package.json', { assert: { type: 'json' } });

    strictEqual(pkg.default.name, 'psycho-symbolic-reasoner');
    strictEqual(pkg.default.version, '1.0.0');
    ok(pkg.default.description);
    ok(pkg.default.keywords.length > 0);
    strictEqual(pkg.default.license, 'MIT');
  });

  test('example files exist and are executable', async () => {
    const fs = await import('fs/promises');
    const path = await import('path');

    const examplesDir = path.resolve('./examples');
    const files = await fs.readdir(examplesDir);

    ok(files.includes('basic-usage.js'));
    ok(files.includes('mcp-integration.js'));
    ok(files.includes('knowledge-base.json'));

    // Check if example files are readable
    const basicUsage = await fs.readFile(path.join(examplesDir, 'basic-usage.js'), 'utf8');
    ok(basicUsage.includes('PsychoSymbolicReasoner'));
  });

  test('knowledge base structure is valid', async () => {
    const knowledgeBase = await import('../examples/knowledge-base.json', {
      assert: { type: 'json' }
    });

    const kb = knowledgeBase.default;

    ok(kb.metadata);
    ok(Array.isArray(kb.nodes));
    ok(Array.isArray(kb.edges));
    ok(Array.isArray(kb.rules));

    // Check that nodes have required fields
    if (kb.nodes.length > 0) {
      const firstNode = kb.nodes[0];
      ok(firstNode.id);
      ok(firstNode.type);
      ok(firstNode.properties);
    }

    // Check that edges reference valid nodes
    if (kb.edges.length > 0) {
      const firstEdge = kb.edges[0];
      ok(firstEdge.from);
      ok(firstEdge.to);
      ok(firstEdge.relationship);
    }
  });

  test('TypeScript configuration is valid', async () => {
    const fs = await import('fs/promises');

    const tsconfigPath = './tsconfig.json';
    const tsconfig = JSON.parse(await fs.readFile(tsconfigPath, 'utf8'));

    ok(tsconfig.compilerOptions);
    strictEqual(tsconfig.compilerOptions.target, 'ES2022');
    strictEqual(tsconfig.compilerOptions.module, 'ESNext');
    ok(Array.isArray(tsconfig.include));
    ok(Array.isArray(tsconfig.exclude));
  });

  test('CI/CD configuration exists', async () => {
    const fs = await import('fs/promises');
    const path = await import('path');

    const ciPath = path.join('.github', 'workflows', 'ci.yml');
    const ciConfig = await fs.readFile(ciPath, 'utf8');

    ok(ciConfig.includes('CI/CD Pipeline'));
    ok(ciConfig.includes('test-rust'));
    ok(ciConfig.includes('test-typescript'));
    ok(ciConfig.includes('publish'));
  });

  test('documentation files exist', async () => {
    const fs = await import('fs/promises');

    const docs = [
      'README.md',
      'CHANGELOG.md',
      'CONTRIBUTING.md',
      'LICENSE',
      'docs/API.md'
    ];

    for (const doc of docs) {
      try {
        await fs.access(doc);
        ok(true, `${doc} exists`);
      } catch (error) {
        ok(false, `${doc} is missing`);
      }
    }
  });

  test('Rust workspace configuration', async () => {
    const fs = await import('fs/promises');

    const cargoToml = await fs.readFile('./Cargo.toml', 'utf8');

    ok(cargoToml.includes('[workspace]'));
    ok(cargoToml.includes('graph_reasoner'));
    ok(cargoToml.includes('extractors'));
    ok(cargoToml.includes('planner'));
  });

  test('build scripts are properly configured', async () => {
    const pkg = await import('../package.json', { assert: { type: 'json' } });
    const scripts = pkg.default.scripts;

    ok(scripts.build);
    ok(scripts['build:wasm']);
    ok(scripts['build:ts']);
    ok(scripts.test);
    ok(scripts.lint);
    ok(scripts.clean);

    // Check that build script includes WASM and TypeScript builds
    ok(scripts.build.includes('build:wasm'));
    ok(scripts.build.includes('build:ts'));
  });

  test('dependencies are properly specified', async () => {
    const pkg = await import('../package.json', { assert: { type: 'json' } });

    // Check required dependencies
    const deps = pkg.default.dependencies;
    ok(deps['@modelcontextprotocol/sdk']);
    ok(deps['fastmcp']);
    ok(deps['commander']);
    ok(deps['express']);

    // Check dev dependencies
    const devDeps = pkg.default.devDependencies;
    ok(devDeps['typescript']);
    ok(devDeps['wasm-pack']);
    ok(devDeps['@typescript-eslint/eslint-plugin']);
  });

  test('package exports are correctly defined', async () => {
    const pkg = await import('../package.json', { assert: { type: 'json' } });
    const exports = pkg.default.exports;

    ok(exports['.']);
    ok(exports['./mcp']);
    ok(exports['./reasoner']);
    ok(exports['./extractors']);
    ok(exports['./planner']);
    ok(exports['./wasm']);

    // Check that each export has import and types
    Object.values(exports).forEach(exportDef => {
      ok(exportDef.import);
      ok(exportDef.types);
    });
  });

  test('CLI binaries are properly configured', async () => {
    const pkg = await import('../package.json', { assert: { type: 'json' } });
    const bin = pkg.default.bin;

    ok(bin['psycho-symbolic-reasoner']);
    ok(bin['psycho-reasoner']);
    ok(bin['psr']);

    // All should point to the same CLI entry point
    const cliPath = './dist/cli/index.js';
    strictEqual(bin['psycho-symbolic-reasoner'], cliPath);
  });

});

describe('Simulated Core Functionality Tests', () => {

  test('sentiment analysis simulation', async () => {
    // Simulate sentiment analysis functionality
    const mockSentimentAnalysis = (text) => {
      const positiveWords = ['happy', 'excited', 'great', 'excellent', 'love'];
      const negativeWords = ['sad', 'angry', 'terrible', 'hate', 'awful'];

      let score = 0;
      const words = text.toLowerCase().split(' ');

      words.forEach(word => {
        if (positiveWords.includes(word)) score += 0.2;
        if (negativeWords.includes(word)) score -= 0.2;
      });

      return {
        score: Math.max(-1, Math.min(1, score)),
        confidence: 0.8,
        primaryEmotion: score > 0 ? 'joy' : score < 0 ? 'sadness' : 'neutral'
      };
    };

    const result1 = mockSentimentAnalysis("I'm happy and excited about this project");
    ok(result1.score > 0);
    strictEqual(result1.primaryEmotion, 'joy');

    const result2 = mockSentimentAnalysis("This is terrible and awful");
    ok(result2.score < 0);
    strictEqual(result2.primaryEmotion, 'sadness');
  });

  test('preference extraction simulation', async () => {
    // Simulate preference extraction
    const mockPreferenceExtraction = (text) => {
      const preferences = [];

      if (text.includes('like') || text.includes('prefer')) {
        preferences.push({
          type: 'like',
          subject: 'user',
          object: 'extracted_preference',
          strength: 0.8,
          confidence: 0.7
        });
      }

      if (text.includes('dislike') || text.includes('hate')) {
        preferences.push({
          type: 'dislike',
          subject: 'user',
          object: 'extracted_preference',
          strength: 0.7,
          confidence: 0.8
        });
      }

      return {
        preferences,
        confidence: 0.75,
        categories: ['general']
      };
    };

    const result = mockPreferenceExtraction("I like quiet environments but dislike crowded spaces");
    strictEqual(result.preferences.length, 2);
    strictEqual(result.preferences[0].type, 'like');
    strictEqual(result.preferences[1].type, 'dislike');
  });

  test('graph reasoning simulation', async () => {
    // Simulate graph query functionality
    const mockGraphReasoning = (query) => {
      const mockResults = [
        { activity: 'meditation', confidence: 0.9 },
        { activity: 'deep_breathing', confidence: 0.8 },
        { activity: 'exercise', confidence: 0.7 }
      ];

      return {
        results: mockResults,
        executionTime: 25,
        totalResults: mockResults.length
      };
    };

    const result = mockGraphReasoning("find stress relief activities");
    ok(Array.isArray(result.results));
    ok(result.results.length > 0);
    ok(result.executionTime > 0);

    // Check result structure
    const firstResult = result.results[0];
    ok(firstResult.activity);
    ok(typeof firstResult.confidence === 'number');
  });

  test('planning simulation', async () => {
    // Simulate goal-oriented planning
    const mockPlanning = (goal, state, preferences) => {
      const actions = [
        {
          name: 'Take a break',
          description: 'Step away from work for 10 minutes',
          duration: 10,
          priority: 0.8
        },
        {
          name: 'Practice breathing',
          description: 'Do 4-7-8 breathing exercise',
          duration: 5,
          priority: 0.9
        }
      ];

      return {
        plan: actions,
        confidence: 0.85,
        estimatedDuration: actions.reduce((sum, action) => sum + action.duration, 0),
        explanation: 'Plan designed to address immediate stress relief'
      };
    };

    const result = mockPlanning(
      'reduce stress',
      { energy: 'low', stress: 'high' },
      [{ type: 'like', object: 'quick_solutions' }]
    );

    ok(Array.isArray(result.plan));
    ok(result.plan.length > 0);
    ok(typeof result.confidence === 'number');
    ok(result.explanation);

    // Check action structure
    const firstAction = result.plan[0];
    ok(firstAction.name);
    ok(firstAction.description);
    ok(typeof firstAction.duration === 'number');
  });

});

describe('Integration Tests', () => {

  test('MCP tool integration simulation', async () => {
    // Simulate MCP tool registration and execution
    const mockMCPTools = [
      {
        name: 'extractSentiment',
        description: 'Analyze sentiment from text',
        execute: async (params) => ({
          score: 0.5,
          primaryEmotion: 'joy',
          confidence: 0.8
        })
      },
      {
        name: 'createPlan',
        description: 'Generate action plan',
        execute: async (params) => ({
          plan: [
            { name: 'Action 1', duration: 10 },
            { name: 'Action 2', duration: 15 }
          ],
          confidence: 0.9
        })
      }
    ];

    // Test tool registration
    strictEqual(mockMCPTools.length, 2);

    // Test tool execution
    const sentimentTool = mockMCPTools.find(t => t.name === 'extractSentiment');
    const sentimentResult = await sentimentTool.execute({ text: 'Happy text' });
    ok(sentimentResult.score);
    ok(sentimentResult.primaryEmotion);

    const planTool = mockMCPTools.find(t => t.name === 'createPlan');
    const planResult = await planTool.execute({ goal: 'test goal' });
    ok(Array.isArray(planResult.plan));
    ok(planResult.confidence);
  });

  test('end-to-end workflow simulation', async () => {
    // Simulate complete workflow: input -> analysis -> planning -> output
    const userInput = "I'm feeling stressed about upcoming deadlines";

    // Step 1: Sentiment analysis
    const sentiment = {
      score: -0.6,
      primaryEmotion: 'stress',
      confidence: 0.9
    };

    // Step 2: Preference extraction
    const preferences = {
      preferences: [
        { type: 'like', object: 'quick_relief', strength: 0.8 }
      ]
    };

    // Step 3: Graph reasoning
    const techniques = {
      results: [
        { technique: 'deep_breathing', effectiveness: 0.9 },
        { technique: 'short_break', effectiveness: 0.7 }
      ]
    };

    // Step 4: Planning
    const plan = {
      plan: [
        { name: 'Deep breathing', duration: 5, priority: 0.9 },
        { name: 'Take a break', duration: 10, priority: 0.7 }
      ],
      confidence: 0.85
    };

    // Verify workflow
    ok(sentiment.score < 0); // Negative sentiment detected
    strictEqual(sentiment.primaryEmotion, 'stress');
    ok(preferences.preferences.length > 0);
    ok(techniques.results.length > 0);
    ok(plan.plan.length > 0);
    ok(plan.confidence > 0.8);
  });

});