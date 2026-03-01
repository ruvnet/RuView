import { Actor } from 'apify';

// Neural Engine - Core neural network implementation
class NeuralEngine {
    constructor(config = {}) {
        this.layers = config.layers || 3;
        this.neurons = config.neurons || [128, 64, 32];
        this.activation = config.activation || 'relu';
        this.dropout = config.dropout || 0.2;
        this.learningRate = config.learningRate || 0.001;
        this.weights = [];
        this.biases = [];
        this.initializeWeights();
    }

    initializeWeights() {
        for (let i = 0; i < this.neurons.length; i++) {
            const inputSize = i === 0 ? 50 : this.neurons[i - 1]; // 50 input features
            const outputSize = this.neurons[i];

            // Xavier initialization
            const limit = Math.sqrt(6 / (inputSize + outputSize));
            this.weights[i] = Array(inputSize).fill(0).map(() =>
                Array(outputSize).fill(0).map(() => (Math.random() * 2 - 1) * limit)
            );
            this.biases[i] = Array(outputSize).fill(0);
        }
    }

    activate(x, func = this.activation) {
        switch (func) {
            case 'relu':
                return Math.max(0, x);
            case 'tanh':
                return Math.tanh(x);
            case 'sigmoid':
                return 1 / (1 + Math.exp(-x));
            case 'leaky_relu':
                return x > 0 ? x : 0.01 * x;
            default:
                return x;
        }
    }

    forward(input) {
        let activations = input;

        for (let i = 0; i < this.weights.length; i++) {
            const layer = [];
            for (let j = 0; j < this.weights[i][0].length; j++) {
                let sum = this.biases[i][j];
                for (let k = 0; k < activations.length; k++) {
                    sum += activations[k] * this.weights[i][k][j];
                }
                layer.push(this.activate(sum));
            }
            activations = layer;

            // Apply dropout during training
            if (Math.random() < this.dropout) {
                activations = activations.map(a => a * (1 - this.dropout));
            }
        }

        return activations;
    }

    train(inputs, targets, epochs = 100) {
        for (let epoch = 0; epoch < epochs; epoch++) {
            let totalLoss = 0;

            for (let i = 0; i < inputs.length; i++) {
                const output = this.forward(inputs[i]);
                const target = targets[i];

                // Calculate loss (MSE)
                const loss = output.reduce((sum, o, idx) =>
                    sum + Math.pow(o - target[idx], 2), 0) / output.length;
                totalLoss += loss;

                // Backpropagation (simplified)
                this.backward(inputs[i], target, output);
            }

            if (epoch % 10 === 0) {
                console.log(`Epoch ${epoch}, Loss: ${totalLoss / inputs.length}`);
            }
        }
    }

    backward(input, target, output) {
        // Simplified gradient descent
        const error = output.map((o, i) => target[i] - o);

        // Update weights and biases
        for (let i = this.weights.length - 1; i >= 0; i--) {
            for (let j = 0; j < this.weights[i].length; j++) {
                for (let k = 0; k < this.weights[i][j].length; k++) {
                    this.weights[i][j][k] += this.learningRate * error[k] *
                        (i === 0 ? input[j] : this.weights[i - 1][j][k]);
                }
            }
            for (let j = 0; j < this.biases[i].length; j++) {
                this.biases[i][j] += this.learningRate * error[j];
            }
        }
    }
}

// LSTM Cell for time series prediction
class LSTMCell {
    constructor(inputSize, hiddenSize) {
        this.inputSize = inputSize;
        this.hiddenSize = hiddenSize;
        this.initializeGates();
    }

    initializeGates() {
        this.Wf = this.randomMatrix(this.inputSize + this.hiddenSize, this.hiddenSize);
        this.Wi = this.randomMatrix(this.inputSize + this.hiddenSize, this.hiddenSize);
        this.Wc = this.randomMatrix(this.inputSize + this.hiddenSize, this.hiddenSize);
        this.Wo = this.randomMatrix(this.inputSize + this.hiddenSize, this.hiddenSize);
    }

    randomMatrix(rows, cols) {
        return Array(rows).fill(0).map(() =>
            Array(cols).fill(0).map(() => (Math.random() * 2 - 1) * 0.1)
        );
    }

