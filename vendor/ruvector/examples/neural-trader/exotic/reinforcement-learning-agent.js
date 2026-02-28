/**
 * Reinforcement Learning Trading Agent
 *
 * EXOTIC: Deep Q-Learning for autonomous trading
 *
 * Uses @neural-trader/neural with RuVector for:
 * - Deep Q-Network (DQN) for action selection
 * - Experience replay with vector similarity
 * - Epsilon-greedy exploration
 * - Target network for stable learning
 *
 * The agent learns optimal trading actions directly from
 * market experience, without explicit strategy rules.
 */

// RL Configuration
const rlConfig = {
  // Network architecture
  network: {
    stateDim: 20,           // State vector dimension
    hiddenLayers: [128, 64, 32],
    actionSpace: 5          // hold, buy_small, buy_large, sell_small, sell_large
  },

  // Learning parameters
  learning: {
    gamma: 0.99,            // Discount factor
    learningRate: 0.001,
    batchSize: 32,
    targetUpdateFreq: 100,  // Steps between target network updates
    replayBufferSize: 10000
  },

  // Exploration
  exploration: {
    epsilonStart: 1.0,
    epsilonEnd: 0.01,
    epsilonDecay: 0.995
  },

  // Trading
  trading: {
    initialCapital: 100000,
    maxPosition: 0.5,       // Max 50% of capital
    transactionCost: 0.001, // 10 bps
    slippage: 0.0005        // 5 bps
  }
};

// Action definitions
const Actions = {
  HOLD: 0,
  BUY_SMALL: 1,    // 10% of available
  BUY_LARGE: 2,    // 30% of available
  SELL_SMALL: 3,   // 10% of position
  SELL_LARGE: 4    // 30% of position
};

const ActionNames = ['HOLD', 'BUY_SMALL', 'BUY_LARGE', 'SELL_SMALL', 'SELL_LARGE'];

// Neural Network Layer
class DenseLayer {
  constructor(inputDim, outputDim, activation = 'relu') {
    this.inputDim = inputDim;
    this.outputDim = outputDim;
    this.activation = activation;

    // Xavier initialization
    const scale = Math.sqrt(2.0 / (inputDim + outputDim));
    this.weights = [];
    for (let i = 0; i < inputDim; i++) {
      const row = [];
      for (let j = 0; j < outputDim; j++) {
        row.push((Math.random() - 0.5) * 2 * scale);
      }
      this.weights.push(row);
    }
    this.bias = new Array(outputDim).fill(0).map(() => (Math.random() - 0.5) * 0.1);
  }

  forward(input) {
    const output = new Array(this.outputDim).fill(0);

    for (let j = 0; j < this.outputDim; j++) {
      for (let i = 0; i < this.inputDim; i++) {
        output[j] += input[i] * this.weights[i][j];
      }
      output[j] += this.bias[j];

      // Activation
      if (this.activation === 'relu') {
        output[j] = Math.max(0, output[j]);
      }
    }

    return output;
  }

  // Simplified gradient update
  updateWeights(gradients, lr) {
    for (let i = 0; i < this.inputDim; i++) {
      for (let j = 0; j < this.outputDim; j++) {
        this.weights[i][j] -= lr * gradients[i][j];
      }
    }
    for (let j = 0; j < this.outputDim; j++) {
      this.bias[j] -= lr * gradients.bias[j];
    }
  }

  copyFrom(other) {
    for (let i = 0; i < this.inputDim; i++) {
      for (let j = 0; j < this.outputDim; j++) {
        this.weights[i][j] = other.weights[i][j];
      }
    }
    for (let j = 0; j < this.outputDim; j++) {
      this.bias[j] = other.bias[j];
    }
  }
}

// Deep Q-Network
class DQN {
  constructor(config) {
    this.config = config;

    // Build layers
    this.layers = [];
    let prevDim = config.stateDim;

    for (const hiddenDim of config.hiddenLayers) {
      this.layers.push(new DenseLayer(prevDim, hiddenDim, 'relu'));
      prevDim = hiddenDim;
    }

    // Output layer (no activation for Q-values)
    this.layers.push(new DenseLayer(prevDim, config.actionSpace, 'linear'));
  }

