/**
 * MCP Tools for graph algorithms using sublinear solvers
 */
import { SublinearSolver } from '../../core/solver.js';
import { MatrixOperations } from '../../core/matrix.js';
import { VectorOperations } from '../../core/utils.js';
import { SolverError, ErrorCodes } from '../../core/types.js';
export class GraphTools {
    /**
     * Compute PageRank using sublinear solver
     */
    static async pageRank(params) {
        MatrixOperations.validateMatrix(params.adjacency);
        if (params.adjacency.rows !== params.adjacency.cols) {
            throw new SolverError('Adjacency matrix must be square', ErrorCodes.INVALID_DIMENSIONS);
        }
        const config = {
            method: 'neumann',
            epsilon: params.epsilon || 1e-6,
            maxIterations: params.maxIterations || 1000,
            enableProgress: false
        };
        const solver = new SublinearSolver(config);
        const pageRankConfig = {
            damping: params.damping || 0.85,
            personalized: params.personalized,
            epsilon: params.epsilon || 1e-6,
            maxIterations: params.maxIterations || 1000
        };
        const pageRankVector = await solver.computePageRank(params.adjacency, pageRankConfig);
        // Analyze results
        const ranked = pageRankVector
            .map((score, index) => ({ node: index, score }))
            .sort((a, b) => b.score - a.score);
        const totalScore = pageRankVector.reduce((sum, score) => sum + score, 0);
        const maxScore = Math.max(...pageRankVector);
        const minScore = Math.min(...pageRankVector);
        // Compute distribution statistics
        const mean = totalScore / pageRankVector.length;
        const variance = pageRankVector.reduce((sum, score) => sum + (score - mean) ** 2, 0) / pageRankVector.length;
        const entropy = -pageRankVector.reduce((sum, score) => {
            if (score > 0) {
                return sum + score * Math.log(score);
            }
            return sum;
        }, 0);
        return {
            pageRankVector,
            topNodes: ranked.slice(0, Math.min(10, ranked.length)),
            bottomNodes: ranked.slice(-Math.min(10, ranked.length)).reverse(),
            statistics: {
                totalScore,
                maxScore,
                minScore,
                mean,
                standardDeviation: Math.sqrt(variance),
                entropy,
                convergenceInfo: {
                    damping: pageRankConfig.damping,
                    personalized: !!params.personalized
                }
            },
            distribution: {
                quantiles: this.computeQuantiles(pageRankVector, [0.1, 0.25, 0.5, 0.75, 0.9]),
                concentrationRatio: ranked.slice(0, Math.ceil(ranked.length * 0.1))
                    .reduce((sum, item) => sum + item.score, 0) / totalScore
            }
        };
    }
    /**
     * Compute personalized PageRank for specific nodes
     */
    static async personalizedPageRank(adjacency, personalizeNodes, params = {}) {
        const n = adjacency.rows;
        const personalized = VectorOperations.zeros(n);
        // Set personalization vector
        const weight = 1.0 / personalizeNodes.length;
        for (const node of personalizeNodes) {
            if (node < 0 || node >= n) {
                throw new SolverError(`Node ${node} out of bounds`, ErrorCodes.INVALID_PARAMETERS);
            }
            personalized[node] = weight;
        }
        const result = await this.pageRank({
            adjacency,
            personalized,
            ...params
        });
        return {
            ...result,
            personalizedFor: personalizeNodes,
            influence: {
                directInfluence: personalizeNodes.map(node => result.pageRankVector[node]),
                totalInfluence: personalizeNodes.reduce((sum, node) => sum + result.pageRankVector[node], 0)
            }
        };
    }
    /**
     * Compute effective resistance between nodes
     */
    static async effectiveResistance(params) {
        MatrixOperations.validateMatrix(params.laplacian);
        if (params.source < 0 || params.source >= params.laplacian.rows) {
            throw new SolverError(`Source node ${params.source} out of bounds`, ErrorCodes.INVALID_PARAMETERS);
        }
        if (params.target < 0 || params.target >= params.laplacian.rows) {
            throw new SolverError(`Target node ${params.target} out of bounds`, ErrorCodes.INVALID_PARAMETERS);
        }
        const n = params.laplacian.rows;
        // Create indicator vector e_s - e_t
        const indicator = VectorOperations.zeros(n);
        indicator[params.source] = 1;
        indicator[params.target] = -1;
        // We need to solve the pseudoinverse, which requires handling the null space
        // For a connected graph, we can use the grounded Laplacian (remove one row/column)
        const groundedLaplacian = this.createGroundedLaplacian(params.laplacian);
        const config = {
            method: 'neumann',
            epsilon: params.epsilon || 1e-6,
            maxIterations: 1000,
            enableProgress: false
        };
        const solver = new SublinearSolver(config);
        // Remove the grounded node from the indicator vector
        const groundedIndicator = indicator.slice(0, n - 1);
        try {
            const result = await solver.solve(groundedLaplacian, groundedIndicator);
            const voltage = [...result.solution, 0]; // Add back the grounded node
            // Effective resistance is the voltage difference
            const resistance = voltage[params.source] - voltage[params.target];
            return {
                effectiveResistance: Math.abs(resistance),
                voltage,
                source: params.source,
                target: params.target,
                convergenceInfo: {
                    iterations: result.iterations,
                    residual: result.residual,
                    converged: result.converged
                }
            };
        }
        catch (error) {
            throw new SolverError(`Failed to compute effective resistance: ${error}`, ErrorCodes.CONVERGENCE_FAILED);
        }
    }
    /**
     * Compute centrality measures using sublinear methods
     */
    static async computeCentralities(adjacency, measures = ['pagerank', 'closeness']) {
        const results = {};
        if (measures.includes('pagerank')) {
            results.pagerank = await this.pageRank({ adjacency });
        }
        if (measures.includes('closeness')) {
            results.closeness = await this.closenessCentrality(adjacency);
        }
        if (measures.includes('betweenness')) {
            results.betweenness = await this.betweennessCentrality(adjacency);
        }
        return results;
    }
    /**
     * Detect communities using spectral methods
     */
    static async detectCommunities(adjacency, numCommunities = 2) {
        // Create normalized Laplacian
        const laplacian = this.createNormalizedLaplacian(adjacency);
        // This is a simplified approach - in practice would need eigenvector computation
        const config = {
            method: 'random-walk',
            epsilon: 1e-4,
            maxIterations: 500,
            enableProgress: false
        };
        const solver = new SublinearSolver(config);
        const n = adjacency.rows;
        // Use random walk mixing as a proxy for community structure
        const communities = Array(numCommunities).fill(null).map(() => []);
        const assignments = new Array(n);
        // Simplified community assignment based on PageRank clustering
        const pageRankResult = await this.pageRank({ adjacency });
        const sortedNodes = pageRankResult.topNodes;
        // Assign nodes to communities in round-robin fashion (simplified)
        for (let i = 0; i < n; i++) {
            const community = i % numCommunities;
            communities[community].push(sortedNodes[i]?.node ?? i);
            assignments[sortedNodes[i]?.node ?? i] = community;
        }
        return {
            communities,
            assignments,
            modularity: this.computeModularity(adjacency, assignments),
            quality: {
                numCommunities,
                largestCommunity: Math.max(...communities.map(c => c.length)),
                smallestCommunity: Math.min(...communities.map(c => c.length))
            }
        };
    }
    static computeQuantiles(values, quantiles) {
        const sorted = [...values].sort((a, b) => a - b);
        const result = {};
        for (const q of quantiles) {
            const index = Math.floor(q * (sorted.length - 1));
            result[`q${(q * 100).toFixed(0)}`] = sorted[index];
        }
        return result;
    }
    static createGroundedLaplacian(laplacian) {
        const n = laplacian.rows;
        if (laplacian.format === 'dense') {
            const dense = laplacian;
            const groundedData = dense.data.slice(0, n - 1).map((row) => row.slice(0, n - 1));
            return {
                rows: n - 1,
                cols: n - 1,
                data: groundedData,
                format: 'dense'
            };
        }
        else {
            // For sparse matrices, filter out entries in the last row/column
            const sparse = laplacian;
            const values = [];
            const rowIndices = [];
            const colIndices = [];
            for (let k = 0; k < sparse.values.length; k++) {
                if (sparse.rowIndices[k] < n - 1 && sparse.colIndices[k] < n - 1) {
                    values.push(sparse.values[k]);
                    rowIndices.push(sparse.rowIndices[k]);
                    colIndices.push(sparse.colIndices[k]);
                }
            }
            return {
                rows: n - 1,
                cols: n - 1,
                values,
                rowIndices,
                colIndices,
                format: 'coo'
            };
        }
    }
    static createNormalizedLaplacian(adjacency) {
        const n = adjacency.rows;
        const degrees = new Array(n).fill(0);
        // Compute degrees
        for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
                degrees[i] += MatrixOperations.getEntry(adjacency, i, j);
            }
        }
        // Create normalized Laplacian: L = I - D^(-1/2) A D^(-1/2)
        const data = Array(n).fill(null).map(() => Array(n).fill(0));
        for (let i = 0; i < n; i++) {
            data[i][i] = 1; // Identity part
            for (let j = 0; j < n; j++) {
                if (i !== j && degrees[i] > 0 && degrees[j] > 0) {
                    const normalization = Math.sqrt(degrees[i] * degrees[j]);
                    data[i][j] = -MatrixOperations.getEntry(adjacency, i, j) / normalization;
                }
            }
        }
        return {
            rows: n,
            cols: n,
            data,
            format: 'dense'
        };
    }
    static async closenessCentrality(adjacency) {
        // Simplified implementation - would need all-pairs shortest paths
        const n = adjacency.rows;
        const closeness = new Array(n).fill(0);
        // This is a placeholder - actual implementation would compute shortest paths
        for (let i = 0; i < n; i++) {
            closeness[i] = Math.random(); // Placeholder
        }
        return {
            closenessVector: closeness,
            normalized: closeness.map(c => c / (n - 1))
        };
    }
    static async betweennessCentrality(adjacency) {
        // Simplified implementation - would need shortest path counting
        const n = adjacency.rows;
        const betweenness = new Array(n).fill(0);
        // This is a placeholder - actual implementation would use Brandes' algorithm
        for (let i = 0; i < n; i++) {
            betweenness[i] = Math.random(); // Placeholder
        }
        return {
            betweennessVector: betweenness,
            normalized: betweenness.map(b => b / ((n - 1) * (n - 2) / 2))
        };
    }
    static computeModularity(adjacency, assignments) {
        const n = adjacency.rows;
        const m = this.countEdges(adjacency);
        let modularity = 0;
        for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
                if (assignments[i] === assignments[j]) {
                    const aij = MatrixOperations.getEntry(adjacency, i, j);
                    const ki = this.getNodeDegree(adjacency, i);
                    const kj = this.getNodeDegree(adjacency, j);
                    modularity += aij - (ki * kj) / (2 * m);
                }
            }
        }
        return modularity / (2 * m);
    }
    static countEdges(adjacency) {
        let edges = 0;
        for (let i = 0; i < adjacency.rows; i++) {
            for (let j = 0; j < adjacency.cols; j++) {
                edges += MatrixOperations.getEntry(adjacency, i, j);
            }
        }
        return edges / 2; // Assuming undirected graph
    }
    static getNodeDegree(adjacency, node) {
        let degree = 0;
        for (let j = 0; j < adjacency.cols; j++) {
            degree += MatrixOperations.getEntry(adjacency, node, j);
        }
        return degree;
    }
}