    sigmoid(x) {
        return 1 / (1 + Math.exp(-x));
    }

    forward(input, hiddenState, cellState) {
        const combined = [...input, ...hiddenState];

        // Forget gate
        const forgetGate = this.matmul(combined, this.Wf).map(this.sigmoid);

        // Input gate
        const inputGate = this.matmul(combined, this.Wi).map(this.sigmoid);

        // Cell candidate
        const cellCandidate = this.matmul(combined, this.Wc).map(Math.tanh);

        // Output gate
        const outputGate = this.matmul(combined, this.Wo).map(this.sigmoid);

        // New cell state
        const newCellState = forgetGate.map((f, i) =>
            f * cellState[i] + inputGate[i] * cellCandidate[i]
        );

        // New hidden state
        const newHiddenState = outputGate.map((o, i) =>
            o * Math.tanh(newCellState[i])
        );

        return { hiddenState: newHiddenState, cellState: newCellState };
    }

    matmul(vec, matrix) {
        return matrix[0].map((_, col) =>
            vec.reduce((sum, val, row) => sum + val * matrix[row][col], 0)
        );
    }
}

// Signal Generator with confidence scoring
class SignalGenerator {
    constructor(config = {}) {
        this.confidenceThreshold = config.confidenceThreshold || 70;
        this.patterns = config.patterns || ['all'];
    }

    generateSignal(predictions, marketData) {
        const signal = {
            timestamp: new Date().toISOString(),
            symbol: marketData.symbol,
            price: marketData.price,
            signal: 'HOLD',
            confidence: 0,
            reasons: [],
            target: null,
            stopLoss: null,
            patterns: []
        };

        // Analyze predictions
        const avgPrediction = predictions.reduce((a, b) => a + b, 0) / predictions.length;
        const variance = predictions.reduce((sum, p) => sum + Math.pow(p - avgPrediction, 2), 0) / predictions.length;
        const stdDev = Math.sqrt(variance);

        // Calculate confidence (lower variance = higher confidence)
        signal.confidence = Math.min(100, (1 - stdDev) * 100);

        // Generate signal based on prediction
        if (avgPrediction > 0.6 && signal.confidence >= this.confidenceThreshold) {
            signal.signal = 'BUY';
            signal.target = marketData.price * (1 + marketData.takeProfit / 100);
            signal.stopLoss = marketData.price * (1 - marketData.stopLoss / 100);
            signal.reasons.push(`Neural prediction: ${(avgPrediction * 100).toFixed(2)}%`);
        } else if (avgPrediction < 0.4 && signal.confidence >= this.confidenceThreshold) {
            signal.signal = 'SELL';
            signal.target = marketData.price * (1 - marketData.takeProfit / 100);
            signal.stopLoss = marketData.price * (1 + marketData.stopLoss / 100);
            signal.reasons.push(`Neural prediction: ${(avgPrediction * 100).toFixed(2)}%`);
        }

        // Pattern recognition
        signal.patterns = this.detectPatterns(marketData);
        if (signal.patterns.length > 0) {
            signal.reasons.push(`Patterns: ${signal.patterns.join(', ')}`);
            signal.confidence = Math.min(100, signal.confidence + signal.patterns.length * 5);
        }

        return signal;
    }

    detectPatterns(marketData) {
        const patterns = [];
        const { prices } = marketData;

        if (!prices || prices.length < 5) return patterns;

        // Head and Shoulders
        if (this.patterns.includes('all') || this.patterns.includes('head_shoulders')) {
            if (this.isHeadAndShoulders(prices)) {
                patterns.push('head_shoulders');
            }
        }

        // Double Top
        if (this.patterns.includes('all') || this.patterns.includes('double_top')) {
            if (this.isDoubleTop(prices)) {
                patterns.push('double_top');
            }
        }

        // Double Bottom
        if (this.patterns.includes('all') || this.patterns.includes('double_bottom')) {
            if (this.isDoubleBottom(prices)) {
                patterns.push('double_bottom');
            }
        }

        return patterns;
    }

    isHeadAndShoulders(prices) {
        if (prices.length < 5) return false;
        const recent = prices.slice(-5);
        return recent[2] > recent[0] && recent[2] > recent[1] &&
               recent[2] > recent[3] && recent[2] > recent[4];
    }