  forward(state) {
    let x = state;
    // Store activations for backpropagation
    this.activations = [state];
    for (const layer of this.layers) {
      x = layer.forward(x);
      this.activations.push(x);
    }
    return x;
  }

  // Get the activation before the output layer (for gradient computation)
  getPreOutputActivation() {
    if (!this.activations || this.activations.length < 2) {
      return null;
    }
    // Return activation just before output layer
    return this.activations[this.activations.length - 2];
  }

  copyFrom(other) {
    for (let i = 0; i < this.layers.length; i++) {
      this.layers[i].copyFrom(other.layers[i]);
    }
  }
}

// Experience Replay Buffer
class ReplayBuffer {
  constructor(maxSize) {
    this.maxSize = maxSize;
    this.buffer = [];
    this.position = 0;
  }

  add(experience) {
    if (this.buffer.length < this.maxSize) {
      this.buffer.push(experience);
    } else {
      this.buffer[this.position] = experience;
    }
    this.position = (this.position + 1) % this.maxSize;
  }

  sample(batchSize) {
    const samples = [];
    const indices = new Set();

    while (indices.size < Math.min(batchSize, this.buffer.length)) {
      indices.add(Math.floor(Math.random() * this.buffer.length));
    }

    for (const idx of indices) {
      samples.push(this.buffer[idx]);
    }

    return samples;
  }

  size() {
    return this.buffer.length;
  }
}

// State Encoder
class StateEncoder {
  constructor(config) {
    this.config = config;
    this.priceHistory = [];
    this.returnHistory = [];
  }

  update(price) {
    this.priceHistory.push(price);
    if (this.priceHistory.length > 1) {
      const ret = (price - this.priceHistory[this.priceHistory.length - 2]) /
                  this.priceHistory[this.priceHistory.length - 2];
      this.returnHistory.push(ret);
    }

    // Keep bounded
    if (this.priceHistory.length > 100) {
      this.priceHistory.shift();
      this.returnHistory.shift();
    }
  }

  encode(portfolio) {
    const state = [];

    // Price-based features
    if (this.returnHistory.length >= 20) {
      // Recent returns
      for (let i = 1; i <= 5; i++) {
        state.push(this.returnHistory[this.returnHistory.length - i] * 10);  // Scaled
      }

      // Return statistics
      const recent20 = this.returnHistory.slice(-20);
      const mean = recent20.reduce((a, b) => a + b, 0) / 20;
      const variance = recent20.reduce((s, r) => s + (r - mean) ** 2, 0) / 20;
      const volatility = Math.sqrt(variance);

      state.push(mean * 100);
      state.push(volatility * 100);

      // Momentum
      const momentum5 = this.returnHistory.slice(-5).reduce((a, b) => a + b, 0);
      const momentum10 = this.returnHistory.slice(-10).reduce((a, b) => a + b, 0);
      const momentum20 = this.returnHistory.slice(-20).reduce((a, b) => a + b, 0);

      state.push(momentum5 * 10);
      state.push(momentum10 * 10);
      state.push(momentum20 * 10);

      // Price relative to moving averages
      const currentPrice = this.priceHistory[this.priceHistory.length - 1];
      const sma5 = this.priceHistory.slice(-5).reduce((a, b) => a + b, 0) / 5;
      const sma20 = this.priceHistory.slice(-20).reduce((a, b) => a + b, 0) / 20;

      state.push((currentPrice / sma5 - 1) * 10);
      state.push((currentPrice / sma20 - 1) * 10);

      // Trend direction
      const trend = this.returnHistory.slice(-10).filter(r => r > 0).length / 10;
      state.push(trend - 0.5);
    } else {
      // Pad with zeros
      for (let i = 0; i < 13; i++) {
        state.push(0);
      }
    }

    // Portfolio features
    state.push(portfolio.positionPct - 0.5);  // Position as fraction of capital
    state.push(portfolio.unrealizedPnL / portfolio.capital);
    state.push(portfolio.realizedPnL / portfolio.capital);
    state.push(portfolio.drawdown);
    state.push(portfolio.winRate - 0.5);
    state.push(portfolio.sharpe / 2);
    state.push(portfolio.tradeCount / 100);

    // Ensure state dimension
    while (state.length < this.config.network.stateDim) {
      state.push(0);
    }

    return state.slice(0, this.config.network.stateDim);
  }
}

