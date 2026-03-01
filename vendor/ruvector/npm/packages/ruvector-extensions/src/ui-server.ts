import express, { Request, Response } from 'express';
import { createServer } from 'http';
import { WebSocketServer, WebSocket } from 'ws';
import path from 'path';
import type { VectorDB } from 'ruvector';

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

export class UIServer {
    private app: express.Application;
    private server: any;
    private wss: WebSocketServer;
    private db: any;
    private clients: Set<WebSocket>;
    private port: number;

    constructor(db: any, port: number = 3000) {
        this.db = db;
        this.port = port;
        this.clients = new Set();

        this.app = express();
        this.server = createServer(this.app);
        this.wss = new WebSocketServer({ server: this.server });

        this.setupMiddleware();
        this.setupRoutes();
        this.setupWebSocket();
    }

    private setupMiddleware(): void {
        // JSON parsing
        this.app.use(express.json());

        // CORS
        this.app.use((req, res, next) => {
            res.header('Access-Control-Allow-Origin', '*');
            res.header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
            res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');
            next();
        });

        // Static files
        const uiPath = path.join(__dirname, 'ui');
        this.app.use(express.static(uiPath));

        // Logging
        this.app.use((req, res, next) => {
            console.log(`${new Date().toISOString()} ${req.method} ${req.path}`);
            next();
        });
    }

