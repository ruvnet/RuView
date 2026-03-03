/**
 * Utility functions for sublinear-time solvers
 */
import { SolverError, ErrorCodes } from './types.js';
export class VectorOperations {
    /**
     * Vector addition: result = a + b
     */
    static add(a, b) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.map((val, i) => val + b[i]);
    }
    /**
     * Vector subtraction: result = a - b
     */
    static subtract(a, b) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.map((val, i) => val - b[i]);
    }
    /**
     * Scalar multiplication: result = scalar * vector
     */
    static scale(vector, scalar) {
        return vector.map(val => val * scalar);
    }
    /**
     * Dot product of two vectors
     */
    static dot(a, b) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.reduce((sum, val, i) => sum + val * b[i], 0);
    }
    /**
     * L2 norm of vector
     */
    static norm2(vector) {
        return Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    }
    /**
     * L1 norm of vector
     */
    static norm1(vector) {
        return vector.reduce((sum, val) => sum + Math.abs(val), 0);
    }
    /**
     * L-infinity norm of vector
     */
    static normInf(vector) {
        return Math.max(...vector.map(Math.abs));
    }
    /**
     * Create zero vector of specified length
     */
    static zeros(length) {
        return new Array(length).fill(0);
    }
    /**
     * Create vector filled with ones
     */
    static ones(length) {
        return new Array(length).fill(1);
    }
    /**
     * Create random vector with values in [0, 1)
     */
    static random(length, seed) {
        const rng = seed !== undefined ? createSeededRandom(seed) : Math.random;
        return Array.from({ length }, () => rng());
    }
    /**
     * Normalize vector to unit length
     */
    static normalize(vector) {
        const norm = this.norm2(vector);
        if (norm === 0) {
            throw new SolverError('Cannot normalize zero vector', ErrorCodes.NUMERICAL_INSTABILITY);
        }
        return this.scale(vector, 1 / norm);
    }
    /**
     * Element-wise multiplication
     */
    static elementwiseMultiply(a, b) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.map((val, i) => val * b[i]);
    }
    /**
     * Element-wise division
     */
    static elementwiseDivide(a, b) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.map((val, i) => {
            if (Math.abs(b[i]) < 1e-15) {
                throw new SolverError(`Division by zero at index ${i}`, ErrorCodes.NUMERICAL_INSTABILITY);
            }
            return val / b[i];
        });
    }
    /**
     * Check if vectors are approximately equal
     */
    static isEqual(a, b, tolerance = 1e-10) {
        if (a.length !== b.length) {
            return false;
        }
        for (let i = 0; i < a.length; i++) {
            if (Math.abs(a[i] - b[i]) > tolerance) {
                return false;
            }
        }
        return true;
    }
    /**
     * Linear interpolation between two vectors
     */
    static lerp(a, b, t) {
        if (a.length !== b.length) {
            throw new SolverError(`Vector dimensions don't match: ${a.length} vs ${b.length}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        return a.map((val, i) => val + t * (b[i] - val));
    }
}
/**
 * Create a seeded random number generator
 */
export function createSeededRandom(seed) {
    let state = seed;
    return function () {
        // Simple linear congruential generator
        state = (state * 1664525 + 1013904223) % 0x100000000;
        return state / 0x100000000;
    };
}
/**
 * Performance monitoring utilities
 */
export class PerformanceMonitor {
    startTime;
    memoryStart;
    constructor() {
        this.startTime = Date.now();
        this.memoryStart = this.getMemoryUsage();
    }
    /**
     * Get elapsed time in milliseconds
     */
    getElapsedTime() {
        return Date.now() - this.startTime;
    }
    /**
     * Get memory usage in MB
     */
    getMemoryUsage() {
        if (typeof process !== 'undefined' && process.memoryUsage) {
            const usage = process.memoryUsage();
            return Math.round(usage.heapUsed / 1024 / 1024);
        }
        return 0;
    }
    /**
     * Get memory increase since start
     */
    getMemoryIncrease() {
        return this.getMemoryUsage() - this.memoryStart;
    }
    /**
     * Reset timer and memory baseline
     */
    reset() {
        this.startTime = Date.now();
        this.memoryStart = this.getMemoryUsage();
    }
}
/**
 * Convergence checking utilities
 */
export class ConvergenceChecker {
    history = [];
    maxHistory;
    constructor(maxHistory = 10) {
        this.maxHistory = maxHistory;
    }
    /**
     * Add residual to history and check convergence
     */
    checkConvergence(residual, tolerance) {
        this.history.push(residual);
        if (this.history.length > this.maxHistory) {
            this.history.shift();
        }
        const converged = residual < tolerance;
        let rate = 1.0;
        let trend = 'improving';
        if (this.history.length >= 2) {
            const recent = this.history.slice(-2);
            rate = recent[1] / recent[0];
            if (rate < 0.95) {
                trend = 'improving';
            }
            else if (rate > 1.05) {
                trend = 'diverging';
            }
            else {
                trend = 'stagnant';
            }
        }
        return { converged, rate, trend };
    }
    /**
     * Get average convergence rate over history
     */
    getAverageRate() {
        if (this.history.length < 2) {
            return 1.0;
        }
        let totalRate = 0;
        let count = 0;
        for (let i = 1; i < this.history.length; i++) {
            if (this.history[i - 1] > 0) {
                totalRate += this.history[i] / this.history[i - 1];
                count++;
            }
        }
        return count > 0 ? totalRate / count : 1.0;
    }
    /**
     * Clear convergence history
     */
    reset() {
        this.history = [];
    }
}
/**
 * Timeout utility
 */
export class TimeoutController {
    startTime;
    timeoutMs;
    constructor(timeoutMs) {
        this.startTime = Date.now();
        this.timeoutMs = timeoutMs;
    }
    /**
     * Check if timeout has been exceeded
     */
    isExpired() {
        return Date.now() - this.startTime > this.timeoutMs;
    }
    /**
     * Get remaining time in milliseconds
     */
    remainingTime() {
        return Math.max(0, this.timeoutMs - (Date.now() - this.startTime));
    }
    /**
     * Throw timeout error if expired
     */
    checkTimeout() {
        if (this.isExpired()) {
            throw new SolverError(`Operation timed out after ${this.timeoutMs}ms`, ErrorCodes.TIMEOUT);
        }
    }
}
/**
 * Validation utilities
 */
export class ValidationUtils {
    /**
     * Validate that value is a finite number
     */
    static validateFiniteNumber(value, name) {
        if (!Number.isFinite(value)) {
            throw new SolverError(`${name} must be a finite number, got ${value}`, ErrorCodes.INVALID_PARAMETERS);
        }
    }
    /**
     * Validate that value is a positive number
     */
    static validatePositiveNumber(value, name) {
        this.validateFiniteNumber(value, name);
        if (value <= 0) {
            throw new SolverError(`${name} must be positive, got ${value}`, ErrorCodes.INVALID_PARAMETERS);
        }
    }
    /**
     * Validate that value is a non-negative number
     */
    static validateNonNegativeNumber(value, name) {
        this.validateFiniteNumber(value, name);
        if (value < 0) {
            throw new SolverError(`${name} must be non-negative, got ${value}`, ErrorCodes.INVALID_PARAMETERS);
        }
    }
    /**
     * Validate that value is within range [min, max]
     */
    static validateRange(value, min, max, name) {
        this.validateFiniteNumber(value, name);
        if (value < min || value > max) {
            throw new SolverError(`${name} must be between ${min} and ${max}, got ${value}`, ErrorCodes.INVALID_PARAMETERS);
        }
    }
    /**
     * Validate that integer is within range [min, max]
     */
    static validateIntegerRange(value, min, max, name) {
        if (!Number.isInteger(value)) {
            throw new SolverError(`${name} must be an integer, got ${value}`, ErrorCodes.INVALID_PARAMETERS);
        }
        this.validateRange(value, min, max, name);
    }
}