    isDoubleTop(prices) {
        if (prices.length < 4) return false;
        const recent = prices.slice(-4);
        return Math.abs(recent[0] - recent[2]) < recent[0] * 0.02 &&
               recent[1] < recent[0] && recent[3] < recent[2];
    }

    isDoubleBottom(prices) {
        if (prices.length < 4) return false;
        const recent = prices.slice(-4);
        return Math.abs(recent[0] - recent[2]) < recent[0] * 0.02 &&
               recent[1] > recent[0] && recent[3] > recent[2];
    }
}

// Portfolio Optimizer
class PortfolioOptimizer {
    constructor(config = {}) {
        this.riskProfile = config.riskProfile || 'moderate';
        this.maxPositionSize = config.maxPositionSize || 10;
    }

    optimize(signals, portfolioValue) {
        const allocation = {
            positions: [],
            totalAllocation: 0,
            expectedReturn: 0,
            riskScore: 0,
            sharpeRatio: 0
        };

        // Filter high-confidence signals
        const validSignals = signals.filter(s =>
            s.signal !== 'HOLD' && s.confidence >= 70
        );

        if (validSignals.length === 0) {
            return allocation;
        }

        // Calculate position sizes using Kelly Criterion
        validSignals.forEach(signal => {
            const kellyFraction = this.calculateKelly(signal);
            const positionSize = Math.min(
                kellyFraction * portfolioValue,
                (this.maxPositionSize / 100) * portfolioValue
            );

            allocation.positions.push({
                symbol: signal.symbol,
                signal: signal.signal,
                allocation: positionSize,
                percentage: (positionSize / portfolioValue) * 100,
                confidence: signal.confidence,
                target: signal.target,
                stopLoss: signal.stopLoss
            });

            allocation.totalAllocation += positionSize;
        });

        // Calculate portfolio metrics
        allocation.expectedReturn = this.calculateExpectedReturn(allocation.positions);
        allocation.riskScore = this.calculateRisk(allocation.positions);
        allocation.sharpeRatio = allocation.expectedReturn / (allocation.riskScore || 1);

        return allocation;
    }

    calculateKelly(signal) {
        // Kelly Criterion: f = (bp - q) / b
        // where b = odds, p = probability of win, q = probability of loss
        const winProb = signal.confidence / 100;
        const lossProb = 1 - winProb;
        const odds = Math.abs(signal.target - signal.price) / Math.abs(signal.stopLoss - signal.price);

        const kelly = (odds * winProb - lossProb) / odds;
        return Math.max(0, Math.min(kelly, 0.25)); // Cap at 25%
    }

    calculateExpectedReturn(positions) {
        return positions.reduce((sum, pos) => {
            const expectedMove = Math.abs(pos.target - pos.stopLoss) / 2;
            return sum + (pos.percentage * expectedMove);
        }, 0);
    }

    calculateRisk(positions) {
        // Simple volatility-based risk
        const variance = positions.reduce((sum, pos) => {
            const risk = Math.abs(pos.stopLoss - pos.target);
            return sum + Math.pow(risk * pos.percentage, 2);
        }, 0);
        return Math.sqrt(variance);
    }
}

// Risk Manager
class RiskManager {
    constructor(config = {}) {
        this.maxDrawdown = config.maxDrawdown || 20;
        this.varConfidence = config.varConfidence || 0.95;
    }

    assessRisk(portfolio, marketData) {
        const risk = {
            valueAtRisk: 0,
            expectedShortfall: 0,
            maxDrawdown: 0,
            positionRisks: [],
            recommendations: []
        };

        // Calculate Value at Risk (VaR)
        risk.valueAtRisk = this.calculateVaR(portfolio, marketData);

        // Calculate Expected Shortfall (CVaR)
        risk.expectedShortfall = risk.valueAtRisk * 1.5;

        // Assess individual positions
        portfolio.positions.forEach(position => {
            const positionRisk = {
                symbol: position.symbol,
                exposure: position.allocation,
                riskAmount: Math.abs(position.allocation *
                    (position.stopLoss - position.target) / position.target),
                riskPercentage: ((position.stopLoss - position.target) / position.target) * 100
            };
            risk.positionRisks.push(positionRisk);

            // Generate recommendations
            if (positionRisk.riskPercentage > 5) {
                risk.recommendations.push(
                    `Reduce position size for ${position.symbol} - high risk (${positionRisk.riskPercentage.toFixed(2)}%)`
                );
            }
        });

        // Portfolio-level recommendations
        if (portfolio.totalAllocation > portfolio.value * 0.8) {
            risk.recommendations.push('Consider reducing overall exposure - portfolio is highly allocated');
        }

        if (risk.valueAtRisk > portfolio.value * 0.1) {
            risk.recommendations.push(`VaR exceeds 10% of portfolio - consider reducing risk`);
        }

        return risk;
    }