    private setupRoutes(): void {
        // Health check
        this.app.get('/health', (req: Request, res: Response) => {
            res.json({
                status: 'ok',
                timestamp: Date.now(),
                version: '1.0.0'
            });
        });

        // Get full graph data
        this.app.get('/api/graph', async (req: Request, res: Response) => {
            try {
                const maxNodes = parseInt(req.query.max as string) || 100;
                const graphData = await this.getGraphData(maxNodes);
                res.json(graphData);
            } catch (error) {
                console.error('Error fetching graph:', error);
                res.status(500).json({
                    error: 'Failed to fetch graph data',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Search nodes
        this.app.get('/api/search', async (req: Request, res: Response) => {
            try {
                const query = req.query.q as string;
                if (!query) {
                    return res.status(400).json({ error: 'Query parameter required' });
                }

                const results = await this.searchNodes(query);
                res.json({ results, count: results.length });
            } catch (error) {
                console.error('Search error:', error);
                res.status(500).json({
                    error: 'Search failed',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Find similar nodes
        this.app.get('/api/similarity/:nodeId', async (req: Request, res: Response) => {
            try {
                const { nodeId } = req.params;
                const threshold = parseFloat(req.query.threshold as string) || 0.5;
                const limit = parseInt(req.query.limit as string) || 10;

                const similar = await this.findSimilarNodes(nodeId, threshold, limit);
                res.json({
                    nodeId,
                    similar,
                    count: similar.length,
                    threshold
                });
            } catch (error) {
                console.error('Similarity search error:', error);
                res.status(500).json({
                    error: 'Similarity search failed',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Get node details
        this.app.get('/api/nodes/:nodeId', async (req: Request, res: Response) => {
            try {
                const { nodeId } = req.params;
                const node = await this.getNodeDetails(nodeId);

                if (!node) {
                    return res.status(404).json({ error: 'Node not found' });
                }

                res.json(node);
            } catch (error) {
                console.error('Error fetching node:', error);
                res.status(500).json({
                    error: 'Failed to fetch node',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Add new node (for testing)
        this.app.post('/api/nodes', async (req: Request, res: Response) => {
            try {
                const { id, embedding, metadata } = req.body;

                if (!id || !embedding) {
                    return res.status(400).json({ error: 'ID and embedding required' });
                }

                await this.db.add(id, embedding, metadata);

                // Notify all clients
                this.broadcast({
                    type: 'node_added',
                    payload: { id, metadata }
                });

                res.status(201).json({ success: true, id });
            } catch (error) {
                console.error('Error adding node:', error);
                res.status(500).json({
                    error: 'Failed to add node',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Database statistics
        this.app.get('/api/stats', async (req: Request, res: Response) => {
            try {
                const stats = await this.db.getStats();
                res.json(stats);
            } catch (error) {
                console.error('Error fetching stats:', error);
                res.status(500).json({
                    error: 'Failed to fetch statistics',
                    message: error instanceof Error ? error.message : 'Unknown error'
                });
            }
        });

        // Serve UI
        this.app.get('*', (req: Request, res: Response) => {
            res.sendFile(path.join(__dirname, 'ui', 'index.html'));
        });
    }

    private setupWebSocket(): void {
        this.wss.on('connection', (ws: WebSocket) => {
            console.log('New WebSocket client connected');
            this.clients.add(ws);

            ws.on('message', async (message: string) => {
                try {
                    const data = JSON.parse(message.toString());
                    await this.handleWebSocketMessage(ws, data);
                } catch (error) {
                    console.error('WebSocket message error:', error);
                    ws.send(JSON.stringify({
                        type: 'error',
                        message: 'Invalid message format'
                    }));
                }
            });

            ws.on('close', () => {
                console.log('WebSocket client disconnected');
                this.clients.delete(ws);
            });

            ws.on('error', (error) => {
                console.error('WebSocket error:', error);
                this.clients.delete(ws);
            });

            // Send initial connection message
            ws.send(JSON.stringify({
                type: 'connected',
                message: 'Connected to RuVector UI Server'
            }));
        });
    }

    private async handleWebSocketMessage(ws: WebSocket, data: any): Promise<void> {
        switch (data.type) {
            case 'subscribe':
                // Handle subscription to updates
                ws.send(JSON.stringify({
                    type: 'subscribed',
                    message: 'Subscribed to graph updates'
                }));
                break;

            case 'request_graph':
                const graphData = await this.getGraphData(data.maxNodes || 100);
                ws.send(JSON.stringify({
                    type: 'graph_data',
                    payload: graphData
                }));
                break;

            case 'similarity_query':
                const similar = await this.findSimilarNodes(
                    data.nodeId,
                    data.threshold || 0.5,
                    data.limit || 10
                );
                ws.send(JSON.stringify({
                    type: 'similarity_result',
                    payload: { nodeId: data.nodeId, similar }
                }));
                break;

            default:
                ws.send(JSON.stringify({
                    type: 'error',
                    message: 'Unknown message type'
                }));
        }
    }

    private broadcast(message: any): void {
        const messageStr = JSON.stringify(message);
        this.clients.forEach(client => {
            if (client.readyState === WebSocket.OPEN) {
                client.send(messageStr);
            }
        });
    }

    private async getGraphData(maxNodes: number): Promise<GraphData> {
        // Get all vectors from database
        const vectors = await this.db.list();
        const nodes: GraphNode[] = [];
        const links: GraphLink[] = [];
        const nodeMap = new Map<string, GraphNode>();

        // Limit nodes
        const limitedVectors = vectors.slice(0, maxNodes);

        // Create nodes
        for (const vector of limitedVectors) {
            const node: GraphNode = {
                id: vector.id,
                label: vector.metadata?.label || vector.id.substring(0, 8),
                metadata: vector.metadata
            };
            nodes.push(node);
            nodeMap.set(vector.id, node);
        }

        // Create links based on similarity
        for (let i = 0; i < limitedVectors.length; i++) {
            const sourceVector = limitedVectors[i];

            // Find top 5 similar nodes
            const similar = await this.db.query(sourceVector.embedding, { topK: 6 });

            for (const result of similar) {
                // Skip self-links and already processed pairs
                if (result.id === sourceVector.id) continue;
                if (!nodeMap.has(result.id)) continue;

                // Only add links above threshold
                if (result.similarity > 0.3) {
                    links.push({
                        source: sourceVector.id,
                        target: result.id,
                        similarity: result.similarity
                    });
                }
            }
        }

        return { nodes, links };
    }

    private async searchNodes(query: string): Promise<GraphNode[]> {
        const vectors = await this.db.list();
        const results: GraphNode[] = [];

        for (const vector of vectors) {
            // Search in ID
            if (vector.id.toLowerCase().includes(query.toLowerCase())) {
                results.push({
                    id: vector.id,
                    label: vector.metadata?.label,
                    metadata: vector.metadata
                });
                continue;
            }

            // Search in metadata
            if (vector.metadata) {
                const metadataStr = JSON.stringify(vector.metadata).toLowerCase();
                if (metadataStr.includes(query.toLowerCase())) {
                    results.push({
                        id: vector.id,
                        label: vector.metadata.label,
                        metadata: vector.metadata
                    });
                }
            }
        }

        return results;
    }

    private async findSimilarNodes(
        nodeId: string,
        threshold: number,
        limit: number
    ): Promise<Array<GraphNode & { similarity: number }>> {
        // Get the source node
        const sourceVector = await this.db.get(nodeId);
        if (!sourceVector) {
            throw new Error('Node not found');
        }

        // Query similar nodes
        const results = await this.db.query(sourceVector.embedding, {
            topK: limit + 1
        });

        // Filter and format results
        return results
            .filter((r: any) => r.id !== nodeId && r.similarity >= threshold)
            .slice(0, limit)
            .map((r: any) => ({
                id: r.id,
                similarity: r.similarity,
                metadata: r.metadata
            }));
    }

    private async getNodeDetails(nodeId: string): Promise<GraphNode | null> {
        const vector = await this.db.get(nodeId);
        if (!vector) return null;

        return {
            id: vector.id,
            label: vector.metadata?.label,
            metadata: vector.metadata
        };
    }

    public start(): Promise<void> {
        return new Promise((resolve) => {
            this.server.listen(this.port, () => {
                console.log(`
╔════════════════════════════════════════════════════════════╗
║         RuVector Graph Explorer UI Server                  ║
╚════════════════════════════════════════════════════════════╝

🌐 Server running at: http://localhost:${this.port}
📊 WebSocket: ws://localhost:${this.port}
🗄️  Database: Connected

Open your browser and navigate to http://localhost:${this.port}
                `);
                resolve();
            });
        });
    }

    public stop(): Promise<void> {
        return new Promise((resolve) => {
            // Close WebSocket connections
            this.clients.forEach(client => client.close());

            // Close WebSocket server
            this.wss.close(() => {
                // Close HTTP server
                this.server.close(() => {
                    console.log('UI Server stopped');
                    resolve();
                });
            });
        });
    }

    public notifyGraphUpdate(): void {
        // Broadcast update to all clients
        this.broadcast({
            type: 'update',
            message: 'Graph data updated'
        });
    }
}

// Example usage
export async function startUIServer(db: any, port: number = 3000): Promise<UIServer> {
    const server = new UIServer(db, port);
    await server.start();
    return server;
}
