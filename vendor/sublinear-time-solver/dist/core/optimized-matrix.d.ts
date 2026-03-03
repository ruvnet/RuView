/**
 * Optimized matrix operations with memory pooling and SIMD-friendly patterns
 * Target: 50% memory reduction and improved cache locality
 */
import { Matrix, Vector, SparseMatrix } from './types.js';
declare class VectorPool {
    private pools;
    private maxPoolSize;
    acquire(size: number): Vector;
    release(vector: Vector): void;
    clear(): void;
    getStats(): {
        poolSizes: Record<number, number>;
        totalVectors: number;
    };
}
export declare class CSRMatrix {
    values: Float64Array;
    colIndices: Uint32Array;
    rowPtr: Uint32Array;
    private rows;
    private cols;
    constructor(rows: number, cols: number, nnz: number);
    static fromCOO(matrix: SparseMatrix): CSRMatrix;
    multiplyVector(x: Vector, result: Vector): void;
    getEntry(row: number, col: number): number;
    rowEntries(row: number): Generator<{
        col: number;
        val: number;
    }>;
    getMemoryUsage(): number;
    getNnz(): number;
    getRows(): number;
    getCols(): number;
}
export declare class CSCMatrix {
    values: Float64Array;
    rowIndices: Uint32Array;
    colPtr: Uint32Array;
    private rows;
    private cols;
    constructor(rows: number, cols: number, nnz: number);
    static fromCSR(csr: CSRMatrix): CSCMatrix;
    multiplyVector(x: Vector, result: Vector): void;
    getMemoryUsage(): number;
    getNnz(): number;
    getRows(): number;
    getCols(): number;
}
export declare class StreamingMatrix {
    private chunks;
    private chunkSize;
    private rows;
    private cols;
    private maxCachedChunks;
    constructor(rows: number, cols: number, chunkSize?: number, maxCachedChunks?: number);
    static fromMatrix(matrix: Matrix, chunkSize?: number): StreamingMatrix;
    getChunk(chunkId: number): CSRMatrix | null;
    multiplyVector(x: Vector, result: Vector): void;
    getMemoryUsage(): number;
}
export declare class OptimizedMatrixOperations {
    private static vectorPool;
    static getVectorPool(): VectorPool;
    static vectorAdd(a: Vector, b: Vector, result?: Vector): Vector;
    static vectorScale(vector: Vector, scalar: number, result?: Vector): Vector;
    static vectorDot(a: Vector, b: Vector): number;
    static vectorNorm2(vector: Vector): number;
    static convertToOptimalFormat(matrix: Matrix): CSRMatrix | CSCMatrix;
    private static denseToSparse;
    static profileMemoryUsage(matrix: CSRMatrix | CSCMatrix | StreamingMatrix): {
        matrixSize: number;
        nnz: number;
        memoryUsed: number;
        compressionRatio: number;
    };
    static cleanup(): void;
}
export {};
