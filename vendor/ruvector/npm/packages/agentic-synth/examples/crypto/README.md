# Cryptocurrency and Blockchain Data Generation Examples

Comprehensive examples for generating realistic cryptocurrency trading, DeFi protocol, and blockchain data using agentic-synth.

## Overview

This directory contains production-ready examples for simulating:

- **Exchange Data**: OHLCV, order books, trades, liquidity pools, arbitrage
- **DeFi Scenarios**: Yield farming, liquidity provision, impermanent loss, gas prices
- **Blockchain Data**: Transactions, wallets, tokens, NFTs, MEV patterns

All examples include **24/7 market patterns** and **cross-exchange scenarios** for realistic crypto market simulation.

## Files

### 1. exchange-data.ts

Cryptocurrency exchange data generation covering both CEX and DEX markets.

**Examples:**
- OHLCV data for multiple cryptocurrencies (BTC, ETH, SOL, AVAX, MATIC)
- Real-time order book snapshots with bid/ask spreads
- Trade execution data with maker/taker fees
- AMM liquidity pool metrics
- Cross-exchange arbitrage opportunities
- 24/7 market data with timezone effects
- Perpetual futures funding rates
- Streaming market data feeds

**Key Features:**
```typescript
// Generate realistic OHLCV with seasonality
await generateOHLCV();

// Order book with realistic spreads and depth
await generateOrderBook();

// 10k trades with realistic patterns
await generateTrades();

// DEX liquidity pool data
await generateLiquidityPools();

// Cross-exchange arbitrage
await generateArbitrageOpportunities();
```

### 2. defi-scenarios.ts

DeFi protocol simulations for yield farming, lending, and advanced strategies.

**Examples:**
- Yield farming across Aave, Compound, Curve, Convex, Yearn
- Liquidity provision scenarios with LP token calculations
- Impermanent loss simulations under various market conditions
- Gas price data with network congestion patterns
- Smart contract interaction sequences
- Lending/borrowing position management
- Staking rewards (liquid staking protocols)
- MEV extraction scenarios

**Key Features:**
```typescript
// Yield farming data
await generateYieldFarmingData();

// LP scenarios with IL analysis
await generateLiquidityProvisionScenarios();

// Impermanent loss under different conditions
await generateImpermanentLossScenarios();

// Gas price optimization
await generateGasPriceData();

// Smart contract interactions
await generateSmartContractInteractions();
```

### 3. blockchain-data.ts

On-chain data generation for transactions, wallets, and blockchain activity.

**Examples:**
- Transaction patterns across multiple networks (Ethereum, Polygon, Arbitrum, Optimism, Base)
- Wallet behavior simulation (HODLers, traders, bots, whales)
- Token transfer events (ERC-20, ERC-721, ERC-1155)
- NFT marketplace activity and trading
- MEV bundle construction and extraction
- Block production and validator performance
- Smart contract deployment tracking
- Cross-chain bridge activity

**Key Features:**
```typescript
// Generate realistic transactions
await generateTransactionPatterns();

// Wallet behavior patterns
await generateWalletBehavior();

// Token transfers
await generateTokenTransfers();

// NFT trading activity
await generateNFTActivity();

// MEV scenarios
await generateMEVPatterns();
```

## Installation

```bash
# Install dependencies
cd packages/agentic-synth
npm install

# Set up API keys
cp .env.example .env
# Add your GEMINI_API_KEY or OPENROUTER_API_KEY
```

## Usage

### Running Individual Examples

```typescript
// Import specific examples
import { generateOHLCV, generateArbitrageOpportunities } from './crypto/exchange-data.js';
import { generateYieldFarmingData } from './crypto/defi-scenarios.js';
import { generateWalletBehavior } from './crypto/blockchain-data.js';

// Run examples
const ohlcvData = await generateOHLCV();
const arbOps = await generateArbitrageOpportunities();
const yieldData = await generateYieldFarmingData();
const wallets = await generateWalletBehavior();
```

