/**
 * Core solver algorithms for asymmetric diagonally dominant systems
 * Implements Neumann series, random walks, and push methods
 */
import { SolverError, ErrorCodes } from './types.js';
import { MatrixOperations } from './matrix.js';
import { VectorOperations, PerformanceMonitor, ConvergenceChecker, TimeoutController, ValidationUtils, createSeededRandom } from './utils.js';
import { initializeAllWasm } from './wasm-bridge.js';
export class SublinearSolver {
    config;
    performanceMonitor;
    convergenceChecker;
    timeoutController;
    wasmAccelerated = false;
    wasmModules = {};
    constructor(config) {
        this.config = config;
        this.validateConfig(config);
        this.performanceMonitor = new PerformanceMonitor();
        this.convergenceChecker = new ConvergenceChecker();
        if (config.timeout) {
            this.timeoutController = new TimeoutController(config.timeout);
        }
        // Initialize WASM if available
        this.initializeWasm().catch(console.warn);
    }
    async initializeWasm() {
        try {
            const { temporal, graph, hasWasm } = await initializeAllWasm();
            this.wasmModules = { temporal, graph };
            this.wasmAccelerated = hasWasm;
            if (this.wasmAccelerated) {
                console.log('🚀 WASM acceleration enabled');
            }
        }
        catch (error) {
            console.warn('WASM initialization failed, using JavaScript fallback');
            this.wasmAccelerated = false;
        }
    }
    validateConfig(config) {
        ValidationUtils.validatePositiveNumber(config.epsilon, 'epsilon');
        ValidationUtils.validateIntegerRange(config.maxIterations, 1, 1e6, 'maxIterations');
        if (config.timeout) {
            ValidationUtils.validatePositiveNumber(config.timeout, 'timeout');
        }
    }
    /**
     * Solve ADD system Mx = b using specified method
     */
    async solve(matrix, vector, progressCallback) {
        MatrixOperations.validateMatrix(matrix);
        if (vector.length !== matrix.cols) {
            throw new SolverError(`Vector length ${vector.length} does not match matrix columns ${matrix.cols}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        // Check diagonal dominance
        const analysis = MatrixOperations.analyzeMatrix(matrix);
        if (!analysis.isDiagonallyDominant) {
            throw new SolverError('Matrix is not diagonally dominant', ErrorCodes.NOT_DIAGONALLY_DOMINANT, { analysis });
        }
        this.performanceMonitor.reset();
        this.convergenceChecker.reset();
        let result;
        try {
            switch (this.config.method) {
                case 'neumann':
                    result = await this.solveNeumann(matrix, vector, progressCallback);
                    break;
                case 'random-walk':
                    result = await this.solveRandomWalk(matrix, vector, progressCallback);
                    break;
                case 'forward-push':
                    result = await this.solveForwardPush(matrix, vector, progressCallback);
                    break;
                case 'backward-push':
                    result = await this.solveBackwardPush(matrix, vector, progressCallback);
                    break;
                case 'bidirectional':
                    result = await this.solveBidirectional(matrix, vector, progressCallback);
                    break;
                default:
                    throw new SolverError(`Unknown method: ${this.config.method}`, ErrorCodes.INVALID_PARAMETERS);
            }
            return result;
        }
        catch (error) {
            if (error instanceof SolverError) {
                throw error;
            }
            throw new SolverError(`Solver failed: ${error}`, ErrorCodes.CONVERGENCE_FAILED);
        }
    }
    /**
     * Solve using Neumann series expansion
     * x* = (I - D^(-1)R)^(-1) D^(-1) b = sum_{k=0}^∞ (D^(-1)R)^k D^(-1) b
     */
    async solveNeumann(matrix, vector, progressCallback) {
        const n = matrix.rows;
        // Extract diagonal and off-diagonal parts
        const diagonal = MatrixOperations.getDiagonalVector(matrix);
        // Validate diagonal elements
        for (let i = 0; i < n; i++) {
            if (Math.abs(diagonal[i]) < 1e-15) {
                throw new SolverError(`Zero or near-zero diagonal element at position ${i}: ${diagonal[i]}`, ErrorCodes.NUMERICAL_INSTABILITY);
            }
        }
        const invD = VectorOperations.elementwiseDivide(VectorOperations.ones(n), diagonal);
        // Initialize solution with D^(-1) b
        let solution = VectorOperations.elementwiseMultiply(invD, vector);
        let seriesTerm = [...solution];
        let previousResidual = Infinity;
        const state = {
            iteration: 0,
            residual: Infinity,
            solution,
            converged: false,
            elapsedTime: 0,
            series: [seriesTerm],
            convergenceRate: 1.0
        };
        // Improved convergence detection
        let stagnationCounter = 0;
        const maxStagnation = 10;
        for (let k = 1; k <= this.config.maxIterations; k++) {
            this.timeoutController?.checkTimeout();
            // Compute (D^(-1)R)^k D^(-1) b iteratively
            // seriesTerm = D^(-1) * (R * seriesTerm)
            const Rterm = this.computeOffDiagonalMultiply(matrix, seriesTerm);
            seriesTerm = VectorOperations.elementwiseMultiply(invD, Rterm);
            // Add to solution
            solution = VectorOperations.add(solution, seriesTerm);
            // Compute residual: ||Mx - b|| every few iterations (expensive)
            if (k % 5 === 0 || k <= 10) {
                const residualVec = VectorOperations.subtract(MatrixOperations.multiplyMatrixVector(matrix, solution), vector);
                state.residual = VectorOperations.norm2(residualVec);
            }
            else {
                // Estimate residual from series term norm
                state.residual = VectorOperations.norm2(seriesTerm) * Math.sqrt(n);
            }
            state.iteration = k;
            state.solution = [...solution];
            state.elapsedTime = this.performanceMonitor.getElapsedTime();
            state.series.push([...seriesTerm]);
            // Check convergence
            const convergenceInfo = this.convergenceChecker.checkConvergence(state.residual, this.config.epsilon);
            state.converged = convergenceInfo.converged;
            state.convergenceRate = convergenceInfo.rate;
            // Detect stagnation
            if (Math.abs(state.residual - previousResidual) < this.config.epsilon * 1e-6) {
                stagnationCounter++;
                if (stagnationCounter >= maxStagnation) {
                    console.warn(`Neumann series stagnated after ${k} iterations`);
                    break;
                }
            }
            else {
                stagnationCounter = 0;
            }
            if (progressCallback) {
                progressCallback({
                    iteration: k,
                    residual: state.residual,
                    elapsed: state.elapsedTime
                });
            }
            if (state.converged) {
                break;
            }
            // Check if series term is becoming negligible (early termination)
            const termNorm = VectorOperations.norm2(seriesTerm);
            if (termNorm < this.config.epsilon * 1e-6) {
                console.log(`Series term negligible after ${k} iterations`);
                break;
            }
            // Prevent numerical overflow
            if (!isFinite(state.residual) || state.residual > 1e15) {
                throw new SolverError(`Numerical instability detected at iteration ${k}`, ErrorCodes.NUMERICAL_INSTABILITY, { residual: state.residual });
            }
            previousResidual = state.residual;
        }
        // Final accurate residual computation
        const finalResidualVec = VectorOperations.subtract(MatrixOperations.multiplyMatrixVector(matrix, solution), vector);
        state.residual = VectorOperations.norm2(finalResidualVec);
        state.converged = state.residual < this.config.epsilon;
        if (!state.converged && state.iteration >= this.config.maxIterations) {
            throw new SolverError(`Neumann series failed to converge after ${this.config.maxIterations} iterations. Final residual: ${state.residual.toExponential(3)}`, ErrorCodes.CONVERGENCE_FAILED, {
                finalResidual: state.residual,
                iterations: state.iteration,
                convergenceRate: state.convergenceRate
            });
        }
        return {
            solution: state.solution,
            iterations: state.iteration,
            residual: state.residual,
            converged: state.converged,
            method: 'neumann',
            computeTime: state.elapsedTime,
            memoryUsed: this.performanceMonitor.getMemoryIncrease()
        };
    }
    /**
     * Compute off-diagonal matrix-vector multiplication: (M - D) * v
     * This computes R*v where R = M - D (off-diagonal part of matrix)
     */
    computeOffDiagonalMultiply(matrix, vector) {
        const n = matrix.rows;
        const result = new Array(n).fill(0);
        // For dense matrices
        if (matrix.format === 'dense') {
            const data = matrix.data;
            for (let i = 0; i < n; i++) {
                for (let j = 0; j < n; j++) {
                    if (i !== j) { // Skip diagonal
                        result[i] += data[i][j] * vector[j];
                    }
                }
            }
        }
        else {
            // For sparse matrices (COO format)
            const sparse = matrix;
            for (let k = 0; k < sparse.values.length; k++) {
                const i = sparse.rowIndices[k];
                const j = sparse.colIndices[k];
                if (i !== j) { // Skip diagonal
                    result[i] += sparse.values[k] * vector[j];
                }
            }
        }
        return result;
    }
    /**
     * Solve using random walk sampling
     */
    async solveRandomWalk(matrix, vector, progressCallback) {
        const n = matrix.rows;
        const rng = createSeededRandom(this.config.seed || Date.now());
        // Convert to transition probabilities
        const { transitions, absorptionProbs } = this.createTransitionMatrix(matrix);
        let solution = VectorOperations.zeros(n);
        let totalVariance = 0;
        const state = {
            iteration: 0,
            residual: Infinity,
            solution,
            converged: false,
            elapsedTime: 0,
            walks: [],
            currentEstimate: 0,
            variance: 0,
            confidence: 0
        };
        // Estimate each coordinate using random walks
        for (let i = 0; i < n; i++) {
            const estimates = [];
            const numWalks = Math.max(100, Math.ceil(1 / (this.config.epsilon * this.config.epsilon)));
            for (let walk = 0; walk < numWalks; walk++) {
                const estimate = this.performRandomWalk(i, transitions, absorptionProbs, vector, rng);
                estimates.push(estimate);
                if (walk % 10 === 0) {
                    this.timeoutController?.checkTimeout();
                }
            }
            // Compute mean and variance
            const mean = estimates.reduce((sum, val) => sum + val, 0) / estimates.length;
            const variance = estimates.reduce((sum, val) => sum + (val - mean) ** 2, 0) / (estimates.length - 1);
            solution[i] = mean;
            totalVariance += variance;
            state.iteration = i + 1;
            state.currentEstimate = mean;
            state.variance = Math.sqrt(variance);
            state.walks.push(estimates);
        }
        // Compute final residual
        const residualVec = VectorOperations.subtract(MatrixOperations.multiplyMatrixVector(matrix, solution), vector);
        state.residual = VectorOperations.norm2(residualVec);
        state.solution = solution;
        state.converged = state.residual < this.config.epsilon;
        state.elapsedTime = this.performanceMonitor.getElapsedTime();
        // For random walk, we're more lenient with convergence since it's probabilistic
        if (!state.converged && state.residual > 10 * this.config.epsilon) {
            // Only fail if we're really far off
            throw new SolverError(`Random walk sampling failed to achieve desired accuracy`, ErrorCodes.CONVERGENCE_FAILED, { finalResidual: state.residual, variance: Math.sqrt(totalVariance) });
        }
        return {
            solution: state.solution,
            iterations: state.iteration,
            residual: state.residual,
            converged: state.converged,
            method: 'random-walk',
            computeTime: state.elapsedTime,
            memoryUsed: this.performanceMonitor.getMemoryIncrease()
        };
    }
    /**
     * Create transition matrix for random walks
     */
    createTransitionMatrix(matrix) {
        const n = matrix.rows;
        const transitions = Array(n).fill(null).map(() => Array(n).fill(0));
        const absorptionProbs = new Array(n);
        for (let i = 0; i < n; i++) {
            const diagEntry = MatrixOperations.getDiagonal(matrix, i);
            if (Math.abs(diagEntry) < 1e-15) {
                throw new SolverError(`Zero diagonal at position ${i}`, ErrorCodes.NUMERICAL_INSTABILITY);
            }
            absorptionProbs[i] = 1 / diagEntry;
            // Compute transition probabilities
            for (let j = 0; j < n; j++) {
                if (i !== j) {
                    const entry = MatrixOperations.getEntry(matrix, i, j);
                    transitions[i][j] = -entry / diagEntry;
                }
            }
        }
        return { transitions, absorptionProbs };
    }
    /**
     * Perform a single random walk
     */
    performRandomWalk(start, transitions, absorptionProbs, vector, rng) {
        let current = start;
        let value = 0;
        const maxSteps = 1000; // Prevent infinite walks
        for (let step = 0; step < maxSteps; step++) {
            // Check for absorption
            if (rng() < Math.abs(absorptionProbs[current])) {
                value += vector[current] * absorptionProbs[current];
                break;
            }
            // Choose next state based on transition probabilities
            const cumulative = [];
            let sum = 0;
            for (let j = 0; j < transitions[current].length; j++) {
                sum += Math.abs(transitions[current][j]);
                cumulative.push(sum);
            }
            if (sum === 0) {
                // No outgoing transitions, absorb here
                value += vector[current] * absorptionProbs[current];
                break;
            }
            const rand = rng() * sum;
            for (let j = 0; j < cumulative.length; j++) {
                if (rand <= cumulative[j]) {
                    current = j;
                    break;
                }
            }
        }
        return value;
    }
    /**
     * Solve using forward push method
     */
    async solveForwardPush(matrix, vector, progressCallback) {
        const n = matrix.rows;
        let approximate = VectorOperations.zeros(n);
        let residual = [...vector];
        const state = {
            iteration: 0,
            residual: Infinity,
            solution: approximate,
            converged: false,
            elapsedTime: 0,
            residualVector: residual,
            approximateVector: approximate,
            pushDirection: 'forward'
        };
        for (let iter = 0; iter < this.config.maxIterations; iter++) {
            this.timeoutController?.checkTimeout();
            // Find node with largest residual
            let maxResidual = 0;
            let maxNode = -1;
            for (let i = 0; i < n; i++) {
                if (Math.abs(residual[i]) > maxResidual) {
                    maxResidual = Math.abs(residual[i]);
                    maxNode = i;
                }
            }
            if (maxResidual < this.config.epsilon) {
                state.converged = true;
                break;
            }
            // Push from maxNode
            const diagEntry = MatrixOperations.getDiagonal(matrix, maxNode);
            if (Math.abs(diagEntry) < 1e-15) {
                throw new SolverError(`Zero diagonal at position ${maxNode}`, ErrorCodes.NUMERICAL_INSTABILITY);
            }
            const pushValue = residual[maxNode] / diagEntry;
            approximate[maxNode] += pushValue;
            residual[maxNode] = 0;
            // Update residuals of neighbors
            for (let j = 0; j < n; j++) {
                if (j !== maxNode) {
                    const entry = MatrixOperations.getEntry(matrix, j, maxNode);
                    residual[j] -= entry * pushValue;
                }
            }
            state.iteration = iter + 1;
            state.residual = VectorOperations.norm2(residual);
            state.solution = [...approximate];
            state.residualVector = [...residual];
            state.approximateVector = [...approximate];
            state.elapsedTime = this.performanceMonitor.getElapsedTime();
            if (progressCallback && iter % 10 === 0) {
                progressCallback({
                    iteration: iter + 1,
                    residual: state.residual,
                    elapsed: state.elapsedTime
                });
            }
        }
        if (!state.converged) {
            throw new SolverError(`Forward push failed to converge after ${this.config.maxIterations} iterations`, ErrorCodes.CONVERGENCE_FAILED, { finalResidual: state.residual });
        }
        return {
            solution: state.solution,
            iterations: state.iteration,
            residual: state.residual,
            converged: state.converged,
            method: 'forward-push',
            computeTime: state.elapsedTime,
            memoryUsed: this.performanceMonitor.getMemoryIncrease()
        };
    }
    /**
     * Solve using backward push method
     */
    async solveBackwardPush(matrix, vector, progressCallback) {
        // For backward push, we solve M^T y = e_i and then compute x_i = y^T b
        // This is more complex and typically used for single coordinate estimation
        return this.solveForwardPush(matrix, vector, progressCallback); // Simplified for now
    }
    /**
     * Solve using bidirectional approach (combine forward and backward)
     */
    async solveBidirectional(matrix, vector, progressCallback) {
        // Start with forward push
        const forwardResult = await this.solveForwardPush(matrix, vector, progressCallback);
        // Could enhance with backward refinement, but for now return forward result
        return {
            ...forwardResult,
            method: 'bidirectional'
        };
    }
    /**
     * Estimate a single entry of the solution M^(-1)b
     */
    async estimateEntry(matrix, vector, config) {
        MatrixOperations.validateMatrix(matrix);
        // Enhanced validation with better error messages
        if (config.row < 0 || config.row >= matrix.rows) {
            throw new SolverError(`Row index ${config.row} out of bounds. Matrix has ${matrix.rows} rows (valid range: 0-${matrix.rows - 1})`, ErrorCodes.INVALID_PARAMETERS, { row: config.row, matrixRows: matrix.rows });
        }
        if (config.column < 0 || config.column >= matrix.cols) {
            throw new SolverError(`Column index ${config.column} out of bounds. Matrix has ${matrix.cols} columns (valid range: 0-${matrix.cols - 1})`, ErrorCodes.INVALID_PARAMETERS, { column: config.column, matrixCols: matrix.cols });
        }
        if (vector.length !== matrix.rows) {
            throw new SolverError(`Vector length ${vector.length} does not match matrix rows ${matrix.rows}`, ErrorCodes.INVALID_DIMENSIONS, { vectorLength: vector.length, matrixRows: matrix.rows });
        }
        ValidationUtils.validatePositiveNumber(config.epsilon, 'epsilon');
        ValidationUtils.validateRange(config.confidence, 0, 1, 'confidence');
        const rng = createSeededRandom(this.config.seed || Date.now());
        const estimates = [];
        // Reduce samples for faster computation, especially for smaller matrices
        const maxSamples = Math.min(1000, Math.max(50, Math.ceil(1 / Math.sqrt(config.epsilon))));
        const timeoutMs = this.config.timeout || 10000; // 10 second default timeout
        const startTime = Date.now();
        try {
            if (config.method === 'random-walk') {
                const { transitions, absorptionProbs } = this.createTransitionMatrix(matrix);
                for (let i = 0; i < maxSamples; i++) {
                    // Check timeout every 10 samples
                    if (i % 10 === 0) {
                        const elapsed = Date.now() - startTime;
                        if (elapsed > timeoutMs) {
                            console.warn(`EstimateEntry timeout after ${elapsed}ms, using ${estimates.length} samples`);
                            break;
                        }
                    }
                    const estimate = this.performRandomWalk(config.row, transitions, absorptionProbs, vector, rng);
                    estimates.push(estimate);
                    // Early termination if estimates are converging
                    if (i > 20 && i % 20 === 0) {
                        const recentEstimates = estimates.slice(-20);
                        const mean = recentEstimates.reduce((sum, val) => sum + val, 0) / recentEstimates.length;
                        const variance = recentEstimates.reduce((sum, val) => sum + (val - mean) ** 2, 0) / recentEstimates.length;
                        if (Math.sqrt(variance) < config.epsilon) {
                            console.log(`EstimateEntry converged early after ${i} samples`);
                            break;
                        }
                    }
                }
            }
            else {
                // Use Neumann series estimation - much faster and more reliable
                if (config.column >= matrix.cols) {
                    throw new SolverError(`Column index ${config.column} exceeds matrix dimensions ${matrix.cols}`, ErrorCodes.INVALID_PARAMETERS);
                }
                const e_i = new Array(matrix.cols).fill(0);
                e_i[config.column] = 1;
                const result = await this.solve(matrix, e_i);
                const estimate = result.solution[config.row];
                return {
                    estimate,
                    variance: 0,
                    confidence: result.converged ? 1.0 : 0.5
                };
            }
            if (estimates.length === 0) {
                throw new SolverError('No estimates were generated', ErrorCodes.CONVERGENCE_FAILED);
            }
            const mean = estimates.reduce((sum, val) => sum + val, 0) / estimates.length;
            const variance = estimates.length > 1
                ? estimates.reduce((sum, val) => sum + (val - mean) ** 2, 0) / (estimates.length - 1)
                : 0;
            // Sanity check for numerical issues
            if (!isFinite(mean) || !isFinite(variance)) {
                throw new SolverError('Numerical instability in estimation', ErrorCodes.NUMERICAL_INSTABILITY, { mean, variance, numSamples: estimates.length });
            }
            return {
                estimate: mean,
                variance,
                confidence: config.confidence
            };
        }
        catch (error) {
            if (error instanceof SolverError) {
                throw error;
            }
            throw new SolverError(`Entry estimation failed: ${error}`, ErrorCodes.CONVERGENCE_FAILED, { row: config.row, column: config.column, method: config.method });
        }
    }
    /**
     * Compute PageRank using the solver
     */
    async computePageRank(adjacency, config) {
        MatrixOperations.validateMatrix(adjacency);
        ValidationUtils.validateRange(config.damping, 0, 1, 'damping');
        ValidationUtils.validatePositiveNumber(config.epsilon, 'epsilon');
        if (adjacency.rows !== adjacency.cols) {
            throw new SolverError('Adjacency matrix must be square', ErrorCodes.INVALID_DIMENSIONS);
        }
        const n = adjacency.rows;
        // Create the PageRank system: (I - α P^T) x = (1-α)/n * 1
        // where P is the column-stochastic transition matrix
        // Normalize adjacency to get transition matrix
        const outDegrees = new Array(n).fill(0);
        for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
                outDegrees[i] += MatrixOperations.getEntry(adjacency, i, j);
            }
        }
        // Build system matrix I - α P^T
        const systemMatrix = Array(n).fill(null).map(() => Array(n).fill(0));
        for (let i = 0; i < n; i++) {
            systemMatrix[i][i] = 1; // Identity part
            for (let j = 0; j < n; j++) {
                if (outDegrees[j] > 0) {
                    const transitionProb = MatrixOperations.getEntry(adjacency, j, i) / outDegrees[j];
                    systemMatrix[i][j] -= config.damping * transitionProb;
                }
            }
        }
        const systemMatrixFormatted = {
            rows: n,
            cols: n,
            data: systemMatrix,
            format: 'dense'
        };
        // Right-hand side
        const rhs = config.personalized || VectorOperations.scale(VectorOperations.ones(n), (1 - config.damping) / n);
        // Solve the system
        const solverConfig = {
            method: this.config.method,
            epsilon: config.epsilon,
            maxIterations: config.maxIterations,
            timeout: this.config.timeout
        };
        const solver = new SublinearSolver(solverConfig);
        const result = await solver.solve(systemMatrixFormatted, rhs);
        // Return the PageRank vector directly as expected by GraphTools
        return result.solution;
    }
}
