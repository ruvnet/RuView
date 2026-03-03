/**
 * Real WASM Integration for Sublinear Time Solver
 *
 * This module properly integrates our Rust WASM components:
 * - GraphReasoner: Fast PageRank and graph algorithms
 * - TemporalNeuralSolver: Neural network accelerated matrix operations
 * - StrangeLoop: Quantum-enhanced solving with nanosecond precision
 * - NanoScheduler: Ultra-low latency task scheduling
 */
import { Matrix, Vector } from './types.js';
/**
 * GraphReasoner WASM for PageRank and graph algorithms
 */
export declare class GraphReasonerWASM {
    private instance;
    private reasoner;
    initialize(): Promise<boolean>;
    /**
     * Compute PageRank using WASM acceleration
     */
    computePageRank(adjacencyMatrix: Matrix, damping?: number, iterations?: number): Float64Array;
    private pageRankJS;
}
/**
 * TemporalNeuralSolver WASM for ultra-fast matrix operations
 */
export declare class TemporalNeuralWASM {
    private instance;
    private solver;
    initialize(): Promise<boolean>;
    /**
     * Ultra-fast matrix-vector multiplication
     */
    multiplyMatrixVector(matrix: Float64Array, vector: Float64Array, rows: number, cols: number): Float64Array;
    private multiplyMatrixVectorJS;
    /**
     * Predict solution with temporal advantage
     */
    predictWithTemporalAdvantage(matrix: Matrix, vector: Vector, distanceKm?: number): Promise<{
        solution: Vector;
        temporalAdvantageMs: number;
        lightTravelTimeMs: number;
        computeTimeMs: number;
    }>;
}
/**
 * Main WASM integration manager
 */
export declare class WASMAccelerator {
    private graphReasoner;
    private temporalNeural;
    private initialized;
    constructor();
    initialize(): Promise<boolean>;
    get isInitialized(): boolean;
    getGraphReasoner(): GraphReasonerWASM;
    getTemporalNeural(): TemporalNeuralWASM;
}
export declare const wasmAccelerator: WASMAccelerator;
