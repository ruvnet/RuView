# 🎯 Agentic-Synth Examples - Comprehensive Guide

**Version**: 0.1.0
**Last Updated**: 2025-11-22
**Total Examples**: 50+
**Total Categories**: 12

---

## 📋 Quick Reference Index

| Category | Description | Difficulty | Files | NPX Command |
|----------|-------------|------------|-------|-------------|
| [Basic Usage](#basic-usage) | Core functionality demos | Beginner | 1 | `npx tsx examples/basic-usage.ts` |
| [CI/CD Automation](#cicd-automation) | Test data generation | Intermediate | 2 | `npx tsx examples/cicd/test-data-generator.ts` |
| [Self-Learning](#self-learning-systems) | RL & feedback loops | Advanced | 3 | `npx tsx examples/self-learning/reinforcement-learning.ts` |
| [Ad ROAS](#ad-roas-optimization) | Marketing analytics | Intermediate | 3 | `npx tsx examples/ad-roas/campaign-data.ts` |
| [Stock Market](#stock-market-simulation) | Financial trading | Intermediate | 3 | `npx tsx examples/stocks/market-data.ts` |
| [Cryptocurrency](#cryptocurrency-trading) | Crypto & DeFi | Intermediate | 3 | `npx tsx examples/crypto/blockchain-data.ts` |
| [Log Analytics](#log-analytics) | Monitoring & security | Intermediate | 4 | `npx tsx examples/logs/application-logs.ts` |
| [Security Testing](#security-testing) | Penetration testing | Advanced | 4 | `npx tsx examples/security/vulnerability-testing.ts` |
| [Swarm Coordination](#swarm-coordination) | Multi-agent systems | Advanced | 4 | `npx tsx examples/swarms/agent-coordination.ts` |
| [Business Management](#business-management) | ERP, CRM, HR | Intermediate | 5 | `npx tsx examples/business-management/erp-data.ts` |
| [Employee Simulation](#employee-simulation) | Workforce modeling | Intermediate | 5 | `npx tsx examples/employee-simulation/workforce-behavior.ts` |
| [Agentic-Jujutsu](#agentic-jujutsu-integration) | Version control | Advanced | 6 | `npx tsx examples/agentic-jujutsu/collaborative-workflows.ts` |
| [DSPy Integration](#dspy-integration) | Neural optimization | Advanced | 3 | `npx tsx examples/dspy-complete-example.ts` |

---

## 📚 Table of Contents

1. [Installation & Setup](#installation--setup)
2. [Basic Usage](#basic-usage)
3. [Example Categories](#example-categories)
   - [CI/CD Automation](#cicd-automation)
   - [Self-Learning Systems](#self-learning-systems)
   - [Ad ROAS Optimization](#ad-roas-optimization)
   - [Stock Market Simulation](#stock-market-simulation)
   - [Cryptocurrency Trading](#cryptocurrency-trading)
   - [Log Analytics](#log-analytics)
   - [Security Testing](#security-testing)
   - [Swarm Coordination](#swarm-coordination)
   - [Business Management](#business-management)
   - [Employee Simulation](#employee-simulation)
   - [Agentic-Jujutsu Integration](#agentic-jujutsu-integration)
   - [DSPy Integration](#dspy-integration)
4. [Integration Patterns](#integration-patterns)
5. [Performance Tips](#performance-tips)
6. [Troubleshooting](#troubleshooting)

---

## Installation & Setup

### Prerequisites

```bash
# Node.js version
node --version  # >= 18.0.0

# Install dependencies
cd /home/user/ruvector/packages/agentic-synth
npm install

# Set API key
export GEMINI_API_KEY=your-gemini-api-key-here
# OR
export OPENROUTER_API_KEY=your-openrouter-key
```

### Quick Start

```bash
# Run any example
npx tsx examples/basic-usage.ts

# Run with custom config
GEMINI_API_KEY=your-key npx tsx examples/stocks/market-data.ts

# Run all examples in a category
npx tsx examples/test-all-examples.ts
```

---

## Basic Usage

**Difficulty**: Beginner
**Files**: `basic-usage.ts`
**Purpose**: Learn core agentic-synth functionality

### What It Demonstrates

- Time-series data generation
- Event stream generation
- Structured data with schemas
- Streaming generation
- Batch processing
- Provider switching (Gemini/OpenRouter)
- Caching strategies
- Error handling

### Quick Start

```bash
npx tsx examples/basic-usage.ts
```

### Code Examples

#### 1. Generate Time-Series Data

```typescript
import { createSynth } from '@ruvector/agentic-synth';

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY
});

const result = await synth.generateTimeSeries({
  count: 100,
  interval: '1h',
  metrics: ['temperature', 'humidity'],
  trend: 'up',
  seasonality: true
});

console.log(result.data.slice(0, 5));
```

#### 2. Generate Events

```typescript
const events = await synth.generateEvents({
  count: 50,
  eventTypes: ['page_view', 'button_click', 'form_submit'],
  distribution: 'poisson',
  userCount: 25,
  timeRange: {
    start: new Date(Date.now() - 24 * 60 * 60 * 1000),
    end: new Date()
  }
});
```

#### 3. Structured Data with Schema

```typescript
const schema = {
  id: { type: 'string', required: true },
  name: { type: 'string', required: true },
  email: { type: 'string', required: true },
  age: { type: 'number', required: true },
  address: {
    type: 'object',
    properties: {
      street: { type: 'string' },
      city: { type: 'string' }
    }
  }
};

const users = await synth.generateStructured({
  count: 20,
  schema,
  format: 'json'
});
```

### Configuration

```typescript
const synth = createSynth({
  provider: 'gemini',              // or 'openrouter'
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory',         // 'memory' | 'redis' | 'none'
  cacheTTL: 3600,                  // seconds
  maxRetries: 3,
  timeout: 30000,                  // ms
  streaming: false
});
```

---

## Example Categories

---

## CI/CD Automation

**Difficulty**: Intermediate
**Files**: `cicd/test-data-generator.ts`, `cicd/pipeline-testing.ts`
**Real-World Use**: Generate test data for continuous integration pipelines

### What It Demonstrates

- Database fixtures generation
- API mock responses
- User session data
- Load testing datasets (100K+ requests)
- Multi-environment configs
- Reproducible test data with seeds

### Files Included

| File | Purpose | Records |
|------|---------|---------|
| `test-data-generator.ts` | Comprehensive test data generator class | Variable |
| `pipeline-testing.ts` | Pipeline-specific test scenarios | 1000+ |

### Quick Start

```bash
# Generate database fixtures
npx tsx examples/cicd/test-data-generator.ts

# Pipeline testing data
npx tsx examples/cicd/pipeline-testing.ts

# Custom configuration
GEMINI_API_KEY=key npx tsx examples/cicd/test-data-generator.ts
```

### Configuration

```typescript
const generator = new CICDTestDataGenerator({
  outputDir: './test-fixtures',
  format: 'json',                    // 'json' | 'csv' | 'array'
  provider: 'gemini',
  seed: process.env.CI_COMMIT_SHA    // Reproducible with git SHA
});
```

### Code Examples

#### Generate Database Fixtures

```typescript
import { CICDTestDataGenerator } from './examples/cicd/test-data-generator';

const generator = new CICDTestDataGenerator({
  outputDir: './test-data',
  seed: 'fixed-seed-for-reproducibility'
});

await generator.generateDatabaseFixtures({
  users: 100,
  posts: 500,
  comments: 1500,
  orders: 200,
  products: 150
});
```

#### Generate Load Test Data

```typescript
const loadTestData = await generator.generateLoadTestData({
  requestCount: 100000,
  concurrent: 100,
  duration: 10  // minutes
});

console.log(`Generated ${loadTestData.data.length} requests`);
```

#### GitHub Actions Integration

```yaml
# .github/workflows/test-data.yml
- name: Generate Test Data
  run: |
    export GEMINI_API_KEY=${{ secrets.GEMINI_API_KEY }}
    npx tsx examples/cicd/test-data-generator.ts

- name: Run Tests with Generated Data
  run: npm test
```

### Real-World Use Cases

1. **Database Seeding**: Generate realistic user, product, order data
2. **API Testing**: Create mock responses for integration tests
3. **Load Testing**: 100K+ requests for performance benchmarks
4. **E2E Testing**: User session data with authentication
5. **Multi-Environment**: Dev/staging/prod config variations

### Key Features

- ✅ Reproducible with seed values
- ✅ Foreign key relationships maintained
- ✅ Constraint validation
- ✅ Multiple output formats (JSON, CSV)
- ✅ Batch processing for large datasets
- ✅ Metadata tracking

---

## Self-Learning Systems

**Difficulty**: Advanced
**Files**: `self-learning/reinforcement-learning.ts`, `self-learning/feedback-loop.ts`, `self-learning/continual-learning.ts`
**Real-World Use**: Training data for reinforcement learning and adaptive systems

### What It Demonstrates

- RL episode generation (Q-learning, DQN, PPO, SAC)
- Feedback loop simulation
- Quality scoring & A/B testing
- Continual learning datasets
- Transfer learning scenarios
- Anti-catastrophic forgetting

### Files Included

| File | Purpose | Focus |
|------|---------|-------|
| `reinforcement-learning.ts` | RL algorithms training data | Q-learning, DQN, PPO, SAC |
| `feedback-loop.ts` | Self-improvement loops | Quality scoring, pattern learning |
| `continual-learning.ts` | Incremental training | Domain adaptation, memory replay |

### Quick Start

```bash
# RL training data
npx tsx examples/self-learning/reinforcement-learning.ts

# Feedback loop simulation
npx tsx examples/self-learning/feedback-loop.ts

# Continual learning
npx tsx examples/self-learning/continual-learning.ts
```

### Code Examples

#### Generate RL Episodes

```typescript
import { generateRLEpisodes } from './examples/self-learning/reinforcement-learning';

const episodes = await generateRLEpisodes({
  algorithm: 'dqn',
  episodes: 1000,
  stepsPerEpisode: 100,
  stateSize: 4,
  actionSize: 2
});

// Each episode contains: state, action, reward, next_state, done
console.log(`Generated ${episodes.length} RL episodes`);
```

#### Create Feedback Loop

```typescript
import { createFeedbackLoop } from './examples/self-learning/feedback-loop';

const feedbackData = await createFeedbackLoop({
  iterations: 50,
  qualityThreshold: 0.8,
  learningRate: 0.01
});

// Track improvement over time
const avgQuality = feedbackData.reduce((sum, d) => sum + d.quality, 0) / feedbackData.length;
```

### Real-World Use Cases

1. **Game AI Training**: Generate training episodes for game agents
2. **Robot Control**: Simulate control policies and trajectories
3. **Recommender Systems**: A/B testing and feedback data
4. **LLM Fine-tuning**: Quality-scored examples for RLHF
5. **Adaptive UI**: User interaction patterns for personalization

### Key Features

- ✅ Multiple RL algorithms supported
- ✅ Realistic reward structures
- ✅ State-action trajectory tracking
- ✅ Transfer learning support
- ✅ Catastrophic forgetting prevention
- ✅ Integration with TensorFlow.js, PyTorch

---

## Ad ROAS Optimization

**Difficulty**: Intermediate
**Files**: `ad-roas/campaign-data.ts`, `ad-roas/optimization-simulator.ts`, `ad-roas/analytics-pipeline.ts`
**Real-World Use**: Marketing campaign optimization and attribution modeling

### What It Demonstrates

- Multi-platform campaign data (Google, Meta, TikTok)
- 6 attribution models (first-touch, last-touch, linear, time-decay, position-based, data-driven)
- LTV and cohort analysis
- Budget allocation strategies
- Bid optimization
- A/B testing scenarios

### Files Included

| File | Purpose | Platforms |
|------|---------|-----------|
| `campaign-data.ts` | Campaign metrics generation | Google, Meta, TikTok, LinkedIn |
| `optimization-simulator.ts` | Budget & bid optimization | All platforms |
| `analytics-pipeline.ts` | Attribution & funnel analysis | Multi-touch attribution |

### Quick Start

```bash
# Campaign data
npx tsx examples/ad-roas/campaign-data.ts

# Optimization simulator
npx tsx examples/ad-roas/optimization-simulator.ts

# Analytics pipeline
npx tsx examples/ad-roas/analytics-pipeline.ts
```

### Code Examples

#### Generate Campaign Data

```typescript
import { generateMultiPlatformCampaigns } from './examples/ad-roas/campaign-data';

const campaigns = await generateMultiPlatformCampaigns({
  platforms: ['google', 'meta', 'tiktok'],
  campaigns: 10,
  duration: 30  // days
});

// Analyze ROAS by platform
campaigns.forEach(campaign => {
  const roas = campaign.revenue / campaign.spend;
  console.log(`${campaign.platform}: ROAS ${roas.toFixed(2)}x`);
});
```

#### Attribution Modeling

```typescript
import { generateAttributionData } from './examples/ad-roas/analytics-pipeline';

const attributionData = await generateAttributionData({
  touchpoints: 1000,
  models: ['first_touch', 'last_touch', 'linear', 'time_decay', 'data_driven']
});

// Compare attribution models
attributionData.models.forEach(model => {
  console.log(`${model.name}: ${model.conversions} conversions attributed`);
});
```

### Real-World Use Cases

1. **Campaign Planning**: Test budget allocation strategies
2. **Attribution Analysis**: Compare attribution models
3. **LTV Modeling**: Customer lifetime value prediction
4. **Cohort Analysis**: Track user groups over time
5. **A/B Testing**: Test creative variations

### Key Features

- ✅ Multi-platform support (Google, Meta, TikTok, etc.)
- ✅ 6 attribution models
- ✅ Realistic conversion funnels
- ✅ Budget optimization algorithms
- ✅ Cohort analysis templates
- ✅ CSV/JSON export for BI tools

---

## Stock Market Simulation

**Difficulty**: Intermediate
**Files**: `stocks/market-data.ts`, `stocks/trading-scenarios.ts`, `stocks/portfolio-simulation.ts`
**Real-World Use**: Trading system backtesting and financial analysis

### What It Demonstrates

- OHLCV (candlestick) data generation
- Technical indicators (SMA, RSI, MACD, Bollinger Bands)
- Multi-timeframe data (1m, 5m, 1h, 1d)
- Market depth (Level 2 order book)
- Tick-by-tick simulation (10K+ ticks)
- Market microstructure patterns

### Files Included

| File | Purpose | Data Types |
|------|---------|------------|
| `market-data.ts` | Core market data generation | OHLCV, indicators, order book |
| `trading-scenarios.ts` | Market conditions | Bull/bear, volatility, crashes |
| `portfolio-simulation.ts` | Portfolio management | Multi-asset, rebalancing |

### Quick Start

```bash
# Market data
npx tsx examples/stocks/market-data.ts

# Trading scenarios
npx tsx examples/stocks/trading-scenarios.ts

# Portfolio simulation
npx tsx examples/stocks/portfolio-simulation.ts
```

### Code Examples

#### Generate OHLCV Data

```typescript
import { generateOHLCVData } from './examples/stocks/market-data';

const ohlcv = await generateOHLCVData({
  symbol: 'AAPL',
  bars: 390,        // One trading day (6.5 hours)
  interval: '1m',
  startPrice: 150.0
});

// Calculate daily statistics
const dailyHigh = Math.max(...ohlcv.map(b => b.high));
const dailyLow = Math.min(...ohlcv.map(b => b.low));
const dailyVolume = ohlcv.reduce((sum, b) => sum + b.volume, 0);
```

#### Generate Technical Indicators

```typescript
import { generateTechnicalIndicators } from './examples/stocks/market-data';

const data = await generateTechnicalIndicators({
  symbol: 'AAPL',
  count: 100
});

// Each bar includes: price, sma_20, sma_50, rsi_14, macd, bollinger bands
data.forEach(bar => {
  if (bar.rsi_14 < 30) console.log(`Oversold at ${bar.timestamp}`);
  if (bar.rsi_14 > 70) console.log(`Overbought at ${bar.timestamp}`);
});
```

#### Market Depth (Order Book)

```typescript
import { generateMarketDepth } from './examples/stocks/market-data';

const orderBook = await generateMarketDepth({
  symbol: 'AAPL',
  snapshots: 100,
  depth: 20  // 20 levels each side
});

// Analyze spread
orderBook.forEach(snapshot => {
  const spread = snapshot.asks[0].price - snapshot.bids[0].price;
  console.log(`Spread: $${spread.toFixed(2)}`);
});
```

### Real-World Use Cases

1. **Trading Bots**: Backtest trading strategies
2. **Risk Management**: Simulate portfolio drawdowns
3. **Market Making**: Order book dynamics
4. **Technical Analysis**: Indicator optimization
5. **Regulatory Compliance**: Audit trail generation

### Key Features

- ✅ Realistic market microstructure
- ✅ Multiple technical indicators
- ✅ Multi-timeframe aggregation
- ✅ Order book simulation
- ✅ Tick-by-tick precision
- ✅ Market condition scenarios

---

## Cryptocurrency Trading

**Difficulty**: Intermediate
**Files**: `crypto/exchange-data.ts`, `crypto/blockchain-data.ts`, `crypto/defi-scenarios.ts`
**Real-World Use**: Crypto trading bots and DeFi protocol testing

### What It Demonstrates

- 24/7 market data (BTC, ETH, SOL, AVAX, MATIC)
- On-chain transaction patterns
- DeFi protocols (Uniswap, Aave, Compound)
- NFT trading activity
- MEV (Maximal Extractable Value) scenarios
- Gas price modeling (EIP-1559)
- Cross-chain bridge activity

### Files Included

| File | Purpose | Focus |
|------|---------|-------|
| `exchange-data.ts` | Exchange trading data | OHLCV, order books, 24/7 |
| `blockchain-data.ts` | On-chain transactions | Wallet behavior, NFTs, MEV |
| `defi-scenarios.ts` | DeFi protocol simulation | Yield farming, liquidity pools |

### Quick Start

```bash
# Exchange data
npx tsx examples/crypto/exchange-data.ts

# Blockchain data
npx tsx examples/crypto/blockchain-data.ts

# DeFi scenarios
npx tsx examples/crypto/defi-scenarios.ts
```

### Code Examples

#### Generate Exchange Data

```typescript
import { generateCryptoExchangeData } from './examples/crypto/exchange-data';

const exchangeData = await generateCryptoExchangeData({
  symbols: ['BTC/USDT', 'ETH/USDT', 'SOL/USDT'],
  bars: 1440,      // 24 hours of 1-minute data
  exchanges: ['binance', 'coinbase', 'kraken']
});
```

#### On-Chain Transactions

```typescript
import { generateTransactionPatterns } from './examples/crypto/blockchain-data';

const transactions = await generateTransactionPatterns({
  networks: ['ethereum', 'polygon', 'arbitrum'],
  count: 10000,
  includeInternalTxs: true
});

// Analyze transaction types
const erc20Transfers = transactions.filter(tx => tx.methodId === '0xa9059cbb');
console.log(`ERC20 transfers: ${erc20Transfers.length}`);
```

#### DeFi Protocol Simulation

```typescript
import { generateYieldFarmingData } from './examples/crypto/defi-scenarios';

const yieldData = await generateYieldFarmingData({
  protocols: ['uniswap_v3', 'aave', 'compound'],
  users: 1000,
  duration: 30  // days
});

// Calculate average APY
const avgAPY = yieldData.reduce((sum, d) => sum + d.apy, 0) / yieldData.length;
```

### Real-World Use Cases

1. **Trading Bots**: Crypto arbitrage and market making
2. **DeFi Analytics**: Protocol TVL and yield tracking
3. **NFT Marketplaces**: Trading activity simulation
4. **MEV Research**: Sandwich attacks and arbitrage
5. **Gas Optimization**: Transaction cost modeling

### Key Features

- ✅ Multi-crypto support (20+ chains)
- ✅ DeFi protocol integration
- ✅ NFT marketplace activity
- ✅ MEV extraction scenarios
- ✅ Gas price modeling (EIP-1559)
- ✅ Cross-chain bridge simulation

---

## Log Analytics

**Difficulty**: Intermediate
**Files**: `logs/application-logs.ts`, `logs/system-logs.ts`, `logs/anomaly-scenarios.ts`, `logs/log-analytics.ts`
**Real-World Use**: Monitoring, anomaly detection, security analysis

### What It Demonstrates

- Application logs (structured JSON, distributed tracing)
- System logs (server, database, K8s, Docker)
- Anomaly scenarios (DDoS, intrusion, degradation)
- Multiple log formats (JSON, Syslog, CEF, GELF)
- ELK Stack integration
- Security incident simulation

### Files Included

| File | Purpose | Log Types |
|------|---------|-----------|
| `application-logs.ts` | App & API logs | Structured JSON, APM traces |
| `system-logs.ts` | Infrastructure logs | Server, DB, container logs |
| `anomaly-scenarios.ts` | Security incidents | DDoS, intrusion, errors |
| `log-analytics.ts` | Log processing | Aggregation, alerting |

### Quick Start

```bash
# Application logs
npx tsx examples/logs/application-logs.ts

# System logs
npx tsx examples/logs/system-logs.ts

# Anomaly scenarios
npx tsx examples/logs/anomaly-scenarios.ts

# Log analytics
npx tsx examples/logs/log-analytics.ts
```

### Code Examples

#### Generate Application Logs

```typescript
import { generateApplicationLogs } from './examples/logs/application-logs';

const logs = await generateApplicationLogs({
  count: 10000,
  logLevels: ['info', 'warn', 'error'],
  includeTracing: true,
  format: 'json'
});

// Filter errors
const errors = logs.filter(log => log.level === 'error');
console.log(`Error rate: ${(errors.length / logs.length * 100).toFixed(2)}%`);
```

#### Anomaly Detection Training Data

```typescript
import { generateDDoSAttackLogs } from './examples/logs/anomaly-scenarios';

const attackLogs = await generateDDoSAttackLogs({
  normalTraffic: 10000,
  attackTraffic: 5000,
  attackDuration: 600  // seconds
});

// Train anomaly detection model
const features = attackLogs.map(log => ({
  requestRate: log.requests_per_second,
  uniqueIPs: log.unique_ips,
  errorRate: log.error_rate,
  isAnomaly: log.is_attack
}));
```

### Real-World Use Cases

1. **SOC Training**: Security operations center scenarios
2. **Anomaly Detection**: ML model training data
3. **Compliance**: GDPR, SOC2, HIPAA reporting
4. **APM Testing**: Application performance monitoring
5. **Incident Response**: Security playbook testing

### Key Features

- ✅ Multiple log formats
- ✅ Distributed tracing support
- ✅ Security incident scenarios
- ✅ ELK Stack compatible
- ✅ Compliance reporting
- ✅ Real-time streaming

---

## Security Testing

**Difficulty**: Advanced
**Files**: `security/vulnerability-testing.ts`, `security/threat-simulation.ts`, `security/security-audit.ts`, `security/penetration-testing.ts`
**Real-World Use**: Penetration testing, security training, vulnerability assessment

### What It Demonstrates

- OWASP Top 10 test cases
- MITRE ATT&CK framework
- Vulnerability scanning data
- Threat actor simulation
- Security audit scenarios
- Penetration testing logs

### ⚠️ **IMPORTANT DISCLAIMER**

**FOR AUTHORIZED SECURITY TESTING ONLY**

These examples are for:
- ✅ Authorized penetration testing
- ✅ Security training and education
- ✅ Defensive security research
- ✅ Vulnerability assessment with permission

**NEVER use for**:
- ❌ Unauthorized access attempts
- ❌ Malicious activities
- ❌ Real-world attacks
- ❌ Testing systems without permission

### Files Included

| File | Purpose | Framework |
|------|---------|-----------|
| `vulnerability-testing.ts` | OWASP Top 10 tests | SQL injection, XSS, CSRF |
| `threat-simulation.ts` | Threat actor TTPs | Brute force, DDoS, malware |
| `security-audit.ts` | Access patterns | Compliance violations |
| `penetration-testing.ts` | Pentest scenarios | Network scanning, exploitation |

### Quick Start

```bash
# Vulnerability testing data
npx tsx examples/security/vulnerability-testing.ts

# Threat simulation
npx tsx examples/security/threat-simulation.ts

# Security audit
npx tsx examples/security/security-audit.ts

# Penetration testing
npx tsx examples/security/penetration-testing.ts
```

### Code Examples

#### OWASP Top 10 Test Cases

```typescript
import { generateOWASPTestCases } from './examples/security/vulnerability-testing';

const testCases = await generateOWASPTestCases({
  vulnerabilities: ['sql_injection', 'xss', 'csrf', 'ssrf'],
  count: 100
});

// Organize by severity
const critical = testCases.filter(tc => tc.severity === 'critical');
console.log(`Critical vulnerabilities: ${critical.length}`);
```

#### Threat Actor Simulation

```typescript
import { simulateThreatActor } from './examples/security/threat-simulation';

const attackScenario = await simulateThreatActor({
  actor: 'advanced_persistent_threat',
  tactics: ['reconnaissance', 'initial_access', 'lateral_movement'],
  duration: 7  // days
});

// Map to MITRE ATT&CK
attackScenario.tactics.forEach(tactic => {
  console.log(`${tactic.name}: ${tactic.techniques.length} techniques`);
});
```

### Real-World Use Cases

1. **SOC Training**: Security analyst training scenarios
2. **WAF Testing**: Web application firewall rules
3. **IDS/IPS**: Intrusion detection system training
4. **Red Team Exercises**: Penetration testing data
5. **Vulnerability Management**: Scanner calibration

### Key Features

- ✅ OWASP Top 10 coverage
- ✅ MITRE ATT&CK mapping
- ✅ Ethical hacking guidelines
- ✅ CVE database integration
- ✅ Compliance frameworks
- ✅ Authorized testing only

---

## Swarm Coordination

**Difficulty**: Advanced
**Files**: `swarms/agent-coordination.ts`, `swarms/distributed-processing.ts`, `swarms/collective-intelligence.ts`, `swarms/agent-lifecycle.ts`
**Real-World Use**: Multi-agent systems, distributed computing, AI orchestration

### What It Demonstrates

- Agent communication patterns
- Task distribution & load balancing
- Consensus protocols (Raft, Paxos, Byzantine)
- Fault tolerance & recovery
- Hierarchical coordination
- Integration with claude-flow, ruv-swarm, flow-nexus

### Files Included

| File | Purpose | Patterns |
|------|---------|----------|
| `agent-coordination.ts` | Communication & consensus | Direct, broadcast, pub/sub |
| `distributed-processing.ts` | Task distribution | Map-reduce, worker pools |
| `collective-intelligence.ts` | Problem-solving | Knowledge sharing, voting |
| `agent-lifecycle.ts` | Agent management | Spawning, health checks |

### Quick Start

```bash
# Agent coordination
npx tsx examples/swarms/agent-coordination.ts

# Distributed processing
npx tsx examples/swarms/distributed-processing.ts

# Collective intelligence
npx tsx examples/swarms/collective-intelligence.ts

# Agent lifecycle
npx tsx examples/swarms/agent-lifecycle.ts
```

### Code Examples

#### Agent Communication

```typescript
import { agentCommunicationPatterns } from './examples/swarms/agent-coordination';

const messages = await agentCommunicationPatterns({
  agents: 20,
  messages: 500,
  patterns: ['direct', 'broadcast', 'multicast', 'pubsub']
});

// Analyze latency
const avgLatency = messages.reduce((sum, m) => sum + m.latency_ms, 0) / messages.length;
console.log(`Average latency: ${avgLatency.toFixed(2)}ms`);
```

#### Task Distribution

```typescript
import { taskDistributionScenarios } from './examples/swarms/agent-coordination';

const tasks = await taskDistributionScenarios({
  agents: 15,
  tasks: 300,
  loadBalancing: 'least_connections'
});

// Check load distribution
const loadPerAgent = new Map();
tasks.forEach(task => {
  loadPerAgent.set(task.assigned_agent,
    (loadPerAgent.get(task.assigned_agent) || 0) + 1);
});
```

#### Consensus Building

```typescript
import { consensusBuildingData } from './examples/swarms/agent-coordination';

const consensus = await consensusBuildingData({
  rounds: 50,
  protocol: 'raft',
  participants: 7
});

// Analyze consensus success
const successRate = consensus.filter(r => r.decision === 'accepted').length / consensus.length;
console.log(`Consensus success rate: ${(successRate * 100).toFixed(1)}%`);
```

### Integration with Swarm Tools

#### Claude-Flow Integration

```bash
# Initialize swarm
npx claude-flow@alpha mcp start

# Use MCP tools
# - swarm_init: Initialize topology
# - agent_spawn: Create agents
# - task_orchestrate: Distribute tasks
# - swarm_monitor: Track performance
```

#### Ruv-Swarm Integration

```bash
# Enhanced coordination
npx ruv-swarm mcp start

# Advanced patterns
# - Hierarchical coordination
# - Byzantine fault tolerance
# - Auto-healing workflows
```

#### Flow-Nexus Cloud

```bash
# Cloud-based swarms
npx flow-nexus@latest login

# Cloud features
# - Distributed sandboxes
# - Real-time monitoring
# - Auto-scaling
```

### Real-World Use Cases

1. **Distributed AI**: Multi-agent AI systems
2. **Microservices**: Service mesh coordination
3. **IoT Networks**: Device swarm management
4. **Cloud Orchestration**: Container coordination
5. **Blockchain**: Consensus protocol testing

### Key Features

- ✅ Multiple consensus protocols
- ✅ Fault tolerance scenarios
- ✅ Load balancing algorithms
- ✅ Message queue integration
- ✅ Auto-healing patterns
- ✅ Cloud deployment support

---

## Business Management

**Difficulty**: Intermediate
**Files**: `business-management/erp-data.ts`, `business-management/crm-simulation.ts`, `business-management/hr-management.ts`, `business-management/financial-planning.ts`, `business-management/operations.ts`
**Real-World Use**: ERP systems, CRM, HR, financial modeling

### What It Demonstrates

- ERP workflows (inventory, purchase orders, supply chain)
- CRM lifecycle (leads, sales pipeline, support)
- HR management (employees, recruitment, payroll)
- Financial planning (budgets, P&L, balance sheets)
- Operations (projects, vendors, workflows)

### Files Included

| File | Purpose | Systems |
|------|---------|---------|
| `erp-data.ts` | ERP workflows | SAP, Oracle, Microsoft Dynamics |
| `crm-simulation.ts` | Customer management | Salesforce, HubSpot, Dynamics 365 |
| `hr-management.ts` | HR processes | Workday, BambooHR, SAP SuccessFactors |
| `financial-planning.ts` | Financial modeling | QuickBooks, NetSuite, Xero |
| `operations.ts` | Operations management | Jira, Asana, Monday.com |

### Quick Start

```bash
# ERP data
npx tsx examples/business-management/erp-data.ts

# CRM simulation
npx tsx examples/business-management/crm-simulation.ts

# HR management
npx tsx examples/business-management/hr-management.ts

# Financial planning
npx tsx examples/business-management/financial-planning.ts

# Operations
npx tsx examples/business-management/operations.ts
```

### Code Examples

#### ERP Data Generation

```typescript
import { generateERPData } from './examples/business-management/erp-data';

const erpData = await generateERPData({
  products: 500,
  purchaseOrders: 200,
  inventory: 1000,
  suppliers: 50
});

// Analyze inventory levels
const lowStock = erpData.inventory.filter(item => item.quantity < item.reorder_point);
console.log(`Low stock items: ${lowStock.length}`);
```

#### CRM Pipeline Simulation

```typescript
import { generateCRMPipeline } from './examples/business-management/crm-simulation';

const pipeline = await generateCRMPipeline({
  leads: 1000,
  opportunities: 500,
  deals: 200
});

// Calculate conversion rates
const leadToOpportunity = pipeline.opportunities.length / pipeline.leads.length;
const opportunityToDeal = pipeline.deals.length / pipeline.opportunities.length;
```

### Real-World Use Cases

1. **ERP Testing**: SAP, Oracle integration tests
2. **CRM Analytics**: Sales pipeline analysis
3. **HR Planning**: Workforce modeling
4. **Financial Audits**: Compliance reporting
5. **Operations**: Project management simulation

### Key Features

- ✅ Complete ERP workflows
- ✅ CRM lifecycle simulation
- ✅ HR compliance data
- ✅ Financial statements
- ✅ Approval workflows
- ✅ Audit trails

---

## Employee Simulation

**Difficulty**: Intermediate
**Files**: `employee-simulation/workforce-behavior.ts`, `employee-simulation/performance-data.ts`, `employee-simulation/organizational-dynamics.ts`, `employee-simulation/workforce-planning.ts`, `employee-simulation/workplace-events.ts`
**Real-World Use**: Workforce modeling, HR analytics, organizational planning

### What It Demonstrates

- Workforce behavior patterns
- Performance metrics (KPIs, OKRs)
- Organizational dynamics (teams, leadership)
- Workforce planning (hiring, turnover)
- Workplace events (onboarding, training, promotions)
- 100% synthetic and privacy-safe

### Files Included

| File | Purpose | Focus |
|------|---------|-------|
| `workforce-behavior.ts` | Daily patterns | Productivity, schedules, collaboration |
| `performance-data.ts` | KPIs & metrics | Code commits, sales targets, reviews |
| `organizational-dynamics.ts` | Team structures | Formation, culture, leadership |
| `workforce-planning.ts` | HR planning | Hiring, skills, turnover prediction |
| `workplace-events.ts` | Employee lifecycle | Onboarding, promotions, training |

### Quick Start

```bash
# Workforce behavior
npx tsx examples/employee-simulation/workforce-behavior.ts

# Performance data
npx tsx examples/employee-simulation/performance-data.ts

# Organizational dynamics
npx tsx examples/employee-simulation/organizational-dynamics.ts

# Workforce planning
npx tsx examples/employee-simulation/workforce-planning.ts

# Workplace events
npx tsx examples/employee-simulation/workplace-events.ts
```

### Code Examples

#### Generate Workforce Behavior

```typescript
import { generateWorkforceBehavior } from './examples/employee-simulation/workforce-behavior';

const behavior = await generateWorkforceBehavior({
  employees: 1000,
  days: 30
});

// Analyze productivity patterns
const avgProductivity = behavior.reduce((sum, d) => sum + d.productivity_score, 0) / behavior.length;
console.log(`Average productivity: ${avgProductivity.toFixed(2)}`);
```

#### Performance Reviews

```typescript
import { generatePerformanceReviews } from './examples/employee-simulation/performance-data';

const reviews = await generatePerformanceReviews({
  employees: 500,
  period: 'quarterly',
  include360: true
});

// Distribution analysis
const topPerformers = reviews.filter(r => r.rating >= 4.5);
console.log(`Top performers: ${(topPerformers.length / reviews.length * 100).toFixed(1)}%`);
```

### Real-World Use Cases

1. **HR Analytics**: Workforce insights and trends
2. **Retention Modeling**: Turnover prediction
3. **Diversity Analysis**: D&I metrics tracking
4. **Succession Planning**: Leadership pipeline
5. **Training ROI**: Learning effectiveness

### Key Features

- ✅ 100% synthetic data
- ✅ Privacy-safe
- ✅ Realistic patterns
- ✅ Diversity metrics
- ✅ Career progression
- ✅ Ethical guidelines

---

## Agentic-Jujutsu Integration

**Difficulty**: Advanced
**Files**: `agentic-jujutsu/collaborative-workflows.ts`, `agentic-jujutsu/reasoning-bank-learning.ts`, `agentic-jujutsu/multi-agent-data-generation.ts`, `agentic-jujutsu/quantum-resistant-data.ts`, `agentic-jujutsu/test-suite.ts`, `agentic-jujutsu/version-control-integration.ts`
**Real-World Use**: Version-controlled data generation, collaborative AI workflows

### What It Demonstrates

- Version-controlled synthetic data
- Collaborative team workflows
- Review processes & quality gates
- ReasoningBank learning integration
- Multi-agent data generation
- Quantum-resistant data patterns

### Files Included

| File | Purpose | Features |
|------|---------|----------|
| `collaborative-workflows.ts` | Team collaboration | Branches, reviews, merges |
| `reasoning-bank-learning.ts` | Adaptive learning | Pattern recognition, optimization |
| `multi-agent-data-generation.ts` | Parallel generation | Distributed workflows |
| `quantum-resistant-data.ts` | Security patterns | Post-quantum crypto |
| `test-suite.ts` | Integration testing | Comprehensive tests |
| `version-control-integration.ts` | VCS workflows | Git-like operations |

### Quick Start

```bash
# Install agentic-jujutsu
npm install agentic-jujutsu

# Collaborative workflows
npx tsx examples/agentic-jujutsu/collaborative-workflows.ts

# ReasoningBank learning
npx tsx examples/agentic-jujutsu/reasoning-bank-learning.ts

# Multi-agent generation
npx tsx examples/agentic-jujutsu/multi-agent-data-generation.ts
```

### Code Examples

#### Collaborative Data Generation

```typescript
import { CollaborativeDataWorkflow } from './examples/agentic-jujutsu/collaborative-workflows';

const workflow = new CollaborativeDataWorkflow('./data-repo');

// Initialize workspace
await workflow.initialize();

// Create teams
const dataTeam = await workflow.createTeam('data-team', 'Data Engineering', ['alice', 'bob']);
const analyticsTeam = await workflow.createTeam('analytics-team', 'Analytics', ['charlie']);

// Teams generate data
await workflow.teamGenerate('data-team', 'alice', schema, 1000, 'User events');

// Create review request
const review = await workflow.createReviewRequest(
  'data-team',
  'alice',
  'Add user event dataset',
  'Generated 1000 user events',
  ['charlie']
);

// Approve and merge
await workflow.approveReview(review.id, 'charlie');
await workflow.mergeReview(review.id);
```

#### ReasoningBank Learning

```typescript
import { ReasoningBankDataGenerator } from './examples/agentic-jujutsu/reasoning-bank-learning';

const generator = new ReasoningBankDataGenerator();

// Generate with learning
const data = await generator.generateWithLearning({
  schema: userSchema,
  count: 1000,
  learningEnabled: true
});

// Patterns learned and applied automatically
console.log(`Quality score: ${generator.getQualityScore()}`);
```

### Real-World Use Cases

1. **Data Versioning**: Track synthetic data evolution
2. **Team Collaboration**: Multi-team data generation
3. **Quality Assurance**: Review processes for data
4. **Reproducibility**: Git-like data snapshots
5. **Learning Systems**: Self-improving generation

### Key Features

- ✅ Git-like version control
- ✅ Branch management
- ✅ Review workflows
- ✅ Quality gates
- ✅ ReasoningBank integration
- ✅ Quantum-resistant patterns

---

## DSPy Integration

**Difficulty**: Advanced
**Files**: `dspy-complete-example.ts`, `dspy-training-example.ts`, `dspy-verify-setup.ts`
**Real-World Use**: Neural optimization, prompt engineering, model training

### What It Demonstrates

- DSPy.ts integration for synthetic data
- Multi-model training (Gemini, OpenRouter)
- Prompt optimization
- Chain-of-thought reasoning
- Evaluation metrics
- Model comparison

### Files Included

| File | Purpose | Models |
|------|---------|--------|
| `dspy-complete-example.ts` | Full DSPy pipeline | All providers |
| `dspy-training-example.ts` | Training workflows | Gemini, Claude, GPT |
| `dspy-verify-setup.ts` | Setup verification | Configuration tests |

### Quick Start

```bash
# Install DSPy.ts
npm install dspy.ts

# Complete example
npx tsx examples/dspy-complete-example.ts

# Training example
npx tsx examples/dspy-training-example.ts

# Verify setup
npx tsx examples/dspy-verify-setup.ts
```

### Code Examples

#### DSPy-Powered Generation

```typescript
import { createSynth } from '@ruvector/agentic-synth';
import { DSPy, ChainOfThought } from 'dspy.ts';

// Initialize with DSPy
const synth = createSynth({
  provider: 'gemini',
  dspyEnabled: true
});

const result = await synth.generateWithDSPy({
  task: 'Generate realistic user profiles',
  schema: userSchema,
  count: 100,
  optimize: true
});

// DSPy automatically optimizes prompts
console.log(`Quality: ${result.metadata.quality_score}`);
```

#### Multi-Model Training

```typescript
import { trainMultiModel } from './examples/dspy-training-example';

const results = await trainMultiModel({
  models: ['gemini-2.0-flash-exp', 'claude-3.5-sonnet', 'gpt-4'],
  trainingData: examples,
  metric: 'f1_score'
});

// Compare model performance
results.forEach(result => {
  console.log(`${result.model}: F1 ${result.f1_score.toFixed(3)}`);
});
```

### Real-World Use Cases

1. **Prompt Engineering**: Optimize generation prompts
2. **Model Selection**: Compare model performance
3. **Quality Improvement**: Iterative refinement
4. **Cost Optimization**: Balance quality vs. cost
5. **A/B Testing**: Test prompt variations

### Key Features

- ✅ DSPy.ts integration
- ✅ Multi-model support
- ✅ Prompt optimization
- ✅ Evaluation metrics
- ✅ Chain-of-thought reasoning
- ✅ Cost tracking

---

## Integration Patterns

### Using with Testing Frameworks

#### Jest Integration

```typescript
// __tests__/data-generation.test.ts
import { createSynth } from '@ruvector/agentic-synth';

describe('Data Generation', () => {
  let synth;

  beforeAll(() => {
    synth = createSynth({
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY
    });
  });

  test('generates valid user data', async () => {
    const users = await synth.generateStructured({
      count: 10,
      schema: userSchema
    });

    expect(users.data).toHaveLength(10);
    users.data.forEach(user => {
      expect(user).toHaveProperty('id');
      expect(user).toHaveProperty('email');
    });
  });
});
```

#### Vitest Integration

```typescript
// tests/integration.test.ts
import { describe, it, expect, beforeAll } from 'vitest';
import { generateOHLCVData } from '../examples/stocks/market-data';

describe('Stock Data Generation', () => {
  it('generates valid OHLCV data', async () => {
    const data = await generateOHLCVData();

    expect(data).toBeDefined();
    expect(data.length).toBeGreaterThan(0);

    data.forEach(bar => {
      expect(bar.high).toBeGreaterThanOrEqual(bar.open);
      expect(bar.low).toBeLessThanOrEqual(bar.close);
    });
  });
});
```

### CI/CD Integration

#### GitHub Actions

```yaml
name: Generate Test Data

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  generate-data:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'

      - name: Install dependencies
        run: npm install

      - name: Generate test data
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
        run: |
          npx tsx examples/cicd/test-data-generator.ts

      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: test-data
          path: ./test-fixtures/
```

#### GitLab CI

```yaml
# .gitlab-ci.yml
generate-data:
  stage: test
  image: node:18
  script:
    - npm install
    - export GEMINI_API_KEY=$GEMINI_API_KEY
    - npx tsx examples/cicd/test-data-generator.ts
  artifacts:
    paths:
      - test-fixtures/
    expire_in: 1 week
  only:
    - main
    - develop
```

### Docker Integration

```dockerfile
# Dockerfile
FROM node:18-alpine

WORKDIR /app

COPY package*.json ./
RUN npm install

COPY . .

ENV GEMINI_API_KEY=""

CMD ["npx", "tsx", "examples/cicd/test-data-generator.ts"]
```

```bash
# Build and run
docker build -t agentic-synth-examples .
docker run -e GEMINI_API_KEY=your-key agentic-synth-examples
```

---

## Performance Tips

### 1. Enable Caching

```typescript
const synth = createSynth({
  cacheStrategy: 'memory',    // or 'redis'
  cacheTTL: 3600,             // 1 hour
  maxCacheSize: 10000         // entries
});

// First call - generates and caches
const data1 = await synth.generateStructured({ count: 100, schema });

// Second call - retrieves from cache (>100x faster)
const data2 = await synth.generateStructured({ count: 100, schema });
```

### 2. Use Streaming for Large Datasets

```typescript
// Memory-efficient for large datasets
for await (const record of synth.generateStream('structured', {
  count: 1_000_000,
  schema: userSchema
})) {
  await processRecord(record);  // Process one at a time
}
```

### 3. Batch Processing

```typescript
// Generate multiple datasets in parallel
const batchOptions = [
  { count: 100, schema: schema1 },
  { count: 200, schema: schema2 },
  { count: 150, schema: schema3 }
];

const results = await synth.generateBatch(
  'structured',
  batchOptions,
  5  // concurrency limit
);
```

### 4. Use Seed Values for Reproducibility

```typescript
// Same seed = same data (perfect for testing)
const synth = createSynth({
  seed: process.env.CI_COMMIT_SHA || 'fixed-seed'
});

// Data will be identical across runs
const data = await synth.generateStructured({ count: 100, schema });
```

### 5. Choose the Right Model

```typescript
// Fast & cheap for simple data
const fastSynth = createSynth({
  model: 'gemini-2.0-flash-exp'  // Fastest, cheapest
});

// High quality for complex data
const qualitySynth = createSynth({
  model: 'gemini-1.5-pro'        // Best quality
});
```

### Benchmarks

| Operation | Records | Time | Throughput |
|-----------|---------|------|------------|
| Simple structured | 1,000 | ~500ms | 2K rec/s |
| Complex nested | 1,000 | ~2s | 500 rec/s |
| Time-series | 10,000 | ~3s | 3.3K rec/s |
| Events | 5,000 | ~1.5s | 3.3K rec/s |
| With caching (hit) | 1,000 | ~5ms | 200K rec/s |
| Streaming | 100,000 | ~30s | 3.3K rec/s |

*Benchmarks: M1 Mac, 16GB RAM, Gemini 2.0 Flash*

---

## Troubleshooting

### Common Issues

#### 1. API Key Not Found

```bash
# Error: GEMINI_API_KEY is not set
# Solution:
export GEMINI_API_KEY=your-api-key-here

# Or create .env file
echo "GEMINI_API_KEY=your-key" > .env
```

#### 2. Rate Limiting (429 Error)

```typescript
// Solution: Implement retries and backoff
const synth = createSynth({
  maxRetries: 5,
  retryDelay: 1000,  // ms
  timeout: 60000
});
```

#### 3. Memory Issues with Large Datasets

```typescript
// Solution: Use streaming instead of loading all at once
for await (const record of synth.generateStream('structured', {
  count: 1_000_000,
  schema
})) {
  // Process one at a time
}
```

#### 4. Slow Generation

```typescript
// Solutions:
// 1. Enable caching
const synth = createSynth({
  cacheStrategy: 'memory',
  model: 'gemini-2.0-flash-exp'  // Fastest model
});

// 2. Reduce complexity
// Simplify schema, reduce count, or use batch processing
```

#### 5. Invalid Schema Errors

```typescript
// Solution: Validate schema before generation
import { z } from 'zod';

const schema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  age: z.number().int().min(0).max(120)
});

// Schema will be validated automatically
```

### Debug Mode

```typescript
// Enable debug logging
const synth = createSynth({
  logLevel: 'debug',  // 'debug' | 'info' | 'warn' | 'error'
  debug: true
});

// Logs will show:
// - API requests/responses
// - Cache hits/misses
// - Generation time
// - Token usage
```

### Getting Help

- **Documentation**: [GitHub README](https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth)
- **Issues**: [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
- **Discussions**: [GitHub Discussions](https://github.com/ruvnet/ruvector/discussions)
- **NPM**: [@ruvector/agentic-synth](https://www.npmjs.com/package/@ruvector/agentic-synth)

---

## Contributing Examples

Want to contribute a new example? Follow this structure:

```
examples/
└── your-category/
    ├── README.md              # Category documentation
    ├── example1.ts            # First example
    ├── example2.ts            # Second example
    └── example3.ts            # Third example
```

### Example Template

```typescript
/**
 * Example Title
 *
 * Brief description of what this example demonstrates.
 *
 * Real-world use cases:
 * - Use case 1
 * - Use case 2
 * - Use case 3
 */

import { createSynth } from '@ruvector/agentic-synth';

export async function yourExampleFunction() {
  console.log('🚀 Example: Your Example Title\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key'
  });

  const result = await synth.generateStructured({
    count: 100,
    schema: {
      // Your schema here
    }
  });

  console.log(`Generated ${result.data.length} records`);

  return result;
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  yourExampleFunction().catch(console.error);
}
```

### Submission Guidelines

1. **Clear Documentation**: Explain what the example does
2. **Real-World Focus**: Demonstrate practical use cases
3. **Code Quality**: Follow TypeScript best practices
4. **Performance**: Optimize for speed and memory
5. **Error Handling**: Include proper error handling
6. **Tests**: Add test coverage if possible

---

## License

MIT License - See [LICENSE](../../LICENSE) file for details

---

## Acknowledgments

Built with:
- **agentic-synth** - Synthetic data generation engine
- **Google Gemini** - AI-powered data generation
- **OpenRouter** - Multi-provider AI access
- **DSPy.ts** - Neural optimization framework
- **TypeScript** - Type-safe development
- **Vitest** - Testing framework

Special thanks to all contributors and the open-source community!

---

**Last Updated**: 2025-11-22
**Version**: 0.1.0
**Total Examples**: 50+
**Total Code**: 25,000+ lines
**Status**: Production Ready ✅
