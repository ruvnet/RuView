/**
 * Quantum-Inspired Portfolio Optimization
 *
 * EXOTIC: Quantum annealing and QAOA for portfolio selection
 *
 * Uses @neural-trader/portfolio with RuVector for:
 * - Quantum Approximate Optimization Algorithm (QAOA) simulation
 * - Simulated quantum annealing for combinatorial optimization
 * - Qubit encoding of portfolio weights
 * - Quantum interference for exploring solution space
 *
 * Classical simulation of quantum concepts for optimization
 * problems that are NP-hard classically.
 */

// Quantum optimization configuration
const quantumConfig = {
  // QAOA parameters
  qaoa: {
    layers: 3,              // Number of QAOA layers (p)
    shots: 1000,            // Measurement samples
    optimizer: 'cobyla',    // Classical optimizer for angles
    maxIterations: 100
  },

  // Annealing parameters
  annealing: {
    initialTemp: 100,
    finalTemp: 0.01,
    coolingRate: 0.99,
    sweeps: 1000
  },

  // Portfolio constraints
  portfolio: {
    numAssets: 10,
    minWeight: 0.0,
    maxWeight: 0.3,
    targetReturn: 0.10,
    riskAversion: 2.0,
    cardinalityConstraint: 5  // Max assets in portfolio
  },

  // Qubit encoding
  encoding: {
    bitsPerWeight: 4,       // Weight precision: 2^4 = 16 levels
    penaltyWeight: 100      // Constraint violation penalty
  }
};

// Object pool for Complex numbers (reduces GC pressure)
class ComplexPool {
  constructor(initialSize = 1024) {
    this.pool = [];
    this.index = 0;
    for (let i = 0; i < initialSize; i++) {
      this.pool.push(new Complex(0, 0));
    }
  }

  acquire(real = 0, imag = 0) {
    if (this.index < this.pool.length) {
      const c = this.pool[this.index++];
      c.real = real;
      c.imag = imag;
      return c;
    }
    // Expand pool if needed
    const c = new Complex(real, imag);
    this.pool.push(c);
    this.index++;
    return c;
  }

  reset() {
    this.index = 0;
  }
}

// Global pool instance for reuse
const complexPool = new ComplexPool(4096);

// Complex number class for quantum states
class Complex {
  constructor(real, imag = 0) {
    this.real = real;
    this.imag = imag;
  }

  add(other) {
    return new Complex(this.real + other.real, this.imag + other.imag);
  }

  // In-place add (avoids allocation)
  addInPlace(other) {
    this.real += other.real;
    this.imag += other.imag;
    return this;
  }

  multiply(other) {
    return new Complex(
      this.real * other.real - this.imag * other.imag,
      this.real * other.imag + this.imag * other.real
    );
  }

  // In-place multiply (avoids allocation)
  multiplyInPlace(other) {
    const newReal = this.real * other.real - this.imag * other.imag;
    const newImag = this.real * other.imag + this.imag * other.real;
    this.real = newReal;
    this.imag = newImag;
    return this;
  }

  scale(s) {
    return new Complex(this.real * s, this.imag * s);
  }

  // In-place scale (avoids allocation)
  scaleInPlace(s) {
    this.real *= s;
    this.imag *= s;
    return this;
  }

  magnitude() {
    return Math.sqrt(this.real * this.real + this.imag * this.imag);
  }

  magnitudeSq() {
    return this.real * this.real + this.imag * this.imag;
  }

  static exp(theta) {
    return new Complex(Math.cos(theta), Math.sin(theta));
  }
}

// Quantum State (simplified simulation)
class QuantumState {
  constructor(numQubits) {
    this.numQubits = numQubits;
    this.dim = Math.pow(2, numQubits);
    this.amplitudes = new Array(this.dim).fill(null).map(() => new Complex(0));
    this.amplitudes[0] = new Complex(1);  // Initialize to |0...0⟩
  }