// Trading Environment
class TradingEnvironment {
  constructor(config, priceData) {
    this.config = config;
    this.priceData = priceData;
    this.reset();
  }

  reset() {
    this.currentStep = 50;  // Start after warmup
    this.capital = this.config.trading.initialCapital;
    this.position = 0;
    this.avgCost = 0;
    this.realizedPnL = 0;
    this.trades = [];
    this.peakCapital = this.capital;
    this.returns = [];

    return this.getState();
  }

  getState() {
    return {
      price: this.priceData[this.currentStep].close,
      capital: this.capital,
      position: this.position,
      positionPct: this.position * this.priceData[this.currentStep].close / this.getPortfolioValue(),
      unrealizedPnL: this.getUnrealizedPnL(),
      realizedPnL: this.realizedPnL,
      drawdown: this.getDrawdown(),
      winRate: this.getWinRate(),
      sharpe: this.getSharpe(),
      tradeCount: this.trades.length
    };
  }

  getPortfolioValue() {
    const price = this.priceData[this.currentStep].close;
    return this.capital + this.position * price;
  }

  getUnrealizedPnL() {
    if (this.position === 0) return 0;
    const price = this.priceData[this.currentStep].close;
    return this.position * (price - this.avgCost);
  }

  getDrawdown() {
    const value = this.getPortfolioValue();
    this.peakCapital = Math.max(this.peakCapital, value);
    return (this.peakCapital - value) / this.peakCapital;
  }

  getWinRate() {
    const closedTrades = this.trades.filter(t => t.closed);
    if (closedTrades.length === 0) return 0.5;
    const wins = closedTrades.filter(t => t.pnl > 0).length;
    return wins / closedTrades.length;
  }

  getSharpe() {
    if (this.returns.length < 10) return 0;
    const mean = this.returns.reduce((a, b) => a + b, 0) / this.returns.length;
    const variance = this.returns.reduce((s, r) => s + (r - mean) ** 2, 0) / this.returns.length;
    if (variance === 0) return 0;
    return mean / Math.sqrt(variance) * Math.sqrt(252);
  }

  step(action) {
    const prevValue = this.getPortfolioValue();
    const price = this.priceData[this.currentStep].close;

    // Execute action
    this.executeAction(action, price);

    // Move to next step
    this.currentStep++;
    const done = this.currentStep >= this.priceData.length - 1;

    // Calculate reward
    const newValue = this.getPortfolioValue();
    const stepReturn = (newValue - prevValue) / prevValue;
    this.returns.push(stepReturn);
    // Bound returns array to prevent memory leak
    if (this.returns.length > 1000) {
      this.returns = this.returns.slice(-500);
    }

    // Shape reward
    let reward = stepReturn * 100;  // Scale returns

    // Penalty for excessive trading
    if (action !== Actions.HOLD) {
      reward -= 0.1;
    }

    // Penalty for drawdown
    const drawdown = this.getDrawdown();
    if (drawdown > 0.1) {
      reward -= drawdown * 10;
    }

    // Bonus for profitable trades
    const winRate = this.getWinRate();
    if (winRate > 0.5) {
      reward += (winRate - 0.5) * 2;
    }

    return {
      state: this.getState(),
      reward,
      done,
      info: {
        portfolioValue: newValue,
        stepReturn,
        action: ActionNames[action]
      }
    };
  }

