/**
 * Training Utilities for MicroLoRA
 *
 * Provides comprehensive training infrastructure including data preprocessing,
 * batch generation, loss computation, EWC for continual learning, and
 * gradient checkpointing for memory efficiency.
 *
 * @module @ruvector/edge-net/models/training-utils
 *
 * @example
 * ```javascript
 * import {
 *   DataPreprocessor,
 *   BatchGenerator,
 *   LossComputer,
 *   EWCManager,
 *   GradientCheckpointer
 * } from '@ruvector/edge-net/models/training-utils';
 *
 * // Preprocess training data
 * const preprocessor = new DataPreprocessor();
 * const processed = await preprocessor.process(examples);
 *
 * // Generate batches
 * const batcher = new BatchGenerator(processed, { batchSize: 8 });
 * for (const batch of batcher) {
 *   const loss = LossComputer.contrastive(batch.anchors, batch.positives, batch.negatives);
 * }
 *
 * // Enable EWC for continual learning
 * const ewc = new EWCManager({ lambda: 2000 });
 * ewc.computeFisher(model, dataloader);
 * ```
 */

import { EventEmitter } from 'events';
import { createHash } from 'crypto';

// ============================================
// TYPE DEFINITIONS (JSDoc)
// ============================================

/**
 * @typedef {Object} TrainingExample
 * @property {string} input - Input text
 * @property {string} output - Expected output
 * @property {number} [quality=1.0] - Example quality weight
 * @property {Object} [metadata] - Optional metadata
 */

/**
 * @typedef {Object} ProcessedExample
 * @property {string} input - Original input text
 * @property {string} output - Original output text
 * @property {Float32Array} inputEmb - Input embedding
 * @property {Float32Array} outputEmb - Output embedding
 * @property {Float32Array} [negativeEmb] - Optional negative embedding
 * @property {number} quality - Example quality weight
 * @property {number[]} inputTokens - Tokenized input
 * @property {number[]} outputTokens - Tokenized output
 */

/**
 * @typedef {Object} Batch
 * @property {ProcessedExample[]} examples - Batch examples
 * @property {Float32Array[]} anchors - Anchor embeddings
 * @property {Float32Array[]} positives - Positive embeddings
 * @property {Float32Array[]} [negatives] - Negative embeddings
 * @property {number} size - Batch size
 */

/**
 * @typedef {Object} LossResult
 * @property {number} loss - Computed loss value
 * @property {Float32Array[]} [gradients] - Optional gradients
 * @property {Object} [components] - Loss components breakdown
 */

// ============================================
// DATA PREPROCESSOR
// ============================================

/**
 * DataPreprocessor - Prepares training data for MicroLoRA
 *
 * Handles tokenization, embedding generation, data augmentation,
 * and quality filtering of training examples.
 *
 * @example
 * ```javascript
 * const preprocessor = new DataPreprocessor({
 *   maxLength: 512,
 *   augmentation: true,
 *   qualityThreshold: 0.5
 * });
 *
 * const processed = await preprocessor.process([
 *   { input: 'Hello', output: 'World' }
 * ]);
 * ```
 */
export class DataPreprocessor extends EventEmitter {
    /**
     * Create a DataPreprocessor
     *
     * @param {Object} [config={}] - Preprocessor configuration
     */
    constructor(config = {}) {
        super();

        this.config = {
            maxLength: 512,
            embeddingDim: 384,
            augmentation: false,
            augmentationFactor: 2,
            qualityThreshold: 0.0,
            normalizeEmbeddings: true,
            ...config,
        };

        // Simple vocabulary for tokenization
        this.vocab = new Map();
        this.vocabSize = 0;

        // Cache for embeddings
        this.embeddingCache = new Map();

        this.stats = {
            processed: 0,
            filtered: 0,
            augmented: 0,
            cacheHits: 0,
        };
    }

    /**
     * Process training examples
     *
     * @param {TrainingExample[]} examples - Raw training examples
     * @returns {Promise<ProcessedExample[]>} Processed examples
     */
    async process(examples) {
        const processed = [];

        for (const example of examples) {
            // Quality filter
            if ((example.quality || 1.0) < this.config.qualityThreshold) {
                this.stats.filtered++;
                continue;
            }

            // Process single example
            const result = await this._processExample(example);
            if (result) {
                processed.push(result);
                this.stats.processed++;
            }

            // Augmentation
            if (this.config.augmentation) {
                const augmented = await this._augmentExample(example);
                for (const aug of augmented) {
                    const augResult = await this._processExample(aug);
                    if (augResult) {
                        processed.push(augResult);
                        this.stats.augmented++;
                    }
                }
            }
        }

        this.emit('process:complete', {
            total: processed.length,
            stats: this.stats,
        });

        return processed;
    }

