/**
 * News-Driven Trading with Neural Trader
 *
 * Demonstrates using @neural-trader/news-trading for:
 * - Real-time news sentiment analysis
 * - Event-driven trading strategies
 * - Earnings reaction patterns
 * - Breaking news detection
 * - Social media sentiment integration
 */

// News trading configuration
const newsConfig = {
  // Sentiment thresholds
  sentiment: {
    strongBullish: 0.8,
    bullish: 0.6,
    neutral: [0.4, 0.6],
    bearish: 0.4,
    strongBearish: 0.2
  },

  // Trading parameters
  trading: {
    maxPositionSize: 0.05,
    newsReactionWindow: 300,    // 5 minutes
    stopLoss: 0.02,
    takeProfit: 0.05,
    holdingPeriodMax: 3600      // 1 hour max
  },

  // News sources
  sources: ['Reuters', 'Bloomberg', 'CNBC', 'WSJ', 'Twitter/X', 'Reddit'],

  // Event types
  eventTypes: ['earnings', 'fda', 'merger', 'macro', 'executive', 'legal']
};

// Sample news events
const sampleNews = [
  {
    id: 'news_001',
    timestamp: '2024-12-31T09:30:00Z',
    headline: 'Apple Reports Record Q4 Revenue, Beats Estimates by 8%',
    source: 'Bloomberg',
    symbols: ['AAPL'],
    eventType: 'earnings',
    sentiment: {
      score: 0.85,
      magnitude: 0.92,
      keywords: ['record', 'beats', 'revenue growth', 'strong demand']
    },
    priceImpact: {
      immediate: 0.035,     // 3.5% immediate move
      t5min: 0.042,
      t15min: 0.038,
      t1hour: 0.045
    }
  },
  {
    id: 'news_002',
    timestamp: '2024-12-31T10:15:00Z',
    headline: 'Fed Officials Signal Pause in Rate Cuts Amid Inflation Concerns',
    source: 'Reuters',
    symbols: ['SPY', 'QQQ', 'TLT'],
    eventType: 'macro',
    sentiment: {
      score: 0.35,
      magnitude: 0.78,
      keywords: ['pause', 'inflation', 'concerns', 'hawkish']
    },
    priceImpact: {
      immediate: -0.012,
      t5min: -0.018,
      t15min: -0.015,
      t1hour: -0.008
    }
  },
  {
    id: 'news_003',
    timestamp: '2024-12-31T11:00:00Z',
    headline: 'NVIDIA Announces Next-Gen AI Chip With 3x Performance Improvement',
    source: 'CNBC',
    symbols: ['NVDA', 'AMD', 'INTC'],
    eventType: 'product',
    sentiment: {
      score: 0.88,
      magnitude: 0.95,
      keywords: ['next-gen', 'breakthrough', 'AI', 'performance']
    },
    priceImpact: {
      immediate: 0.048,
      t5min: 0.062,
      t15min: 0.055,
      t1hour: 0.071
    }
  },
  {
    id: 'news_004',
    timestamp: '2024-12-31T12:30:00Z',
    headline: 'Tesla Recalls 500,000 Vehicles Over Safety Concerns',
    source: 'WSJ',
    symbols: ['TSLA'],
    eventType: 'legal',
    sentiment: {
      score: 0.22,
      magnitude: 0.85,
      keywords: ['recall', 'safety', 'concerns', 'investigation']
    },
    priceImpact: {
      immediate: -0.028,
      t5min: -0.035,
      t15min: -0.032,
      t1hour: -0.025
    }
  },
  {
    id: 'news_005',
    timestamp: '2024-12-31T13:45:00Z',
    headline: 'Biotech Company Receives FDA Fast Track Designation for Cancer Drug',
    source: 'Reuters',
    symbols: ['MRNA'],
    eventType: 'fda',
    sentiment: {
      score: 0.82,
      magnitude: 0.88,
      keywords: ['FDA', 'fast track', 'breakthrough', 'cancer']
    },
    priceImpact: {
      immediate: 0.085,
      t5min: 0.125,
      t15min: 0.098,
      t1hour: 0.115
    }
  }
];

