/**
 * @ruvector/edge-net Benchmark Utilities
 *
 * Comprehensive benchmarking for model optimization
 *
 * @module @ruvector/edge-net/models/benchmark
 */

import { EventEmitter } from 'events';
import { ModelOptimizer, TARGET_MODELS, QUANTIZATION_CONFIGS } from './model-optimizer.js';

// ============================================
// BENCHMARK CONFIGURATION
// ============================================

/**
 * Benchmark profiles for different scenarios
 */
export const BENCHMARK_PROFILES = {
    'quick': {
        iterations: 50,
        warmupIterations: 5,
        inputSizes: [[1, 128]],
        quantMethods: ['int8'],
    },
    'standard': {
        iterations: 100,
        warmupIterations: 10,
        inputSizes: [[1, 128], [1, 512], [4, 256]],
        quantMethods: ['int8', 'int4', 'fp16'],
    },
    'comprehensive': {
        iterations: 500,
        warmupIterations: 50,
        inputSizes: [[1, 64], [1, 128], [1, 256], [1, 512], [1, 1024], [4, 256], [8, 128]],
        quantMethods: ['int8', 'int4', 'fp16', 'int8-fp16-mixed'],
    },
    'edge-device': {
        iterations: 100,
        warmupIterations: 10,
        inputSizes: [[1, 128], [1, 256]],
        quantMethods: ['int4'],
        memoryLimit: 512, // MB
    },
    'accuracy-focus': {
        iterations: 200,
        warmupIterations: 20,
        inputSizes: [[1, 512]],
        quantMethods: ['fp16', 'int8'],
        measureAccuracy: true,
    },
};

// ============================================
// ACCURACY MEASUREMENT
// ============================================

/**
 * Accuracy metrics for quantized models
 */
export class AccuracyMeter {
    constructor() {
        this.predictions = [];
        this.groundTruth = [];
        this.originalOutputs = [];
        this.quantizedOutputs = [];
    }

    /**
     * Add prediction pair for accuracy measurement
     */
    addPrediction(original, quantized, groundTruth = null) {
        this.originalOutputs.push(original);
        this.quantizedOutputs.push(quantized);
        if (groundTruth !== null) {
            this.groundTruth.push(groundTruth);
        }
    }

    /**
     * Compute Mean Squared Error
     */
    computeMSE() {
        if (this.originalOutputs.length === 0) return 0;

        let totalMSE = 0;
        let count = 0;

        for (let i = 0; i < this.originalOutputs.length; i++) {
            const orig = this.originalOutputs[i];
            const quant = this.quantizedOutputs[i];

            let mse = 0;
            const len = Math.min(orig.length, quant.length);
            for (let j = 0; j < len; j++) {
                const diff = orig[j] - quant[j];
                mse += diff * diff;
            }
            totalMSE += mse / len;
            count++;
        }

        return totalMSE / count;
    }

    /**
     * Compute cosine similarity between original and quantized
     */
    computeCosineSimilarity() {
        if (this.originalOutputs.length === 0) return 1.0;

        let totalSim = 0;

        for (let i = 0; i < this.originalOutputs.length; i++) {
            const orig = this.originalOutputs[i];
            const quant = this.quantizedOutputs[i];

            let dot = 0, normA = 0, normB = 0;
            const len = Math.min(orig.length, quant.length);

            for (let j = 0; j < len; j++) {
                dot += orig[j] * quant[j];
                normA += orig[j] * orig[j];
                normB += quant[j] * quant[j];
            }

            totalSim += dot / (Math.sqrt(normA) * Math.sqrt(normB) + 1e-8);
        }

        return totalSim / this.originalOutputs.length;
    }

    /**
     * Compute max absolute error
     */
    computeMaxError() {
        let maxError = 0;

        for (let i = 0; i < this.originalOutputs.length; i++) {
            const orig = this.originalOutputs[i];
            const quant = this.quantizedOutputs[i];
            const len = Math.min(orig.length, quant.length);

            for (let j = 0; j < len; j++) {
                maxError = Math.max(maxError, Math.abs(orig[j] - quant[j]));
            }
        }

        return maxError;
    }

    /**
     * Get comprehensive accuracy metrics
     */
    getMetrics() {
        const mse = this.computeMSE();

        return {
            mse,
            rmse: Math.sqrt(mse),
            cosineSimilarity: this.computeCosineSimilarity(),
            maxError: this.computeMaxError(),
            samples: this.originalOutputs.length,
            accuracyRetained: this.computeCosineSimilarity() * 100,
        };
    }

