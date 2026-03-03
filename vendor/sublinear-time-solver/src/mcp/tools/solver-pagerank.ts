/**
 * PageRank integration with O(log n) WASM solver
 */

import { WasmSublinearSolverTools } from './wasm-sublinear-solver.js';

export class PageRankTools {
  private static wasmSolver: WasmSublinearSolverTools | null = null;

  /**
   * Get or create WASM solver instance
   */
  private static getWasmSolver(): WasmSublinearSolverTools {
    if (!this.wasmSolver) {
      this.wasmSolver = new WasmSublinearSolverTools();
    }
    return this.wasmSolver;
  }

  /**
   * Compute PageRank with O(log n) complexity using enhanced WASM
   */
  static async pageRank(params: {
    adjacency: any;
    damping?: number;
    personalized?: number[];
  }) {
    // Priority 1: Try O(log n) WASM PageRank
    try {
      const wasmSolver = this.getWasmSolver();
      if (wasmSolver.isEnhancedWasmAvailable()) {
        console.log('🚀 Using O(log n) WASM PageRank with Johnson-Lindenstrauss embedding');

        // Convert adjacency matrix format if needed
        let adjacency: number[][];
        if (params.adjacency.format === 'dense' && Array.isArray(params.adjacency.data)) {
          adjacency = params.adjacency.data as number[][];
        } else if (Array.isArray(params.adjacency) && Array.isArray(params.adjacency[0])) {
          adjacency = params.adjacency as number[][];
        } else {
          throw new Error('Adjacency matrix format not supported for WASM PageRank');
        }

        const result = await wasmSolver.pageRankSublinear(
          adjacency,
          params.damping || 0.85,
          params.personalized
        );

        return {
          pageRankVector: result.pageRankVector,
          topNodes: result.pageRankVector
            .map((score: number, index: number) => ({ node: index, score }))
            .sort((a: any, b: any) => b.score - a.score),
          totalScore: result.pageRankVector.reduce((sum: number, score: number) => sum + score, 0),
          maxScore: Math.max(...result.pageRankVector),
          minScore: Math.min(...result.pageRankVector),
          complexity_bound: result.complexity_bound,
          compression_ratio: result.compression_ratio,
          algorithm: result.algorithm,
          mathematical_guarantee: result.mathematical_guarantee,
          metadata: result.metadata
        };
      }
    } catch (error) {
      console.warn('⚠️  O(log n) WASM PageRank failed, falling back:', error.message);
    }

    // Fallback to traditional PageRank implementation
    throw new Error('Traditional PageRank fallback not implemented - WASM required for O(log n) complexity');
  }

  /**
   * Get PageRank capabilities
   */
  static getCapabilities() {
    const wasmSolver = this.getWasmSolver();
    return wasmSolver.getCapabilities();
  }
}

export default PageRankTools;