    calculateVaR(portfolio, marketData, confidence = this.varConfidence) {
        // Simplified VaR calculation using historical volatility
        const returns = marketData.returns || [];
        if (returns.length === 0) return 0;

        const sortedReturns = [...returns].sort((a, b) => a - b);
        const varIndex = Math.floor((1 - confidence) * sortedReturns.length);
        const varReturn = sortedReturns[varIndex];

        return Math.abs(portfolio.totalAllocation * varReturn);
    }
}

// Swarm Coordinator for multi-agent ensemble
class SwarmCoordinator {
    constructor(config = {}) {
        this.numAgents = config.swarmAgents || 5;
        this.agents = [];
        this.initializeAgents(config);
    }

    initializeAgents(config) {
        for (let i = 0; i < this.numAgents; i++) {
            // Create diverse agents with different configurations
            const agentConfig = {
                ...config.neuralConfig,
                learningRate: config.neuralConfig.learningRate * (0.5 + Math.random()),
                dropout: config.neuralConfig.dropout * (0.5 + Math.random() * 1.5)
            };
            this.agents.push(new NeuralEngine(agentConfig));
        }
    }

    predict(input) {
        // Get predictions from all agents
        const predictions = this.agents.map(agent => {
            const output = agent.forward(input);
            return output[0]; // Get first output (prediction)
        });

        // Consensus voting with weighted average
        const weights = predictions.map((_, i) => 1 / this.numAgents);
        const consensus = predictions.reduce((sum, pred, i) =>
            sum + pred * weights[i], 0
        );

        return {
            consensus,
            predictions,
            agreement: 1 - this.calculateVariance(predictions),
            individual: predictions
        };
    }

    calculateVariance(predictions) {
        const mean = predictions.reduce((a, b) => a + b, 0) / predictions.length;
        const variance = predictions.reduce((sum, p) =>
            sum + Math.pow(p - mean, 2), 0) / predictions.length;
        return Math.sqrt(variance);
    }
}

// Technical Indicators
class TechnicalIndicators {
    static calculateRSI(prices, period = 14) {
        if (prices.length < period + 1) return 50;

        const changes = prices.slice(1).map((price, i) => price - prices[i]);
        const gains = changes.map(c => c > 0 ? c : 0);
        const losses = changes.map(c => c < 0 ? -c : 0);

        const avgGain = gains.slice(-period).reduce((a, b) => a + b, 0) / period;
        const avgLoss = losses.slice(-period).reduce((a, b) => a + b, 0) / period;

        if (avgLoss === 0) return 100;
        const rs = avgGain / avgLoss;
        return 100 - (100 / (1 + rs));
    }

    static calculateMACD(prices, fast = 12, slow = 26, signal = 9) {
        const emaFast = this.calculateEMA(prices, fast);
        const emaSlow = this.calculateEMA(prices, slow);
        const macdLine = emaFast - emaSlow;

        return {
            macd: macdLine,
            signal: this.calculateEMA([macdLine], signal),
            histogram: macdLine - this.calculateEMA([macdLine], signal)
        };
    }

    static calculateEMA(prices, period) {
        if (prices.length === 0) return 0;
        const k = 2 / (period + 1);
        let ema = prices[0];

        for (let i = 1; i < prices.length; i++) {
            ema = prices[i] * k + ema * (1 - k);
        }

        return ema;
    }