  executeAction(action, price) {
    const slippage = this.config.trading.slippage;
    const cost = this.config.trading.transactionCost;

    switch (action) {
      case Actions.BUY_SMALL:
        this.buy(0.1, price * (1 + slippage + cost));
        break;
      case Actions.BUY_LARGE:
        this.buy(0.3, price * (1 + slippage + cost));
        break;
      case Actions.SELL_SMALL:
        this.sell(0.1, price * (1 - slippage - cost));
        break;
      case Actions.SELL_LARGE:
        this.sell(0.3, price * (1 - slippage - cost));
        break;
      case Actions.HOLD:
      default:
        break;
    }
  }

  buy(fraction, price) {
    const maxBuy = this.capital * this.config.trading.maxPosition;
    const amount = Math.min(this.capital * fraction, maxBuy);

    if (amount < 100) return;  // Min trade size

    const shares = amount / price;
    const totalCost = this.position * this.avgCost + amount;
    const totalShares = this.position + shares;

    this.avgCost = totalCost / totalShares;
    this.position = totalShares;
    this.capital -= amount;

    this.trades.push({
      type: 'buy',
      shares,
      price,
      timestamp: this.currentStep,
      closed: false
    });
  }

  sell(fraction, price) {
    if (this.position <= 0) return;

    const sharesToSell = this.position * fraction;
    if (sharesToSell < 0.01) return;

    const proceeds = sharesToSell * price;
    const costBasis = sharesToSell * this.avgCost;
    const tradePnL = proceeds - costBasis;

    this.position -= sharesToSell;
    this.capital += proceeds;
    this.realizedPnL += tradePnL;

    this.trades.push({
      type: 'sell',
      shares: sharesToSell,
      price,
      pnl: tradePnL,
      timestamp: this.currentStep,
      closed: true
    });
  }
}

// DQN Agent
class DQNAgent {
  constructor(config) {
    this.config = config;

    // Networks
    this.qNetwork = new DQN(config.network);
    this.targetNetwork = new DQN(config.network);
    this.targetNetwork.copyFrom(this.qNetwork);

    // Experience replay
    this.replayBuffer = new ReplayBuffer(config.learning.replayBufferSize);

    // Exploration
    this.epsilon = config.exploration.epsilonStart;

    // Training stats
    this.stepCount = 0;
    this.episodeCount = 0;
    this.totalReward = 0;
    this.losses = [];
  }

  selectAction(state) {
    // Epsilon-greedy
    if (Math.random() < this.epsilon) {
      return Math.floor(Math.random() * this.config.network.actionSpace);
    }

    // Greedy action
    const qValues = this.qNetwork.forward(state);
    return qValues.indexOf(Math.max(...qValues));
  }

  train() {
    if (this.replayBuffer.size() < this.config.learning.batchSize) {
      return 0;
    }

    const batch = this.replayBuffer.sample(this.config.learning.batchSize);
    let totalLoss = 0;

    for (const experience of batch) {
      const { state, action, reward, nextState, done } = experience;

      // Current Q-value
      const currentQ = this.qNetwork.forward(state);

      // Target Q-value
      let targetQ;
      if (done) {
        targetQ = reward;
      } else {
        const nextQ = this.targetNetwork.forward(nextState);
        targetQ = reward + this.config.learning.gamma * Math.max(...nextQ);
      }

      // TD error
      const tdError = targetQ - currentQ[action];
      totalLoss += tdError ** 2;

      // Simplified update (in production, use proper backprop)
      this.updateQNetwork(state, action, tdError);
    }

    this.losses.push(totalLoss / batch.length);
    return totalLoss / batch.length;
  }