  // Create uniform superposition (Hadamard on all qubits)
  hadamardAll() {
    const newAmps = new Array(this.dim).fill(null).map(() => new Complex(0));
    const norm = 1 / Math.sqrt(this.dim);

    for (let i = 0; i < this.dim; i++) {
      newAmps[i] = new Complex(norm);
    }

    this.amplitudes = newAmps;
  }

  // Apply cost Hamiltonian phase (problem encoding)
  applyCostPhase(gamma, costFunction) {
    for (let i = 0; i < this.dim; i++) {
      const cost = costFunction(i);
      const phase = Complex.exp(-gamma * cost);
      this.amplitudes[i] = this.amplitudes[i].multiply(phase);
    }
  }

  // Apply mixer Hamiltonian (exploration)
  // Implements exp(-i * beta * sum_j X_j) where X_j is Pauli-X on qubit j
  applyMixerPhase(beta) {
    const cos = Math.cos(beta);
    const sin = Math.sin(beta);

    // Apply Rx(2*beta) to each qubit individually
    // Rx(theta) = cos(theta/2)*I - i*sin(theta/2)*X
    for (let q = 0; q < this.numQubits; q++) {
      const newAmps = new Array(this.dim).fill(null).map(() => new Complex(0));

      for (let i = 0; i < this.dim; i++) {
        const neighbor = i ^ (1 << q);  // Flip qubit q

        // |i⟩ -> cos(beta)|i⟩ - i*sin(beta)|neighbor⟩
        newAmps[i] = newAmps[i].add(this.amplitudes[i].scale(cos));
        newAmps[i] = newAmps[i].add(
          new Complex(0, -sin).multiply(this.amplitudes[neighbor])
        );
      }

      // Update amplitudes after each qubit rotation
      for (let i = 0; i < this.dim; i++) {
        this.amplitudes[i] = newAmps[i];
      }
    }

    // Normalize to handle numerical errors
    let norm = 0;
    for (const amp of this.amplitudes) {
      norm += amp.magnitude() ** 2;
    }
    norm = Math.sqrt(norm);

    // Guard against division by zero
    if (norm > 1e-10) {
      for (let i = 0; i < this.dim; i++) {
        this.amplitudes[i] = this.amplitudes[i].scale(1 / norm);
      }
    }
  }

  // Measure (sample from probability distribution)
  measure() {
    const probabilities = this.amplitudes.map(a => a.magnitude() ** 2);

    // Normalize probabilities with guard against zero total
    const total = probabilities.reduce((a, b) => a + b, 0);
    if (total < 1e-10) {
      // Fallback to uniform distribution
      return Math.floor(Math.random() * this.dim);
    }
    const normalized = probabilities.map(p => p / total);

    // Sample
    const r = Math.random();
    let cumulative = 0;

    for (let i = 0; i < this.dim; i++) {
      cumulative += normalized[i];
      if (r < cumulative) {
        return i;
      }
    }

    return this.dim - 1;
  }

  // Get probability distribution
  getProbabilities() {
    const probs = this.amplitudes.map(a => a.magnitude() ** 2);
    const total = probs.reduce((a, b) => a + b, 0);
    return probs.map(p => p / total);
  }
}

// QAOA Optimizer
class QAOAOptimizer {
  constructor(config) {
    this.config = config;
    this.bestSolution = null;
    this.bestCost = Infinity;
    this.history = [];
  }

  // Define cost function (portfolio objective)
  createCostFunction(expectedReturns, covarianceMatrix, riskAversion) {
    return (bitstring) => {
      const weights = this.decodeWeights(bitstring);

      // Expected return
      const expectedReturn = weights.reduce((sum, w, i) => sum + w * expectedReturns[i], 0);

      // Portfolio variance
      let variance = 0;
      for (let i = 0; i < weights.length; i++) {
        for (let j = 0; j < weights.length; j++) {
          variance += weights[i] * weights[j] * covarianceMatrix[i][j];
        }
      }

      // Mean-variance objective (maximize return, minimize variance)
      // Cost = -return + riskAversion * variance
      let cost = -expectedReturn + riskAversion * variance;

      // Penalty for constraint violations
      const totalWeight = weights.reduce((a, b) => a + b, 0);
      if (Math.abs(totalWeight - 1.0) > 0.1) {
        cost += this.config.encoding.penaltyWeight * (totalWeight - 1.0) ** 2;
      }

      // Cardinality constraint penalty
      const numAssets = weights.filter(w => w > 0.01).length;
      if (numAssets > this.config.portfolio.cardinalityConstraint) {
        cost += this.config.encoding.penaltyWeight * (numAssets - this.config.portfolio.cardinalityConstraint);
      }

      return cost;
    };
  }

