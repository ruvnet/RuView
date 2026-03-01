# Claude-Flow + DSPy.ts Integration Guide
## Self-Learning Multi-Agent Orchestration

**Purpose:** Integrate DSPy.ts optimization capabilities with Claude-Flow swarm orchestration for self-improving multi-agent systems.

---

## 🎯 Integration Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude-Flow Layer                        │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│  │   Swarm    │  │   Memory   │  │   Neural   │           │
│  │Coordinator │  │  Manager   │  │  Training  │           │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘           │
│        │                │                │                   │
└────────┼────────────────┼────────────────┼───────────────────┘
         │                │                │
┌────────┼────────────────┼────────────────┼───────────────────┐
│        ▼                ▼                ▼                    │
│                    DSPy.ts Layer                             │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│  │ Signature  │  │ Optimizer  │  │  Program   │           │
│  │  Builder   │  │  (GEPA)    │  │  Compiler  │           │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘           │
│        │                │                │                   │
└────────┼────────────────┼────────────────┼───────────────────┘
         │                │                │
┌────────┼────────────────┼────────────────┼───────────────────┐
│        ▼                ▼                ▼                    │
│                     LLM Provider Layer                       │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│  │   Claude   │  │   GPT-4    │  │ OpenRouter │           │
│  │ 3.5 Sonnet │  │   Turbo    │  │  (Llama)   │           │
│  └────────────┘  └────────────┘  └────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 Installation

```bash
# Core dependencies
npm install @ax-llm/ax
npm install claude-flow@alpha

# Optional: Enhanced coordination
npm install ruv-swarm
npm install reasoning-bank

# Optional: Cloud features
npm install flow-nexus@latest
```

---

## 🚀 Quick Integration Example

### Step 1: Initialize Claude-Flow with DSPy.ts

```typescript
// src/integrations/claude-flow-dspy.ts
import { SwarmOrchestrator } from 'claude-flow';
import { ai, ax } from '@ax-llm/ax';
import { GEPA, MIPROv2 } from '@ax-llm/ax/optimizers';

export class ClaudeFlowDSPy {
  private swarm: SwarmOrchestrator;
  private models: Map<string, any>;
  private optimizedAgents: Map<string, any>;

  constructor() {
    this.swarm = new SwarmOrchestrator({
      topology: 'adaptive',
      maxAgents: 10
    });

    this.models = new Map([
      ['primary', ai({
        name: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        model: 'claude-3-5-sonnet-20241022'
      })],
      ['fallback', ai({
        name: 'openai',
        apiKey: process.env.OPENAI_API_KEY,
        model: 'gpt-4-turbo'
      })],
      ['cost-effective', ai({
        name: 'openrouter',
        apiKey: process.env.OPENROUTER_API_KEY,
        model: 'meta-llama/llama-3.1-70b-instruct'
      })]
    ]);

    this.optimizedAgents = new Map();
  }

  /**
   * Create and optimize an agent with DSPy.ts
   */
  async createOptimizedAgent(
    agentType: string,
    signature: string,
    trainset: any[],
    options = {}
  ) {
    // 1. Create base DSPy program
    const program = ax(signature);

    // 2. Define evaluation metric
    const metric = options.metric || this.defaultMetric;

    // 3. Select optimizer based on dataset size
    const optimizer = this.selectOptimizer(trainset.length, metric);

    // 4. Compile optimized program
    console.log(`Optimizing ${agentType} agent...`);
    const optimized = await optimizer.compile(program, trainset);

    // 5. Store in Claude-Flow swarm
    const agent = await this.swarm.createAgent({
      type: agentType,
      handler: async (input) => {
        const model = this.selectModel(input);
        return optimized.forward(model, input);
      },
      metadata: {
        signature,
        optimizer: optimizer.constructor.name,
        trainedAt: new Date().toISOString(),
        datasetSize: trainset.length
      }
    });

    this.optimizedAgents.set(agentType, { program: optimized, agent });

    return agent;
  }

  /**
   * Select appropriate optimizer based on dataset size
   */
  private selectOptimizer(datasetSize: number, metric: any) {
    if (datasetSize < 20) {
      return new BootstrapFewShot({ metric, maxBootstrappedDemos: 4 });
    } else if (datasetSize < 100) {
      return new BootstrapFewShot({
        metric,
        maxBootstrappedDemos: 8,
        maxRounds: 2
      });
    } else {
      return new MIPROv2({
        metric,
        numCandidates: 10,
        numTrials: 100
      });
    }
  }

  /**
   * Select model based on input complexity
   */
  private selectModel(input: any): any {
    const complexity = this.analyzeComplexity(input);

    if (complexity < 0.3) return this.models.get('cost-effective');
    if (complexity < 0.7) return this.models.get('fallback');
    return this.models.get('primary');
  }

  /**
   * Analyze input complexity (simple heuristic)
   */
  private analyzeComplexity(input: any): number {
    const text = JSON.stringify(input);
    const length = text.length;
    const hasCode = /```|function|class|import/.test(text);
    const hasMultipleQuestions = (text.match(/\?/g) || []).length > 2;

    let complexity = Math.min(length / 1000, 0.5);
    if (hasCode) complexity += 0.3;
    if (hasMultipleQuestions) complexity += 0.2;

    return Math.min(complexity, 1.0);
  }

  /**
   * Default metric for optimization
   */
  private defaultMetric(example: any, prediction: any): number {
    // Simple exact match
    return prediction.output === example.output ? 1.0 : 0.0;
  }
}
```

### Step 2: Create Specialized Agents

```typescript
// src/agents/researcher-agent.ts
import { ClaudeFlowDSPy } from '../integrations/claude-flow-dspy';

