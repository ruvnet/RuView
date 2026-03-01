/**
 * Risk Management Layer
 *
 * Comprehensive risk controls for trading systems:
 * - Position limits (per-asset and portfolio)
 * - Stop-loss orders (fixed, trailing, volatility-based)
 * - Circuit breakers (drawdown, loss rate, volatility)
 * - Exposure management
 * - Correlation risk
 * - Leverage control
 */

// Risk Management Configuration
const riskConfig = {
  // Position limits
  positions: {
    maxPositionSize: 0.10,      // Max 10% per position
    maxPositionValue: 50000,    // Max $50k per position
    minPositionSize: 0.01,      // Min 1% position
    maxOpenPositions: 20,       // Max concurrent positions
    maxSectorExposure: 0.30,    // Max 30% per sector
    maxCorrelatedExposure: 0.40 // Max 40% in correlated assets
  },

  // Portfolio limits
  portfolio: {
    maxLongExposure: 1.0,       // Max 100% long
    maxShortExposure: 0.5,      // Max 50% short
    maxGrossExposure: 1.5,      // Max 150% gross
    maxNetExposure: 1.0,        // Max 100% net
    maxLeverage: 2.0,           // Max 2x leverage
    minCashReserve: 0.05        // Keep 5% cash
  },

  // Stop-loss settings
  stopLoss: {
    defaultType: 'trailing',    // fixed, trailing, volatility
    fixedPercent: 0.05,         // 5% fixed stop
    trailingPercent: 0.03,      // 3% trailing stop
    volatilityMultiplier: 2.0,  // 2x ATR for vol stop
    maxLossPerTrade: 0.02,      // Max 2% loss per trade
    maxDailyLoss: 0.05          // Max 5% daily loss
  },

  // Circuit breakers
  circuitBreakers: {
    drawdownThreshold: 0.10,    // 10% drawdown triggers
    drawdownCooldown: 86400000, // 24h cooldown
    lossRateThreshold: 0.70,    // 70% loss rate in window
    lossRateWindow: 20,         // 20 trade window
    volatilityThreshold: 0.04,  // 4% daily vol threshold
    volatilityMultiplier: 3.0,  // 3x normal vol
    consecutiveLosses: 5        // 5 consecutive losses
  },

  // Risk scoring
  scoring: {
    updateFrequency: 60000,     // Update every minute
    historyWindow: 252,         // 1 year of daily data
    correlationThreshold: 0.7   // High correlation threshold
  }
};

/**
 * Stop-Loss Manager
 */
class StopLossManager {
  constructor(config = riskConfig.stopLoss) {
    this.config = config;
    this.stops = new Map();  // symbol -> stop config
    this.volatility = new Map();  // symbol -> ATR
  }

  // Set stop-loss for a position
  setStop(symbol, entryPrice, type = null, params = {}) {
    const stopType = type || this.config.defaultType;
    let stopPrice;

    switch (stopType) {
      case 'fixed':
        stopPrice = entryPrice * (1 - this.config.fixedPercent);
        break;

      case 'trailing':
        stopPrice = entryPrice * (1 - this.config.trailingPercent);
        break;

      case 'volatility':
        const atr = this.volatility.get(symbol) || entryPrice * 0.02;
        stopPrice = entryPrice - (atr * this.config.volatilityMultiplier);
        break;

      default:
        stopPrice = entryPrice * (1 - this.config.fixedPercent);
    }

    this.stops.set(symbol, {
      type: stopType,
      entryPrice,
      stopPrice,
      highWaterMark: entryPrice,
      params,
      createdAt: Date.now()
    });

    return this.stops.get(symbol);
  }

  // Update trailing stop with new price
  updateTrailingStop(symbol, currentPrice) {
    const stop = this.stops.get(symbol);
    if (!stop || stop.type !== 'trailing') return null;

    if (currentPrice > stop.highWaterMark) {
      stop.highWaterMark = currentPrice;
      stop.stopPrice = currentPrice * (1 - this.config.trailingPercent);
    }

    return stop;
  }