    /**
     * Process a single example
     * @private
     */
    async _processExample(example) {
        const inputTokens = this._tokenize(example.input);
        const outputTokens = this._tokenize(example.output);

        // Truncate if needed
        const truncatedInput = inputTokens.slice(0, this.config.maxLength);
        const truncatedOutput = outputTokens.slice(0, this.config.maxLength);

        // Generate embeddings
        const inputEmb = this._embed(example.input);
        const outputEmb = this._embed(example.output);

        return {
            input: example.input,
            output: example.output,
            inputEmb,
            outputEmb,
            quality: example.quality || 1.0,
            inputTokens: truncatedInput,
            outputTokens: truncatedOutput,
            metadata: example.metadata,
        };
    }

    /**
     * Simple tokenization (character-level with common subwords)
     * @private
     */
    _tokenize(text) {
        const tokens = [];

        // Split into words and characters
        const words = text.split(/\s+/);
        for (const word of words) {
            // Check vocabulary
            if (this.vocab.has(word)) {
                tokens.push(this.vocab.get(word));
            } else {
                // Add to vocab or use character tokens
                if (this.vocabSize < 50000) {
                    this.vocab.set(word, this.vocabSize);
                    tokens.push(this.vocabSize);
                    this.vocabSize++;
                } else {
                    // Fall back to character-level
                    for (const char of word) {
                        const charToken = char.charCodeAt(0) % 256;
                        tokens.push(charToken);
                    }
                }
            }
            // Add space token
            tokens.push(32);
        }

        return tokens;
    }

    /**
     * Generate embedding for text
     * @private
     */
    _embed(text) {
        // Check cache
        const cacheKey = createHash('md5').update(text).digest('hex');
        if (this.embeddingCache.has(cacheKey)) {
            this.stats.cacheHits++;
            return this.embeddingCache.get(cacheKey);
        }

        const dim = this.config.embeddingDim;
        const embedding = new Float32Array(dim);

        // Hash-based embedding
        const hash = createHash('sha256').update(text).digest();
        for (let i = 0; i < dim; i++) {
            embedding[i] = (hash[i % 32] - 128) / 128;
        }

        // Add positional character features
        for (let i = 0; i < text.length && i < dim; i++) {
            embedding[i] += (text.charCodeAt(i) - 64) / 256;
        }

        // Normalize
        if (this.config.normalizeEmbeddings) {
            let norm = 0;
            for (let i = 0; i < dim; i++) {
                norm += embedding[i] * embedding[i];
            }
            norm = Math.sqrt(norm) || 1;
            for (let i = 0; i < dim; i++) {
                embedding[i] /= norm;
            }
        }

        // Cache
        this.embeddingCache.set(cacheKey, embedding);

        return embedding;
    }

    /**
     * Augment a training example
     * @private
     */
    async _augmentExample(example) {
        const augmented = [];

        // Synonym replacement (simplified)
        if (Math.random() < 0.5) {
            augmented.push({
                input: this._synonymReplace(example.input),
                output: example.output,
                quality: (example.quality || 1.0) * 0.9,
            });
        }

        // Random insertion
        if (Math.random() < 0.3) {
            augmented.push({
                input: this._randomInsert(example.input),
                output: example.output,
                quality: (example.quality || 1.0) * 0.85,
            });
        }

        // Case variation
        if (Math.random() < 0.3) {
            augmented.push({
                input: this._caseVariation(example.input),
                output: example.output,
                quality: (example.quality || 1.0) * 0.95,
            });
        }

        return augmented.slice(0, this.config.augmentationFactor - 1);
    }

    /**
     * Simple synonym replacement
     * @private
     */
    _synonymReplace(text) {
        const synonyms = {
            'write': ['create', 'make', 'generate'],
            'function': ['method', 'procedure', 'routine'],
            'code': ['program', 'script', 'implementation'],
            'help': ['assist', 'aid', 'support'],
            'explain': ['describe', 'clarify', 'elaborate'],
        };

        let result = text;
        for (const [word, syns] of Object.entries(synonyms)) {
            if (result.toLowerCase().includes(word)) {
                const syn = syns[Math.floor(Math.random() * syns.length)];
                result = result.replace(new RegExp(word, 'gi'), syn);
                break;
            }
        }
        return result;
    }

    /**
     * Random word insertion
     * @private
     */
    _randomInsert(text) {
        const words = text.split(' ');
        const insertWords = ['please', 'now', 'just', 'simply'];
        const insertWord = insertWords[Math.floor(Math.random() * insertWords.length)];
        const position = Math.floor(Math.random() * words.length);
        words.splice(position, 0, insertWord);
        return words.join(' ');
    }

    /**
     * Case variation
     * @private
     */
    _caseVariation(text) {
        const variations = [
            text.toLowerCase(),
            text.charAt(0).toUpperCase() + text.slice(1).toLowerCase(),
            text.toUpperCase(),
        ];
        return variations[Math.floor(Math.random() * variations.length)];
    }