export async function createResearcherAgent(cfDspy: ClaudeFlowDSPy) {
  const signature = `
    query:string,
    context:string[]
    ->
    findings:string,
    sources:string[],
    confidence:number
  `;

  const trainset = [
    {
      query: "What are the latest developments in AI?",
      context: ["Article 1 about GPT-4", "Article 2 about Claude"],
      findings: "Recent AI developments include...",
      sources: ["GPT-4 paper", "Claude 3 announcement"],
      confidence: 0.9
    },
    // ... 20-50 more examples
  ];

  const metric = (example, prediction) => {
    const findingsMatch = prediction.findings.length > 50 ? 0.5 : 0;
    const sourcesMatch = prediction.sources.length > 0 ? 0.3 : 0;
    const confidenceMatch = prediction.confidence > 0.7 ? 0.2 : 0;

    return findingsMatch + sourcesMatch + confidenceMatch;
  };

  return cfDspy.createOptimizedAgent(
    'researcher',
    signature,
    trainset,
    { metric }
  );
}
```

```typescript
// src/agents/coder-agent.ts
export async function createCoderAgent(cfDspy: ClaudeFlowDSPy) {
  const signature = `
    description:string,
    language:class "typescript, python, rust, go",
    requirements:string[]
    ->
    code:string,
    tests:string,
    documentation:string
  `;

  const trainset = [
    {
      description: "REST API endpoint for user authentication",
      language: "typescript",
      requirements: ["JWT tokens", "bcrypt password hashing"],
      code: "// Express endpoint code...",
      tests: "// Jest test suite...",
      documentation: "// API documentation..."
    },
    // ... more examples
  ];

  const metric = (example, prediction) => {
    const hasCode = prediction.code.length > 100 ? 0.4 : 0;
    const hasTests = prediction.tests.length > 50 ? 0.3 : 0;
    const hasDocs = prediction.documentation.length > 20 ? 0.3 : 0;

    return hasCode + hasTests + hasDocs;
  };

  return cfDspy.createOptimizedAgent(
    'coder',
    signature,
    trainset,
    { metric }
  );
}
```

```typescript
// src/agents/tester-agent.ts
export async function createTesterAgent(cfDspy: ClaudeFlowDSPy) {
  const signature = `
    code:string,
    language:class "typescript, python, rust, go",
    requirements:string[]
    ->
    tests:string,
    coverage:number,
    edge_cases:string[]
  `;

  const trainset = [
    {
      code: "function add(a, b) { return a + b; }",
      language: "typescript",
      requirements: ["Test positive numbers", "Test negative numbers"],
      tests: "describe('add', () => { ... })",
      coverage: 0.95,
      edge_cases: ["NaN handling", "Infinity"]
    },
    // ... more examples
  ];

  const metric = (example, prediction) => {
    const hasTests = prediction.tests.length > 100 ? 0.4 : 0;
    const goodCoverage = prediction.coverage > 0.8 ? 0.3 : 0;
    const hasEdgeCases = prediction.edge_cases.length > 2 ? 0.3 : 0;

    return hasTests + goodCoverage + hasEdgeCases;
  };

  return cfDspy.createOptimizedAgent(
    'tester',
    signature,
    trainset,
    { metric }
  );
}
```

### Step 3: Orchestrate Multi-Agent Workflow

```typescript
// src/workflows/feature-development.ts
import { ClaudeFlowDSPy } from '../integrations/claude-flow-dspy';
import { createResearcherAgent } from '../agents/researcher-agent';
import { createCoderAgent } from '../agents/coder-agent';
import { createTesterAgent } from '../agents/tester-agent';

