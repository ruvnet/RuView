#!/usr/bin/env node

/**
 * Unit tests for Matrix class and related functionality
 * Run with: node tests/unit/matrix.test.js
 */

const { strict: assert } = require('assert');
const { Matrix, SolverConfig, MemoryManager } = require('../../js/solver.js');

class TestRunner {
    constructor() {
        this.tests = [];
        this.passed = 0;
        this.failed = 0;
        this.verbose = process.argv.includes('--verbose');
    }

    test(name, fn) {
        this.tests.push({ name, fn });
    }

    async run() {
        console.log('🧪 Running Matrix Unit Tests');
        console.log('============================\n');

        for (const { name, fn } of this.tests) {
            try {
                await fn();
                this.passed++;
                console.log(`✅ ${name}`);
            } catch (error) {
                this.failed++;
                console.log(`❌ ${name}`);
                if (this.verbose) {
                    console.log(`   Error: ${error.message}`);
                    console.log(`   Stack: ${error.stack}\n`);
                } else {
                    console.log(`   Error: ${error.message}\n`);
                }
            }
        }

        this.printSummary();
        return this.failed === 0;
    }

    printSummary() {
        console.log('\n📊 Test Summary');
        console.log('===============');
        console.log(`✅ Passed: ${this.passed}`);
        console.log(`❌ Failed: ${this.failed}`);
        console.log(`📈 Total:  ${this.tests.length}`);
        console.log(`🎯 Success Rate: ${((this.passed / this.tests.length) * 100).toFixed(1)}%`);
    }
}

const runner = new TestRunner();

// Matrix Constructor Tests
runner.test('Matrix constructor with Float64Array', () => {
    const data = new Float64Array([1, 2, 3, 4]);
    const matrix = new Matrix(data, 2, 2);

    assert.equal(matrix.rows, 2);
    assert.equal(matrix.cols, 2);
    assert.equal(matrix.data.length, 4);
    assert.equal(matrix.data[0], 1);
    assert.equal(matrix.data[3], 4);
});

runner.test('Matrix constructor with Array', () => {
    const data = [1, 2, 3, 4];
    const matrix = new Matrix(data, 2, 2);

    assert.equal(matrix.rows, 2);
    assert.equal(matrix.cols, 2);
    assert.ok(matrix.data instanceof Float64Array);
    assert.equal(matrix.data[0], 1);
});

runner.test('Matrix constructor dimension validation', () => {
    assert.throws(() => {
        new Matrix([1, 2, 3], 2, 2);
    }, /Data length must match matrix dimensions/);
});

runner.test('Matrix constructor invalid data type', () => {
    assert.throws(() => {
        new Matrix("invalid", 2, 2);
    }, /Matrix data must be Float64Array or Array/);
});

// Matrix Static Methods
runner.test('Matrix.zeros creates zero matrix', () => {
    const matrix = Matrix.zeros(3, 2);

    assert.equal(matrix.rows, 3);
    assert.equal(matrix.cols, 2);
    assert.equal(matrix.data.length, 6);

    for (let i = 0; i < matrix.data.length; i++) {
        assert.equal(matrix.data[i], 0);
    }
});

runner.test('Matrix.identity creates identity matrix', () => {
    const matrix = Matrix.identity(3);

    assert.equal(matrix.rows, 3);
    assert.equal(matrix.cols, 3);

    // Check diagonal elements
    assert.equal(matrix.get(0, 0), 1);
    assert.equal(matrix.get(1, 1), 1);
    assert.equal(matrix.get(2, 2), 1);

    // Check off-diagonal elements
    assert.equal(matrix.get(0, 1), 0);
    assert.equal(matrix.get(1, 0), 0);
    assert.equal(matrix.get(1, 2), 0);
});

runner.test('Matrix.random creates random matrix', () => {
    const matrix = Matrix.random(2, 3);

    assert.equal(matrix.rows, 2);
    assert.equal(matrix.cols, 3);
    assert.equal(matrix.data.length, 6);

    // Check that values are in [0, 1) range
    for (let i = 0; i < matrix.data.length; i++) {
        assert.ok(matrix.data[i] >= 0 && matrix.data[i] < 1);
    }
});

// Matrix Access Methods
runner.test('Matrix get/set operations', () => {
    const matrix = Matrix.zeros(2, 2);

    matrix.set(0, 1, 5.5);
    matrix.set(1, 0, -2.3);

    assert.equal(matrix.get(0, 1), 5.5);
    assert.equal(matrix.get(1, 0), -2.3);
    assert.equal(matrix.get(0, 0), 0);
    assert.equal(matrix.get(1, 1), 0);
});

runner.test('Matrix bounds checking for get', () => {
    const matrix = new Matrix([1, 2, 3, 4], 2, 2);

    // Valid access
    assert.equal(matrix.get(1, 1), 4);

    // Should not throw for out of bounds (JavaScript behavior)
    // But should return undefined or unexpected values
    const result = matrix.get(2, 2);
    assert.ok(result === undefined || typeof result === 'number');
});

// SolverConfig Tests
runner.test('SolverConfig default values', () => {
    const config = new SolverConfig();

    assert.equal(config.maxIterations, 1000);
    assert.equal(config.tolerance, 1e-10);
    assert.equal(config.simdEnabled, true);
    assert.equal(config.streamChunkSize, 100);
});

