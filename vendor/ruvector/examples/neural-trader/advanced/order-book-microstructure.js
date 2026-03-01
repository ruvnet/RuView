/**
 * Order Book & Market Microstructure Analysis
 *
 * PRACTICAL: Deep analysis of order book dynamics
 *
 * Features:
 * - Level 2 order book reconstruction
 * - Order flow imbalance detection
 * - Spread analysis and toxicity metrics
 * - Hidden liquidity estimation
 * - Price impact modeling
 * - Trade classification (buyer/seller initiated)
 */

// Order book configuration
const microstructureConfig = {
  // Book levels to analyze
  bookDepth: 10,

  // Tick size
  tickSize: 0.01,

  // Time granularity
  snapshotIntervalMs: 100,

  // Toxicity thresholds
  toxicity: {
    vpin: 0.7,      // Volume-synchronized probability of informed trading
    spreadThreshold: 0.005,
    imbalanceThreshold: 0.3
  }
};

// Order book level
class BookLevel {
  constructor(price, size, orders = 1) {
    this.price = price;
    this.size = size;
    this.orders = orders;
    this.timestamp = Date.now();
  }
}

// Full order book
class OrderBook {
  constructor(symbol) {
    this.symbol = symbol;
    this.bids = [];  // Sorted descending by price
    this.asks = [];  // Sorted ascending by price
    this.lastUpdate = Date.now();
    this.trades = [];
    this.snapshots = [];
  }

  updateBid(price, size, orders = 1) {
    this.updateLevel(this.bids, price, size, orders, true);
    this.lastUpdate = Date.now();
  }

  updateAsk(price, size, orders = 1) {
    this.updateLevel(this.asks, price, size, orders, false);
    this.lastUpdate = Date.now();
  }

  updateLevel(levels, price, size, orders, isBid) {
    const idx = levels.findIndex(l => l.price === price);

    if (size === 0) {
      if (idx >= 0) levels.splice(idx, 1);
      return;
    }

    if (idx >= 0) {
      levels[idx].size = size;
      levels[idx].orders = orders;
      levels[idx].timestamp = Date.now();
    } else {
      levels.push(new BookLevel(price, size, orders));
      levels.sort((a, b) => isBid ? b.price - a.price : a.price - b.price);
    }
  }

  // Best bid/ask
  get bestBid() { return this.bids[0]; }
  get bestAsk() { return this.asks[0]; }

  // Mid price
  get midPrice() {
    if (!this.bestBid || !this.bestAsk) return null;
    return (this.bestBid.price + this.bestAsk.price) / 2;
  }

  // Spread metrics
  get spread() {
    if (!this.bestBid || !this.bestAsk) return null;
    return this.bestAsk.price - this.bestBid.price;
  }

  get spreadBps() {
    return this.spread ? (this.spread / this.midPrice) * 10000 : null;
  }

  // Book imbalance
  getImbalance(levels = 5) {
    const bidVolume = this.bids.slice(0, levels).reduce((sum, l) => sum + l.size, 0);
    const askVolume = this.asks.slice(0, levels).reduce((sum, l) => sum + l.size, 0);
    const totalVolume = bidVolume + askVolume;

    return {
      bidVolume,
      askVolume,
      imbalance: totalVolume > 0 ? (bidVolume - askVolume) / totalVolume : 0,
      bidRatio: totalVolume > 0 ? bidVolume / totalVolume : 0.5
    };
  }

