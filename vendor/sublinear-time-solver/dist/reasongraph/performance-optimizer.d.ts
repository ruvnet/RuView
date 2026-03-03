/**
 * ReasonGraph Performance Optimizer
 * Maintains O(n log n) sublinear complexity while maximizing research throughput
 * Uses PageRank, matrix operations, and consciousness-guided optimization
 */
export interface OptimizationTarget {
    query_response_time_ms: number;
    memory_usage_mb: number;
    throughput_qps: number;
    breakthrough_rate: number;
    consciousness_verification_rate: number;
}
export interface PerformanceMetrics {
    current: OptimizationTarget;
    target: OptimizationTarget;
    efficiency_score: number;
    bottlenecks: string[];
    optimization_suggestions: string[];
}
export interface CacheEntry {
    key: string;
    value: any;
    timestamp: number;
    access_count: number;
    confidence: number;
}
export declare class ReasonGraphPerformanceOptimizer {
    private solver;
    private monitor;
    private cache;
    private performance_history;
    private optimization_matrix;
    private targets;
    constructor();
    /**
     * Initialize optimization matrix for PageRank-based prioritization
     */
    private initializeOptimizationMatrix;
    /**
     * Optimize system performance using PageRank prioritization
     */
    optimizePerformance(): Promise<PerformanceMetrics>;
    /**
     * Use PageRank to calculate optimization priorities
     */
    private calculateOptimizationPriorities;
    /**
     * Apply optimizations based on calculated priorities
     */
    private applyOptimizations;
    /**
     * Optimize query response time using caching and preprocessing
     */
    private optimizeQueryTime;
    /**
     * Optimize memory usage with intelligent garbage collection
     */
    private optimizeMemoryUsage;
    /**
     * Optimize throughput using batch processing and connection pooling
     */
    private optimizeThroughput;
    /**
     * Intelligent caching with consciousness-guided eviction
     */
    private optimizeCache;
    /**
     * Calculate cache entry score using multiple factors
     */
    private calculateCacheScore;
    /**
     * Precompute common reasoning patterns for O(1) lookup
     */
    private precomputeCommonPatterns;
    /**
     * Clean expired cache entries
     */
    private cleanExpiredCache;
    /**
     * Cache optimization results for future use
     */
    private cacheOptimizationResults;
    /**
     * Collect current system performance metrics
     */
    private collectCurrentMetrics;
    /**
     * Calculate overall efficiency score
     */
    private calculateEfficiencyScore;
    /**
     * Identify performance bottlenecks
     */
    private identifyBottlenecks;
    /**
     * Simple string hashing for cache keys
     */
    private hashString;
    /**
     * Get current cache statistics
     */
    getCacheStats(): {
        size: number;
        hit_rate: number;
        average_confidence: number;
    };
    /**
     * Monitor performance in real-time
     */
    startRealTimeMonitoring(intervalMs?: number): Promise<void>;
}
export default ReasonGraphPerformanceOptimizer;