    /**
     * Clear embedding cache
     */
    clearCache() {
        this.embeddingCache.clear();
        this.stats.cacheHits = 0;
    }

    /**
     * Get preprocessor statistics
     */
    getStats() {
        return {
            ...this.stats,
            vocabSize: this.vocabSize,
            cacheSize: this.embeddingCache.size,
        };
    }
}

// ============================================
// BATCH GENERATOR
// ============================================

/**
 * BatchGenerator - Generates training batches with shuffling and sampling
 *
 * Supports various batching strategies including random sampling,
 * hard negative mining, and curriculum learning.
 *
 * @example
 * ```javascript
 * const batcher = new BatchGenerator(processedData, {
 *   batchSize: 8,
 *   shuffle: true,
 *   dropLast: false
 * });
 *
 * for (const batch of batcher) {
 *   console.log(`Batch size: ${batch.size}`);
 * }
 * ```
 */
export class BatchGenerator {
    /**
     * Create a BatchGenerator
     *
     * @param {ProcessedExample[]} data - Processed training data
     * @param {Object} [config={}] - Generator configuration
     */
    constructor(data, config = {}) {
        this.data = [...data];
        this.config = {
            batchSize: 8,
            shuffle: true,
            dropLast: false,
            hardNegatives: false,
            curriculum: false,
            curriculumEpochs: 5,
            ...config,
        };

        this.currentIndex = 0;
        this.epoch = 0;
        this.indices = this._createIndices();
    }

    /**
     * Create index array (with shuffling if enabled)
     * @private
     */
    _createIndices() {
        const indices = Array.from({ length: this.data.length }, (_, i) => i);

        if (this.config.shuffle) {
            // Fisher-Yates shuffle
            for (let i = indices.length - 1; i > 0; i--) {
                const j = Math.floor(Math.random() * (i + 1));
                [indices[i], indices[j]] = [indices[j], indices[i]];
            }
        }

        // Curriculum learning: sort by quality/difficulty for early epochs
        if (this.config.curriculum && this.epoch < this.config.curriculumEpochs) {
            indices.sort((a, b) => {
                const qualityA = this.data[a].quality || 1;
                const qualityB = this.data[b].quality || 1;
                // Higher quality first in early epochs
                return qualityB - qualityA;
            });
        }

        return indices;
    }

    /**
     * Iterator implementation
     */
    [Symbol.iterator]() {
        return this;
    }

    /**
     * Get next batch
     * @returns {{value: Batch, done: boolean}}
     */
    next() {
        if (this.currentIndex >= this.indices.length) {
            // Check if we should drop last incomplete batch
            if (this.config.dropLast && this.currentIndex > 0) {
                return { done: true };
            }
            return { done: true };
        }

        const endIndex = Math.min(
            this.currentIndex + this.config.batchSize,
            this.indices.length
        );

        // Skip if this would be an incomplete batch and dropLast is true
        if (this.config.dropLast &&
            endIndex - this.currentIndex < this.config.batchSize) {
            return { done: true };
        }

        const batchIndices = this.indices.slice(this.currentIndex, endIndex);
        const examples = batchIndices.map(i => this.data[i]);

        // Extract embeddings
        const anchors = examples.map(e => e.inputEmb);
        const positives = examples.map(e => e.outputEmb);

        // Generate negatives if needed
        let negatives = null;
        if (this.config.hardNegatives) {
            negatives = this._mineHardNegatives(anchors, positives);
        }

        this.currentIndex = endIndex;

        return {
            value: {
                examples,
                anchors,
                positives,
                negatives,
                size: examples.length,
            },
            done: false,
        };
    }

    /**
     * Mine hard negatives for contrastive learning
     * @private
     */
    _mineHardNegatives(anchors, positives) {
        const negatives = [];

        for (let i = 0; i < anchors.length; i++) {
            // Find hardest negative (most similar non-positive)
            let hardestIdx = -1;
            let hardestSim = -Infinity;

            for (let j = 0; j < positives.length; j++) {
                if (i === j) continue;

                const sim = this._cosineSimilarity(anchors[i], positives[j]);
                if (sim > hardestSim) {
                    hardestSim = sim;
                    hardestIdx = j;
                }
            }

            negatives.push(hardestIdx >= 0 ? positives[hardestIdx] : positives[(i + 1) % positives.length]);
        }

        return negatives;
    }