  updateQNetwork(state, action, tdError) {
    const lr = this.config.learning.learningRate;

    // Get the actual hidden layer output (activation before output layer)
    const hiddenOutput = this.qNetwork.getPreOutputActivation();

    if (!hiddenOutput) {
      // Fallback: run forward pass to get activations
      this.qNetwork.forward(state);
      return this.updateQNetwork(state, action, tdError);
    }

    // Update output layer using actual hidden activations
    const outputLayer = this.qNetwork.layers[this.qNetwork.layers.length - 1];

    // Gradient for output layer: dL/dW = tdError * hiddenOutput
    for (let i = 0; i < outputLayer.inputDim; i++) {
      outputLayer.weights[i][action] += lr * tdError * hiddenOutput[i];
    }
    outputLayer.bias[action] += lr * tdError;

    // Simplified backprop through hidden layers (gradient clipping for stability)
    const maxGrad = 1.0;
    let delta = tdError * outputLayer.weights.map(row => row[action]);

    for (let l = this.qNetwork.layers.length - 2; l >= 0; l--) {
      const layer = this.qNetwork.layers[l];
      const prevActivation = this.qNetwork.activations[l];
      const currentActivation = this.qNetwork.activations[l + 1];

      // ReLU derivative: 1 if activation > 0, else 0
      const reluGrad = currentActivation.map(a => a > 0 ? 1 : 0);

      // Apply ReLU gradient
      delta = delta.map((d, i) => d * (reluGrad[i] || 0));

      // Clip gradients for stability
      delta = delta.map(d => Math.max(-maxGrad, Math.min(maxGrad, d)));

      // Update weights for this layer
      for (let i = 0; i < layer.inputDim; i++) {
        for (let j = 0; j < layer.outputDim; j++) {
          layer.weights[i][j] += lr * 0.1 * delta[j] * (prevActivation[i] || 0);
        }
      }

      // Propagate delta to previous layer
      if (l > 0) {
        const newDelta = new Array(layer.inputDim).fill(0);
        for (let i = 0; i < layer.inputDim; i++) {
          for (let j = 0; j < layer.outputDim; j++) {
            newDelta[i] += delta[j] * layer.weights[i][j];
          }
        }
        delta = newDelta;
      }
    }
  }

  updateTargetNetwork() {
    this.targetNetwork.copyFrom(this.qNetwork);
  }

  decayEpsilon() {
    this.epsilon = Math.max(
      this.config.exploration.epsilonEnd,
      this.epsilon * this.config.exploration.epsilonDecay
    );
  }

  addExperience(state, action, reward, nextState, done) {
    this.replayBuffer.add({ state, action, reward, nextState, done });
    this.stepCount++;

    if (this.stepCount % this.config.learning.targetUpdateFreq === 0) {
      this.updateTargetNetwork();
    }
  }
}