### Running All Examples

```typescript
// Exchange data examples
import { runExchangeDataExamples } from './crypto/exchange-data.js';
await runExchangeDataExamples();

// DeFi scenario examples
import { runDeFiScenarioExamples } from './crypto/defi-scenarios.js';
await runDeFiScenarioExamples();

// Blockchain data examples
import { runBlockchainDataExamples } from './crypto/blockchain-data.js';
await runBlockchainDataExamples();
```

### Command Line Usage

```bash
# Run via Node.js
node --experimental-modules examples/crypto/exchange-data.js
node --experimental-modules examples/crypto/defi-scenarios.js
node --experimental-modules examples/crypto/blockchain-data.js

# Run via ts-node
ts-node examples/crypto/exchange-data.ts
```

## Configuration

### Basic Configuration

```typescript
import { createSynth } from '@ruvector/agentic-synth';

const synth = createSynth({
  provider: 'gemini',           // or 'openrouter'
  apiKey: process.env.GEMINI_API_KEY,
  model: 'gemini-2.0-flash-exp', // or 'anthropic/claude-3.5-sonnet'
  cacheStrategy: 'memory',       // Enable caching
  cacheTTL: 3600                 // Cache for 1 hour
});
```

### Provider Options

**Gemini (Recommended for crypto data):**
```typescript
{
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  model: 'gemini-2.0-flash-exp'
}
```

**OpenRouter (For Claude/GPT models):**
```typescript
{
  provider: 'openrouter',
  apiKey: process.env.OPENROUTER_API_KEY,
  model: 'anthropic/claude-3.5-sonnet'
}
```

## Key Features

### 24/7 Market Patterns

All examples include realistic 24/7 cryptocurrency market patterns:

- **Asian Session**: Increased volatility, lower volume
- **European Session**: Medium volatility, building volume
- **US Session**: Highest volume, major moves
- **Weekend Effect**: 30% lower volume typically
- **Holiday Impact**: Reduced activity during major holidays

```typescript
const result = await synth.generateTimeSeries({
  count: 168 * 12, // 1 week of 5-minute data
  interval: '5m',
  seasonality: true, // Enable session patterns
  // ...
});
```

### Cross-Exchange Arbitrage

Realistic price differences and arbitrage opportunities:

```typescript
const arbOps = await generateArbitrageOpportunities();
// Returns opportunities across Binance, Coinbase, Kraken, OKX
// Includes:
// - Price spreads
// - Execution times
// - Fee calculations
// - Feasibility analysis
```

### Gas Price Optimization

Network congestion modeling for transaction cost analysis:

```typescript
const gasData = await generateGasPriceData();
// Includes:
// - Base fee dynamics (EIP-1559)
// - Priority fees
// - Network congestion levels
// - Cost estimates for different transaction types
```

### Impermanent Loss Calculations

Accurate IL simulation for liquidity providers:

```typescript
const ilData = await generateImpermanentLossScenarios();
// Formula: 2 * sqrt(priceRatio) / (1 + priceRatio) - 1
// Includes:
// - Price divergence analysis
// - Fee compensation
// - Break-even calculations
// - Recommendations
```

## Data Schemas

### OHLCV Schema

```typescript
{
  timestamp: string,      // ISO 8601
  symbol: string,         // e.g., "BTC/USDT"
  open: number,
  high: number,           // >= max(open, close, low)
  low: number,            // <= min(open, close, high)
  close: number,
  volume: number,
  vwap: number,          // Volume-weighted average price
  trades: number         // Number of trades
}
```

### Order Book Schema

```typescript
{
  timestamp: string,
  exchange: string,
  symbol: string,
  bids: [
    { price: number, quantity: number, total: number }
  ],
  asks: [
    { price: number, quantity: number, total: number }
  ],
  spread: number,        // asks[0].price - bids[0].price
  midPrice: number,      // (bids[0].price + asks[0].price) / 2
  liquidity: {
    bidDepth: number,
    askDepth: number,
    totalDepth: number
  }
}
```