  // Check if stop is triggered
  checkStop(symbol, currentPrice) {
    const stop = this.stops.get(symbol);
    if (!stop) return { triggered: false };

    // Update trailing stop first
    if (stop.type === 'trailing') {
      this.updateTrailingStop(symbol, currentPrice);
    }

    const triggered = currentPrice <= stop.stopPrice;

    return {
      triggered,
      stopPrice: stop.stopPrice,
      currentPrice,
      loss: triggered ? (stop.entryPrice - currentPrice) / stop.entryPrice : 0,
      type: stop.type
    };
  }

  // Set volatility for volatility-based stops
  setVolatility(symbol, atr) {
    this.volatility.set(symbol, atr);
  }

  // Remove stop
  removeStop(symbol) {
    this.stops.delete(symbol);
  }

  // Get all active stops
  getActiveStops() {
    return Object.fromEntries(this.stops);
  }
}

/**
 * Circuit Breaker System
 */
class CircuitBreaker {
  constructor(config = riskConfig.circuitBreakers) {
    this.config = config;
    this.state = {
      isTripped: false,
      tripReason: null,
      tripTime: null,
      cooldownUntil: null
    };

    // Tracking data
    this.peakEquity = 0;
    this.currentEquity = 0;
    this.consecutiveLosses = 0;

    // Optimized: Use ring buffers instead of arrays with shift/slice
    const tradeWindowSize = config.lossRateWindow * 2;
    this._tradeBuffer = new Array(tradeWindowSize);
    this._tradeIndex = 0;
    this._tradeCount = 0;
    this._tradeLossCount = 0;  // Track losses incrementally

    this._volBuffer = new Array(20);
    this._volIndex = 0;
    this._volCount = 0;
    this._volSum = 0;  // Running sum for O(1) average
  }

  // Update with new equity value
  updateEquity(equity) {
    this.currentEquity = equity;
    if (equity > this.peakEquity) {
      this.peakEquity = equity;
    }

    // Check drawdown breaker
    const drawdown = (this.peakEquity - equity) / this.peakEquity;
    if (drawdown >= this.config.drawdownThreshold) {
      this.trip('drawdown', `Drawdown ${(drawdown * 100).toFixed(1)}% exceeds threshold`);
    }
  }

  // Optimized: Record trade with O(1) ring buffer
  recordTrade(profit) {
    const bufferSize = this._tradeBuffer.length;
    const windowSize = this.config.lossRateWindow;

    // If overwriting an old trade, adjust loss count
    if (this._tradeCount >= bufferSize) {
      const oldTrade = this._tradeBuffer[this._tradeIndex];
      if (oldTrade && oldTrade.profit < 0) {
        this._tradeLossCount--;
      }
    }

    // Add new trade
    this._tradeBuffer[this._tradeIndex] = { profit, timestamp: Date.now() };
    if (profit < 0) this._tradeLossCount++;

    this._tradeIndex = (this._tradeIndex + 1) % bufferSize;
    if (this._tradeCount < bufferSize) this._tradeCount++;

    // Update consecutive losses
    if (profit < 0) {
      this.consecutiveLosses++;
    } else {
      this.consecutiveLosses = 0;
    }

    // Check loss rate breaker (O(1) using tracked count)
    if (this._tradeCount >= windowSize) {
      // Count losses in recent window
      let recentLosses = 0;
      const startIdx = (this._tradeIndex - windowSize + bufferSize) % bufferSize;
      for (let i = 0; i < windowSize; i++) {
        const idx = (startIdx + i) % bufferSize;
        if (this._tradeBuffer[idx] && this._tradeBuffer[idx].profit < 0) {
          recentLosses++;
        }
      }
      const lossRate = recentLosses / windowSize;

      if (lossRate >= this.config.lossRateThreshold) {
        this.trip('lossRate', `Loss rate ${(lossRate * 100).toFixed(1)}% exceeds threshold`);
      }
    }

    // Check consecutive losses breaker
    if (this.consecutiveLosses >= this.config.consecutiveLosses) {
      this.trip('consecutiveLosses', `${this.consecutiveLosses} consecutive losses`);
    }
  }

