# Stock Market Data Generation Examples

Comprehensive examples for generating realistic financial market data using agentic-synth. These examples are designed for testing trading systems, backtesting strategies, and financial analysis.

## Overview

This package provides three main categories of financial data generation:

1. **Market Data** (`market-data.ts`) - Time-series price data with technical indicators
2. **Trading Scenarios** (`trading-scenarios.ts`) - Market regime simulations for system testing
3. **Portfolio Simulation** (`portfolio-simulation.ts`) - Multi-asset portfolio management data

## Features

### Market Data Generation

Generate realistic market microstructure data including:

- **OHLCV Data**: Open, High, Low, Close, Volume candlestick bars
- **Technical Indicators**: SMA, RSI, MACD, Bollinger Bands
- **Multi-Timeframe**: 1m, 5m, 1h, 1d aggregation
- **Market Depth**: Level 2 order book data
- **Tick Data**: High-frequency tick-by-tick trades
- **Microstructure Metrics**: Spreads, liquidity, toxicity

### Trading Scenarios

Realistic market conditions for testing trading systems:

- **Bull Markets**: Sustained uptrends with occasional pullbacks
- **Bear Markets**: Downtrends with volatility spikes
- **Volatility Regimes**: Low, medium, high, extreme volatility
- **Flash Crashes**: Rapid price declines with recovery
- **Earnings Events**: Announcement impact with IV crush
- **Market Correlations**: Multi-asset correlation patterns

### Portfolio Simulation

Complete portfolio management workflow:

- **Multi-Asset Portfolios**: Diversified across asset classes
- **Rebalancing**: Calendar, threshold, and opportunistic strategies
- **Risk Metrics**: Sharpe, Sortino, Calmar, Information ratios
- **Drawdown Analysis**: Peak-to-trough analysis with recovery
- **Performance Attribution**: Alpha, beta, tracking error

## Installation

```bash
cd packages/agentic-synth
npm install
```

## Usage

### Running Individual Examples

```bash
# Market data generation
npx ts-node examples/stocks/market-data.ts

# Trading scenarios
npx ts-node examples/stocks/trading-scenarios.ts

# Portfolio simulation
npx ts-node examples/stocks/portfolio-simulation.ts
```

### Importing in Your Code

```typescript
import {
  generateOHLCVData,
  generateTechnicalIndicators,
  generateMultiTimeframeData,
} from './examples/stocks/market-data';

import {
  generateBullMarket,
  generateBearMarket,
  generateFlashCrash,
} from './examples/stocks/trading-scenarios';

import {
  generateMultiAssetPortfolio,
  generateRebalancingScenarios,
  generateRiskAdjustedReturns,
} from './examples/stocks/portfolio-simulation';

// Use in your application
const ohlcvData = await generateOHLCVData();
const bullMarket = await generateBullMarket();
const portfolio = await generateMultiAssetPortfolio();
```

## Examples

### 1. OHLCV Data Generation

Generate realistic candlestick data with proper OHLCV relationships:

```typescript
const ohlcvData = await generateOHLCVData();
// Returns: Array of 390 1-minute bars for a trading day
// Each bar: { timestamp, open, high, low, close, volume, symbol }
```

**Key Features:**
- High >= max(open, close)
- Low <= min(open, close)
- Next bar opens at previous close
- Realistic volume patterns

### 2. Technical Indicators

Calculate common technical indicators on price data:

```typescript
const technicalData = await generateTechnicalIndicators();
// Returns: Price data with SMA, RSI, MACD, Bollinger Bands
```

**Indicators Included:**
- SMA 20 & 50 (Simple Moving Averages)
- RSI 14 (Relative Strength Index)
- MACD & Signal Line
- Bollinger Bands (upper, middle, lower)

### 3. Multi-Timeframe Data

Generate data across multiple timeframes with proper aggregation:

```typescript
const multiTF = await generateMultiTimeframeData();
// Returns: { '1m': [], '5m': [], '1h': [], '1d': [] }
```

