"use strict";
/**
 * Training Pipeline for SONA
 *
 * Comprehensive training infrastructure with metrics tracking,
 * learning rate scheduling, and checkpoint management.
 *
 * @example
 * ```typescript
 * import { TrainingPipeline, TrainingConfig } from '@ruvector/ruvllm';
 *
 * const pipeline = new TrainingPipeline({
 *   learningRate: 0.001,
 *   batchSize: 32,
 *   epochs: 10,
 * });
 *
 * // Add training data
 * pipeline.addBatch(inputs, targets, qualities);
 *
 * // Run training
 * const result = pipeline.train();
 * console.log(`Final loss: ${result.finalLoss}`);
 * ```
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.TrainingFactory = exports.TrainingPipeline = exports.MetricsTracker = exports.LRScheduler = void 0;
const lora_1 = require("./lora");
const sona_1 = require("./sona");
/**
 * Default training config
 */
const DEFAULT_TRAINING_CONFIG = {
    learningRate: 0.001,
    batchSize: 32,
    epochs: 10,
    scheduler: 'cosine',
    warmupSteps: 100,
    weightDecay: 0.01,
    gradientClip: 1.0,
    earlyStoppingPatience: 3,
    checkpointInterval: 1,
    ewcLambda: 2000,
    validationSplit: 0.1,
};
/**
 * Learning Rate Scheduler
 */
class LRScheduler {
    constructor(config, totalSteps) {
        this.currentStep = 0;
        this.config = config;
        this.initialLR = config.learningRate;
        this.totalSteps = totalSteps;
    }
    /**
     * Get learning rate for current step
     */
    getLR() {
        switch (this.config.scheduler) {
            case 'constant':
                return this.initialLR;
            case 'linear':
                return this.initialLR * (1 - this.currentStep / this.totalSteps);
            case 'cosine':
                return this.initialLR * 0.5 * (1 + Math.cos(Math.PI * this.currentStep / this.totalSteps));
            case 'warmup':
                if (this.currentStep < this.config.warmupSteps) {
                    return this.initialLR * (this.currentStep / this.config.warmupSteps);
                }
                // Cosine decay after warmup
                const decaySteps = this.totalSteps - this.config.warmupSteps;
                const decayProgress = (this.currentStep - this.config.warmupSteps) / decaySteps;
                return this.initialLR * 0.5 * (1 + Math.cos(Math.PI * decayProgress));
            default:
                return this.initialLR;
        }
    }
    /**
     * Step the scheduler
     */
    step() {
        this.currentStep++;
    }
    /**
     * Reset scheduler
     */
    reset() {
        this.currentStep = 0;
    }
}
exports.LRScheduler = LRScheduler;
/**
 * Training Metrics Tracker
 */
class MetricsTracker {
    constructor() {
        this.lossHistory = [];
        this.valLossHistory = [];
        this.gradNormHistory = [];
        this.startTime = Date.now();
        this.stepTimes = [];
    }
    /**
     * Record training loss
     */
    recordLoss(loss) {
        this.lossHistory.push(loss);
    }
    /**
     * Record validation loss
     */
    recordValLoss(loss) {
        this.valLossHistory.push(loss);
    }
    /**
     * Record gradient norm
     */
    recordGradNorm(norm) {
        this.gradNormHistory.push(norm);
    }
    /**
     * Record step time
     */
    recordStepTime(ms) {
        this.stepTimes.push(ms);
    }
    /**
     * Get average loss over last N steps
     */
    avgLoss(n = 100) {
        const recent = this.lossHistory.slice(-n);
        return recent.length > 0 ? recent.reduce((a, b) => a + b, 0) / recent.length : 0;
    }
    /**
     * Get average validation loss
     */
    avgValLoss(n = 10) {
        const recent = this.valLossHistory.slice(-n);
        return recent.length > 0 ? recent.reduce((a, b) => a + b, 0) / recent.length : 0;
    }
    /**
     * Get steps per second
     */
    stepsPerSecond() {
        if (this.stepTimes.length === 0)
            return 0;
        const avgStepTime = this.stepTimes.slice(-100).reduce((a, b) => a + b, 0) / Math.min(this.stepTimes.length, 100);
        return avgStepTime > 0 ? 1000 / avgStepTime : 0;
    }
    /**
     * Get ETA in seconds
     */
    eta(remainingSteps) {
        const sps = this.stepsPerSecond();
        return sps > 0 ? remainingSteps / sps : 0;
    }
    /**
     * Get best validation loss
     */
    bestValLoss() {
        return this.valLossHistory.length > 0 ? Math.min(...this.valLossHistory) : Infinity;
    }
    /**
     * Get total duration
     */
    duration() {
        return Date.now() - this.startTime;
    }
    /**
     * Get all loss history
     */
    getLossHistory() {
        return [...this.lossHistory];
    }
    /**
     * Get all validation loss history
     */
    getValLossHistory() {
        return [...this.valLossHistory];
    }
    /**
     * Reset tracker
     */
    reset() {
        this.lossHistory = [];
        this.valLossHistory = [];
        this.gradNormHistory = [];
        this.stepTimes = [];
        this.startTime = Date.now();
    }
}
exports.MetricsTracker = MetricsTracker;
/**
 * Training Pipeline
 *
 * Full training infrastructure for SONA models.
 */