  // Weighted mid price (based on volume at top levels)
  getWeightedMid(levels = 3) {
    let bidWeight = 0, askWeight = 0;
    let bidSum = 0, askSum = 0;

    for (let i = 0; i < Math.min(levels, this.bids.length); i++) {
      bidWeight += this.bids[i].size;
      bidSum += this.bids[i].price * this.bids[i].size;
    }

    for (let i = 0; i < Math.min(levels, this.asks.length); i++) {
      askWeight += this.asks[i].size;
      askSum += this.asks[i].price * this.asks[i].size;
    }

    const bidAvg = bidWeight > 0 ? bidSum / bidWeight : this.bestBid?.price || 0;
    const askAvg = askWeight > 0 ? askSum / askWeight : this.bestAsk?.price || 0;

    // Weight by opposite side volume (more volume = more weight)
    const totalWeight = bidWeight + askWeight;
    if (totalWeight === 0) return this.midPrice;

    return (bidAvg * askWeight + askAvg * bidWeight) / totalWeight;
  }

  // Add trade
  addTrade(trade) {
    this.trades.push({
      ...trade,
      timestamp: Date.now()
    });

    // Keep last 1000 trades
    if (this.trades.length > 1000) {
      this.trades.shift();
    }
  }

  // Take snapshot
  takeSnapshot() {
    const snapshot = {
      timestamp: Date.now(),
      midPrice: this.midPrice,
      spread: this.spread,
      spreadBps: this.spreadBps,
      imbalance: this.getImbalance(),
      weightedMid: this.getWeightedMid(),
      bidDepth: this.bids.slice(0, 10).map(l => ({ ...l })),
      askDepth: this.asks.slice(0, 10).map(l => ({ ...l }))
    };

    this.snapshots.push(snapshot);

    // Keep last 1000 snapshots
    if (this.snapshots.length > 1000) {
      this.snapshots.shift();
    }

    return snapshot;
  }
}

// Market microstructure analyzer
class MicrostructureAnalyzer {
  constructor(orderBook) {
    this.book = orderBook;
  }

  // Calculate VPIN (Volume-synchronized Probability of Informed Trading)
  calculateVPIN(bucketSize = 50) {
    const trades = this.book.trades;
    if (trades.length < bucketSize * 2) return null;

    // Classify trades as buy/sell initiated
    const classifiedTrades = this.classifyTrades(trades);

    // Create volume buckets
    let currentBucket = { buyVolume: 0, sellVolume: 0, totalVolume: 0 };
    const buckets = [];

    for (const trade of classifiedTrades) {
      const volume = trade.size;

      if (trade.side === 'buy') {
        currentBucket.buyVolume += volume;
      } else {
        currentBucket.sellVolume += volume;
      }
      currentBucket.totalVolume += volume;

      if (currentBucket.totalVolume >= bucketSize) {
        buckets.push({ ...currentBucket });
        currentBucket = { buyVolume: 0, sellVolume: 0, totalVolume: 0 };
      }
    }

    if (buckets.length < 10) return null;

    // Calculate VPIN over last N buckets
    const recentBuckets = buckets.slice(-50);
    let totalImbalance = 0;
    let totalVolume = 0;

    for (const bucket of recentBuckets) {
      totalImbalance += Math.abs(bucket.buyVolume - bucket.sellVolume);
      totalVolume += bucket.totalVolume;
    }

    return totalVolume > 0 ? totalImbalance / totalVolume : 0;
  }

  // Trade classification using tick rule
  classifyTrades(trades) {
    const classified = [];
    let lastPrice = null;
    let lastDirection = 'buy';

    for (const trade of trades) {
      let side;

      if (lastPrice === null) {
        side = 'buy';
      } else if (trade.price > lastPrice) {
        side = 'buy';
      } else if (trade.price < lastPrice) {
        side = 'sell';
      } else {
        side = lastDirection;
      }

      classified.push({
        ...trade,
        side
      });

      lastDirection = side;
      lastPrice = trade.price;
    }

    return classified;
  }

  // Calculate Kyle's Lambda (price impact coefficient)
  calculateKyleLambda() {
    const snapshots = this.book.snapshots;
    if (snapshots.length < 100) return null;

    // Regression: ΔP = λ * OrderImbalance + ε
    const data = [];

    for (let i = 1; i < snapshots.length; i++) {
      const deltaP = snapshots[i].midPrice - snapshots[i - 1].midPrice;
      const imbalance = snapshots[i - 1].imbalance.imbalance;

      data.push({ deltaP, imbalance });
    }

    // Simple linear regression
    let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0;
    const n = data.length;

    for (const d of data) {
      sumX += d.imbalance;
      sumY += d.deltaP;
      sumXY += d.imbalance * d.deltaP;
      sumX2 += d.imbalance * d.imbalance;
    }

    const lambda = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX);