  // Decode bitstring to portfolio weights
  decodeWeights(bitstring) {
    const numAssets = this.config.portfolio.numAssets;
    const bitsPerWeight = this.config.encoding.bitsPerWeight;
    const maxLevel = Math.pow(2, bitsPerWeight) - 1;

    const weights = [];

    for (let i = 0; i < numAssets; i++) {
      let value = 0;
      for (let b = 0; b < bitsPerWeight; b++) {
        const bitIndex = i * bitsPerWeight + b;
        if (bitstring & (1 << bitIndex)) {
          value += Math.pow(2, b);
        }
      }

      // Normalize to weight range
      const weight = (value / maxLevel) * this.config.portfolio.maxWeight;
      weights.push(weight);
    }

    // Normalize to sum to 1
    const total = weights.reduce((a, b) => a + b, 0);
    if (total > 0) {
      return weights.map(w => w / total);
    }

    return weights;
  }

  // Run QAOA
  runQAOA(expectedReturns, covarianceMatrix) {
    const numQubits = this.config.portfolio.numAssets * this.config.encoding.bitsPerWeight;
    const costFunction = this.createCostFunction(
      expectedReturns,
      covarianceMatrix,
      this.config.portfolio.riskAversion
    );

    // Initialize angles
    let gammas = new Array(this.config.qaoa.layers).fill(0.5);
    let betas = new Array(this.config.qaoa.layers).fill(0.3);

    // Classical optimization loop (simplified gradient-free)
    for (let iter = 0; iter < this.config.qaoa.maxIterations; iter++) {
      const result = this.evaluateQAOA(numQubits, gammas, betas, costFunction);

      if (result.avgCost < this.bestCost) {
        this.bestCost = result.avgCost;
        this.bestSolution = result.bestBitstring;
      }

      this.history.push({
        iteration: iter,
        avgCost: result.avgCost,
        bestCost: this.bestCost
      });

      // Simple parameter update (gradient-free)
      for (let l = 0; l < this.config.qaoa.layers; l++) {
        gammas[l] += (Math.random() - 0.5) * 0.1 * (1 - iter / this.config.qaoa.maxIterations);
        betas[l] += (Math.random() - 0.5) * 0.1 * (1 - iter / this.config.qaoa.maxIterations);
      }
    }

    return {
      bestBitstring: this.bestSolution,
      bestWeights: this.decodeWeights(this.bestSolution),
      bestCost: this.bestCost,
      history: this.history
    };
  }

  // Evaluate QAOA for given angles
  evaluateQAOA(numQubits, gammas, betas, costFunction) {
    // Use smaller qubit count for simulation
    const effectiveQubits = Math.min(numQubits, 12);

    const state = new QuantumState(effectiveQubits);
    state.hadamardAll();

    // Apply QAOA layers
    for (let l = 0; l < this.config.qaoa.layers; l++) {
      state.applyCostPhase(gammas[l], costFunction);
      state.applyMixerPhase(betas[l]);
    }

    // Sample solutions
    let totalCost = 0;
    let bestCost = Infinity;
    let bestBitstring = 0;

    for (let shot = 0; shot < this.config.qaoa.shots; shot++) {
      const measured = state.measure();
      const cost = costFunction(measured);
      totalCost += cost;

      if (cost < bestCost) {
        bestCost = cost;
        bestBitstring = measured;
      }
    }

    return {
      avgCost: totalCost / this.config.qaoa.shots,
      bestCost,
      bestBitstring
    };
  }
}