    /**
     * Reset meter
     */
    reset() {
        this.predictions = [];
        this.groundTruth = [];
        this.originalOutputs = [];
        this.quantizedOutputs = [];
    }
}

// ============================================
// LATENCY PROFILER
// ============================================

/**
 * Detailed latency profiling
 */
export class LatencyProfiler {
    constructor() {
        this.measurements = new Map();
    }

    /**
     * Start timing a section
     */
    start(label) {
        if (!this.measurements.has(label)) {
            this.measurements.set(label, {
                samples: [],
                running: null,
            });
        }
        this.measurements.get(label).running = performance.now();
    }

    /**
     * End timing a section
     */
    end(label) {
        const entry = this.measurements.get(label);
        if (entry && entry.running !== null) {
            const duration = performance.now() - entry.running;
            entry.samples.push(duration);
            entry.running = null;
            return duration;
        }
        return 0;
    }

    /**
     * Get statistics for a label
     */
    getStats(label) {
        const entry = this.measurements.get(label);
        if (!entry || entry.samples.length === 0) {
            return null;
        }

        const samples = [...entry.samples].sort((a, b) => a - b);
        const sum = samples.reduce((a, b) => a + b, 0);

        return {
            label,
            count: samples.length,
            mean: sum / samples.length,
            median: samples[Math.floor(samples.length / 2)],
            min: samples[0],
            max: samples[samples.length - 1],
            p95: samples[Math.floor(samples.length * 0.95)],
            p99: samples[Math.floor(samples.length * 0.99)],
            std: Math.sqrt(samples.reduce((acc, v) => acc + Math.pow(v - sum / samples.length, 2), 0) / samples.length),
        };
    }

    /**
     * Get all statistics
     */
    getAllStats() {
        const stats = {};
        for (const label of this.measurements.keys()) {
            stats[label] = this.getStats(label);
        }
        return stats;
    }

    /**
     * Reset profiler
     */
    reset() {
        this.measurements.clear();
    }
}

// ============================================
// MEMORY PROFILER
// ============================================

/**
 * Memory usage profiler
 */
export class MemoryProfiler {
    constructor() {
        this.snapshots = [];
        this.peakMemory = 0;
    }

    /**
     * Take memory snapshot
     */
    snapshot(label = 'snapshot') {
        const memUsage = this.getMemoryUsage();
        const snapshot = {
            label,
            timestamp: Date.now(),
            ...memUsage,
        };

        this.snapshots.push(snapshot);
        this.peakMemory = Math.max(this.peakMemory, memUsage.heapUsed);

        return snapshot;
    }

    /**
     * Get current memory usage
     */
    getMemoryUsage() {
        if (typeof process !== 'undefined' && process.memoryUsage) {
            const usage = process.memoryUsage();
            return {
                heapUsed: usage.heapUsed / (1024 * 1024),
                heapTotal: usage.heapTotal / (1024 * 1024),
                external: usage.external / (1024 * 1024),
                rss: usage.rss / (1024 * 1024),
            };
        }

        // Browser fallback
        if (typeof performance !== 'undefined' && performance.memory) {
            return {
                heapUsed: performance.memory.usedJSHeapSize / (1024 * 1024),
                heapTotal: performance.memory.totalJSHeapSize / (1024 * 1024),
                external: 0,
                rss: 0,
            };
        }

        return { heapUsed: 0, heapTotal: 0, external: 0, rss: 0 };
    }

    /**
     * Get memory delta between two snapshots
     */
    getDelta(startLabel, endLabel) {
        const start = this.snapshots.find(s => s.label === startLabel);
        const end = this.snapshots.find(s => s.label === endLabel);

        if (!start || !end) return null;

        return {
            heapDelta: end.heapUsed - start.heapUsed,
            timeDelta: end.timestamp - start.timestamp,
        };
    }

    /**
     * Get profiler summary
     */
    getSummary() {
        return {
            snapshots: this.snapshots.length,
            peakMemoryMB: this.peakMemory,
            currentMemoryMB: this.getMemoryUsage().heapUsed,
            history: this.snapshots,
        };
    }

    /**
     * Reset profiler
     */
    reset() {
        this.snapshots = [];
        this.peakMemory = 0;
    }
}

// ============================================
// COMPREHENSIVE BENCHMARK RUNNER
// ============================================

/**
 * ComprehensiveBenchmark - Full benchmark suite for model optimization
 */
export class ComprehensiveBenchmark extends EventEmitter {
    constructor(options = {}) {
        super();
        this.optimizer = options.optimizer || new ModelOptimizer();
        this.latencyProfiler = new LatencyProfiler();
        this.memoryProfiler = new MemoryProfiler();
        this.accuracyMeter = new AccuracyMeter();
        this.results = [];
    }