class TrainingPipeline {
    constructor(config, adapter) {
        this.scheduler = null;
        this.batches = [];
        this.checkpoints = [];
        this.currentEpoch = 0;
        this.currentStep = 0;
        this.bestValLoss = Infinity;
        this.patienceCounter = 0;
        this.config = { ...DEFAULT_TRAINING_CONFIG, ...config };
        this.adapter = adapter || new lora_1.LoraAdapter({ rank: 8 });
        this.ewcManager = new sona_1.EwcManager(this.config.ewcLambda);
        this.metrics = new MetricsTracker();
    }
    /**
     * Add training batch
     */
    addBatch(inputs, targets, qualities) {
        this.batches.push({ inputs, targets, qualities });
    }
    /**
     * Add training data
     */
    addData(data) {
        // Group into batches
        for (let i = 0; i < data.length; i += this.config.batchSize) {
            const batch = data.slice(i, i + this.config.batchSize);
            this.addBatch(batch.map(d => d.input), batch.map(d => d.target), batch.map(d => d.quality));
        }
    }
    /**
     * Run training
     */
    train() {
        const totalSteps = this.batches.length * this.config.epochs;
        this.scheduler = new LRScheduler(this.config, totalSteps);
        this.metrics.reset();
        this.adapter.startTraining(this.config.learningRate);
        let earlyStopped = false;
        for (let epoch = 0; epoch < this.config.epochs; epoch++) {
            this.currentEpoch = epoch;
            // Shuffle batches
            const shuffledBatches = this.shuffleBatches();
            // Split into train/val
            const valSize = Math.floor(shuffledBatches.length * this.config.validationSplit);
            const trainBatches = shuffledBatches.slice(valSize);
            const valBatches = shuffledBatches.slice(0, valSize);
            // Training epoch
            for (const batch of trainBatches) {
                const stepStart = Date.now();
                const loss = this.trainStep(batch);
                this.metrics.recordLoss(loss);
                this.metrics.recordStepTime(Date.now() - stepStart);
                this.scheduler.step();
                this.currentStep++;
            }
            // Validation
            if (valBatches.length > 0) {
                const valLoss = this.validate(valBatches);
                this.metrics.recordValLoss(valLoss);
                // Early stopping
                if (valLoss < this.bestValLoss) {
                    this.bestValLoss = valLoss;
                    this.patienceCounter = 0;
                }
                else {
                    this.patienceCounter++;
                    if (this.patienceCounter >= this.config.earlyStoppingPatience) {
                        earlyStopped = true;
                        break;
                    }
                }
            }
            // Checkpoint
            if ((epoch + 1) % this.config.checkpointInterval === 0) {
                this.saveCheckpoint();
            }
        }
        this.adapter.endTraining();
        // Register with EWC for continual learning
        const weights = this.adapter.merge().flat();
        this.ewcManager.registerTask(`task-${Date.now()}`, weights);
        return {
            epochs: this.currentEpoch + 1,
            steps: this.currentStep,
            finalLoss: this.metrics.avgLoss(100),
            bestValLoss: this.bestValLoss,
            durationMs: this.metrics.duration(),
            lossHistory: this.metrics.getLossHistory(),
            valLossHistory: this.metrics.getValLossHistory(),
            earlyStopped,
        };
    }
    /**
     * Single training step
     */
    trainStep(batch) {
        let totalLoss = 0;
        const lr = this.scheduler?.getLR() || this.config.learningRate;
        for (let i = 0; i < batch.inputs.length; i++) {
            const input = batch.inputs[i];
            const target = batch.targets[i];
            const quality = batch.qualities[i];
            // Forward pass
            const output = this.adapter.forward(input);
            // Compute loss (MSE weighted by quality)
            const gradOutput = [];
            let loss = 0;
            for (let j = 0; j < output.length; j++) {
                const diff = output[j] - (target[j] || 0);
                loss += diff * diff;
                gradOutput.push(2 * diff * quality); // Quality-weighted gradient
            }
            loss = (loss / output.length) * quality;
            // Add EWC penalty
            const ewcPenalty = this.ewcManager.computePenalty(this.adapter.merge().flat());
            loss += ewcPenalty * 0.001;
            // Backward pass
            this.adapter.backward(input, gradOutput, lr);
            totalLoss += loss;
        }
        return totalLoss / batch.inputs.length;
    }
    /**
     * Validation pass
     */
    validate(batches) {
        let totalLoss = 0;
        let count = 0;
        for (const batch of batches) {
            for (let i = 0; i < batch.inputs.length; i++) {
                const output = this.adapter.forward(batch.inputs[i]);
                const target = batch.targets[i];
                let loss = 0;
                for (let j = 0; j < output.length; j++) {
                    const diff = output[j] - (target[j] || 0);
                    loss += diff * diff;
                }
                totalLoss += loss / output.length;
                count++;
            }
        }
        return count > 0 ? totalLoss / count : 0;
    }
    /**
     * Save checkpoint
     */
    saveCheckpoint() {
        this.checkpoints.push({
            epoch: this.currentEpoch,
            step: this.currentStep,
            loss: this.metrics.avgLoss(100),
            weights: this.adapter.toJSON(),
            timestamp: Date.now(),
        });
    }
    /**
     * Load checkpoint
     */
    loadCheckpoint(index) {
        const checkpoint = this.checkpoints[index];
        if (!checkpoint)
            return false;
        this.adapter = lora_1.LoraAdapter.fromJSON(checkpoint.weights);
        this.currentEpoch = checkpoint.epoch;
        this.currentStep = checkpoint.step;
        return true;
    }
    /**
     * Get current metrics
     */
    getMetrics() {
        return {
            epoch: this.currentEpoch,
            step: this.currentStep,
            trainLoss: this.metrics.avgLoss(100),
            valLoss: this.metrics.avgValLoss(10),
            learningRate: this.scheduler?.getLR() || this.config.learningRate,
            gradNorm: 0,
            stepsPerSecond: this.metrics.stepsPerSecond(),
            etaSeconds: this.metrics.eta((this.config.epochs - this.currentEpoch) * this.batches.length),
        };
    }
    /**
     * Get adapter
     */
    getAdapter() {
        return this.adapter;
    }
    /**
     * Get EWC manager
     */
    getEwcManager() {
        return this.ewcManager;
    }
    /**
     * Get checkpoints
     */
    getCheckpoints() {
        return [...this.checkpoints];
    }
    /**
     * Reset pipeline
     */
    reset() {
        this.batches = [];
        this.checkpoints = [];
        this.currentEpoch = 0;
        this.currentStep = 0;
        this.bestValLoss = Infinity;
        this.patienceCounter = 0;
        this.metrics.reset();
        this.adapter.reset();
    }
    shuffleBatches() {
        const shuffled = [...this.batches];
        for (let i = shuffled.length - 1; i > 0; i--) {
            const j = Math.floor(Math.random() * (i + 1));
            [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
        }
        return shuffled;
    }
}
exports.TrainingPipeline = TrainingPipeline;
/**
 * Training Factory
 *
 * Create pre-configured training pipelines for common scenarios.
 */
class TrainingFactory {
    /**
     * Create pipeline for quick fine-tuning
     */
    static quickFinetune() {
        return new TrainingPipeline({
            learningRate: 0.01,
            epochs: 3,
            batchSize: 16,
            scheduler: 'constant',
        });
    }
    /**
     * Create pipeline for deep training
     */
    static deepTraining() {
        return new TrainingPipeline({
            learningRate: 0.001,
            epochs: 50,
            batchSize: 32,
            scheduler: 'warmup',
            warmupSteps: 500,
            earlyStoppingPatience: 5,
        });
    }
    /**
     * Create pipeline for continual learning
     */
    static continualLearning(ewcLambda = 5000) {
        return new TrainingPipeline({
            learningRate: 0.0005,
            epochs: 10,
            batchSize: 16,
            scheduler: 'cosine',
            ewcLambda,
            earlyStoppingPatience: 10,
        });
    }
    /**
     * Create pipeline for federated aggregation
     */
    static federatedAggregation() {
        return new TrainingPipeline({
            learningRate: 0.0001,
            epochs: 5,
            batchSize: 64,
            scheduler: 'linear',
            ewcLambda: 2000,
        });
    }
}
exports.TrainingFactory = TrainingFactory;
//# sourceMappingURL=training.js.map