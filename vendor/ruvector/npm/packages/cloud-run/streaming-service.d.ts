/**
 * Cloud Run Streaming Service - Main Entry Point
 *
 * High-performance HTTP/2 + WebSocket server for massive concurrent connections.
 * Optimized for 500M concurrent learning streams with adaptive scaling.
 */
export declare class StreamingService {
    private app;
    private vectorClient;
    private loadBalancer;
    private connectionManager;
    private isShuttingDown;
    constructor();
    private setupMiddleware;
    private setupRoutes;
    private setupShutdownHandlers;
    start(): Promise<void>;
}
//# sourceMappingURL=streaming-service.d.ts.map