    /**
     * Cosine similarity between two embeddings
     * @private
     */
    _cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dot / (Math.sqrt(normA) * Math.sqrt(normB) + 1e-8);
    }

    /**
     * Reset generator for new epoch
     */
    reset() {
        this.currentIndex = 0;
        this.epoch++;
        this.indices = this._createIndices();
    }

    /**
     * Get total number of batches
     */
    get length() {
        if (this.config.dropLast) {
            return Math.floor(this.data.length / this.config.batchSize);
        }
        return Math.ceil(this.data.length / this.config.batchSize);
    }

    /**
     * Get number of remaining batches
     */
    get remaining() {
        const remaining = this.indices.length - this.currentIndex;
        if (this.config.dropLast) {
            return Math.floor(remaining / this.config.batchSize);
        }
        return Math.ceil(remaining / this.config.batchSize);
    }
}

// ============================================
// LOSS COMPUTER
// ============================================

/**
 * LossComputer - Various loss functions for training
 *
 * Implements multiple loss functions optimized for different training
 * scenarios: contrastive learning, cross-entropy, triplet loss, etc.
 *
 * @example
 * ```javascript
 * // Contrastive loss
 * const loss = LossComputer.contrastive(anchors, positives, negatives, {
 *   temperature: 0.07,
 *   margin: 0.5
 * });
 *
 * // Cross-entropy loss
 * const ceLoss = LossComputer.crossEntropy(predictions, targets);
 *
 * // Combined loss
 * const combined = LossComputer.combine([
 *   { loss: contrastiveLoss, weight: 0.7 },
 *   { loss: ceLoss, weight: 0.3 }
 * ]);
 * ```
 */
export class LossComputer {
    /**
     * Contrastive loss (InfoNCE)
     *
     * @param {Float32Array[]} anchors - Anchor embeddings
     * @param {Float32Array[]} positives - Positive embeddings
     * @param {Float32Array[]} [negatives] - Negative embeddings
     * @param {Object} [options={}] - Loss options
     * @returns {LossResult}
     */
    static contrastive(anchors, positives, negatives = null, options = {}) {
        const { temperature = 0.07, margin = 0.0 } = options;

        let totalLoss = 0;
        const n = anchors.length;

        for (let i = 0; i < n; i++) {
            const anchor = anchors[i];
            const positive = positives[i];

            // Positive similarity
            const posSim = LossComputer._cosineSimilarity(anchor, positive);

            // Negative similarities (in-batch or explicit)
            let negSum = 0;
            const negCount = negatives ? negatives.length : n - 1;

            if (negatives) {
                for (let j = 0; j < negatives.length; j++) {
                    const negSim = LossComputer._cosineSimilarity(anchor, negatives[j]);
                    negSum += Math.exp((negSim - margin) / temperature);
                }
            } else {
                // In-batch negatives
                for (let j = 0; j < n; j++) {
                    if (i === j) continue;
                    const negSim = LossComputer._cosineSimilarity(anchor, positives[j]);
                    negSum += Math.exp((negSim - margin) / temperature);
                }
            }

            // InfoNCE loss: -log(exp(pos/t) / (exp(pos/t) + sum(exp(neg/t))))
            const posExp = Math.exp((posSim - margin) / temperature);
            const loss = -Math.log(posExp / (posExp + negSum + 1e-8));
            totalLoss += loss;
        }

        return {
            loss: totalLoss / n,
            components: { contrastive: totalLoss / n },
        };
    }

    /**
     * Triplet loss with margin
     *
     * @param {Float32Array[]} anchors - Anchor embeddings
     * @param {Float32Array[]} positives - Positive embeddings
     * @param {Float32Array[]} negatives - Negative embeddings
     * @param {Object} [options={}] - Loss options
     * @returns {LossResult}
     */
    static triplet(anchors, positives, negatives, options = {}) {
        const { margin = 0.5 } = options;

        let totalLoss = 0;
        const n = anchors.length;

        for (let i = 0; i < n; i++) {
            const posDistance = LossComputer._euclideanDistance(anchors[i], positives[i]);
            const negDistance = LossComputer._euclideanDistance(anchors[i], negatives[i]);

            // Triplet loss: max(0, pos_dist - neg_dist + margin)
            const loss = Math.max(0, posDistance - negDistance + margin);
            totalLoss += loss;
        }

        return {
            loss: totalLoss / n,
            components: { triplet: totalLoss / n },
        };
    }

    /**
     * Cross-entropy loss
     *
     * @param {number[][]} predictions - Predicted logits/probabilities
     * @param {number[]} targets - Target class indices
     * @param {Object} [options={}] - Loss options
     * @returns {LossResult}
     */
    static crossEntropy(predictions, targets, options = {}) {
        const { labelSmoothing = 0.0 } = options;

        let totalLoss = 0;
        const n = predictions.length;

        for (let i = 0; i < n; i++) {
            const pred = predictions[i];
            const target = targets[i];

            // Softmax
            const maxLogit = Math.max(...pred);
            const expSum = pred.reduce((sum, p) => sum + Math.exp(p - maxLogit), 0);
            const logProbs = pred.map(p => p - maxLogit - Math.log(expSum));

            // Cross-entropy with optional label smoothing
            if (labelSmoothing > 0) {
                const numClasses = pred.length;
                const smoothTarget = new Array(numClasses).fill(labelSmoothing / numClasses);
                smoothTarget[target] = 1 - labelSmoothing + labelSmoothing / numClasses;

                let loss = 0;
                for (let c = 0; c < numClasses; c++) {
                    loss -= smoothTarget[c] * logProbs[c];
                }
                totalLoss += loss;
            } else {
                totalLoss -= logProbs[target];
            }
        }

        return {
            loss: totalLoss / n,
            components: { crossEntropy: totalLoss / n },
        };
    }