// Simulated Quantum Annealing
class QuantumAnnealer {
  constructor(config) {
    this.config = config;
    this.bestSolution = null;
    this.bestEnergy = Infinity;
    this.history = [];
  }

  // QUBO formulation for portfolio optimization
  createQUBOMatrix(expectedReturns, covarianceMatrix, riskAversion) {
    const n = expectedReturns.length;
    const Q = Array(n).fill(null).map(() => Array(n).fill(0));

    // Linear terms (returns)
    for (let i = 0; i < n; i++) {
      Q[i][i] = -expectedReturns[i];
    }

    // Quadratic terms (covariance)
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        Q[i][j] += riskAversion * covarianceMatrix[i][j];
      }
    }

    return Q;
  }

  // Calculate QUBO energy
  calculateEnergy(Q, solution) {
    let energy = 0;
    const n = Q.length;

    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        energy += Q[i][j] * solution[i] * solution[j];
      }
    }

    // Constraint: sum of weights should be close to 1
    const totalWeight = solution.reduce((a, b) => a + b, 0);
    const constraint = this.config.encoding.penaltyWeight * (totalWeight / n - 1) ** 2;

    return energy + constraint;
  }

  // Run simulated quantum annealing
  runAnnealing(expectedReturns, covarianceMatrix) {
    const Q = this.createQUBOMatrix(
      expectedReturns,
      covarianceMatrix,
      this.config.portfolio.riskAversion
    );
    const n = expectedReturns.length;

    // Initialize random binary solution
    let solution = Array(n).fill(0).map(() => Math.random() < 0.5 ? 1 : 0);
    let energy = this.calculateEnergy(Q, solution);

    this.bestSolution = [...solution];
    this.bestEnergy = energy;

    let temp = this.config.annealing.initialTemp;

    for (let sweep = 0; sweep < this.config.annealing.sweeps; sweep++) {
      // Quantum tunneling probability (higher at low temps in quantum annealing)
      const tunnelProb = Math.exp(-sweep / this.config.annealing.sweeps);

      for (let i = 0; i < n; i++) {
        // Propose flip
        const newSolution = [...solution];
        newSolution[i] = 1 - newSolution[i];

        // Also consider tunneling (flip multiple bits)
        if (Math.random() < tunnelProb * 0.1) {
          const j = Math.floor(Math.random() * n);
          if (j !== i) newSolution[j] = 1 - newSolution[j];
        }

        const newEnergy = this.calculateEnergy(Q, newSolution);
        const deltaE = newEnergy - energy;

        // Metropolis-Hastings with quantum tunneling
        if (deltaE < 0 || Math.random() < Math.exp(-deltaE / temp) + tunnelProb * 0.01) {
          solution = newSolution;
          energy = newEnergy;

          if (energy < this.bestEnergy) {
            this.bestSolution = [...solution];
            this.bestEnergy = energy;
          }
        }
      }

      temp *= this.config.annealing.coolingRate;

      if (sweep % 100 === 0) {
        this.history.push({
          sweep,
          temperature: temp,
          energy: energy,
          bestEnergy: this.bestEnergy
        });
      }
    }

    // Convert binary to weights
    const weights = this.bestSolution.map(b => b / this.bestSolution.reduce((a, b) => a + b, 1));

    return {
      bestSolution: this.bestSolution,
      bestWeights: weights,
      bestEnergy: this.bestEnergy,
      history: this.history
    };
  }
}