    return lambda;
  }

  // Estimate hidden liquidity
  estimateHiddenLiquidity() {
    const trades = this.book.trades.slice(-100);
    const snapshot = this.book.takeSnapshot();

    // Compare executed volume to visible liquidity
    let volumeAtBest = 0;
    let executedVolume = 0;

    for (const trade of trades) {
      executedVolume += trade.size;
    }

    // Visible at best
    volumeAtBest = (snapshot.bidDepth[0]?.size || 0) + (snapshot.askDepth[0]?.size || 0);

    // If executed >> visible, there's hidden liquidity
    const hiddenRatio = volumeAtBest > 0
      ? Math.max(0, (executedVolume / 100) - volumeAtBest) / (executedVolume / 100)
      : 0;

    return {
      visibleLiquidity: volumeAtBest,
      estimatedExecuted: executedVolume / trades.length,
      hiddenLiquidityRatio: Math.min(1, Math.max(0, hiddenRatio)),
      confidence: trades.length > 50 ? 'high' : 'low'
    };
  }

  // Calculate spread components (realized spread, adverse selection)
  calculateSpreadComponents() {
    const trades = this.book.trades.slice(-200);
    if (trades.length < 50) return null;

    let realizedSpread = 0;
    let adverseSelection = 0;
    let count = 0;

    for (let i = 0; i < trades.length - 10; i++) {
      const trade = trades[i];
      const midAtTrade = trade.midPrice || this.book.midPrice;
      const midAfter = trades[i + 10].midPrice || this.book.midPrice;

      if (!midAtTrade || !midAfter) continue;

      // Effective spread
      const effectiveSpread = Math.abs(trade.price - midAtTrade) * 2;

      // Realized spread (profit to market maker)
      const direction = trade.side === 'buy' ? 1 : -1;
      const realized = (trade.price - midAfter) * direction * 2;

      // Adverse selection (cost to market maker)
      const adverse = (midAfter - midAtTrade) * direction * 2;

      realizedSpread += realized;
      adverseSelection += adverse;
      count++;
    }

    return {
      effectiveSpread: this.book.spread,
      realizedSpread: count > 0 ? realizedSpread / count : 0,
      adverseSelection: count > 0 ? adverseSelection / count : 0,
      observations: count
    };
  }

  // Full analysis report
  getAnalysisReport() {
    const snapshot = this.book.takeSnapshot();
    const vpin = this.calculateVPIN();
    const lambda = this.calculateKyleLambda();
    const hidden = this.estimateHiddenLiquidity();
    const spreadComponents = this.calculateSpreadComponents();

    return {
      timestamp: new Date().toISOString(),
      symbol: this.book.symbol,

      // Basic metrics
      midPrice: snapshot.midPrice,
      spread: snapshot.spread,
      spreadBps: snapshot.spreadBps,

      // Order book metrics
      imbalance: snapshot.imbalance,
      weightedMid: snapshot.weightedMid,

      // Microstructure metrics
      vpin,
      kyleLambda: lambda,
      hiddenLiquidity: hidden,
      spreadComponents,

      // Toxicity assessment
      toxicity: this.assessToxicity(vpin, snapshot.imbalance.imbalance, snapshot.spreadBps)
    };
  }

  assessToxicity(vpin, imbalance, spreadBps) {
    let score = 0;
    const reasons = [];

    if (vpin && vpin > microstructureConfig.toxicity.vpin) {
      score += 0.4;
      reasons.push(`High VPIN (${(vpin * 100).toFixed(1)}%)`);
    }

    if (Math.abs(imbalance) > microstructureConfig.toxicity.imbalanceThreshold) {
      score += 0.3;
      reasons.push(`Strong imbalance (${(imbalance * 100).toFixed(1)}%)`);
    }

    if (spreadBps > microstructureConfig.toxicity.spreadThreshold * 10000) {
      score += 0.3;
      reasons.push(`Wide spread (${spreadBps.toFixed(1)} bps)`);
    }

    return {
      score: Math.min(1, score),
      level: score > 0.7 ? 'HIGH' : score > 0.4 ? 'MEDIUM' : 'LOW',
      reasons
    };
  }
}