    /**
     * Mean Squared Error loss
     *
     * @param {Float32Array[]} predictions - Predicted embeddings
     * @param {Float32Array[]} targets - Target embeddings
     * @returns {LossResult}
     */
    static mse(predictions, targets) {
        let totalLoss = 0;
        const n = predictions.length;

        for (let i = 0; i < n; i++) {
            const pred = predictions[i];
            const target = targets[i];
            const dim = Math.min(pred.length, target.length);

            let loss = 0;
            for (let d = 0; d < dim; d++) {
                const diff = pred[d] - target[d];
                loss += diff * diff;
            }
            totalLoss += loss / dim;
        }

        return {
            loss: totalLoss / n,
            components: { mse: totalLoss / n },
        };
    }

    /**
     * Cosine embedding loss
     *
     * @param {Float32Array[]} predictions - Predicted embeddings
     * @param {Float32Array[]} targets - Target embeddings
     * @param {number[]} [labels] - Labels: 1 for similar, -1 for dissimilar
     * @param {Object} [options={}] - Loss options
     * @returns {LossResult}
     */
    static cosineEmbedding(predictions, targets, labels = null, options = {}) {
        const { margin = 0.0 } = options;

        let totalLoss = 0;
        const n = predictions.length;

        for (let i = 0; i < n; i++) {
            const sim = LossComputer._cosineSimilarity(predictions[i], targets[i]);
            const label = labels ? labels[i] : 1;

            if (label === 1) {
                totalLoss += 1 - sim;
            } else {
                totalLoss += Math.max(0, sim - margin);
            }
        }

        return {
            loss: totalLoss / n,
            components: { cosineEmbedding: totalLoss / n },
        };
    }

    /**
     * Combine multiple losses with weights
     *
     * @param {Array<{loss: LossResult, weight: number}>} losses - Weighted losses
     * @returns {LossResult}
     */
    static combine(losses) {
        let totalLoss = 0;
        const components = {};

        for (const { loss, weight } of losses) {
            totalLoss += loss.loss * weight;
            for (const [name, value] of Object.entries(loss.components || {})) {
                components[name] = (components[name] || 0) + value * weight;
            }
        }

        return {
            loss: totalLoss,
            components,
        };
    }

    /**
     * Cosine similarity helper
     * @private
     */
    static _cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dot / (Math.sqrt(normA) * Math.sqrt(normB) + 1e-8);
    }

    /**
     * Euclidean distance helper
     * @private
     */
    static _euclideanDistance(a, b) {
        let sum = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            const diff = a[i] - b[i];
            sum += diff * diff;
        }
        return Math.sqrt(sum);
    }
}

// ============================================
// EWC MANAGER
// ============================================

/**
 * EWCManager - Elastic Weight Consolidation for Continual Learning
 *
 * Prevents catastrophic forgetting when training on new tasks by
 * regularizing important weights based on Fisher information.
 *
 * @example
 * ```javascript
 * const ewc = new EWCManager({ lambda: 2000 });
 *
 * // After training on task 1
 * ewc.computeFisher(adapters, task1Data);
 *
 * // When training on task 2, add EWC penalty
 * const ewcLoss = ewc.computePenalty(currentAdapters);
 * totalLoss = taskLoss + ewcLoss;
 * ```
 */
export class EWCManager extends EventEmitter {
    /**
     * Create an EWCManager
     *
     * @param {Object} [config={}] - EWC configuration
     */
    constructor(config = {}) {
        super();

        this.config = {
            lambda: 2000,      // Regularization strength
            sampleSize: 200,   // Samples for Fisher estimation
            normalize: true,   // Normalize Fisher values
            online: false,     // Online EWC (cumulative Fisher)
            ...config,
        };

        // Stored Fisher information and optimal parameters
        this.fisherInfo = new Map();
        this.optimalParams = new Map();

        this.stats = {
            tasksLearned: 0,
            totalPenalties: 0,
        };
    }

