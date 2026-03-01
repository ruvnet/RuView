/**
 * Regional Agent - Per-region agent implementation for distributed processing
 *
 * Handles:
 * - Region-specific initialization
 * - Local query processing
 * - Cross-region communication
 * - State synchronization
 * - Metrics reporting
 */
import { EventEmitter } from 'events';
export interface RegionalAgentConfig {
    agentId: string;
    region: string;
    coordinatorEndpoint: string;
    localStoragePath: string;
    maxConcurrentStreams: number;
    metricsReportInterval: number;
    syncInterval: number;
    enableClaudeFlowHooks: boolean;
    vectorDimensions: number;
    capabilities: string[];
}
export interface QueryRequest {
    id: string;
    vector: number[];
    topK: number;
    filters?: Record<string, any>;
    timeout: number;
}
export interface QueryResult {
    id: string;
    matches: Array<{
        id: string;
        score: number;
        metadata: Record<string, any>;
    }>;
    latency: number;
    region: string;
}
export interface SyncPayload {
    type: 'index' | 'update' | 'delete';
    data: any;
    timestamp: number;
    sourceRegion: string;
}
export declare class RegionalAgent extends EventEmitter {
    private config;
    private activeStreams;
    private totalQueries;
    private totalLatency;
    private metricsTimer?;
    private syncTimer?;
    private localIndex;
    private syncQueue;
    private rateLimiter;
    constructor(config: RegionalAgentConfig);
    /**
     * Initialize regional agent
     */
    private initialize;
    /**
     * Load local index from persistent storage
     */
    private loadLocalIndex;
    /**
     * Register with coordinator
     */
    private registerWithCoordinator;
    /**
     * Process query request locally
     */
    processQuery(request: QueryRequest): Promise<QueryResult>;
    /**
     * Validate query request
     */
    private validateQuery;
    /**
     * Search vectors in local index
     */
    private searchVectors;
    /**
     * Calculate cosine similarity between vectors
     */
    private calculateSimilarity;
    /**
     * Check if metadata matches filters
     */
    private matchesFilters;
    /**
     * Add/update vectors in local index
     */
    indexVectors(vectors: Array<{
        id: string;
        vector: number[];
        metadata?: Record<string, any>;
    }>): Promise<void>;
    /**
     * Delete vectors from local index
     */
    deleteVectors(ids: string[]): Promise<void>;
    /**
     * Handle sync payload from other regions
     */
    handleSyncPayload(payload: SyncPayload): Promise<void>;
    /**
     * Start metrics reporting loop
     */
    private startMetricsReporting;
    /**
     * Report metrics to coordinator
     */
    private reportMetrics;
    /**
     * Get CPU usage (placeholder)
     */
    private getCpuUsage;
    /**
     * Get memory usage (placeholder)
     */
    private getMemoryUsage;
    /**
     * Check if agent is healthy
     */
    private isHealthy;
    /**
     * Start sync process loop
     */
    private startSyncProcess;
    /**
     * Process sync queue (send to other regions)
     */
    private processSyncQueue;
    /**
     * Get agent status
     */
    getStatus(): {
        agentId: string;
        region: string;
        healthy: boolean;
        activeStreams: number;
        indexSize: number;
        syncQueueSize: number;
        avgQueryLatency: number;
    };
    /**
     * Shutdown agent gracefully
     */
    shutdown(): Promise<void>;
    /**
     * Save local index to persistent storage
     */
    private saveLocalIndex;
}
//# sourceMappingURL=regional-agent.d.ts.map