    static calculateBollinger(prices, period = 20, stdDev = 2) {
        const sma = prices.slice(-period).reduce((a, b) => a + b, 0) / period;
        const variance = prices.slice(-period)
            .reduce((sum, p) => sum + Math.pow(p - sma, 2), 0) / period;
        const std = Math.sqrt(variance);

        return {
            upper: sma + stdDev * std,
            middle: sma,
            lower: sma - stdDev * std
        };
    }

    static calculateATR(highs, lows, closes, period = 14) {
        const trs = [];
        for (let i = 1; i < closes.length; i++) {
            const tr = Math.max(
                highs[i] - lows[i],
                Math.abs(highs[i] - closes[i - 1]),
                Math.abs(lows[i] - closes[i - 1])
            );
            trs.push(tr);
        }
        return trs.slice(-period).reduce((a, b) => a + b, 0) / period;
    }
}

// Main Actor
await Actor.main(async () => {
    console.log('🚀 Neural Trader System - Starting...');

    const input = await Actor.getInput();
    const {
        mode = 'signals',
        symbols = ['BTC/USD'],
        strategy = 'ensemble',
        riskProfile = 'moderate',
        maxPositionSize = 10,
        stopLoss = 2.5,
        takeProfit = 5,
        timeframe = '1h',
        lookbackPeriod = 100,
        neuralConfig = {},
        enableSwarm = true,
        swarmAgents = 5,
        outputFormat = 'full_analysis',
        webhookUrl = null,
        backtestDays = 30,
        enableGpu = true,
        confidenceThreshold = 70,
        patterns = ['all'],
        indicators = {}
    } = input;

    console.log(`📊 Mode: ${mode}`);
    console.log(`💹 Symbols: ${symbols.join(', ')}`);
    console.log(`🧠 Strategy: ${strategy}`);
    console.log(`🎯 Risk Profile: ${riskProfile}`);

    // Initialize components
    const neuralEngine = new NeuralEngine(neuralConfig);
    const signalGenerator = new SignalGenerator({ confidenceThreshold, patterns });
    const portfolioOptimizer = new PortfolioOptimizer({ riskProfile, maxPositionSize });
    const riskManager = new RiskManager();
    const swarmCoordinator = enableSwarm ? new SwarmCoordinator({ swarmAgents, neuralConfig }) : null;

    const results = [];

    // Process each symbol
    for (const symbol of symbols) {
        console.log(`\n📈 Analyzing ${symbol}...`);

        // Generate synthetic market data (in production, fetch real data)
        const marketData = generateMarketData(symbol, lookbackPeriod, {
            stopLoss,
            takeProfit,
            timeframe
        });

        // Calculate technical indicators
        const technicalData = {
            rsi: indicators.rsi ? TechnicalIndicators.calculateRSI(marketData.prices) : null,
            macd: indicators.macd ? TechnicalIndicators.calculateMACD(marketData.prices) : null,
            bollinger: indicators.bollinger ? TechnicalIndicators.calculateBollinger(marketData.prices) : null,
            atr: indicators.atr ? TechnicalIndicators.calculateATR(
                marketData.highs, marketData.lows, marketData.prices
            ) : null
        };

        // Prepare neural network input
        const features = prepareFeatures(marketData, technicalData);

        // Get predictions
        let predictions;
        if (enableSwarm && swarmCoordinator) {
            const swarmResult = swarmCoordinator.predict(features);
            predictions = swarmResult.individual;
            console.log(`🤖 Swarm consensus: ${(swarmResult.consensus * 100).toFixed(2)}%`);
            console.log(`🎯 Agreement: ${(swarmResult.agreement * 100).toFixed(2)}%`);
        } else {
            const output = neuralEngine.forward(features);
            predictions = [output[0]];
        }

        // Generate trading signal
        const signal = signalGenerator.generateSignal(predictions, marketData);

        console.log(`${signal.signal === 'BUY' ? '🟢' : signal.signal === 'SELL' ? '🔴' : '⚪'} Signal: ${signal.signal}`);
        console.log(`💪 Confidence: ${signal.confidence.toFixed(2)}%`);

        // Create result object
        const result = {
            ...signal,
            technical: technicalData,
            prediction: predictions.reduce((a, b) => a + b, 0) / predictions.length,
            swarmPredictions: enableSwarm ? predictions : null,
            timeframe,
            strategy
        };

        results.push(result);

        // Push to dataset
        await Actor.pushData(result);
    }

    // Portfolio optimization
    if (mode === 'optimize' || outputFormat === 'portfolio') {
        console.log('\n💼 Optimizing portfolio...');

        const portfolioValue = 100000; // Example portfolio value
        const portfolio = portfolioOptimizer.optimize(results, portfolioValue);

        console.log(`📊 Total Allocation: $${portfolio.totalAllocation.toFixed(2)}`);
        console.log(`📈 Expected Return: ${portfolio.expectedReturn.toFixed(2)}%`);
        console.log(`⚠️ Risk Score: ${portfolio.riskScore.toFixed(2)}`);
        console.log(`📉 Sharpe Ratio: ${portfolio.sharpeRatio.toFixed(2)}`);

        // Risk assessment
        const risk = riskManager.assessRisk(
            { ...portfolio, value: portfolioValue },
            { returns: generateReturns(lookbackPeriod) }
        );

        console.log(`\n🛡️ Risk Assessment:`);
        console.log(`💰 Value at Risk (95%): $${risk.valueAtRisk.toFixed(2)}`);
        console.log(`📉 Expected Shortfall: $${risk.expectedShortfall.toFixed(2)}`);

        if (risk.recommendations.length > 0) {
            console.log(`\n💡 Recommendations:`);
            risk.recommendations.forEach(rec => console.log(`  • ${rec}`));
        }

        await Actor.pushData({
            type: 'portfolio',
            portfolio,
            risk,
            timestamp: new Date().toISOString()
        });
    }

    // Send webhook if configured
    if (webhookUrl && results.length > 0) {
        console.log(`\n🔔 Sending webhook to ${webhookUrl}...`);
        try {
            await fetch(webhookUrl, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    signals: results,
                    timestamp: new Date().toISOString(),
                    strategy,
                    mode
                })
            });
            console.log('✅ Webhook sent successfully');
        } catch (error) {
            console.error('❌ Webhook failed:', error.message);
        }
    }

    console.log(`\n✅ Neural Trader System completed`);
    console.log(`📊 Processed ${symbols.length} symbols`);
    console.log(`🎯 Generated ${results.filter(r => r.signal !== 'HOLD').length} signals`);
});