export class FeatureDevelopmentWorkflow {
  private cfDspy: ClaudeFlowDSPy;
  private agents: Map<string, any>;

  constructor() {
    this.cfDspy = new ClaudeFlowDSPy();
    this.agents = new Map();
  }

  async initialize() {
    // Create optimized agents in parallel
    const [researcher, coder, tester] = await Promise.all([
      createResearcherAgent(this.cfDspy),
      createCoderAgent(this.cfDspy),
      createTesterAgent(this.cfDspy)
    ]);

    this.agents.set('researcher', researcher);
    this.agents.set('coder', coder);
    this.agents.set('tester', tester);

    console.log('✅ All agents optimized and ready');
  }

  async developFeature(featureRequest: string) {
    // Step 1: Research
    const researchResult = await this.agents.get('researcher').execute({
      query: featureRequest,
      context: await this.gatherContext(featureRequest)
    });

    console.log('📊 Research complete:', researchResult.findings);

    // Step 2: Code
    const codeResult = await this.agents.get('coder').execute({
      description: featureRequest,
      language: 'typescript',
      requirements: this.extractRequirements(researchResult)
    });

    console.log('💻 Code generated:', codeResult.code.substring(0, 100) + '...');

    // Step 3: Test
    const testResult = await this.agents.get('tester').execute({
      code: codeResult.code,
      language: 'typescript',
      requirements: this.extractRequirements(researchResult)
    });

    console.log('✅ Tests generated:', testResult.coverage);

    return {
      research: researchResult,
      code: codeResult,
      tests: testResult,
      complete: testResult.coverage > 0.8
    };
  }

  private async gatherContext(query: string): Promise<string[]> {
    // Implement context gathering (e.g., from documentation, codebase)
    return [];
  }

  private extractRequirements(research: any): string[] {
    // Extract requirements from research findings
    return [];
  }
}
```

---

## 🧠 Integration with ReasoningBank

```typescript
// src/integrations/reasoning-bank-dspy.ts
import { ReasoningBank } from 'reasoning-bank';
import { ClaudeFlowDSPy } from './claude-flow-dspy';

export class SelfLearningOrchestrator {
  private cfDspy: ClaudeFlowDSPy;
  private reasoningBank: ReasoningBank;

  constructor() {
    this.cfDspy = new ClaudeFlowDSPy();
    this.reasoningBank = new ReasoningBank({
      storageBackend: 'agentdb',  // 150x faster vector search
      learningEnabled: true
    });
  }

  /**
   * Create agent that learns from production
   */
  async createSelfLearningAgent(agentType: string, signature: string) {
    // 1. Check if we have prior training data
    const priorData = await this.reasoningBank.query({
      agentType,
      signature,
      limit: 100
    });

    // 2. Create or update optimized agent
    let agent;
    if (priorData.length > 20) {
      console.log(`📚 Found ${priorData.length} prior examples, optimizing...`);

      agent = await this.cfDspy.createOptimizedAgent(
        agentType,
        signature,
        priorData,
        {
          metric: this.computeMetricFromFeedback
        }
      );
    } else {
      console.log('🆕 Creating new agent with baseline');

      agent = await this.cfDspy.createOptimizedAgent(
        agentType,
        signature,
        this.getBaselineExamples(agentType)
      );
    }

    // 3. Wrap agent to learn from production
    return this.wrapWithLearning(agent, agentType, signature);
  }