// Simulation
function simulateOrderBook(symbol) {
  const book = new OrderBook(symbol);

  // Initialize with realistic levels
  const basePrice = 100;

  // Bid side
  for (let i = 0; i < 10; i++) {
    const price = basePrice - 0.01 - i * 0.01;
    const size = Math.floor(100 + Math.random() * 500);
    const orders = Math.floor(1 + Math.random() * 10);
    book.updateBid(price, size, orders);
  }

  // Ask side
  for (let i = 0; i < 10; i++) {
    const price = basePrice + 0.01 + i * 0.01;
    const size = Math.floor(100 + Math.random() * 500);
    const orders = Math.floor(1 + Math.random() * 10);
    book.updateAsk(price, size, orders);
  }

  // Simulate trades
  for (let i = 0; i < 200; i++) {
    const isBuy = Math.random() > 0.5;
    const price = isBuy
      ? book.bestAsk?.price || basePrice + 0.01
      : book.bestBid?.price || basePrice - 0.01;

    book.addTrade({
      price,
      size: Math.floor(10 + Math.random() * 100),
      side: isBuy ? 'buy' : 'sell',
      midPrice: book.midPrice
    });

    // Take periodic snapshots
    if (i % 10 === 0) {
      book.takeSnapshot();

      // Update book slightly
      const drift = (Math.random() - 0.5) * 0.02;
      for (let j = 0; j < book.bids.length; j++) {
        book.bids[j].price += drift;
        book.bids[j].size = Math.max(10, book.bids[j].size + (Math.random() - 0.5) * 50);
      }
      for (let j = 0; j < book.asks.length; j++) {
        book.asks[j].price += drift;
        book.asks[j].size = Math.max(10, book.asks[j].size + (Math.random() - 0.5) * 50);
      }
    }
  }

  return book;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('ORDER BOOK & MARKET MICROSTRUCTURE ANALYSIS');
  console.log('═'.repeat(70));
  console.log();

  // 1. Create and simulate order book
  console.log('1. Simulating Order Book...');
  const book = simulateOrderBook('AAPL');
  console.log(`   Symbol: ${book.symbol}`);
  console.log(`   Trades: ${book.trades.length}`);
  console.log(`   Snapshots: ${book.snapshots.length}`);
  console.log();

  // 2. Display order book
  console.log('2. Order Book (Top 5 Levels):');
  console.log('─'.repeat(70));
  console.log('   BID                                  │                                  ASK');
  console.log('   Orders   Size     Price              │              Price     Size   Orders');
  console.log('─'.repeat(70));

  for (let i = 0; i < 5; i++) {
    const bid = book.bids[i];
    const ask = book.asks[i];

    const bidStr = bid
      ? `   ${bid.orders.toString().padStart(6)}  ${bid.size.toString().padStart(6)}  $${bid.price.toFixed(2).padStart(8)}`
      : '                                  ';

    const askStr = ask
      ? `$${ask.price.toFixed(2).padEnd(8)}  ${ask.size.toString().padEnd(6)}  ${ask.orders.toString().padEnd(6)}`
      : '';

    console.log(`${bidStr}              │              ${askStr}`);
  }

  console.log('─'.repeat(70));
  console.log(`   Mid: $${book.midPrice?.toFixed(4)} | Spread: $${book.spread?.toFixed(4)} (${book.spreadBps?.toFixed(2)} bps)`);
  console.log();

  // 3. Run microstructure analysis
  console.log('3. Microstructure Analysis:');
  console.log('─'.repeat(70));

  const analyzer = new MicrostructureAnalyzer(book);
  const report = analyzer.getAnalysisReport();

  console.log(`   Weighted Mid Price:    $${report.weightedMid?.toFixed(4)}`);
  console.log(`   Order Imbalance:       ${(report.imbalance.imbalance * 100).toFixed(2)}% (${report.imbalance.imbalance > 0 ? 'bid heavy' : 'ask heavy'})`);
  console.log(`   Bid Volume (5 lvl):    ${report.imbalance.bidVolume.toLocaleString()}`);
  console.log(`   Ask Volume (5 lvl):    ${report.imbalance.askVolume.toLocaleString()}`);
  console.log();

  // 4. Toxicity metrics
  console.log('4. Flow Toxicity Metrics:');
  console.log('─'.repeat(70));

  console.log(`   VPIN:                  ${report.vpin ? (report.vpin * 100).toFixed(2) + '%' : 'N/A'}`);
  console.log(`   Kyle's Lambda:         ${report.kyleLambda ? report.kyleLambda.toFixed(6) : 'N/A'}`);
  console.log();

  if (report.toxicity) {
    const tox = report.toxicity;
    const toxIcon = tox.level === 'HIGH' ? '🔴' : tox.level === 'MEDIUM' ? '🟡' : '🟢';
    console.log(`   Toxicity Level:        ${toxIcon} ${tox.level} (score: ${(tox.score * 100).toFixed(0)}%)`);
    if (tox.reasons.length > 0) {
      console.log(`   Reasons:`);
      tox.reasons.forEach(r => console.log(`     - ${r}`));
    }
  }
  console.log();

  // 5. Hidden liquidity
  console.log('5. Hidden Liquidity Estimation:');
  console.log('─'.repeat(70));

  const hidden = report.hiddenLiquidity;
  console.log(`   Visible at Best:       ${hidden.visibleLiquidity.toLocaleString()} shares`);
  console.log(`   Avg Executed Size:     ${hidden.estimatedExecuted.toFixed(0)} shares`);
  console.log(`   Hidden Liquidity:      ~${(hidden.hiddenLiquidityRatio * 100).toFixed(0)}%`);
  console.log(`   Confidence:            ${hidden.confidence}`);
  console.log();

  // 6. Spread components
  console.log('6. Spread Component Analysis:');
  console.log('─'.repeat(70));

  if (report.spreadComponents) {
    const sc = report.spreadComponents;
    console.log(`   Effective Spread:      $${sc.effectiveSpread?.toFixed(4)}`);
    console.log(`   Realized Spread:       $${sc.realizedSpread?.toFixed(4)} (MM profit)`);
    console.log(`   Adverse Selection:     $${sc.adverseSelection?.toFixed(4)} (info cost)`);
    console.log(`   Based on:              ${sc.observations} observations`);
  }
  console.log();

  // 7. Trading signal
  console.log('7. Trading Signal:');
  console.log('─'.repeat(70));

  const imbalance = report.imbalance.imbalance;
  const signal = imbalance > 0.15 ? 'BULLISH' : imbalance < -0.15 ? 'BEARISH' : 'NEUTRAL';
  const signalIcon = signal === 'BULLISH' ? '🟢' : signal === 'BEARISH' ? '🔴' : '⚪';

  console.log(`   Signal:                ${signalIcon} ${signal}`);
  console.log(`   Reason:                Imbalance ${(imbalance * 100).toFixed(1)}%`);
  console.log(`   Recommended Action:    ${signal === 'BULLISH' ? 'Consider long' : signal === 'BEARISH' ? 'Consider short' : 'Wait'}`);
  console.log();

  console.log('═'.repeat(70));
  console.log('Microstructure analysis completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