### Trade Schema

```typescript
{
  tradeId: string,
  timestamp: string,
  exchange: string,
  symbol: string,
  side: 'buy' | 'sell',
  orderType: 'market' | 'limit' | 'stop' | 'stop_limit',
  price: number,
  quantity: number,
  total: number,
  fee: number,
  feeAsset: string,
  makerTaker: 'maker' | 'taker',
  latency: number        // milliseconds
}
```

### Liquidity Pool Schema

```typescript
{
  timestamp: string,
  dex: string,
  poolAddress: string,
  tokenA: string,
  tokenB: string,
  reserveA: number,
  reserveB: number,
  totalLiquidity: number,
  price: number,         // reserveB / reserveA
  volume24h: number,
  fees24h: number,
  apy: number,
  impermanentLoss: number
}
```

## Use Cases

### 1. Trading Algorithm Development

Generate realistic market data for backtesting trading strategies:

```typescript
const historicalData = await generateOHLCV();
const orderBook = await generateOrderBook();
const trades = await generateTrades();

// Use for:
// - Strategy backtesting
// - Order execution simulation
// - Market impact analysis
```

### 2. DeFi Protocol Testing

Test DeFi applications with realistic scenarios:

```typescript
const yieldData = await generateYieldFarmingData();
const lpScenarios = await generateLiquidityProvisionScenarios();
const gasData = await generateGasPriceData();

// Use for:
// - APY calculation testing
// - IL mitigation strategies
// - Gas optimization
```

### 3. Risk Analysis

Simulate various market conditions for risk assessment:

```typescript
const ilScenarios = await generateImpermanentLossScenarios();
const lendingScenarios = await generateLendingScenarios();

// Use for:
// - Portfolio risk assessment
// - Liquidation analysis
// - Stress testing
```

### 4. Blockchain Analytics

Generate on-chain data for analytics platforms:

```typescript
const txPatterns = await generateTransactionPatterns();
const wallets = await generateWalletBehavior();
const nftActivity = await generateNFTActivity();

// Use for:
// - Wallet profiling
// - Transaction pattern analysis
// - Network activity monitoring
```

### 5. MEV Research

Study MEV extraction patterns and strategies:

```typescript
const mevPatterns = await generateMEVPatterns();
const arbOps = await generateArbitrageOpportunities();

// Use for:
// - MEV strategy development
// - Sandwich attack analysis
// - Flashbot simulation
```

## Performance Optimization

### Caching

Enable caching for repeated queries:

```typescript
const synth = createSynth({
  cacheStrategy: 'memory',
  cacheTTL: 3600  // 1 hour
});

// First call: generates data
const data1 = await synth.generateTimeSeries({...});

// Second call: returns cached data
const data2 = await synth.generateTimeSeries({...}); // Fast!
```

### Batch Generation

Generate multiple datasets in parallel:

```typescript
const batches = [
  { count: 100, interval: '1h' },
  { count: 200, interval: '5m' },
  { count: 50, interval: '1d' }
];

const results = await synth.generateBatch('timeseries', batches, 3);
// Processes 3 batches concurrently
```

### Streaming

Use streaming for real-time data generation:

```typescript
for await (const tick of synth.generateStream('timeseries', {
  count: 100,
  interval: '1s',
  metrics: ['price', 'volume']
})) {
  console.log('New tick:', tick);
  // Process data in real-time
}
```

## Best Practices

1. **Use Appropriate Intervals**
   - 1s-1m: High-frequency trading, tick data
   - 5m-1h: Intraday trading, short-term analysis
   - 4h-1d: Swing trading, daily analysis
   - 1d-1w: Long-term analysis, backtesting

2. **Set Realistic Constraints**
   - Use market-appropriate price ranges
   - Set sensible volatility levels (0.1-0.3 for crypto)
   - Include seasonality for realistic patterns

3. **Validate Generated Data**
   - Check for price consistency (high >= max(open, close, low))
   - Verify volume patterns
   - Ensure timestamp ordering

