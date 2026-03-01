"use strict";
/**
 * Self-Learning Generator
 * Adaptive system that improves output quality through feedback loops
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SelfLearningGenerator = void 0;
const events_1 = require("events");
class SelfLearningGenerator extends events_1.EventEmitter {
    constructor(config) {
        super();
        this.history = [];
        this.config = config;
        this.currentQuality = 0.5; // Start at baseline
    }
    /**
     * Generate with self-learning and improvement
     */
    async generate(options) {
        const startQuality = options.initialQuality || this.currentQuality;
        let bestOutput = null;
        let bestQuality = 0;
        this.emit('start', { task: this.config.task, iterations: this.config.iterations });
        for (let i = 1; i <= this.config.iterations; i++) {
            const iterationStart = Date.now();
            // Generate output
            const output = await this.generateOutput(options.prompt, i);
            // Evaluate quality
            const quality = await this.evaluate(output, options.tests);
            // Apply learning
            const improvement = quality - this.currentQuality;
            this.currentQuality = Math.min(1.0, this.currentQuality + improvement * this.config.learningRate);
            // Track metrics
            const metrics = {
                iteration: i,
                quality,
                testsPassingRate: options.tests ? this.calculateTestPassRate(output, options.tests) : undefined,
                improvement: improvement * 100,
                feedback: this.generateFeedback(quality, improvement)
            };
            this.history.push(metrics);
            this.emit('improvement', metrics);
            // Update best result
            if (quality > bestQuality) {
                bestQuality = quality;
                bestOutput = output;
            }
            // Check if quality threshold reached
            if (this.config.qualityThreshold && quality >= this.config.qualityThreshold) {
                this.emit('threshold-reached', { iteration: i, quality });
                break;
            }
        }
        const finalImprovement = ((bestQuality - startQuality) / startQuality) * 100;
        this.emit('complete', {
            finalQuality: bestQuality,
            improvement: finalImprovement,
            iterations: this.history.length
        });
        return {
            output: bestOutput,
            finalQuality: bestQuality,
            improvement: finalImprovement,
            iterations: this.history.length,
            metrics: this.history
        };
    }
    /**
     * Generate output for current iteration
     */
    async generateOutput(prompt, iteration) {
        // Simulate generation with progressive improvement
        const baseQuality = 0.5 + (iteration / this.config.iterations) * 0.3;
        const learningBonus = this.currentQuality * 0.2;
        const randomVariation = (Math.random() - 0.5) * 0.1;
        const quality = Math.min(0.98, baseQuality + learningBonus + randomVariation);
        // Simulate API delay
        await new Promise(resolve => setTimeout(resolve, 50 + Math.random() * 100));
        return {
            content: `Generated content for: ${prompt} (iteration ${iteration})`,
            quality,
            metadata: {
                iteration,
                prompt,
                timestamp: new Date()
            }
        };
    }
    /**
     * Evaluate output quality
     */
    async evaluate(output, tests) {
        let quality = output.quality || 0.5;
        // Apply test results if provided
        if (tests && tests.length > 0) {
            const passRate = this.calculateTestPassRate(output, tests);
            quality = quality * 0.7 + passRate * 0.3; // Weighted combination
        }
        return quality;
    }
    /**
     * Calculate test pass rate
     */
    calculateTestPassRate(output, tests) {
        const passed = tests.filter(test => {
            try {
                return test(output);
            }
            catch {
                return false;
            }
        }).length;
        return passed / tests.length;
    }
    /**
     * Generate feedback for current iteration
     */
    generateFeedback(quality, improvement) {
        const feedback = [];
        if (quality < 0.6) {
            feedback.push('Quality below acceptable threshold, increasing learning rate');
        }
        else if (quality < 0.8) {
            feedback.push('Moderate quality achieved, continue optimization');
        }
        else {
            feedback.push('High quality achieved, fine-tuning parameters');
        }
        if (improvement > 0.1) {
            feedback.push('Significant improvement detected');
        }
        else if (improvement < 0) {
            feedback.push('Quality regression, adjusting approach');
        }
        return feedback;
    }
    /**
     * Get learning history
     */
    getHistory() {
        return [...this.history];
    }
    /**
     * Reset learning state
     */
    reset() {
        this.history = [];
        this.currentQuality = 0.5;
        this.emit('reset');
    }
}
exports.SelfLearningGenerator = SelfLearningGenerator;
//# sourceMappingURL=self-learning.js.map