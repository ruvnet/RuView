/**
 * Load Balancer - Intelligent request routing and traffic management
 *
 * Features:
 * - Circuit breaker pattern
 * - Rate limiting per client
 * - Regional routing
 * - Request prioritization
 * - Health-based routing
 */
import { EventEmitter } from 'events';
export interface LoadBalancerConfig {
    maxRequestsPerSecond?: number;
    circuitBreakerThreshold?: number;
    circuitBreakerTimeout?: number;
    halfOpenMaxRequests?: number;
    backends?: BackendConfig[];
    enableRegionalRouting?: boolean;
    priorityQueueSize?: number;
}
export interface BackendConfig {
    id: string;
    host: string;
    region?: string;
    weight?: number;
    maxConcurrency?: number;
}
declare enum RequestPriority {
    LOW = 0,
    NORMAL = 1,
    HIGH = 2,
    CRITICAL = 3
}
/**
 * Load Balancer
 */
export declare class LoadBalancer extends EventEmitter {
    private rateLimiter;
    private backendManager;
    private requestQueue;
    private config;
    constructor(config: LoadBalancerConfig);
    route(collection: string, query: any, clientId?: string, priority?: RequestPriority): Promise<boolean>;
    executeWithLoadBalancing<T>(fn: () => Promise<T>, region?: string, priority?: RequestPriority): Promise<T>;
    updateBackendHealth(backendId: string, healthScore: number): void;
    private updateMetrics;
    getStats(): {
        rateLimit: {
            totalClients: number;
            limitedClients: number;
        };
        backends: Record<string, any>;
        queueSize: number;
    };
    reset(): void;
}
export {};
//# sourceMappingURL=load-balancer.d.ts.map