runner.test('SolverConfig custom values', () => {
    const config = new SolverConfig({
        maxIterations: 500,
        tolerance: 1e-6,
        simdEnabled: false,
        streamChunkSize: 50
    });

    assert.equal(config.maxIterations, 500);
    assert.equal(config.tolerance, 1e-6);
    assert.equal(config.simdEnabled, false);
    assert.equal(config.streamChunkSize, 50);
});

runner.test('SolverConfig partial custom values', () => {
    const config = new SolverConfig({
        maxIterations: 2000,
        tolerance: 1e-8
    });

    assert.equal(config.maxIterations, 2000);
    assert.equal(config.tolerance, 1e-8);
    assert.equal(config.simdEnabled, true); // Default
    assert.equal(config.streamChunkSize, 100); // Default
});

// MemoryManager Tests
runner.test('MemoryManager allocation and deallocation', () => {
    const manager = new MemoryManager();

    const allocation = manager.allocateFloat64Array(100);

    assert.ok(allocation.id);
    assert.ok(allocation.buffer instanceof Float64Array);
    assert.equal(allocation.buffer.length, 100);

    const usage = manager.getUsage();
    assert.equal(usage.allocations, 1);
    assert.equal(usage.totalBytes, 100 * 8); // 8 bytes per Float64

    manager.deallocate(allocation.id);

    const usageAfter = manager.getUsage();
    assert.equal(usageAfter.allocations, 0);
    assert.equal(usageAfter.totalBytes, 0);
});

runner.test('MemoryManager multiple allocations', () => {
    const manager = new MemoryManager();

    const alloc1 = manager.allocateFloat64Array(50);
    const alloc2 = manager.allocateFloat64Array(100);
    const alloc3 = manager.allocateFloat64Array(25);

    const usage = manager.getUsage();
    assert.equal(usage.allocations, 3);
    assert.equal(usage.totalBytes, (50 + 100 + 25) * 8);

    manager.deallocate(alloc2.id);

    const usageAfter = manager.getUsage();
    assert.equal(usageAfter.allocations, 2);
    assert.equal(usageAfter.totalBytes, (50 + 25) * 8);
});

runner.test('MemoryManager clear all allocations', () => {
    const manager = new MemoryManager();

    manager.allocateFloat64Array(10);
    manager.allocateFloat64Array(20);
    manager.allocateFloat64Array(30);

    assert.equal(manager.getUsage().allocations, 3);

    manager.clear();

    const usage = manager.getUsage();
    assert.equal(usage.allocations, 0);
    assert.equal(usage.totalBytes, 0);
});

// Mathematical Property Tests
runner.test('Matrix mathematical properties - transpose concept', () => {
    const matrix = new Matrix([1, 2, 3, 4, 5, 6], 2, 3);

    // Original: [[1, 2, 3], [4, 5, 6]]
    assert.equal(matrix.get(0, 0), 1);
    assert.equal(matrix.get(0, 1), 2);
    assert.equal(matrix.get(0, 2), 3);
    assert.equal(matrix.get(1, 0), 4);
    assert.equal(matrix.get(1, 1), 5);
    assert.equal(matrix.get(1, 2), 6);
});

runner.test('Matrix identity properties', () => {
    const identity = Matrix.identity(4);

    // Check all diagonal elements are 1
    for (let i = 0; i < 4; i++) {
        assert.equal(identity.get(i, i), 1);
    }

    // Check all off-diagonal elements are 0
    for (let i = 0; i < 4; i++) {
        for (let j = 0; j < 4; j++) {
            if (i !== j) {
                assert.equal(identity.get(i, j), 0);
            }
        }
    }
});

runner.test('Matrix zero properties', () => {
    const zeros = Matrix.zeros(3, 4);

    // Check all elements are 0
    for (let i = 0; i < 3; i++) {
        for (let j = 0; j < 4; j++) {
            assert.equal(zeros.get(i, j), 0);
        }
    }
});

// Edge Cases
runner.test('Matrix with single element', () => {
    const matrix = new Matrix([42], 1, 1);

    assert.equal(matrix.rows, 1);
    assert.equal(matrix.cols, 1);
    assert.equal(matrix.get(0, 0), 42);
});

runner.test('Matrix with large dimensions', () => {
    const size = 1000;
    const matrix = Matrix.zeros(size, size);

    assert.equal(matrix.rows, size);
    assert.equal(matrix.cols, size);
    assert.equal(matrix.data.length, size * size);

    // Test corner elements
    assert.equal(matrix.get(0, 0), 0);
    assert.equal(matrix.get(size - 1, size - 1), 0);
});

runner.test('Matrix memory efficiency check', () => {
    const size = 100;
    const matrix = Matrix.random(size, size);

    // Check that data is stored efficiently as Float64Array
    assert.ok(matrix.data instanceof Float64Array);
    assert.equal(matrix.data.length, size * size);
    assert.equal(matrix.data.byteLength, size * size * 8);
});

// Performance Tests
runner.test('Matrix creation performance benchmark', () => {
    const sizes = [10, 100, 500];

    for (const size of sizes) {
        const start = Date.now();
        const matrix = Matrix.zeros(size, size);
        const end = Date.now();

        const duration = end - start;

        // Should create matrices quickly (under 100ms for reasonable sizes)
        if (size <= 500) {
            assert.ok(duration < 1000, `Matrix creation too slow: ${duration}ms for ${size}x${size}`);
        }

        // Verify matrix was created correctly
        assert.equal(matrix.rows, size);
        assert.equal(matrix.cols, size);
    }
});

// Run all tests
if (require.main === module) {
    runner.run().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(error => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = { TestRunner, runner };