  // Optimized: Update volatility with O(1) ring buffer and running sum
  updateVolatility(dailyReturn) {
    const absReturn = Math.abs(dailyReturn);
    const bufferSize = this._volBuffer.length;

    // If overwriting old value, subtract from running sum
    if (this._volCount >= bufferSize) {
      this._volSum -= this._volBuffer[this._volIndex];
    }

    // Add new value
    this._volBuffer[this._volIndex] = absReturn;
    this._volSum += absReturn;

    this._volIndex = (this._volIndex + 1) % bufferSize;
    if (this._volCount < bufferSize) this._volCount++;

    // Check volatility spike (O(1) using running sum)
    if (this._volCount >= 5) {
      const avgVol = (this._volSum - absReturn) / (this._volCount - 1);
      const currentVol = absReturn;

      if (currentVol > avgVol * this.config.volatilityMultiplier ||
          currentVol > this.config.volatilityThreshold) {
        this.trip('volatility', `Volatility spike: ${(currentVol * 100).toFixed(2)}%`);
      }
    }
  }

  // Trip the circuit breaker
  trip(reason, message) {
    if (this.state.isTripped) return;  // Already tripped

    this.state = {
      isTripped: true,
      tripReason: reason,
      tripMessage: message,
      tripTime: Date.now(),
      cooldownUntil: Date.now() + this.config.drawdownCooldown
    };

    console.warn(`🔴 CIRCUIT BREAKER TRIPPED: ${message}`);
  }

  // Check if trading is allowed
  canTrade() {
    if (!this.state.isTripped) return { allowed: true };

    // Check if cooldown has passed
    if (Date.now() >= this.state.cooldownUntil) {
      this.reset();
      return { allowed: true };
    }

    return {
      allowed: false,
      reason: this.state.tripReason,
      message: this.state.tripMessage,
      cooldownRemaining: this.state.cooldownUntil - Date.now()
    };
  }

  // Reset circuit breaker
  reset() {
    this.state = {
      isTripped: false,
      tripReason: null,
      tripTime: null,
      cooldownUntil: null
    };
    this.consecutiveLosses = 0;
    console.log('🟢 Circuit breaker reset');
  }

  // Force reset (manual override)
  forceReset() {
    this.reset();
    this.peakEquity = this.currentEquity;
    // Reset ring buffers
    this._tradeIndex = 0;
    this._tradeCount = 0;
    this._tradeLossCount = 0;
    this._volIndex = 0;
    this._volCount = 0;
    this._volSum = 0;
  }

  getState() {
    return {
      ...this.state,
      drawdown: this.peakEquity > 0 ? (this.peakEquity - this.currentEquity) / this.peakEquity : 0,
      consecutiveLosses: this.consecutiveLosses,
      recentLossRate: this.calculateRecentLossRate()
    };
  }

  // Optimized: O(windowSize) but only called for reporting
  calculateRecentLossRate() {
    const windowSize = this.config.lossRateWindow;
    const count = Math.min(this._tradeCount, windowSize);
    if (count === 0) return 0;

    let losses = 0;
    const bufferSize = this._tradeBuffer.length;
    const startIdx = (this._tradeIndex - count + bufferSize) % bufferSize;

    for (let i = 0; i < count; i++) {
      const idx = (startIdx + i) % bufferSize;
      if (this._tradeBuffer[idx] && this._tradeBuffer[idx].profit < 0) {
        losses++;
      }
    }

    return losses / count;
  }
}

/**
 * Position Limit Manager
 */
class PositionLimitManager {
  constructor(config = riskConfig.positions) {
    this.config = config;
    this.positions = new Map();
    this.sectors = new Map();  // symbol -> sector mapping
  }

  // Set sector for a symbol
  setSector(symbol, sector) {
    this.sectors.set(symbol, sector);
  }

