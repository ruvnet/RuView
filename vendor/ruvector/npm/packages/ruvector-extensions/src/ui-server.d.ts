export interface GraphNode {
    id: string;
    label?: string;
    metadata?: Record<string, any>;
    x?: number;
    y?: number;
}
export interface GraphLink {
    source: string;
    target: string;
    similarity: number;
}
export interface GraphData {
    nodes: GraphNode[];
    links: GraphLink[];
}
export declare class UIServer {
    private app;
    private server;
    private wss;
    private db;
    private clients;
    private port;
    constructor(db: any, port?: number);
    private setupMiddleware;
    private setupRoutes;
    private setupWebSocket;
    private handleWebSocketMessage;
    private broadcast;
    private getGraphData;
    private searchNodes;
    private findSimilarNodes;
    private getNodeDetails;
    start(): Promise<void>;
    stop(): Promise<void>;
    notifyGraphUpdate(): void;
}
export declare function startUIServer(db: any, port?: number): Promise<UIServer>;
//# sourceMappingURL=ui-server.d.ts.map