/**
 * Utility functions for sublinear-time solvers
 */
import { Vector } from './types.js';
export declare class VectorOperations {
    /**
     * Vector addition: result = a + b
     */
    static add(a: Vector, b: Vector): Vector;
    /**
     * Vector subtraction: result = a - b
     */
    static subtract(a: Vector, b: Vector): Vector;
    /**
     * Scalar multiplication: result = scalar * vector
     */
    static scale(vector: Vector, scalar: number): Vector;
    /**
     * Dot product of two vectors
     */
    static dot(a: Vector, b: Vector): number;
    /**
     * L2 norm of vector
     */
    static norm2(vector: Vector): number;
    /**
     * L1 norm of vector
     */
    static norm1(vector: Vector): number;
    /**
     * L-infinity norm of vector
     */
    static normInf(vector: Vector): number;
    /**
     * Create zero vector of specified length
     */
    static zeros(length: number): Vector;
    /**
     * Create vector filled with ones
     */
    static ones(length: number): Vector;
    /**
     * Create random vector with values in [0, 1)
     */
    static random(length: number, seed?: number): Vector;
    /**
     * Normalize vector to unit length
     */
    static normalize(vector: Vector): Vector;
    /**
     * Element-wise multiplication
     */
    static elementwiseMultiply(a: Vector, b: Vector): Vector;
    /**
     * Element-wise division
     */
    static elementwiseDivide(a: Vector, b: Vector): Vector;
    /**
     * Check if vectors are approximately equal
     */
    static isEqual(a: Vector, b: Vector, tolerance?: number): boolean;
    /**
     * Linear interpolation between two vectors
     */
    static lerp(a: Vector, b: Vector, t: number): Vector;
}
/**
 * Create a seeded random number generator
 */
export declare function createSeededRandom(seed: number): () => number;
/**
 * Performance monitoring utilities
 */
export declare class PerformanceMonitor {
    private startTime;
    private memoryStart;
    constructor();
    /**
     * Get elapsed time in milliseconds
     */
    getElapsedTime(): number;
    /**
     * Get memory usage in MB
     */
    getMemoryUsage(): number;
    /**
     * Get memory increase since start
     */
    getMemoryIncrease(): number;
    /**
     * Reset timer and memory baseline
     */
    reset(): void;
}
/**
 * Convergence checking utilities
 */
export declare class ConvergenceChecker {
    private history;
    private readonly maxHistory;
    constructor(maxHistory?: number);
    /**
     * Add residual to history and check convergence
     */
    checkConvergence(residual: number, tolerance: number): {
        converged: boolean;
        rate: number;
        trend: 'improving' | 'stagnant' | 'diverging';
    };
    /**
     * Get average convergence rate over history
     */
    getAverageRate(): number;
    /**
     * Clear convergence history
     */
    reset(): void;
}
/**
 * Timeout utility
 */
export declare class TimeoutController {
    private startTime;
    private timeoutMs;
    constructor(timeoutMs: number);
    /**
     * Check if timeout has been exceeded
     */
    isExpired(): boolean;
    /**
     * Get remaining time in milliseconds
     */
    remainingTime(): number;
    /**
     * Throw timeout error if expired
     */
    checkTimeout(): void;
}
/**
 * Validation utilities
 */
export declare class ValidationUtils {
    /**
     * Validate that value is a finite number
     */
    static validateFiniteNumber(value: number, name: string): void;
    /**
     * Validate that value is a positive number
     */
    static validatePositiveNumber(value: number, name: string): void;
    /**
     * Validate that value is a non-negative number
     */
    static validateNonNegativeNumber(value: number, name: string): void;
    /**
     * Validate that value is within range [min, max]
     */
    static validateRange(value: number, min: number, max: number, name: string): void;
    /**
     * Validate that integer is within range [min, max]
     */
    static validateIntegerRange(value: number, min: number, max: number, name: string): void;
}