  // Check if position size is allowed
  checkPositionSize(symbol, proposedSize, portfolioValue) {
    const sizePercent = proposedSize / portfolioValue;
    const violations = [];

    // Check max position size
    if (sizePercent > this.config.maxPositionSize) {
      violations.push({
        type: 'maxPositionSize',
        message: `Position ${(sizePercent * 100).toFixed(1)}% exceeds max ${(this.config.maxPositionSize * 100)}%`,
        limit: this.config.maxPositionSize * portfolioValue
      });
    }

    // Check max position value
    if (proposedSize > this.config.maxPositionValue) {
      violations.push({
        type: 'maxPositionValue',
        message: `Position $${proposedSize.toFixed(0)} exceeds max $${this.config.maxPositionValue}`,
        limit: this.config.maxPositionValue
      });
    }

    // Check min position size
    if (sizePercent < this.config.minPositionSize && proposedSize > 0) {
      violations.push({
        type: 'minPositionSize',
        message: `Position ${(sizePercent * 100).toFixed(1)}% below min ${(this.config.minPositionSize * 100)}%`,
        limit: this.config.minPositionSize * portfolioValue
      });
    }

    return {
      allowed: violations.length === 0,
      violations,
      adjustedSize: this.adjustPositionSize(proposedSize, portfolioValue)
    };
  }

  // Adjust position size to comply with limits
  adjustPositionSize(proposedSize, portfolioValue) {
    let adjusted = proposedSize;

    // Apply max position size
    const maxByPercent = portfolioValue * this.config.maxPositionSize;
    adjusted = Math.min(adjusted, maxByPercent);

    // Apply max position value
    adjusted = Math.min(adjusted, this.config.maxPositionValue);

    return adjusted;
  }

  // Check sector exposure
  checkSectorExposure(symbol, proposedSize, currentPositions, portfolioValue) {
    const sector = this.sectors.get(symbol);
    if (!sector) return { allowed: true };

    // Calculate current sector exposure
    let sectorExposure = 0;
    for (const [sym, pos] of Object.entries(currentPositions)) {
      if (this.sectors.get(sym) === sector) {
        sectorExposure += Math.abs(pos.value || 0);
      }
    }

    const totalSectorExposure = (sectorExposure + proposedSize) / portfolioValue;

    if (totalSectorExposure > this.config.maxSectorExposure) {
      return {
        allowed: false,
        message: `Sector ${sector} exposure ${(totalSectorExposure * 100).toFixed(1)}% exceeds max ${(this.config.maxSectorExposure * 100)}%`,
        currentExposure: sectorExposure,
        maxAllowed: this.config.maxSectorExposure * portfolioValue - sectorExposure
      };
    }

    return { allowed: true, sectorExposure: totalSectorExposure };
  }

  // Check number of open positions
  checkPositionCount(currentPositions) {
    const count = Object.keys(currentPositions).filter(s => currentPositions[s].quantity !== 0).length;

    if (count >= this.config.maxOpenPositions) {
      return {
        allowed: false,
        message: `Max open positions (${this.config.maxOpenPositions}) reached`,
        currentCount: count
      };
    }

    return { allowed: true, currentCount: count };
  }
}

/**
 * Exposure Manager
 */
class ExposureManager {
  constructor(config = riskConfig.portfolio) {
    this.config = config;
  }

  // Calculate portfolio exposure
  calculateExposure(positions, portfolioValue) {
    let longExposure = 0;
    let shortExposure = 0;

    for (const pos of Object.values(positions)) {
      const value = pos.value || (pos.quantity * pos.price) || 0;
      if (value > 0) {
        longExposure += value;
      } else {
        shortExposure += Math.abs(value);
      }
    }

    const grossExposure = longExposure + shortExposure;
    const netExposure = longExposure - shortExposure;

    return {
      long: longExposure / portfolioValue,
      short: shortExposure / portfolioValue,
      gross: grossExposure / portfolioValue,
      net: netExposure / portfolioValue,
      leverage: grossExposure / portfolioValue,
      longValue: longExposure,
      shortValue: shortExposure
    };
  }