4. **Optimize for Scale**
   - Use caching for repeated queries
   - Batch generation for multiple datasets
   - Stream data for real-time applications

5. **Security Considerations**
   - Never hardcode API keys
   - Use environment variables
   - Implement rate limiting
   - Validate all inputs

## Examples Output

### OHLCV Data Sample

```json
{
  "timestamp": "2025-01-22T10:00:00.000Z",
  "symbol": "BTC/USDT",
  "open": 42150.50,
  "high": 42380.25,
  "low": 42080.00,
  "close": 42295.75,
  "volume": 125.48,
  "vwap": 42225.33,
  "trades": 342
}
```

### Arbitrage Opportunity Sample

```json
{
  "timestamp": "2025-01-22T10:15:32.000Z",
  "symbol": "ETH/USDT",
  "buyExchange": "binance",
  "sellExchange": "coinbase",
  "buyPrice": 2245.50,
  "sellPrice": 2258.25,
  "spread": 12.75,
  "spreadPercent": 0.568,
  "profitUSD": 127.50,
  "feasible": true
}
```

### Impermanent Loss Sample

```json
{
  "timestamp": "2025-01-22T10:00:00.000Z",
  "scenario": "high_volatility",
  "priceRatio": 1.5,
  "impermanentLoss": -2.02,
  "impermanentLossPercent": -2.02,
  "hodlValue": 10000,
  "lpValue": 9798,
  "feesEarned": 150,
  "netProfit": -52,
  "recommendation": "rebalance"
}
```

## Troubleshooting

### API Rate Limits

If you hit rate limits:

```typescript
const synth = createSynth({
  maxRetries: 5,
  timeout: 60000  // Increase timeout
});
```

### Memory Issues

For large datasets:

```typescript
// Use streaming instead of batch generation
for await (const data of synth.generateStream(...)) {
  processData(data);
  // Process one at a time
}
```

### Data Quality Issues

If generated data doesn't meet requirements:

```typescript
// Add more specific constraints
const result = await synth.generateTimeSeries({
  // ...
  constraints: {
    custom: [
      'high >= Math.max(open, close, low)',
      'low <= Math.min(open, close, high)',
      'volume > 1000',
      'realistic market microstructure'
    ]
  }
});
```

## Integration Examples

### With Trading Bots

```typescript
import { generateOHLCV, generateOrderBook } from './crypto/exchange-data.js';

async function backtestStrategy() {
  const historicalData = await generateOHLCV();
  const orderBook = await generateOrderBook();

  // Run your trading strategy
  const results = runBacktest(historicalData, orderBook);

  return results;
}
```

### With DeFi Protocols

```typescript
import { generateYieldFarmingData, generateGasPriceData } from './crypto/defi-scenarios.js';

async function optimizeYield() {
  const yieldData = await generateYieldFarmingData();
  const gasData = await generateGasPriceData();

  // Calculate optimal farming strategy
  const strategy = calculateOptimal(yieldData, gasData);

  return strategy;
}
```

### With Analytics Platforms

```typescript
import { generateWalletBehavior, generateTransactionPatterns } from './crypto/blockchain-data.js';

async function analyzeUserBehavior() {
  const wallets = await generateWalletBehavior();
  const transactions = await generateTransactionPatterns();

  // Perform analytics
  const insights = analyzePatterns(wallets, transactions);

  return insights;
}
```

## Contributing

To add new crypto data examples:

1. Follow existing patterns in the example files
2. Include realistic constraints and validations
3. Add comprehensive documentation
4. Include sample outputs
5. Test with multiple data sizes

## Resources

- [agentic-synth Documentation](../../README.md)
- [Crypto Market Data Standards](https://www.ccxt.pro/)
- [DeFi Protocol Documentation](https://defillama.com/)
- [Blockchain Data APIs](https://www.alchemy.com/)

## Support

For issues or questions:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth

## License

MIT License - see [LICENSE](../../LICENSE) for details