    /**
     * Compute Fisher information matrix diagonal for adapters
     *
     * @param {Map<string, Object>} adapters - Adapter weights
     * @param {ProcessedExample[]} data - Training data for estimation
     */
    computeFisher(adapters, data) {
        this.emit('fisher:start', { samples: data.length });

        const sampleData = data.length > this.config.sampleSize
            ? this._sampleData(data, this.config.sampleSize)
            : data;

        for (const [name, adapter] of adapters) {
            // Store optimal parameters
            this.optimalParams.set(name, {
                loraA: adapter.loraA.map(row => [...row]),
                loraB: adapter.loraB.map(row => [...row]),
            });

            // Compute Fisher diagonal (squared gradients)
            const fisherA = this._zeros(adapter.loraA.length, adapter.loraA[0].length);
            const fisherB = this._zeros(adapter.loraB.length, adapter.loraB[0].length);

            for (const example of sampleData) {
                const grads = this._computeGradients(example, adapter);

                // Accumulate squared gradients
                for (let i = 0; i < fisherA.length; i++) {
                    for (let j = 0; j < fisherA[0].length; j++) {
                        fisherA[i][j] += grads.gradA[i][j] * grads.gradA[i][j];
                    }
                }
                for (let i = 0; i < fisherB.length; i++) {
                    for (let j = 0; j < fisherB[0].length; j++) {
                        fisherB[i][j] += grads.gradB[i][j] * grads.gradB[i][j];
                    }
                }
            }

            // Normalize
            const n = sampleData.length;
            for (let i = 0; i < fisherA.length; i++) {
                for (let j = 0; j < fisherA[0].length; j++) {
                    fisherA[i][j] /= n;
                }
            }
            for (let i = 0; i < fisherB.length; i++) {
                for (let j = 0; j < fisherB[0].length; j++) {
                    fisherB[i][j] /= n;
                }
            }

            // Online EWC: accumulate with previous Fisher
            if (this.config.online && this.fisherInfo.has(name)) {
                const prevFisher = this.fisherInfo.get(name);
                for (let i = 0; i < fisherA.length; i++) {
                    for (let j = 0; j < fisherA[0].length; j++) {
                        fisherA[i][j] = 0.5 * (fisherA[i][j] + prevFisher.fisherA[i][j]);
                    }
                }
                for (let i = 0; i < fisherB.length; i++) {
                    for (let j = 0; j < fisherB[0].length; j++) {
                        fisherB[i][j] = 0.5 * (fisherB[i][j] + prevFisher.fisherB[i][j]);
                    }
                }
            }

            this.fisherInfo.set(name, { fisherA, fisherB });
        }

        this.stats.tasksLearned++;
        this.emit('fisher:complete', { adapters: adapters.size });
    }

    /**
     * Compute EWC penalty for current adapter values
     *
     * @param {Map<string, Object>} adapters - Current adapter weights
     * @returns {number} EWC penalty value
     */
    computePenalty(adapters) {
        if (this.fisherInfo.size === 0) {
            return 0;
        }

        let penalty = 0;

        for (const [name, adapter] of adapters) {
            const fisher = this.fisherInfo.get(name);
            const optimal = this.optimalParams.get(name);

            if (!fisher || !optimal) continue;

            // Sum of F_i * (theta_i - theta*_i)^2
            for (let i = 0; i < adapter.loraA.length; i++) {
                for (let j = 0; j < adapter.loraA[0].length; j++) {
                    const diff = adapter.loraA[i][j] - optimal.loraA[i][j];
                    penalty += fisher.fisherA[i][j] * diff * diff;
                }
            }
            for (let i = 0; i < adapter.loraB.length; i++) {
                for (let j = 0; j < adapter.loraB[0].length; j++) {
                    const diff = adapter.loraB[i][j] - optimal.loraB[i][j];
                    penalty += fisher.fisherB[i][j] * diff * diff;
                }
            }
        }

        this.stats.totalPenalties++;
        return this.config.lambda * penalty * 0.5;
    }

    /**
     * Apply EWC gradient to adapters
     *
     * @param {Map<string, Object>} adapters - Adapter weights to update
     * @param {number} learningRate - Learning rate
     */
    applyGradient(adapters, learningRate) {
        for (const [name, adapter] of adapters) {
            const fisher = this.fisherInfo.get(name);
            const optimal = this.optimalParams.get(name);

            if (!fisher || !optimal) continue;

            for (let i = 0; i < adapter.loraA.length; i++) {
                for (let j = 0; j < adapter.loraA[0].length; j++) {
                    const diff = adapter.loraA[i][j] - optimal.loraA[i][j];
                    adapter.loraA[i][j] -= learningRate * this.config.lambda * fisher.fisherA[i][j] * diff;
                }
            }
            for (let i = 0; i < adapter.loraB.length; i++) {
                for (let j = 0; j < adapter.loraB[0].length; j++) {
                    const diff = adapter.loraB[i][j] - optimal.loraB[i][j];
                    adapter.loraB[i][j] -= learningRate * this.config.lambda * fisher.fisherB[i][j] * diff;
                }
            }
        }
    }