  // Check if trade would violate exposure limits
  checkExposure(proposedTrade, currentPositions, portfolioValue) {
    // Simulate new exposure
    const newPositions = { ...currentPositions };
    const symbol = proposedTrade.symbol;
    const value = proposedTrade.value || (proposedTrade.quantity * proposedTrade.price);
    const side = proposedTrade.side;

    newPositions[symbol] = {
      ...newPositions[symbol],
      value: (newPositions[symbol]?.value || 0) + (side === 'buy' ? value : -value)
    };

    const exposure = this.calculateExposure(newPositions, portfolioValue);
    const violations = [];

    if (exposure.long > this.config.maxLongExposure) {
      violations.push({
        type: 'maxLongExposure',
        message: `Long exposure ${(exposure.long * 100).toFixed(1)}% exceeds max ${(this.config.maxLongExposure * 100)}%`
      });
    }

    if (exposure.short > this.config.maxShortExposure) {
      violations.push({
        type: 'maxShortExposure',
        message: `Short exposure ${(exposure.short * 100).toFixed(1)}% exceeds max ${(this.config.maxShortExposure * 100)}%`
      });
    }

    if (exposure.gross > this.config.maxGrossExposure) {
      violations.push({
        type: 'maxGrossExposure',
        message: `Gross exposure ${(exposure.gross * 100).toFixed(1)}% exceeds max ${(this.config.maxGrossExposure * 100)}%`
      });
    }

    if (exposure.leverage > this.config.maxLeverage) {
      violations.push({
        type: 'maxLeverage',
        message: `Leverage ${exposure.leverage.toFixed(2)}x exceeds max ${this.config.maxLeverage}x`
      });
    }

    return {
      allowed: violations.length === 0,
      violations,
      currentExposure: this.calculateExposure(currentPositions, portfolioValue),
      projectedExposure: exposure
    };
  }

  // Check cash reserve
  checkCashReserve(cash, portfolioValue) {
    const cashPercent = cash / portfolioValue;

    if (cashPercent < this.config.minCashReserve) {
      return {
        allowed: false,
        message: `Cash reserve ${(cashPercent * 100).toFixed(1)}% below min ${(this.config.minCashReserve * 100)}%`,
        required: this.config.minCashReserve * portfolioValue
      };
    }

    return { allowed: true, cashPercent };
  }
}

/**
 * Risk Manager - Main integration class
 */
class RiskManager {
  constructor(config = riskConfig) {
    this.config = config;
    this.stopLossManager = new StopLossManager(config.stopLoss);
    this.circuitBreaker = new CircuitBreaker(config.circuitBreakers);
    this.positionLimits = new PositionLimitManager(config.positions);
    this.exposureManager = new ExposureManager(config.portfolio);

    // State
    this.blockedSymbols = new Set();
    this.dailyLoss = 0;
    this.dailyStartEquity = 0;
  }

  // Initialize for trading day
  startDay(equity) {
    this.dailyStartEquity = equity;
    this.dailyLoss = 0;
  }

  // Main check - can this trade be executed?
  canTrade(symbol, trade, portfolio) {
    const results = {
      allowed: true,
      checks: {},
      warnings: [],
      adjustments: {}
    };

    // Check circuit breaker
    const circuitCheck = this.circuitBreaker.canTrade();
    results.checks.circuitBreaker = circuitCheck;
    if (!circuitCheck.allowed) {
      results.allowed = false;
      return results;
    }

    // Check if symbol is blocked
    if (this.blockedSymbols.has(symbol)) {
      results.allowed = false;
      results.checks.blocked = { allowed: false, message: `Symbol ${symbol} is blocked` };
      return results;
    }

    // Check position limits
    const positionCheck = this.positionLimits.checkPositionSize(
      symbol,
      trade.value,
      portfolio.equity
    );
    results.checks.positionSize = positionCheck;
    if (!positionCheck.allowed) {
      results.warnings.push(...positionCheck.violations.map(v => v.message));
      results.adjustments.size = positionCheck.adjustedSize;
    }

    // Check position count
    const countCheck = this.positionLimits.checkPositionCount(portfolio.positions);
    results.checks.positionCount = countCheck;
    if (!countCheck.allowed) {
      results.allowed = false;
      return results;
    }

    // Check sector exposure
    const sectorCheck = this.positionLimits.checkSectorExposure(
      symbol,
      trade.value,
      portfolio.positions,
      portfolio.equity
    );
    results.checks.sectorExposure = sectorCheck;
    if (!sectorCheck.allowed) {
      results.warnings.push(sectorCheck.message);
    }

    // Check portfolio exposure
    const exposureCheck = this.exposureManager.checkExposure(
      trade,
      portfolio.positions,
      portfolio.equity
    );
    results.checks.exposure = exposureCheck;
    if (!exposureCheck.allowed) {
      results.allowed = false;
      return results;
    }

    // Check cash reserve
    const cashAfterTrade = portfolio.cash - trade.value;
    const cashCheck = this.exposureManager.checkCashReserve(cashAfterTrade, portfolio.equity);
    results.checks.cashReserve = cashCheck;
    if (!cashCheck.allowed) {
      results.warnings.push(cashCheck.message);
    }

    // Check daily loss limit
    const dailyLossCheck = this.checkDailyLoss(portfolio.equity);
    results.checks.dailyLoss = dailyLossCheck;
    if (!dailyLossCheck.allowed) {
      results.allowed = false;
      return results;
    }

    return results;
  }