// Classical portfolio optimizer for comparison
class ClassicalOptimizer {
  optimize(expectedReturns, covarianceMatrix, riskAversion) {
    const n = expectedReturns.length;

    // Simple gradient descent on Markowitz objective
    let weights = new Array(n).fill(1 / n);
    const lr = 0.01;

    for (let iter = 0; iter < 1000; iter++) {
      // Calculate gradient
      const gradients = new Array(n).fill(0);

      for (let i = 0; i < n; i++) {
        // d/dw_i of (-return + riskAversion * variance)
        gradients[i] = -expectedReturns[i];

        for (let j = 0; j < n; j++) {
          gradients[i] += 2 * riskAversion * covarianceMatrix[i][j] * weights[j];
        }
      }

      // Update weights
      for (let i = 0; i < n; i++) {
        weights[i] -= lr * gradients[i];
        weights[i] = Math.max(0, weights[i]);  // Non-negative
      }

      // Normalize
      const total = weights.reduce((a, b) => a + b, 0);
      weights = weights.map(w => w / total);
    }

    // Calculate final metrics
    const expectedReturn = weights.reduce((sum, w, i) => sum + w * expectedReturns[i], 0);
    let variance = 0;
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        variance += weights[i] * weights[j] * covarianceMatrix[i][j];
      }
    }

    return {
      weights,
      expectedReturn,
      variance,
      sharpe: expectedReturn / Math.sqrt(variance)
    };
  }
}

// Generate synthetic market data
function generateMarketData(numAssets, seed = 42) {
  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  const assetNames = ['AAPL', 'MSFT', 'GOOGL', 'AMZN', 'NVDA', 'JPM', 'BAC', 'XOM', 'JNJ', 'WMT'];

  // Generate expected returns (5-15% annualized)
  const expectedReturns = [];
  for (let i = 0; i < numAssets; i++) {
    expectedReturns.push(0.05 + random() * 0.10);
  }

  // Generate covariance matrix (positive semi-definite)
  const volatilities = [];
  for (let i = 0; i < numAssets; i++) {
    volatilities.push(0.15 + random() * 0.20);  // 15-35% vol
  }

  // Correlation matrix with sector structure
  const correlations = Array(numAssets).fill(null).map(() => Array(numAssets).fill(0));
  for (let i = 0; i < numAssets; i++) {
    for (let j = 0; j < numAssets; j++) {
      if (i === j) {
        correlations[i][j] = 1;
      } else {
        // Higher correlation within "sectors"
        const sameSector = Math.floor(i / 3) === Math.floor(j / 3);
        correlations[i][j] = sameSector ? 0.5 + random() * 0.3 : 0.2 + random() * 0.3;
        correlations[j][i] = correlations[i][j];
      }
    }
  }

  // Covariance = correlation * vol_i * vol_j
  const covarianceMatrix = Array(numAssets).fill(null).map(() => Array(numAssets).fill(0));
  for (let i = 0; i < numAssets; i++) {
    for (let j = 0; j < numAssets; j++) {
      covarianceMatrix[i][j] = correlations[i][j] * volatilities[i] * volatilities[j];
    }
  }

  return {
    assetNames: assetNames.slice(0, numAssets),
    expectedReturns,
    volatilities,
    correlations,
    covarianceMatrix
  };
}