    /**
     * Run benchmark suite on a model
     */
    async runSuite(model, profile = 'standard') {
        const profileConfig = BENCHMARK_PROFILES[profile] || BENCHMARK_PROFILES.standard;
        const modelConfig = TARGET_MODELS[model];

        if (!modelConfig) {
            throw new Error(`Unknown model: ${model}`);
        }

        this.emit('suite:start', { model, profile });

        const suiteResults = {
            model,
            profile,
            modelConfig,
            timestamp: new Date().toISOString(),
            benchmarks: [],
        };

        // Memory baseline
        this.memoryProfiler.snapshot('baseline');

        // Benchmark each quantization method
        for (const method of profileConfig.quantMethods) {
            const methodResult = await this.benchmarkQuantization(
                model,
                method,
                profileConfig
            );
            suiteResults.benchmarks.push(methodResult);
        }

        // Memory after benchmarks
        this.memoryProfiler.snapshot('after-benchmarks');

        // Add memory profile
        suiteResults.memory = this.memoryProfiler.getSummary();

        // Add summary
        suiteResults.summary = this.generateSummary(suiteResults);

        this.results.push(suiteResults);
        this.emit('suite:complete', suiteResults);

        return suiteResults;
    }

    /**
     * Benchmark a specific quantization method
     */
    async benchmarkQuantization(model, method, config) {
        this.emit('benchmark:start', { model, method });

        const quantConfig = QUANTIZATION_CONFIGS[method];
        const modelConfig = TARGET_MODELS[model];

        // Quantize model
        this.latencyProfiler.start('quantization');
        const quantResult = await this.optimizer.quantize(model, method);
        this.latencyProfiler.end('quantization');

        // Simulate inference benchmarks for each input size
        const inferenceBenchmarks = [];

        for (const inputSize of config.inputSizes) {
            const batchSize = inputSize[0];
            const seqLen = inputSize[1];

            this.latencyProfiler.start(`inference-${batchSize}x${seqLen}`);

            // Warmup
            for (let i = 0; i < config.warmupIterations; i++) {
                await this.simulateInference(modelConfig, batchSize, seqLen, method);
            }

            // Measure
            const times = [];
            for (let i = 0; i < config.iterations; i++) {
                const start = performance.now();
                await this.simulateInference(modelConfig, batchSize, seqLen, method);
                times.push(performance.now() - start);
            }

            this.latencyProfiler.end(`inference-${batchSize}x${seqLen}`);

            times.sort((a, b) => a - b);

            inferenceBenchmarks.push({
                inputSize: `${batchSize}x${seqLen}`,
                iterations: config.iterations,
                meanMs: times.reduce((a, b) => a + b) / times.length,
                medianMs: times[Math.floor(times.length / 2)],
                p95Ms: times[Math.floor(times.length * 0.95)],
                minMs: times[0],
                maxMs: times[times.length - 1],
                tokensPerSecond: (seqLen * batchSize * 1000) / (times.reduce((a, b) => a + b) / times.length),
            });
        }

        // Measure accuracy if requested
        let accuracyMetrics = null;
        if (config.measureAccuracy) {
            // Generate test outputs
            for (let i = 0; i < 100; i++) {
                const original = new Float32Array(modelConfig.hiddenSize).map(() => Math.random());
                const quantized = this.simulateQuantizedOutput(original, method);
                this.accuracyMeter.addPrediction(Array.from(original), Array.from(quantized));
            }
            accuracyMetrics = this.accuracyMeter.getMetrics();
            this.accuracyMeter.reset();
        }

        const result = {
            method,
            quantization: quantResult,
            inference: inferenceBenchmarks,
            accuracy: accuracyMetrics,
            latencyProfile: this.latencyProfiler.getAllStats(),
            compression: {
                original: modelConfig.originalSize,
                quantized: modelConfig.originalSize / quantConfig.compression,
                ratio: quantConfig.compression,
            },
            recommendation: this.getRecommendation(model, method, inferenceBenchmarks),
        };

        this.emit('benchmark:complete', result);

        return result;
    }

    /**
     * Simulate model inference
     */
    async simulateInference(config, batchSize, seqLen, method) {
        // Base latency depends on model size and batch
        const quantConfig = QUANTIZATION_CONFIGS[method];
        const baseLatency = (config.originalSize / 100) * (batchSize * seqLen / 512);
        const speedup = quantConfig?.speedup || 1;

        const latency = baseLatency / speedup;
        await new Promise(resolve => setTimeout(resolve, latency));

        return new Float32Array(config.hiddenSize).map(() => Math.random());
    }

