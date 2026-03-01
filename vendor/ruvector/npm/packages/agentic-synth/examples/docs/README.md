# AgenticSynth Examples

Comprehensive examples demonstrating AgenticSynth's capabilities for synthetic data generation, DSPy integration, and agentic workflows.

## 📚 Table of Contents

- [Quick Start](#quick-start)
- [Core Examples](#core-examples)
- [DSPy Integration](#dspy-integration)
- [Specialized Examples](#specialized-examples)
- [Testing](#testing)
- [Configuration](#configuration)

## 🚀 Quick Start

### Prerequisites

```bash
# Node.js version
node >= 18.0.0

# Environment setup
cp .env.example .env
# Edit .env with your API keys
```

### Basic Usage

```bash
# Install dependencies
npm install

# Build the package
npm run build

# Run an example
npx tsx examples/basic-usage.ts
```

## 📖 Core Examples

### 1. Basic Usage (`basic-usage.ts`)

**Purpose**: Introduction to AgenticSynth's core functionality

**Features**:
- Structured data generation
- Time-series generation
- Event generation
- Streaming support
- Batch processing

**Run**:
```bash
export GEMINI_API_KEY=...
npx tsx examples/basic-usage.ts
```

### 2. Integration Examples (`integration-examples.ts`)

**Purpose**: Real-world integration patterns

**Features**:
- Vector database integration (AgenticDB)
- Streaming with Midstreamer
- Robotics simulation
- Multi-provider orchestration

**Run**:
```bash
npx tsx examples/integration-examples.ts
```

### 3. Benchmark Example (`benchmark-example.ts`)

**Purpose**: Performance testing and comparison

**Features**:
- Provider comparison (Gemini, OpenRouter, Claude)
- Latency measurement
- Token usage tracking
- Quality assessment

**Run**:
```bash
npx tsx examples/benchmark-example.ts
```

## 🧠 DSPy Integration

### DSPy Complete Example (`dspy-complete-example.ts`) ⭐ NEW

**Purpose**: Production-ready DSPy.ts + AgenticSynth integration

**What It Does**:
1. Generates baseline e-commerce product data with AgenticSynth
2. Sets up DSPy ChainOfThought reasoning module
3. Uses BootstrapFewShot to learn from high-quality examples
4. Compares baseline vs optimized results
5. Generates detailed quality metrics and reports

**Key Features**:
- ✅ Real DSPy.ts v2.1.1 modules (ChainOfThought, BootstrapFewShot)
- ✅ Integration with AgenticSynth for baseline generation
- ✅ Quality metrics (completeness, coherence, persuasiveness, SEO)
- ✅ Cost and performance comparison
- ✅ Production-ready error handling
- ✅ Comprehensive documentation

**Run**:
```bash
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...
npx tsx examples/dspy-complete-example.ts
```

**Expected Results**:
- Baseline Quality: ~68%
- Optimized Quality: ~84%
- Quality Improvement: +23.6%
- Cost Efficiency: +14.8%

**Documentation**: See [dspy-complete-example-guide.md](./docs/dspy-complete-example-guide.md)

### DSPy Training Example (`dspy-training-example.ts`)

**Purpose**: Multi-model DSPy training framework

**Features**:
- Multi-model training sessions
- Automatic prompt optimization
- Cross-model learning
- Cost-optimized training
- Quality-focused training
- Benchmark comparison

**Run**:
```bash
# Run specific example (0-4)
npx tsx examples/dspy-training-example.ts 0
```

### Verify DSPy Setup (`dspy-verify-setup.ts`)

**Purpose**: Pre-flight checks before running DSPy examples

**Run**:
```bash
npx tsx examples/dspy-verify-setup.ts
```

## 🎯 Specialized Examples

### Business & Finance

#### Ad ROAS Optimization (`ad-roas/`)
- `ad-campaign-optimizer.ts` - Campaign optimization
- `roas-benchmark.ts` - ROAS benchmarking
- `multi-channel-optimizer.ts` - Multi-channel campaigns

#### Stock Market (`stocks/`)
- `stock-data-generator.ts` - Market data generation
- `portfolio-simulator.ts` - Portfolio simulation
- `risk-analyzer.ts` - Risk analysis

#### Crypto (`crypto/`)
- `crypto-market-generator.ts` - Crypto market data
- `defi-simulator.ts` - DeFi simulation
- `nft-metadata-generator.ts` - NFT metadata

### Enterprise

#### Business Management (`business-management/`)
- `crm-data-generator.ts` - CRM data
- `inventory-simulator.ts` - Inventory management
- `supply-chain-simulator.ts` - Supply chain

#### Employee Simulation (`employee-simulation/`)
- `employee-generator.ts` - Employee profiles
- `performance-simulator.ts` - Performance tracking
- `org-chart-generator.ts` - Organization charts

### Development

#### CI/CD (`cicd/`)
- `pipeline-generator.ts` - Pipeline configuration
- `test-data-generator.ts` - Test data
- `deployment-simulator.ts` - Deployment simulation

#### Security (`security/`)
- `security-audit-generator.ts` - Security audits
- `threat-simulator.ts` - Threat simulation
- `compliance-checker.ts` - Compliance checks

### AI & Learning

#### Self-Learning (`self-learning/`)
- `pattern-learner.ts` - Pattern recognition
- `adaptive-generator.ts` - Adaptive generation
- `feedback-optimizer.ts` - Feedback optimization

#### Agentic Jujutsu (`agentic-jujutsu/`)
- `version-control-integration.ts` - VCS integration
- `multi-agent-coordination.ts` - Agent coordination
- `self-learning-commit.ts` - Self-learning commits

### Swarms (`swarms/`)
- `multi-agent-generator.ts` - Multi-agent systems
- `swarm-coordinator.ts` - Swarm coordination
- `consensus-builder.ts` - Consensus mechanisms

## 🧪 Testing

### Run All Examples

```bash
npx tsx examples/test-all-examples.ts
```

### Run Specific Category

```bash
# Business examples
npx tsx examples/test-all-examples.ts --category business

# DSPy examples
npx tsx examples/test-all-examples.ts --category dspy

# Integration examples
npx tsx examples/test-all-examples.ts --category integration
```

### Run Unit Tests

```bash
npm run test:unit
```

## ⚙️ Configuration

### Environment Variables

Create a `.env` file in the package root:

```bash
# Required for most examples
GEMINI_API_KEY=...

# Required for DSPy examples
OPENAI_API_KEY=sk-...

# Optional
ANTHROPIC_API_KEY=sk-ant-...
OPENROUTER_API_KEY=sk-or-...
TOGETHER_API_KEY=...

# Database (optional)
AGENTDB_PATH=./data/agentdb
REDIS_URL=redis://localhost:6379
```

### Common Configuration Patterns

#### Provider Selection

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

// Gemini (Fast, cost-effective)
const synthGemini = new AgenticSynth({
  provider: 'gemini',
  model: 'gemini-2.0-flash-exp'
});

// OpenRouter (Access to many models)
const synthOpenRouter = new AgenticSynth({
  provider: 'openrouter',
  model: 'anthropic/claude-3.5-sonnet'
});

// Claude (High quality)
const synthClaude = new AgenticSynth({
  provider: 'anthropic',
  model: 'claude-3-5-sonnet-20241022'
});
```

#### Caching

```typescript
// Memory cache (default)
const synth = new AgenticSynth({
  cacheStrategy: 'memory',
  cacheTTL: 3600
});

// Redis cache (for distributed systems)
const synth = new AgenticSynth({
  cacheStrategy: 'redis',
  cacheTTL: 3600,
  redisUrl: process.env.REDIS_URL
});
```

#### Streaming

```typescript
// Enable streaming
const synth = new AgenticSynth({
  streaming: true
});

// Use streaming
for await (const item of synth.generateStream('structured', options)) {
  console.log('Generated:', item);
}
```

## 📊 Example Comparison

| Example | Complexity | API Keys Required | Output | Use Case |
|---------|-----------|-------------------|---------|----------|
| basic-usage | ⭐ | GEMINI | Console | Learning basics |
| dspy-complete-example | ⭐⭐⭐ | OPENAI, GEMINI | JSON + Report | Production DSPy |
| dspy-training-example | ⭐⭐⭐ | Multiple | Metrics | Model training |
| integration-examples | ⭐⭐ | GEMINI | Console | Integrations |
| benchmark-example | ⭐⭐ | Multiple | Metrics | Performance |
| ad-roas | ⭐⭐ | GEMINI | JSON | Marketing |
| stocks | ⭐⭐ | GEMINI | JSON | Finance |
| employee-simulation | ⭐ | GEMINI | JSON | HR |

## 🎓 Learning Path

### Beginner
1. Start with `basic-usage.ts`
2. Review `benchmark-example.ts`
3. Try a specialized example (e.g., `employee-generator.ts`)

### Intermediate
1. Review `integration-examples.ts`
2. Try `dspy-verify-setup.ts`
3. Run `dspy-complete-example.ts`
4. Experiment with different categories

### Advanced
1. Study `dspy-training-example.ts`
2. Implement custom DSPy modules
3. Build multi-agent systems with swarms
4. Integrate with AgenticDB and vector databases

## 🔧 Troubleshooting

### Common Issues

#### Import Errors

```bash
Error: Cannot find module '@ruvector/agentic-synth'
```

**Solution**: Build the package
```bash
npm run build
```

#### API Key Errors

```bash
Error: Missing API key
```

**Solution**: Set environment variables
```bash
export GEMINI_API_KEY=...
```

#### Module Not Found (DSPy)

```bash
Error: Cannot find module 'dspy.ts'
```

**Solution**: Install dependencies
```bash
npm install
```

#### TypeScript Errors

```bash
Error: Cannot find type definitions
```

**Solution**: Check TypeScript version
```bash
npm run typecheck
```

### Getting Help

1. Check the specific example's documentation
2. Review the main [README.md](../README.md)
3. Open an issue on [GitHub](https://github.com/ruvnet/ruvector/issues)
4. Join the [Discord](https://discord.gg/ruvector)

## 📝 Contributing

Want to add an example?

1. Create a new file in the appropriate category
2. Follow the existing patterns
3. Add comprehensive comments
4. Update this README
5. Submit a PR

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.

## 📄 License

MIT License - See [LICENSE](../LICENSE) file for details.

## 🙏 Credits

Built with ❤️ by [rUv](https://github.com/ruvnet)

Special thanks to:
- Stanford's DSPy team
- AgenticDB contributors
- The open-source community

---

**Need help?** Open an issue or join our [Discord](https://discord.gg/ruvector)