**Timeframes:**
- 1-minute bars (base timeframe)
- 5-minute bars (aggregated from 1m)
- 1-hour bars (aggregated from 1m)
- 1-day bars (aggregated from 1m)

### 4. Market Depth (Order Book)

Generate Level 2 market depth data:

```typescript
const marketDepth = await generateMarketDepth();
// Returns: Order book snapshots with bids/asks
```

**Order Book Features:**
- 20 levels on each side
- Realistic size distribution
- Order count per level
- Spread and mid-price calculation

### 5. Bull Market Scenario

Simulate a sustained uptrend:

```typescript
const bullMarket = await generateBullMarket();
// Generates: 252 days of bull market with ~20% annual return
```

**Characteristics:**
- Upward drift with occasional pullbacks
- Lower volatility
- Volume increases on breakouts
- Momentum indicators trend positive

### 6. Flash Crash Simulation

Model rapid price decline and recovery:

```typescript
const flashCrash = await generateFlashCrash();
// Phases: Normal → Crash (15% drop) → Recovery
```

**Phases:**
- **Normal**: Typical trading patterns
- **Crash**: Exponential price decay, wide spreads, liquidity evaporation
- **Recovery**: Quick rebound with reduced liquidity

### 7. Multi-Asset Portfolio

Create a diversified portfolio across asset classes:

```typescript
const portfolio = await generateMultiAssetPortfolio();
// Returns: { portfolioData, portfolioMetrics, assets }
```

**Asset Allocation:**
- 60% Equities (SPY, QQQ, IWM, EFA)
- 30% Fixed Income (AGG, TLT)
- 10% Alternatives (GLD, VNQ)

**Metrics Tracked:**
- Total value and returns
- Sharpe ratio
- Maximum drawdown
- Volatility
- Alpha and beta

### 8. Rebalancing Scenarios

Simulate portfolio rebalancing strategies:

```typescript
const rebalancing = await generateRebalancingScenarios();
// Returns: Rebalance events with trades and costs
```

**Rebalancing Types:**
- **Calendar**: Quarterly (every 63 trading days)
- **Threshold**: When drift exceeds 5%
- **Opportunistic**: Based on market conditions

### 9. Drawdown Analysis

Comprehensive drawdown tracking and analysis:

```typescript
const drawdowns = await generateDrawdownAnalysis();
// Returns: All drawdown periods with recovery info
```

**Drawdown Metrics:**
- Maximum drawdown (peak to trough)
- Drawdown duration
- Recovery duration
- Currently underwater status
- Top 5 largest drawdowns

## Realistic Patterns

All generated data includes realistic market microstructure patterns:

### Price Dynamics
- **Mean Reversion**: Prices tend to revert to moving averages
- **Momentum**: Trends persist with gradual reversals
- **Volatility Clustering**: Volatile periods cluster together
- **Fat Tails**: Extreme moves occur more than normal distribution

### Volume Patterns
- **Volume-Price Relationship**: Volume increases with volatility
- **Institutional Activity**: Block trades and large orders
- **Time-of-Day**: Higher volume at open and close
- **Event-Driven**: Spikes during announcements

### Market Microstructure
- **Bid-Ask Spread**: Realistic spread dynamics
- **Market Impact**: Large orders move prices
- **Liquidity**: Depth varies with market conditions
- **Order Imbalance**: Buy/sell pressure affects prices

## Regulatory Compliance

All generated data follows regulatory standards:

### Trade Conditions
- `BLOCK`: Large institutional trades (100+ shares)
- `INSTITUTIONAL`: Very large orders (10,000+ shares)
- `ODD_LOT`: Non-standard lot sizes
- `EXTENDED_HOURS`: Pre-market and after-hours trades

### Data Quality
- No negative prices or volumes
- OHLCV relationships enforced
- Realistic tick sizes (pennies)
- Proper timestamp ordering

### Risk Disclosures
⚠️ **IMPORTANT**: This is simulated data for testing purposes only. Do not use for:
- Production trading decisions
- Financial advice
- Regulatory reporting
- Real money trading without proper validation