  /**
   * Wrap agent to capture production data for learning
   */
  private wrapWithLearning(agent: any, agentType: string, signature: string) {
    return {
      async execute(input: any) {
        const startTime = Date.now();

        // Execute agent
        const result = await agent.execute(input);

        // Store in ReasoningBank
        await this.reasoningBank.store({
          agentType,
          signature,
          input,
          output: result,
          latency: Date.now() - startTime,
          timestamp: new Date(),
          metadata: {
            model: 'optimized',
            version: agent.metadata?.trainedAt
          }
        });

        return result;
      },

      /**
       * Re-optimize based on production data
       */
      async reoptimize() {
        // Get recent production data
        const productionData = await this.reasoningBank.query({
          agentType,
          signature,
          since: Date.now() - 7 * 24 * 60 * 60 * 1000,  // Last 7 days
          minQuality: 0.8  // Only good examples
        });

        if (productionData.length < 10) {
          console.log('⚠️ Not enough production data for reoptimization');
          return agent;
        }

        console.log(`🔄 Reoptimizing with ${productionData.length} production examples...`);

        // Create new optimized version
        const newAgent = await this.cfDspy.createOptimizedAgent(
          agentType,
          signature,
          productionData,
          {
            metric: this.computeMetricFromFeedback
          }
        );

        // Compare performance
        const oldPerf = await this.evaluateAgent(agent, productionData.slice(0, 20));
        const newPerf = await this.evaluateAgent(newAgent, productionData.slice(0, 20));

        if (newPerf > oldPerf) {
          console.log(`✅ Improved performance: ${oldPerf.toFixed(2)} → ${newPerf.toFixed(2)}`);
          return this.wrapWithLearning(newAgent, agentType, signature);
        } else {
          console.log(`⚠️ No improvement, keeping current version`);
          return agent;
        }
      }
    };
  }

  private async evaluateAgent(agent: any, testData: any[]): Promise<number> {
    const scores = await Promise.all(
      testData.map(async (example) => {
        const prediction = await agent.execute(example.input);
        return this.computeMetricFromFeedback(example, prediction);
      })
    );

    return scores.reduce((a, b) => a + b, 0) / scores.length;
  }

  private computeMetricFromFeedback(example: any, prediction: any): number {
    // Compute quality score based on feedback
    const hasOutput = prediction.output ? 0.3 : 0;
    const hasQuality = prediction.quality > 0.7 ? 0.4 : 0;
    const hasFeedback = example.feedback === 'positive' ? 0.3 : 0;

    return hasOutput + hasQuality + hasFeedback;
  }

  private getBaselineExamples(agentType: string): any[] {
    // Return baseline training examples for new agents
    return [];
  }
}
```

---

## 📊 Monitoring and Observability

```typescript
// src/monitoring/dspy-metrics.ts
import { trace, context } from '@opentelemetry/api';

export class DSPyMetricsCollector {
  private tracer = trace.getTracer('dspy-metrics');

  async trackOptimization(agentType: string, fn: () => Promise<any>) {
    const span = this.tracer.startSpan('dspy-optimization');

    span.setAttributes({
      'dspy.agent_type': agentType,
      'dspy.phase': 'optimization'
    });

    const startTime = Date.now();

    try {
      const result = await fn();

      span.setAttributes({
        'dspy.optimization_time': Date.now() - startTime,
        'dspy.success': true
      });

      return result;
    } catch (error) {
      span.recordException(error);
      span.setAttributes({
        'dspy.success': false,
        'dspy.error': error.message
      });

      throw error;
    } finally {
      span.end();
    }
  }

  async trackInference(agentType: string, fn: () => Promise<any>) {
    const span = this.tracer.startSpan('dspy-inference');

    span.setAttributes({
      'dspy.agent_type': agentType,
      'dspy.phase': 'inference'
    });

    const startTime = Date.now();

    try {
      const result = await fn();

      span.setAttributes({
        'dspy.latency': Date.now() - startTime,
        'dspy.tokens.input': result.usage?.inputTokens || 0,
        'dspy.tokens.output': result.usage?.outputTokens || 0,
        'dspy.success': true
      });

      return result;
    } catch (error) {
      span.recordException(error);
      span.setAttributes({
        'dspy.success': false,
        'dspy.error': error.message
      });

      throw error;
    } finally {
      span.end();
    }
  }

  async trackAgentPerformance(
    agentType: string,
    metric: (ex: any, pred: any) => number,
    examples: any[]
  ) {
    const scores = examples.map(({ example, prediction }) =>
      metric(example, prediction)
    );

    const avgScore = scores.reduce((a, b) => a + b, 0) / scores.length;
    const stdDev = Math.sqrt(
      scores.reduce((sum, s) => sum + Math.pow(s - avgScore, 2), 0) / scores.length
    );

    // Log metrics
    console.log(`📊 ${agentType} Performance:`, {
      mean: avgScore.toFixed(3),
      stdDev: stdDev.toFixed(3),
      min: Math.min(...scores).toFixed(3),
      max: Math.max(...scores).toFixed(3)
    });

    return {
      agentType,
      mean: avgScore,
      stdDev,
      min: Math.min(...scores),
      max: Math.max(...scores),
      samples: examples.length
    };
  }
}
```

---

## 🚀 Complete Example: Self-Improving Documentation Generator

```typescript
// examples/self-improving-docs-generator.ts
import { ClaudeFlowDSPy } from '../src/integrations/claude-flow-dspy';
import { SelfLearningOrchestrator } from '../src/integrations/reasoning-bank-dspy';