async function main() {
  console.log('═'.repeat(70));
  console.log('QUANTUM-INSPIRED PORTFOLIO OPTIMIZATION');
  console.log('═'.repeat(70));
  console.log();

  // 1. Generate market data
  console.log('1. Market Data Generation:');
  console.log('─'.repeat(70));

  const numAssets = quantumConfig.portfolio.numAssets;
  const marketData = generateMarketData(numAssets);

  console.log(`   Assets:           ${numAssets}`);
  console.log(`   Risk aversion:    ${quantumConfig.portfolio.riskAversion}`);
  console.log(`   Max weight:       ${(quantumConfig.portfolio.maxWeight * 100).toFixed(0)}%`);
  console.log(`   Cardinality:      Max ${quantumConfig.portfolio.cardinalityConstraint} assets`);
  console.log();

  console.log('   Asset Characteristics:');
  console.log('   Asset │ E[R]    │ Vol     │');
  console.log('─'.repeat(70));
  for (let i = 0; i < Math.min(5, numAssets); i++) {
    console.log(`   ${marketData.assetNames[i].padEnd(5)} │ ${(marketData.expectedReturns[i] * 100).toFixed(1)}%   │ ${(marketData.volatilities[i] * 100).toFixed(1)}%   │`);
  }
  console.log(`   ... (${numAssets - 5} more assets)`);
  console.log();

  // 2. Classical optimization (baseline)
  console.log('2. Classical Optimization (Baseline):');
  console.log('─'.repeat(70));

  const classical = new ClassicalOptimizer();
  const classicalResult = classical.optimize(
    marketData.expectedReturns,
    marketData.covarianceMatrix,
    quantumConfig.portfolio.riskAversion
  );

  console.log(`   Expected Return:  ${(classicalResult.expectedReturn * 100).toFixed(2)}%`);
  console.log(`   Portfolio Vol:    ${(Math.sqrt(classicalResult.variance) * 100).toFixed(2)}%`);
  console.log(`   Sharpe Ratio:     ${classicalResult.sharpe.toFixed(3)}`);
  console.log();

  console.log('   Weights:');
  const sortedClassical = classicalResult.weights
    .map((w, i) => ({ name: marketData.assetNames[i], weight: w }))
    .sort((a, b) => b.weight - a.weight)
    .slice(0, 5);

  for (const { name, weight } of sortedClassical) {
    const bar = '█'.repeat(Math.floor(weight * 40));
    console.log(`   ${name.padEnd(5)} ${bar.padEnd(40)} ${(weight * 100).toFixed(1)}%`);
  }
  console.log();

  // 3. Quantum Annealing
  console.log('3. Quantum Annealing Optimization:');
  console.log('─'.repeat(70));

  const annealer = new QuantumAnnealer(quantumConfig);
  const annealingResult = annealer.runAnnealing(
    marketData.expectedReturns,
    marketData.covarianceMatrix
  );

  // Calculate metrics for annealing result
  const annealingReturn = annealingResult.bestWeights.reduce(
    (sum, w, i) => sum + w * marketData.expectedReturns[i], 0
  );
  let annealingVariance = 0;
  for (let i = 0; i < numAssets; i++) {
    for (let j = 0; j < numAssets; j++) {
      annealingVariance += annealingResult.bestWeights[i] * annealingResult.bestWeights[j] *
                          marketData.covarianceMatrix[i][j];
    }
  }

  console.log(`   Expected Return:  ${(annealingReturn * 100).toFixed(2)}%`);
  console.log(`   Portfolio Vol:    ${(Math.sqrt(annealingVariance) * 100).toFixed(2)}%`);
  console.log(`   Sharpe Ratio:     ${(annealingReturn / Math.sqrt(annealingVariance)).toFixed(3)}`);
  console.log(`   Final Energy:     ${annealingResult.bestEnergy.toFixed(4)}`);
  console.log();

  console.log('   Binary Solution:');
  console.log(`   ${annealingResult.bestSolution.join('')}`);
  console.log();

  console.log('   Weights:');
  const sortedAnnealing = annealingResult.bestWeights
    .map((w, i) => ({ name: marketData.assetNames[i], weight: w }))
    .sort((a, b) => b.weight - a.weight)
    .filter(x => x.weight > 0.01)
    .slice(0, 5);

  for (const { name, weight } of sortedAnnealing) {
    const bar = '█'.repeat(Math.floor(weight * 40));
    console.log(`   ${name.padEnd(5)} ${bar.padEnd(40)} ${(weight * 100).toFixed(1)}%`);
  }
  console.log();

  // 4. QAOA (simplified)
  console.log('4. QAOA Optimization (Simplified):');
  console.log('─'.repeat(70));

  // Use smaller problem for QAOA simulation
  const qaoaConfig = { ...quantumConfig, portfolio: { ...quantumConfig.portfolio, numAssets: 4 } };
  const smallMarketData = generateMarketData(4);

  const qaoa = new QAOAOptimizer(qaoaConfig);
  const qaoaResult = qaoa.runQAOA(
    smallMarketData.expectedReturns,
    smallMarketData.covarianceMatrix
  );

  console.log(`   QAOA Layers (p):  ${quantumConfig.qaoa.layers}`);
  console.log(`   Measurement shots: ${quantumConfig.qaoa.shots}`);
  console.log(`   Best Cost:        ${qaoaResult.bestCost.toFixed(4)}`);
  console.log();

  console.log('   QAOA Weights (4-asset subset):');
  for (let i = 0; i < qaoaResult.bestWeights.length; i++) {
    const w = qaoaResult.bestWeights[i];
    const bar = '█'.repeat(Math.floor(w * 40));
    console.log(`   Asset ${i + 1} ${bar.padEnd(40)} ${(w * 100).toFixed(1)}%`);
  }
  console.log();

  // 5. Annealing convergence
  console.log('5. Annealing Convergence:');
  console.log('─'.repeat(70));

  console.log('   Energy vs Temperature:');
  let curve = '   ';
  const energies = annealingResult.history.map(h => h.energy);
  const minE = Math.min(...energies);
  const maxE = Math.max(...energies);
  const rangeE = maxE - minE || 1;

  for (const h of annealingResult.history.slice(-40)) {
    const norm = 1 - (h.energy - minE) / rangeE;
    if (norm < 0.25) curve += '▁';
    else if (norm < 0.5) curve += '▃';
    else if (norm < 0.75) curve += '▅';
    else curve += '█';
  }
  console.log(curve);
  console.log(`   Start Energy: ${maxE.toFixed(3)}  Final: ${minE.toFixed(3)}`);
  console.log();

  // 6. Quantum advantage discussion
  console.log('6. Quantum Advantage Analysis:');
  console.log('─'.repeat(70));

  console.log('   Problem Complexity:');
  console.log(`   - Classical: O(n³) for Markowitz with constraints`);
  console.log(`   - Quantum:   O(√n) potential speedup via Grover`);
  console.log();

  console.log('   This simulation demonstrates:');
  console.log('   - QUBO formulation of portfolio optimization');
  console.log('   - Quantum annealing energy landscape exploration');
  console.log('   - QAOA variational quantum-classical hybrid');
  console.log();

  console.log('   Real quantum hardware benefits:');
  console.log('   - Combinatorial (cardinality) constraints');
  console.log('   - Large-scale problems (1000+ assets)');
  console.log('   - Non-convex objectives');
  console.log();

  // 7. Comparison summary
  console.log('7. Method Comparison:');
  console.log('─'.repeat(70));

  const classicalSharpe = classicalResult.sharpe;
  const annealingSharpe = annealingReturn / Math.sqrt(annealingVariance);

  console.log('   Method          │ Return │ Vol    │ Sharpe │ Assets');
  console.log('─'.repeat(70));
  console.log(`   Classical       │ ${(classicalResult.expectedReturn * 100).toFixed(1)}%  │ ${(Math.sqrt(classicalResult.variance) * 100).toFixed(1)}%  │ ${classicalSharpe.toFixed(3)}  │ ${classicalResult.weights.filter(w => w > 0.01).length}`);
  console.log(`   Quantum Anneal  │ ${(annealingReturn * 100).toFixed(1)}%  │ ${(Math.sqrt(annealingVariance) * 100).toFixed(1)}%  │ ${annealingSharpe.toFixed(3)}  │ ${annealingResult.bestWeights.filter(w => w > 0.01).length}`);
  console.log();

  // 8. RuVector integration
  console.log('8. RuVector Vector Storage:');
  console.log('─'.repeat(70));
  console.log('   Portfolio weight vectors can be stored:');
  console.log();
  console.log(`   Classical weights: [${classicalResult.weights.slice(0, 4).map(w => w.toFixed(3)).join(', ')}, ...]`);
  console.log(`   Quantum weights:   [${annealingResult.bestWeights.slice(0, 4).map(w => w.toFixed(3)).join(', ')}, ...]`);
  console.log();
  console.log('   Use cases:');
  console.log('   - Similarity search for portfolio allocation patterns');
  console.log('   - Regime-based portfolio retrieval');
  console.log('   - Factor exposure analysis via vector operations');
  console.log();

  console.log('═'.repeat(70));
  console.log('Quantum-inspired portfolio optimization completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