## Performance

Generation performance for typical use cases:

| Dataset | Size | Generation Time |
|---------|------|----------------|
| 1-day OHLCV (1m) | 390 bars | ~50ms |
| 1-year daily | 252 bars | ~30ms |
| Tick data | 10,000 ticks | ~200ms |
| Order book | 100 snapshots | ~150ms |
| Multi-asset portfolio | 252 days | ~500ms |

## Advanced Usage

### Custom Asset Classes

```typescript
const customAssets: Asset[] = [
  {
    symbol: 'CUSTOM',
    assetClass: 'equity',
    weight: 0.50,
    expectedReturn: 0.15,
    volatility: 0.25,
  },
  // Add more assets...
];
```

### Custom Rebalancing Logic

```typescript
const rebalanceThreshold = 0.10; // 10% drift
const rebalanceFrequency = 21; // Monthly

// Implement custom rebalancing logic
if (shouldRebalance(portfolio, threshold)) {
  await rebalance(portfolio, targetWeights);
}
```

### Custom Risk Metrics

```typescript
// Calculate custom risk metrics
const varCalc = (returns: number[], confidence: number) => {
  const sorted = returns.sort((a, b) => a - b);
  const index = Math.floor(returns.length * (1 - confidence));
  return sorted[index];
};

const var95 = varCalc(returns, 0.95);
const cvar95 = returns.filter(r => r <= var95).reduce((a, b) => a + b) / returns.length;
```

## Testing Trading Systems

These examples are ideal for:

1. **Backtesting**: Test strategies against historical scenarios
2. **Stress Testing**: Evaluate performance under extreme conditions
3. **Risk Management**: Validate risk models and limits
4. **Algorithm Development**: Develop and tune trading algorithms
5. **Portfolio Optimization**: Test allocation strategies

### Example Backtest

```typescript
// Generate test data
const bullMarket = await generateBullMarket();
const bearMarket = await generateBearMarket();
const flashCrash = await generateFlashCrash();

// Test strategy on each scenario
const results = {
  bull: await backtest(strategy, bullMarket),
  bear: await backtest(strategy, bearMarket),
  crash: await backtest(strategy, flashCrash),
};

// Analyze results
console.log('Strategy Performance:');
console.log(`Bull Market: ${results.bull.return}%`);
console.log(`Bear Market: ${results.bear.return}%`);
console.log(`Flash Crash: ${results.crash.maxDrawdown}%`);
```

## Contributing

Contributions are welcome! Areas for improvement:

- [ ] Options pricing data
- [ ] Futures and derivatives
- [ ] Cryptocurrency markets
- [ ] FX (foreign exchange) data
- [ ] High-frequency market making scenarios
- [ ] Credit spreads and fixed income
- [ ] Alternative data integration

## Resources

### Financial Concepts
- [Market Microstructure](https://en.wikipedia.org/wiki/Market_microstructure)
- [Technical Analysis](https://www.investopedia.com/technical-analysis-4689657)
- [Portfolio Theory](https://www.investopedia.com/terms/m/modernportfoliotheory.asp)
- [Risk Metrics](https://www.investopedia.com/terms/r/riskadjustedreturn.asp)

### Trading System Development
- [Quantitative Trading](https://www.quantstart.com/)
- [Algorithmic Trading](https://www.algorithmictrading.net/)
- [Backtesting Best Practices](https://www.quantconnect.com/docs/)

### Regulatory Guidelines
- [SEC Trading Rules](https://www.sec.gov/fast-answers)
- [FINRA Regulations](https://www.finra.org/rules-guidance)
- [Market Data Standards](https://www.iso20022.org/)

## License

MIT License - see LICENSE file for details

## Disclaimer

This software is for educational and testing purposes only. The authors are not responsible for any financial losses incurred from using this software. Always consult with a qualified financial advisor before making investment decisions.

**Past performance does not guarantee future results.**