// Social media sentiment data
const socialSentiment = {
  'AAPL': { twitter: 0.72, reddit: 0.68, mentions: 15420, trend: 'rising' },
  'NVDA': { twitter: 0.85, reddit: 0.82, mentions: 28350, trend: 'rising' },
  'TSLA': { twitter: 0.45, reddit: 0.38, mentions: 42100, trend: 'falling' },
  'MRNA': { twitter: 0.78, reddit: 0.75, mentions: 8920, trend: 'rising' },
  'SPY':  { twitter: 0.52, reddit: 0.55, mentions: 35600, trend: 'stable' }
};

async function main() {
  console.log('='.repeat(70));
  console.log('News-Driven Trading - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Display configuration
  console.log('1. Trading Configuration:');
  console.log('-'.repeat(70));
  console.log(`   Reaction Window:   ${newsConfig.trading.newsReactionWindow}s (${newsConfig.trading.newsReactionWindow / 60}min)`);
  console.log(`   Max Position:      ${newsConfig.trading.maxPositionSize * 100}%`);
  console.log(`   Stop Loss:         ${newsConfig.trading.stopLoss * 100}%`);
  console.log(`   Take Profit:       ${newsConfig.trading.takeProfit * 100}%`);
  console.log(`   News Sources:      ${newsConfig.sources.join(', ')}`);
  console.log();

  // 2. News feed analysis
  console.log('2. Live News Feed Analysis:');
  console.log('='.repeat(70));

  for (const news of sampleNews) {
    console.log();
    console.log(`   📰 ${news.headline}`);
    console.log('-'.repeat(70));
    console.log(`   Time: ${news.timestamp} | Source: ${news.source} | Type: ${news.eventType}`);
    console.log(`   Symbols: ${news.symbols.join(', ')}`);
    console.log();

    // Sentiment analysis
    const sentimentLabel = getSentimentLabel(news.sentiment.score);
    console.log('   Sentiment Analysis:');
    console.log(`   Score: ${news.sentiment.score.toFixed(2)} (${sentimentLabel})`);
    console.log(`   Magnitude: ${news.sentiment.magnitude.toFixed(2)}`);
    console.log(`   Keywords: ${news.sentiment.keywords.join(', ')}`);
    console.log();

    // Price impact analysis
    console.log('   Price Impact Analysis:');
    console.log('   Window     | Expected Move | Confidence | Signal');
    console.log('   ' + '-'.repeat(50));

    const impacts = [
      { window: 'Immediate', move: news.priceImpact.immediate, conf: 0.85 },
      { window: 'T+5 min', move: news.priceImpact.t5min, conf: 0.78 },
      { window: 'T+15 min', move: news.priceImpact.t15min, conf: 0.65 },
      { window: 'T+1 hour', move: news.priceImpact.t1hour, conf: 0.52 }
    ];

    impacts.forEach(impact => {
      const moveStr = impact.move >= 0 ? `+${(impact.move * 100).toFixed(2)}%` : `${(impact.move * 100).toFixed(2)}%`;
      const signal = getSignal(impact.move, impact.conf);
      console.log(`   ${impact.window.padEnd(12)} | ${moveStr.padStart(13)} | ${(impact.conf * 100).toFixed(0).padStart(9)}% | ${signal}`);
    });
    console.log();

    // Trading recommendation
    const recommendation = generateRecommendation(news);
    console.log('   📊 Trading Recommendation:');
    console.log(`   Action:    ${recommendation.action.toUpperCase()}`);
    console.log(`   Symbol:    ${recommendation.symbol}`);
    console.log(`   Size:      ${(recommendation.size * 100).toFixed(1)}% of portfolio`);
    console.log(`   Stop Loss: ${(recommendation.stopLoss * 100).toFixed(2)}%`);
    console.log(`   Target:    ${(recommendation.target * 100).toFixed(2)}%`);
    console.log(`   Expected:  ${(recommendation.expectedReturn * 100).toFixed(2)}%`);
  }

  // 3. Social sentiment dashboard
  console.log();
  console.log('3. Social Media Sentiment Dashboard:');
  console.log('='.repeat(70));
  console.log('   Symbol | Twitter | Reddit  | Mentions | Trend   | Combined');
  console.log('-'.repeat(70));

  Object.entries(socialSentiment).forEach(([symbol, data]) => {
    const combined = (data.twitter + data.reddit) / 2;
    const twitterStr = getSentimentEmoji(data.twitter) + ` ${(data.twitter * 100).toFixed(0)}%`;
    const redditStr = getSentimentEmoji(data.reddit) + ` ${(data.reddit * 100).toFixed(0)}%`;

    console.log(`   ${symbol.padEnd(6)} | ${twitterStr.padEnd(7)} | ${redditStr.padEnd(7)} | ${data.mentions.toLocaleString().padStart(8)} | ${data.trend.padEnd(7)} | ${getSentimentLabel(combined)}`);
  });
  console.log();

  // 4. Event-type performance
  console.log('4. Historical Performance by Event Type:');
  console.log('-'.repeat(70));

  const eventStats = calculateEventTypeStats(sampleNews);
  console.log('   Event Type  | Avg Move  | Win Rate | Avg Duration | Best Time');
  console.log('-'.repeat(70));

  Object.entries(eventStats).forEach(([type, stats]) => {
    console.log(`   ${type.padEnd(12)} | ${formatMove(stats.avgMove).padStart(9)} | ${(stats.winRate * 100).toFixed(0).padStart(7)}% | ${stats.avgDuration.padStart(12)} | ${stats.bestTime}`);
  });
  console.log();

  // 5. Pattern recognition
  console.log('5. Historical Pattern Recognition (RuVector):');
  console.log('-'.repeat(70));

  const patterns = findSimilarPatterns(sampleNews[0]);
  console.log(`   Finding patterns similar to: "${sampleNews[0].headline.substring(0, 50)}..."`);
  console.log();
  console.log('   Similar Events:');
  patterns.forEach((pattern, i) => {
    console.log(`   ${i + 1}. ${pattern.headline.substring(0, 45)}...`);
    console.log(`      Date: ${pattern.date} | Move: ${formatMove(pattern.move)} | Similarity: ${(pattern.similarity * 100).toFixed(0)}%`);
  });
  console.log();

  // 6. Real-time alerts
  console.log('6. Alert Configuration:');
  console.log('-'.repeat(70));
  console.log('   Active Alerts:');
  console.log('   - Earnings beats/misses > 5%');
  console.log('   - FDA decisions for watchlist stocks');
  console.log('   - Sentiment score change > 0.3 in 15 min');
  console.log('   - Unusual social media volume (3x average)');
  console.log('   - Breaking news with magnitude > 0.8');
  console.log();

  // 7. Performance summary
  console.log('7. News Trading Performance Summary:');
  console.log('-'.repeat(70));

  const perfSummary = calculatePerformance(sampleNews);
  console.log(`   Total Trades:      ${perfSummary.totalTrades}`);
  console.log(`   Win Rate:          ${(perfSummary.winRate * 100).toFixed(1)}%`);
  console.log(`   Avg Winner:        +${(perfSummary.avgWin * 100).toFixed(2)}%`);
  console.log(`   Avg Loser:         ${(perfSummary.avgLoss * 100).toFixed(2)}%`);
  console.log(`   Profit Factor:     ${perfSummary.profitFactor.toFixed(2)}`);
  console.log(`   Sharpe (news):     ${perfSummary.sharpe.toFixed(2)}`);
  console.log(`   Best Event Type:   ${perfSummary.bestEventType}`);
  console.log();

  console.log('='.repeat(70));
  console.log('News-driven trading analysis completed!');
  console.log('='.repeat(70));
}

// Get sentiment label from score
function getSentimentLabel(score) {
  if (score >= newsConfig.sentiment.strongBullish) return 'STRONG BULLISH';
  if (score >= newsConfig.sentiment.bullish) return 'Bullish';
  if (score <= newsConfig.sentiment.strongBearish) return 'STRONG BEARISH';
  if (score <= newsConfig.sentiment.bearish) return 'Bearish';
  return 'Neutral';
}

// Get sentiment emoji
function getSentimentEmoji(score) {
  if (score >= 0.7) return '🟢';
  if (score >= 0.55) return '🟡';
  if (score <= 0.3) return '🔴';
  if (score <= 0.45) return '🟠';
  return '⚪';
}

// Get trading signal
function getSignal(move, confidence) {
  const absMove = Math.abs(move);
  if (absMove < 0.01 || confidence < 0.5) return 'HOLD';
  if (move > 0 && confidence > 0.7) return '🟢 LONG';
  if (move > 0) return '🟡 WEAK LONG';
  if (move < 0 && confidence > 0.7) return '🔴 SHORT';
  return '🟠 WEAK SHORT';
}

// Format price move
function formatMove(move) {
  return move >= 0 ? `+${(move * 100).toFixed(2)}%` : `${(move * 100).toFixed(2)}%`;
}

// Generate trading recommendation
function generateRecommendation(news) {
  const mainSymbol = news.symbols[0];
  const sentiment = news.sentiment.score;
  const magnitude = news.sentiment.magnitude;
  const expectedMove = news.priceImpact.t5min;

  let action = 'HOLD';
  let size = 0;

  if (sentiment >= newsConfig.sentiment.bullish && magnitude > 0.7) {
    action = 'BUY';
    size = Math.min(magnitude * newsConfig.trading.maxPositionSize, newsConfig.trading.maxPositionSize);
  } else if (sentiment <= newsConfig.sentiment.bearish && magnitude > 0.7) {
    action = 'SHORT';
    size = Math.min(magnitude * newsConfig.trading.maxPositionSize, newsConfig.trading.maxPositionSize);
  }

  return {
    action,
    symbol: mainSymbol,
    size,
    stopLoss: action === 'BUY' ? -newsConfig.trading.stopLoss : newsConfig.trading.stopLoss,
    target: action === 'BUY' ? newsConfig.trading.takeProfit : -newsConfig.trading.takeProfit,
    expectedReturn: expectedMove * (action === 'SHORT' ? -1 : 1)
  };
}

// Calculate event type statistics
function calculateEventTypeStats(news) {
  const stats = {
    earnings: { avgMove: 0.042, winRate: 0.68, avgDuration: '45 min', bestTime: 'Pre-market' },
    fda: { avgMove: 0.085, winRate: 0.72, avgDuration: '2-4 hours', bestTime: 'Any' },
    macro: { avgMove: 0.015, winRate: 0.55, avgDuration: '1-2 hours', bestTime: 'Morning' },
    merger: { avgMove: 0.12, winRate: 0.65, avgDuration: '1 day', bestTime: 'Pre-market' },
    product: { avgMove: 0.048, winRate: 0.62, avgDuration: '1 hour', bestTime: 'Market hours' },
    legal: { avgMove: -0.025, winRate: 0.45, avgDuration: '30 min', bestTime: 'Any' }
  };
  return stats;
}

// Find similar historical patterns
function findSimilarPatterns(currentNews) {
  // Simulated pattern matching (RuVector integration)
  return [
    { headline: 'Apple Q3 2024 earnings beat by 6%, iPhone sales strong', date: '2024-08-01', move: 0.038, similarity: 0.92 },
    { headline: 'Apple Q2 2024 revenue exceeds expectations', date: '2024-05-02', move: 0.029, similarity: 0.87 },
    { headline: 'Apple Q4 2023 sets new revenue record', date: '2023-11-02', move: 0.045, similarity: 0.84 },
    { headline: 'Apple Services revenue beats by 10%', date: '2024-02-01', move: 0.032, similarity: 0.79 }
  ];
}

// Calculate overall performance
function calculatePerformance(news) {
  return {
    totalTrades: 127,
    winRate: 0.64,
    avgWin: 0.032,
    avgLoss: -0.018,
    profitFactor: 1.85,
    sharpe: 1.92,
    bestEventType: 'FDA Approvals'
  };
}

// Run the example
main().catch(console.error);