    /**
     * Simulate quantized output with added noise
     */
    simulateQuantizedOutput(original, method) {
        const quantConfig = QUANTIZATION_CONFIGS[method];
        const noise = quantConfig?.accuracyLoss || 0.01;

        return new Float32Array(original.length).map((_, i) => {
            return original[i] + (Math.random() - 0.5) * 2 * noise;
        });
    }

    /**
     * Generate recommendation based on benchmark results
     */
    getRecommendation(model, method, inferenceBenchmarks) {
        const modelConfig = TARGET_MODELS[model];
        const quantConfig = QUANTIZATION_CONFIGS[method];

        const avgLatency = inferenceBenchmarks.reduce((a, b) => a + b.meanMs, 0) / inferenceBenchmarks.length;
        const targetMet = (modelConfig.originalSize / quantConfig.compression) <= modelConfig.targetSize;

        let score = 0;
        let reasons = [];

        // Size target met
        if (targetMet) {
            score += 30;
            reasons.push('Meets size target');
        }

        // Good latency
        if (avgLatency < 10) {
            score += 30;
            reasons.push('Excellent latency (<10ms)');
        } else if (avgLatency < 50) {
            score += 20;
            reasons.push('Good latency (<50ms)');
        }

        // Low accuracy loss
        if (quantConfig.accuracyLoss < 0.02) {
            score += 25;
            reasons.push('Minimal accuracy loss (<2%)');
        } else if (quantConfig.accuracyLoss < 0.05) {
            score += 15;
            reasons.push('Acceptable accuracy loss (<5%)');
        }

        // Compression ratio
        if (quantConfig.compression >= 4) {
            score += 15;
            reasons.push('High compression (4x+)');
        }

        return {
            score,
            rating: score >= 80 ? 'Excellent' : score >= 60 ? 'Good' : score >= 40 ? 'Acceptable' : 'Poor',
            reasons,
            recommended: score >= 60,
        };
    }

    /**
     * Generate suite summary
     */
    generateSummary(suiteResults) {
        const benchmarks = suiteResults.benchmarks;

        // Find best method
        let bestMethod = null;
        let bestScore = 0;

        for (const b of benchmarks) {
            if (b.recommendation.score > bestScore) {
                bestScore = b.recommendation.score;
                bestMethod = b.method;
            }
        }

        // Calculate averages
        const avgLatency = benchmarks.reduce((sum, b) => {
            return sum + b.inference.reduce((s, i) => s + i.meanMs, 0) / b.inference.length;
        }, 0) / benchmarks.length;

        return {
            modelKey: suiteResults.model,
            modelType: suiteResults.modelConfig.type,
            originalSizeMB: suiteResults.modelConfig.originalSize,
            targetSizeMB: suiteResults.modelConfig.targetSize,
            bestMethod,
            bestScore,
            avgLatencyMs: avgLatency,
            methodsEvaluated: benchmarks.length,
            recommendation: bestMethod ? `Use ${bestMethod} quantization for optimal edge deployment` : 'No suitable method found',
        };
    }

    /**
     * Run benchmarks on all target models
     */
    async runAllModels(profile = 'standard') {
        const allResults = [];

        for (const modelKey of Object.keys(TARGET_MODELS)) {
            try {
                const result = await this.runSuite(modelKey, profile);
                allResults.push(result);
            } catch (error) {
                allResults.push({
                    model: modelKey,
                    error: error.message,
                });
            }
        }

        return {
            timestamp: new Date().toISOString(),
            profile,
            results: allResults,
            summary: this.generateOverallSummary(allResults),
        };
    }

    /**
     * Generate overall summary for all models
     */
    generateOverallSummary(allResults) {
        const successful = allResults.filter(r => !r.error);

        return {
            totalModels: allResults.length,
            successfulBenchmarks: successful.length,
            failedBenchmarks: allResults.length - successful.length,
            recommendations: successful.map(r => ({
                model: r.model,
                bestMethod: r.summary?.bestMethod,
                score: r.summary?.bestScore,
            })),
        };
    }

    /**
     * Export results to JSON
     */
    exportResults() {
        return {
            exported: new Date().toISOString(),
            results: this.results,
        };
    }

    /**
     * Reset benchmark state
     */
    reset() {
        this.latencyProfiler.reset();
        this.memoryProfiler.reset();
        this.accuracyMeter.reset();
        this.results = [];
    }
}

// ============================================
// EXPORTS
// ============================================

// BENCHMARK_PROFILES already exported at declaration (line 19)
export default ComprehensiveBenchmark;