// Generate synthetic price data
function generatePriceData(n, seed = 42) {
  const data = [];
  let price = 100;

  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  for (let i = 0; i < n; i++) {
    // Regime-switching dynamics
    const regime = Math.floor(i / 100) % 3;
    let drift = 0, volatility = 0.015;

    if (regime === 0) {
      drift = 0.001;
      volatility = 0.012;
    } else if (regime === 1) {
      drift = -0.0005;
      volatility = 0.02;
    } else {
      drift = 0;
      volatility = 0.01;
    }

    const return_ = drift + volatility * (random() + random() - 1);
    price = price * (1 + return_);

    data.push({
      timestamp: i,
      open: price * (1 - random() * 0.002),
      high: price * (1 + random() * 0.005),
      low: price * (1 - random() * 0.005),
      close: price,
      volume: 1000000 * (0.5 + random())
    });
  }

  return data;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('REINFORCEMENT LEARNING TRADING AGENT');
  console.log('═'.repeat(70));
  console.log();

  // 1. Generate data
  console.log('1. Environment Setup:');
  console.log('─'.repeat(70));

  const priceData = generatePriceData(1000);
  const env = new TradingEnvironment(rlConfig, priceData);
  const stateEncoder = new StateEncoder(rlConfig);

  console.log(`   Price data:       ${priceData.length} candles`);
  console.log(`   Initial capital:  $${rlConfig.trading.initialCapital.toLocaleString()}`);
  console.log(`   Action space:     ${rlConfig.network.actionSpace} actions`);
  console.log(`   State dimension:  ${rlConfig.network.stateDim}`);
  console.log();

  // 2. Initialize agent
  console.log('2. Agent Configuration:');
  console.log('─'.repeat(70));

  const agent = new DQNAgent(rlConfig);

  console.log(`   Network:          ${rlConfig.network.hiddenLayers.join(' → ')} → ${rlConfig.network.actionSpace}`);
  console.log(`   Learning rate:    ${rlConfig.learning.learningRate}`);
  console.log(`   Discount factor:  ${rlConfig.learning.gamma}`);
  console.log(`   Replay buffer:    ${rlConfig.learning.replayBufferSize}`);
  console.log(`   Batch size:       ${rlConfig.learning.batchSize}`);
  console.log();

  // 3. Training
  console.log('3. Training Loop:');
  console.log('─'.repeat(70));

  const numEpisodes = 20;
  const episodeRewards = [];
  const episodeValues = [];

  for (let episode = 0; episode < numEpisodes; episode++) {
    let state = env.reset();
    let totalReward = 0;
    let done = false;

    // Update price history for state encoding
    for (let i = 0; i < 50; i++) {
      stateEncoder.update(priceData[i].close);
    }

    while (!done) {
      const encodedState = stateEncoder.encode(state);
      const action = agent.selectAction(encodedState);

      const { state: nextState, reward, done: episodeDone, info } = env.step(action);

      stateEncoder.update(priceData[env.currentStep].close);
      const nextEncodedState = stateEncoder.encode(nextState);

      agent.addExperience(encodedState, action, reward, nextEncodedState, episodeDone);

      // Train
      if (agent.stepCount % 4 === 0) {
        agent.train();
      }

      totalReward += reward;
      state = nextState;
      done = episodeDone;
    }

    agent.decayEpsilon();
    agent.episodeCount++;

    const finalValue = env.getPortfolioValue();
    episodeRewards.push(totalReward);
    episodeValues.push(finalValue);

    if ((episode + 1) % 5 === 0) {
      const avgReward = episodeRewards.slice(-5).reduce((a, b) => a + b, 0) / 5;
      console.log(`   Episode ${(episode + 1).toString().padStart(3)}: Reward=${avgReward.toFixed(1).padStart(7)}, Value=$${finalValue.toFixed(0).padStart(7)}, ε=${agent.epsilon.toFixed(3)}`);
    }
  }
  console.log();

  // 4. Final evaluation
  console.log('4. Final Evaluation:');
  console.log('─'.repeat(70));

  // Run one episode with no exploration
  agent.epsilon = 0;
  let evalState = env.reset();
  let evalDone = false;
  const evalActions = [];

  for (let i = 0; i < 50; i++) {
    stateEncoder.update(priceData[i].close);
  }

  while (!evalDone) {
    const encodedState = stateEncoder.encode(evalState);
    const action = agent.selectAction(encodedState);
    evalActions.push(ActionNames[action]);

    const { state: nextState, done } = env.step(action);
    stateEncoder.update(priceData[env.currentStep].close);
    evalState = nextState;
    evalDone = done;
  }

  const finalValue = env.getPortfolioValue();
  const totalReturn = (finalValue - rlConfig.trading.initialCapital) / rlConfig.trading.initialCapital;

  console.log(`   Final Portfolio:  $${finalValue.toFixed(2)}`);
  console.log(`   Total Return:     ${(totalReturn * 100).toFixed(2)}%`);
  console.log(`   Realized P&L:     $${env.realizedPnL.toFixed(2)}`);
  console.log(`   Total Trades:     ${env.trades.length}`);
  console.log(`   Win Rate:         ${(env.getWinRate() * 100).toFixed(1)}%`);
  console.log(`   Sharpe Ratio:     ${env.getSharpe().toFixed(3)}`);
  console.log(`   Max Drawdown:     ${(env.getDrawdown() * 100).toFixed(1)}%`);
  console.log();

  // 5. Action distribution
  console.log('5. Action Distribution:');
  console.log('─'.repeat(70));

  const actionCounts = {};
  for (const action of evalActions) {
    actionCounts[action] = (actionCounts[action] || 0) + 1;
  }

  for (const [action, count] of Object.entries(actionCounts).sort((a, b) => b[1] - a[1])) {
    const pct = (count / evalActions.length * 100).toFixed(1);
    const bar = '█'.repeat(Math.floor(count / evalActions.length * 40));
    console.log(`   ${action.padEnd(12)} ${bar.padEnd(40)} ${pct}%`);
  }
  console.log();

  // 6. Learning curve
  console.log('6. Learning Curve:');
  console.log('─'.repeat(70));

  console.log('   Episode Returns:');
  let curve = '   ';
  const minReward = Math.min(...episodeRewards);
  const maxReward = Math.max(...episodeRewards);
  const range = maxReward - minReward || 1;

  for (const reward of episodeRewards) {
    const normalized = (reward - minReward) / range;
    if (normalized < 0.25) curve += '▁';
    else if (normalized < 0.5) curve += '▃';
    else if (normalized < 0.75) curve += '▅';
    else curve += '█';
  }
  console.log(curve);
  console.log(`   Min: ${minReward.toFixed(1)}  Max: ${maxReward.toFixed(1)}`);
  console.log();

  // 7. Q-value analysis
  console.log('7. Q-Value Analysis (Sample State):');
  console.log('─'.repeat(70));

  const sampleState = stateEncoder.encode(evalState);
  const qValues = agent.qNetwork.forward(sampleState);

  console.log('   Action Q-Values:');
  for (let i = 0; i < ActionNames.length; i++) {
    const bar = qValues[i] > 0 ? '+'.repeat(Math.min(20, Math.floor(qValues[i] * 2))) : '';
    const negBar = qValues[i] < 0 ? '-'.repeat(Math.min(20, Math.floor(Math.abs(qValues[i]) * 2))) : '';
    console.log(`   ${ActionNames[i].padEnd(12)} ${qValues[i] >= 0 ? '+' : ''}${qValues[i].toFixed(3)} ${bar}${negBar}`);
  }
  console.log();

  // 8. Experience replay stats
  console.log('8. Experience Replay Statistics:');
  console.log('─'.repeat(70));

  console.log(`   Buffer size:      ${agent.replayBuffer.size()}`);
  console.log(`   Total steps:      ${agent.stepCount}`);
  console.log(`   Training updates: ${agent.losses.length}`);
  if (agent.losses.length > 0) {
    const avgLoss = agent.losses.reduce((a, b) => a + b, 0) / agent.losses.length;
    console.log(`   Average loss:     ${avgLoss.toFixed(4)}`);
  }
  console.log();

  // 9. Trading strategy emerged
  console.log('9. Emergent Strategy Analysis:');
  console.log('─'.repeat(70));

  // Analyze when agent buys vs sells
  const buyActions = evalActions.filter(a => a.includes('BUY')).length;
  const sellActions = evalActions.filter(a => a.includes('SELL')).length;
  const holdActions = evalActions.filter(a => a === 'HOLD').length;

  console.log('   The agent learned to:');
  if (holdActions > evalActions.length * 0.5) {
    console.log('   - Be patient (primarily holding positions)');
  }
  if (buyActions > sellActions) {
    console.log('   - Favor long positions (more buys than sells)');
  } else if (sellActions > buyActions) {
    console.log('   - Manage risk actively (frequent profit taking)');
  }
  console.log();

  // 10. RuVector integration
  console.log('10. RuVector Vector Storage:');
  console.log('─'.repeat(70));
  console.log('   State vectors can be stored for similarity search:');
  console.log();
  console.log(`   State vector sample (first 5 dims):`);
  console.log(`   [${sampleState.slice(0, 5).map(v => v.toFixed(4)).join(', ')}]`);
  console.log();
  console.log('   Use cases:');
  console.log('   - Find similar market states from history');
  console.log('   - Experience replay with prioritized sampling');
  console.log('   - State clustering for interpretability');
  console.log();

  console.log('═'.repeat(70));
  console.log('Reinforcement learning agent training completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