// Helper functions
function generateMarketData(symbol, periods, config) {
    const prices = [];
    const highs = [];
    const lows = [];
    const volumes = [];

    let price = 100 + Math.random() * 900; // Random starting price

    for (let i = 0; i < periods; i++) {
        const change = (Math.random() - 0.5) * price * 0.03; // 3% max change
        price += change;

        prices.push(price);
        highs.push(price * (1 + Math.random() * 0.01));
        lows.push(price * (1 - Math.random() * 0.01));
        volumes.push(Math.random() * 1000000);
    }

    return {
        symbol,
        price: prices[prices.length - 1],
        prices,
        highs,
        lows,
        volumes,
        stopLoss: config.stopLoss,
        takeProfit: config.takeProfit,
        timeframe: config.timeframe
    };
}

function prepareFeatures(marketData, technicalData) {
    const features = [];

    // Price features (normalized)
    const prices = marketData.prices.slice(-20);
    const priceNorm = prices.map(p => p / marketData.price);
    features.push(...priceNorm);

    // Technical indicators
    if (technicalData.rsi !== null) {
        features.push(technicalData.rsi / 100);
    }

    if (technicalData.macd !== null) {
        features.push(
            technicalData.macd.macd / 100,
            technicalData.macd.signal / 100,
            technicalData.macd.histogram / 100
        );
    }

    if (technicalData.bollinger !== null) {
        features.push(
            technicalData.bollinger.upper / marketData.price,
            technicalData.bollinger.middle / marketData.price,
            technicalData.bollinger.lower / marketData.price
        );
    }

    // Pad to 50 features
    while (features.length < 50) {
        features.push(0);
    }

    return features.slice(0, 50);
}

function generateReturns(periods) {
    const returns = [];
    for (let i = 0; i < periods; i++) {
        // Generate random returns with normal distribution
        returns.push((Math.random() - 0.5) * 0.05);
    }
    return returns;
}