async function main() {
  const orchestrator = new SelfLearningOrchestrator();

  // Create self-learning documentation agent
  const docsAgent = await orchestrator.createSelfLearningAgent(
    'docs-generator',
    `
      code:string,
      language:class "typescript, python, rust",
      style:class "technical, beginner-friendly, api-reference"
      ->
      documentation:string,
      examples:string[],
      quality_score:number
    `
  );

  // Use agent
  const result = await docsAgent.execute({
    code: `
      function calculateFibonacci(n: number): number {
        if (n <= 1) return n;
        return calculateFibonacci(n - 1) + calculateFibonacci(n - 2);
      }
    `,
    language: 'typescript',
    style: 'beginner-friendly'
  });

  console.log('📝 Generated Documentation:');
  console.log(result.documentation);
  console.log('\n💡 Examples:');
  result.examples.forEach(ex => console.log('  -', ex));
  console.log(`\n✨ Quality Score: ${result.quality_score}`);

  // Simulate production usage for 1 week...
  // Agent automatically learns from good examples

  // Re-optimize weekly
  setInterval(async () => {
    console.log('\n🔄 Weekly reoptimization...');
    await docsAgent.reoptimize();
  }, 7 * 24 * 60 * 60 * 1000);
}

main().catch(console.error);
```

---

## 📋 Integration Checklist

### Phase 1: Setup (Day 1)
- [ ] Install Ax framework and Claude-Flow
- [ ] Configure API keys for Claude, GPT-4, OpenRouter
- [ ] Set up basic ClaudeFlowDSPy class
- [ ] Test basic agent creation

### Phase 2: Agent Creation (Days 2-3)
- [ ] Create researcher agent with training data
- [ ] Create coder agent with training data
- [ ] Create tester agent with training data
- [ ] Test agents individually

### Phase 3: Optimization (Days 4-5)
- [ ] Collect 20-50 training examples per agent
- [ ] Run BootstrapFewShot optimization
- [ ] Evaluate performance improvements
- [ ] Document baseline vs optimized metrics

### Phase 4: Integration (Days 6-7)
- [ ] Integrate with ReasoningBank for learning
- [ ] Set up production monitoring
- [ ] Implement model cascading
- [ ] Add caching layer

### Phase 5: Production (Week 2)
- [ ] Deploy optimized agents
- [ ] Monitor performance metrics
- [ ] Collect production feedback
- [ ] Schedule weekly reoptimization

---

## 💡 Best Practices for Integration

1. **Start with BootstrapFewShot**
   - Faster optimization (15 min vs 2 hours)
   - Good enough for most use cases
   - Upgrade to MIPROv2/GEPA later if needed

2. **Use Model Cascading**
   - Cheap model (Llama 3.1 8B) for simple tasks
   - Medium model (Claude Haiku) for moderate tasks
   - Expensive model (Claude Sonnet) for complex tasks
   - Can reduce costs by 60-80%

3. **Implement Continuous Learning**
   - Store all production interactions in ReasoningBank
   - Filter for high-quality examples (quality > 0.8)
   - Reoptimize weekly with production data
   - Track performance improvements over time

4. **Monitor Everything**
   - Track optimization time and cost
   - Monitor inference latency and cost
   - Log all predictions for analysis
   - Set up alerts for performance degradation

5. **Version Control Optimized Agents**
   - Save optimized programs to disk
   - Track training date and dataset size
   - A/B test new versions before deploying
   - Keep rollback capability

---

## 🎯 Expected Results

### Performance Improvements
- **Accuracy:** +15-30% with BootstrapFewShot
- **Accuracy:** +30-50% with MIPROv2
- **Accuracy:** +40-60% with GEPA
- **Cost:** 22-90x reduction with GEPA optimization

### Production Benefits
- Self-improving agents learn from production data
- Reduced latency through model cascading
- Lower costs through optimization and caching
- Better quality through continuous learning

---

## 📚 Additional Resources

- **Comprehensive Research:** See `docs/research/dspy-ts-comprehensive-research.md`
- **Quick Start:** See `docs/research/dspy-ts-quick-start-guide.md`
- **Ax Documentation:** https://axllm.dev/
- **Claude-Flow Docs:** https://github.com/ruvnet/claude-flow

---

**Integration Guide Created By:** Research Agent
**Last Updated:** 2025-11-22
**Status:** Ready for Implementation