  // Check daily loss limit
  checkDailyLoss(currentEquity) {
    if (this.dailyStartEquity === 0) return { allowed: true };

    const dailyReturn = (currentEquity - this.dailyStartEquity) / this.dailyStartEquity;

    if (dailyReturn < -this.config.stopLoss.maxDailyLoss) {
      return {
        allowed: false,
        message: `Daily loss ${(Math.abs(dailyReturn) * 100).toFixed(1)}% exceeds max ${(this.config.stopLoss.maxDailyLoss * 100)}%`,
        dailyLoss: dailyReturn
      };
    }

    return { allowed: true, dailyLoss: dailyReturn };
  }

  // Set stop-loss for a position
  setStopLoss(symbol, entryPrice, type, params) {
    return this.stopLossManager.setStop(symbol, entryPrice, type, params);
  }

  // Check all stops
  checkAllStops(prices) {
    const triggered = [];

    for (const [symbol, price] of Object.entries(prices)) {
      const check = this.stopLossManager.checkStop(symbol, price);
      if (check.triggered) {
        triggered.push({ symbol, ...check });
      }
    }

    return triggered;
  }

  // Update circuit breaker with equity
  updateEquity(equity) {
    this.circuitBreaker.updateEquity(equity);
  }

  // Record trade for circuit breaker
  recordTrade(profit) {
    this.circuitBreaker.recordTrade(profit);
  }

  // Block a symbol
  blockSymbol(symbol, reason) {
    this.blockedSymbols.add(symbol);
    console.warn(`🚫 Symbol ${symbol} blocked: ${reason}`);
  }

  // Unblock a symbol
  unblockSymbol(symbol) {
    this.blockedSymbols.delete(symbol);
  }

  // Get full risk report
  getRiskReport(portfolio) {
    const exposure = this.exposureManager.calculateExposure(portfolio.positions, portfolio.equity);

    return {
      circuitBreaker: this.circuitBreaker.getState(),
      exposure,
      stops: this.stopLossManager.getActiveStops(),
      blockedSymbols: [...this.blockedSymbols],
      dailyLoss: this.checkDailyLoss(portfolio.equity),
      limits: {
        maxPositionSize: this.config.positions.maxPositionSize,
        maxLeverage: this.config.portfolio.maxLeverage,
        maxDrawdown: this.config.circuitBreakers.drawdownThreshold
      }
    };
  }
}

// Exports
export {
  RiskManager,
  StopLossManager,
  CircuitBreaker,
  PositionLimitManager,
  ExposureManager,
  riskConfig
};

