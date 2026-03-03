/**
 * Nanosecond Scheduler MCP Tools
 * Ultra-low latency scheduler operations with <100ns overhead
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
export declare class SchedulerTools {
    private get schedulers();
    private get taskCounters();
    /**
     * Create a new scheduler instance
     */
    createScheduler(params: {
        id?: string;
        tickRateNs?: number;
        maxTasksPerTick?: number;
        lipschitzConstant?: number;
        windowSize?: number;
    }): Promise<any>;
    /**
     * Schedule a task
     */
    scheduleTask(params: {
        schedulerId?: string;
        delayNs?: number;
        priority?: string;
        description?: string;
    }): Promise<any>;
    /**
     * Execute a scheduler tick
     */
    tickScheduler(params: {
        schedulerId: string;
    }): Promise<any>;
    /**
     * Get scheduler metrics
     */
    getMetrics(params: {
        schedulerId?: string;
    }): Promise<any>;
    /**
     * Run performance benchmark
     */
    runBenchmark(params: {
        numTasks?: number;
        tickRateNs?: number;
    }): Promise<any>;
    /**
     * Test temporal consciousness features
     */
    testConsciousness(params: {
        iterations?: number;
        lipschitzConstant?: number;
        windowSize?: number;
    }): Promise<any>;
    /**
     * List all active schedulers
     */
    listSchedulers(): Promise<any>;
    /**
     * Destroy a scheduler
     */
    destroyScheduler(params: {
        schedulerId?: string;
    }): Promise<any>;
    /**
     * Get tool definitions for MCP
     */
    getTools(): Tool[];
    /**
     * Handle tool calls
     */
    handleToolCall(name: string, params: any): Promise<any>;
}