    /**
     * Compute gradients for a single example
     * @private
     */
    _computeGradients(example, adapter) {
        const input = example.inputEmb;
        const target = example.outputEmb;
        const rank = adapter.loraA[0].length;
        const dim = Math.min(input.length, adapter.loraA.length);

        // Forward
        const hidden = new Float64Array(rank);
        for (let r = 0; r < rank; r++) {
            for (let d = 0; d < dim; d++) {
                hidden[r] += input[d] * adapter.loraA[d][r];
            }
        }

        const adapted = [...input];
        for (let d = 0; d < dim; d++) {
            for (let r = 0; r < rank; r++) {
                adapted[d] += adapter.scaling * hidden[r] * adapter.loraB[r][d];
            }
        }

        // Output gradient
        const gradOutput = adapted.map((val, i) =>
            2 * (val - (target[i] || 0)) / dim
        );

        // Gradient for B
        const gradB = this._zeros(rank, dim);
        for (let r = 0; r < rank; r++) {
            for (let d = 0; d < dim; d++) {
                gradB[r][d] = hidden[r] * gradOutput[d] * adapter.scaling;
            }
        }

        // Gradient for hidden
        const gradHidden = new Float64Array(rank);
        for (let r = 0; r < rank; r++) {
            for (let d = 0; d < dim; d++) {
                gradHidden[r] += gradOutput[d] * adapter.loraB[r][d] * adapter.scaling;
            }
        }

        // Gradient for A
        const gradA = this._zeros(dim, rank);
        for (let d = 0; d < dim; d++) {
            for (let r = 0; r < rank; r++) {
                gradA[d][r] = input[d] * gradHidden[r];
            }
        }

        return { gradA, gradB };
    }

    /**
     * Random sample from data
     * @private
     */
    _sampleData(data, size) {
        const indices = [];
        for (let i = 0; i < size; i++) {
            indices.push(Math.floor(Math.random() * data.length));
        }
        return indices.map(i => data[i]);
    }

    /**
     * Zero matrix helper
     * @private
     */
    _zeros(rows, cols) {
        return Array(rows).fill(null).map(() => Array(cols).fill(0));
    }

    /**
     * Clear stored Fisher information
     */
    reset() {
        this.fisherInfo.clear();
        this.optimalParams.clear();
        this.stats.tasksLearned = 0;
    }

    /**
     * Export EWC state for persistence
     */
    export() {
        return {
            fisherInfo: Object.fromEntries(this.fisherInfo),
            optimalParams: Object.fromEntries(this.optimalParams),
            config: this.config,
            stats: this.stats,
        };
    }

    /**
     * Import EWC state
     */
    import(data) {
        this.fisherInfo = new Map(Object.entries(data.fisherInfo || {}));
        this.optimalParams = new Map(Object.entries(data.optimalParams || {}));
        if (data.config) this.config = { ...this.config, ...data.config };
        if (data.stats) this.stats = data.stats;
    }
}

// ============================================
// GRADIENT CHECKPOINTER
// ============================================

/**
 * GradientCheckpointer - Memory-efficient training with gradient checkpointing
 *
 * Reduces memory usage during training by recomputing intermediate
 * activations during backward pass instead of storing them.
 *
 * @example
 * ```javascript
 * const checkpointer = new GradientCheckpointer({
 *   checkpointSegments: 4,
 *   enabled: true
 * });
 *
 * // Wrap forward pass
 * const output = checkpointer.checkpoint(forwardFn, inputs);
 *
 * // Backward pass with recomputation
 * const gradients = checkpointer.backward(output, gradOutput);
 * ```
 */
export class GradientCheckpointer extends EventEmitter {
    /**
     * Create a GradientCheckpointer
     *
     * @param {Object} [config={}] - Checkpointer configuration
     */
    constructor(config = {}) {
        super();

        this.config = {
            enabled: true,
            checkpointSegments: 4,    // Number of segments to divide computation
            maxStoredActivations: 10, // Maximum activations to store
            ...config,
        };

        // Stored checkpoints
        this.checkpoints = [];

        // Stored forward functions for recomputation
        this.forwardFns = [];

        this.stats = {
            checkpoints: 0,
            recomputations: 0,
            memoryReduction: 0,
        };
    }

    /**
     * Run forward pass with checkpointing
     *
     * @param {Function} forwardFn - Forward function to checkpoint
     * @param {any} input - Input to forward function
     * @returns {any} Forward output
     */
    checkpoint(forwardFn, input) {
        if (!this.config.enabled) {
            return forwardFn(input);
        }

        // Store function for potential recomputation
        this.forwardFns.push({ fn: forwardFn, input: this._shallowCopy(input) });
        this.stats.checkpoints++;

        // Run forward
        const output = forwardFn(input);

        // Store checkpoint if within limit
        if (this.checkpoints.length < this.config.maxStoredActivations) {
            this.checkpoints.push({
                input: this._shallowCopy(input),
                output: this._shallowCopy(output),
            });
        }

        return output;
    }

