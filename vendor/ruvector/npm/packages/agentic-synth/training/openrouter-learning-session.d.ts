/**
 * Comprehensive Agentic-Synth Training & Learning Session
 *
 * This script demonstrates a complete training workflow using OpenRouter API:
 * 1. Baseline generation and measurement
 * 2. Learning from successful patterns
 * 3. Adaptive optimization
 * 4. Comprehensive benchmarking
 * 5. Final optimized generation
 *
 * Usage:
 *   export OPENROUTER_API_KEY=your-key-here
 *   npx tsx training/openrouter-learning-session.ts
 */
declare class TrainingSession {
    private synth;
    private metrics;
    private patterns;
    private bestSchema;
    private bestQuality;
    constructor();
    /**
     * Run complete training session
     */
    run(): Promise<void>;
    /**
     * Phase 1: Baseline Generation
     */
    private runBaselineGeneration;
    /**
     * Phase 2: Learning Loop
     */
    private runLearningLoop;
    /**
     * Phase 3: Model Comparison
     */
    private runModelComparison;
    /**
     * Phase 4: Comprehensive Benchmarking
     */
    private runComprehensiveBenchmarks;
    /**
     * Phase 5: Final Optimized Generation
     */
    private runOptimizedGeneration;
    /**
     * Phase 6: Generate Reports
     */
    private generateReports;
    /**
     * Calculate quality score for generated data
     */
    private calculateQuality;
    /**
     * Calculate diversity score
     */
    private calculateDiversity;
    /**
     * Record training metrics
     */
    private recordMetrics;
    /**
     * Learn from successful generation
     */
    private learnFromSuccess;
    /**
     * Evolve schema based on learning
     */
    private evolveSchema;
    /**
     * Save data to file
     */
    private saveData;
    /**
     * Generate markdown report
     */
    private generateMarkdownReport;
}
export { TrainingSession };
//# sourceMappingURL=openrouter-learning-session.d.ts.map