// Demo if run directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('RISK MANAGEMENT LAYER');
  console.log('══════════════════════════════════════════════════════════════════════\n');

  const riskManager = new RiskManager();

  // Initialize for trading day
  const portfolio = {
    equity: 100000,
    cash: 50000,
    positions: {
      AAPL: { quantity: 100, price: 150, value: 15000 },
      MSFT: { quantity: 50, price: 300, value: 15000 }
    }
  };

  riskManager.startDay(portfolio.equity);

  console.log('1. Portfolio Status:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log(`   Equity: $${portfolio.equity.toLocaleString()}`);
  console.log(`   Cash: $${portfolio.cash.toLocaleString()}`);
  console.log(`   Positions: ${Object.keys(portfolio.positions).length}`);
  console.log();

  console.log('2. Trade Check - Buy $20,000 GOOGL:');
  console.log('──────────────────────────────────────────────────────────────────────');
  const trade1 = { symbol: 'GOOGL', side: 'buy', value: 20000, quantity: 100, price: 200 };
  const check1 = riskManager.canTrade('GOOGL', trade1, portfolio);
  console.log(`   Allowed: ${check1.allowed ? '✓ Yes' : '✗ No'}`);
  if (check1.warnings.length > 0) {
    console.log(`   Warnings: ${check1.warnings.join(', ')}`);
  }
  console.log();

  console.log('3. Trade Check - Buy $60,000 TSLA (exceeds limits):');
  console.log('──────────────────────────────────────────────────────────────────────');
  const trade2 = { symbol: 'TSLA', side: 'buy', value: 60000, quantity: 300, price: 200 };
  const check2 = riskManager.canTrade('TSLA', trade2, portfolio);
  console.log(`   Allowed: ${check2.allowed ? '✓ Yes' : '✗ No'}`);
  if (check2.checks.positionSize?.violations) {
    for (const v of check2.checks.positionSize.violations) {
      console.log(`   Violation: ${v.message}`);
    }
  }
  if (check2.adjustments.size) {
    console.log(`   Adjusted Size: $${check2.adjustments.size.toLocaleString()}`);
  }
  console.log();

  console.log('4. Stop-Loss Management:');
  console.log('──────────────────────────────────────────────────────────────────────');
  const stop = riskManager.setStopLoss('AAPL', 150, 'trailing');
  console.log(`   AAPL trailing stop set at $${stop.stopPrice.toFixed(2)}`);

  // Simulate price movement
  riskManager.stopLossManager.updateTrailingStop('AAPL', 160);  // Price went up
  const updatedStop = riskManager.stopLossManager.stops.get('AAPL');
  console.log(`   After price rise to $160: stop at $${updatedStop.stopPrice.toFixed(2)}`);

  const stopCheck = riskManager.stopLossManager.checkStop('AAPL', 145);  // Price dropped
  console.log(`   Check at $145: ${stopCheck.triggered ? '🔴 TRIGGERED' : '🟢 OK'}`);
  console.log();

  console.log('5. Circuit Breaker Test:');
  console.log('──────────────────────────────────────────────────────────────────────');
  // Simulate losses
  for (let i = 0; i < 4; i++) {
    riskManager.recordTrade(-500);
  }
  console.log(`   4 losing trades recorded`);
  console.log(`   Consecutive losses: ${riskManager.circuitBreaker.consecutiveLosses}`);

  riskManager.recordTrade(-500);  // 5th loss
  const cbState = riskManager.circuitBreaker.getState();
  console.log(`   5th loss recorded`);
  console.log(`   Circuit breaker: ${cbState.isTripped ? '🔴 TRIPPED' : '🟢 OK'}`);
  if (cbState.isTripped) {
    console.log(`   Reason: ${cbState.tripMessage}`);
  }
  console.log();

  console.log('6. Risk Report:');
  console.log('──────────────────────────────────────────────────────────────────────');
  riskManager.circuitBreaker.forceReset();  // Reset for demo
  const report = riskManager.getRiskReport(portfolio);
  console.log(`   Long Exposure: ${(report.exposure.long * 100).toFixed(1)}%`);
  console.log(`   Short Exposure: ${(report.exposure.short * 100).toFixed(1)}%`);
  console.log(`   Gross Exposure: ${(report.exposure.gross * 100).toFixed(1)}%`);
  console.log(`   Leverage: ${report.exposure.leverage.toFixed(2)}x`);
  console.log(`   Circuit Breaker: ${report.circuitBreaker.isTripped ? 'TRIPPED' : 'OK'}`);
  console.log(`   Active Stops: ${Object.keys(report.stops).length}`);

  console.log();
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('Risk management layer ready');
  console.log('══════════════════════════════════════════════════════════════════════');
}