    /**
     * Recompute activations for backward pass
     *
     * @param {number} segmentIdx - Segment index to recompute
     * @returns {Object} Recomputed activations
     */
    recompute(segmentIdx) {
        if (segmentIdx < this.checkpoints.length) {
            return this.checkpoints[segmentIdx];
        }

        // Recompute from stored forward functions
        if (segmentIdx < this.forwardFns.length) {
            const { fn, input } = this.forwardFns[segmentIdx];
            const output = fn(input);
            this.stats.recomputations++;

            return { input, output };
        }

        throw new Error(`Cannot recompute segment ${segmentIdx}`);
    }

    /**
     * Clear stored checkpoints
     */
    clear() {
        this.checkpoints = [];
        this.forwardFns = [];
    }

    /**
     * Shallow copy helper
     * @private
     */
    _shallowCopy(obj) {
        if (Array.isArray(obj)) {
            return [...obj];
        }
        if (obj instanceof Float32Array || obj instanceof Float64Array) {
            return new obj.constructor(obj);
        }
        if (typeof obj === 'object' && obj !== null) {
            return { ...obj };
        }
        return obj;
    }

    /**
     * Estimate memory savings
     */
    estimateMemorySavings(totalLayers, activationSize) {
        const withoutCheckpointing = totalLayers * activationSize;
        const withCheckpointing = this.config.checkpointSegments * activationSize +
            (totalLayers / this.config.checkpointSegments) * activationSize;

        const savings = 1 - (withCheckpointing / withoutCheckpointing);
        this.stats.memoryReduction = savings;

        return {
            without: withoutCheckpointing,
            with: withCheckpointing,
            savings: savings * 100,
            savingsPercentage: `${(savings * 100).toFixed(1)}%`,
        };
    }

    /**
     * Get checkpointer statistics
     */
    getStats() {
        return {
            ...this.stats,
            storedCheckpoints: this.checkpoints.length,
            storedFunctions: this.forwardFns.length,
        };
    }
}

// ============================================
// LEARNING RATE SCHEDULERS
// ============================================

/**
 * Learning rate scheduler implementations
 */
export const LRSchedulers = {
    /**
     * Constant learning rate
     */
    constant: (baseLR, step, totalSteps) => baseLR,

    /**
     * Linear decay
     */
    linear: (baseLR, step, totalSteps) =>
        baseLR * (1 - step / totalSteps),

    /**
     * Cosine annealing
     */
    cosine: (baseLR, step, totalSteps) =>
        baseLR * 0.5 * (1 + Math.cos(Math.PI * step / totalSteps)),

    /**
     * Cosine with warm restarts
     */
    cosineWarmRestarts: (baseLR, step, totalSteps, opts = {}) => {
        const { restartPeriod = 100, restartMultiplier = 2 } = opts;
        const cycleStep = step % restartPeriod;
        return baseLR * 0.5 * (1 + Math.cos(Math.PI * cycleStep / restartPeriod));
    },

    /**
     * Exponential decay
     */
    exponential: (baseLR, step, totalSteps, opts = {}) => {
        const { gamma = 0.95, stepSize = 100 } = opts;
        return baseLR * Math.pow(gamma, Math.floor(step / stepSize));
    },

    /**
     * Warmup + cosine decay
     */
    warmupCosine: (baseLR, step, totalSteps, opts = {}) => {
        const { warmupSteps = 100 } = opts;
        if (step < warmupSteps) {
            return baseLR * (step / warmupSteps);
        }
        const decayStep = step - warmupSteps;
        const decayTotal = totalSteps - warmupSteps;
        return baseLR * 0.5 * (1 + Math.cos(Math.PI * decayStep / decayTotal));
    },

    /**
     * One-cycle policy
     */
    oneCycle: (baseLR, step, totalSteps, opts = {}) => {
        const { maxLR = baseLR * 10, divFactor = 25, finalDiv = 10000 } = opts;
        const midPoint = totalSteps / 2;

        if (step < midPoint) {
            // Warmup phase
            const progress = step / midPoint;
            return baseLR + progress * (maxLR - baseLR);
        } else {
            // Annealing phase
            const progress = (step - midPoint) / midPoint;
            const finalLR = baseLR / finalDiv;
            return maxLR - progress * (maxLR - finalLR);
        }
    },
};

// ============================================
// EXPORTS
// ============================================

export default {
    DataPreprocessor,
    BatchGenerator,
    LossComputer,
    EWCManager,
    GradientCheckpointer,
    LRSchedulers,
};
