/**
 * Streaming support for MidStream - WebSocket and SSE
 */

import { WebSocketServer, WebSocket } from 'ws';
import { createServer, IncomingMessage, ServerResponse } from 'http';
import { MidStreamAgent } from './agent.js';

// ============================================================================
// WebSocket Streaming Server
// ============================================================================

export class WebSocketStreamServer {
  private wss: WebSocketServer | null = null;
  private port: number;
  private agent: MidStreamAgent;
  private clients: Set<WebSocket> = new Set();

  constructor(port: number = 3001) {
    this.port = port;
    this.agent = new MidStreamAgent();
  }

  async start(): Promise<void> {
    this.wss = new WebSocketServer({ port: this.port });

    this.wss.on('connection', (ws: WebSocket) => {
      console.log('WebSocket client connected');
      this.clients.add(ws);

      ws.on('message', async (data: Buffer) => {
        try {
          const message = data.toString();
          const parsed = JSON.parse(message);

          const response = await this.handleMessage(parsed);
          ws.send(JSON.stringify(response));
        } catch (error) {
          ws.send(JSON.stringify({
            error: error instanceof Error ? error.message : String(error),
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

      // Send welcome message
      ws.send(JSON.stringify({
        type: 'connected',
        message: 'Connected to MidStream WebSocket server',
        timestamp: Date.now(),
      }));
    });

    console.log(`WebSocket server listening on port ${this.port}`);
  }

  async stop(): Promise<void> {
    if (this.wss) {
      this.clients.forEach(client => client.close());
      this.wss.close();
      this.wss = null;
    }
  }

  broadcast(data: any): void {
    const message = JSON.stringify(data);
    this.clients.forEach(client => {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    });
  }

  private async handleMessage(message: any): Promise<any> {
    const { type, payload } = message;

    switch (type) {
      case 'process':
        return {
          type: 'result',
          data: this.agent.processMessage(payload.message),
          timestamp: Date.now(),
        };

      case 'analyze':
        return {
          type: 'analysis',
          data: this.agent.analyzeConversation(payload.messages),
          timestamp: Date.now(),
        };

      case 'compare':
        return {
          type: 'comparison',
          data: {
            similarity: this.agent.compareSequences(
              payload.sequence1,
              payload.sequence2,
              payload.algorithm || 'dtw'
            ),
          },
          timestamp: Date.now(),
        };

      case 'detect_pattern':
        return {
          type: 'pattern',
          data: {
            positions: this.agent.detectPattern(payload.sequence, payload.pattern),
          },
          timestamp: Date.now(),
        };

      case 'behavior':
        return {
          type: 'behavior_analysis',
          data: this.agent.analyzeBehavior(payload.rewards),
          timestamp: Date.now(),
        };

      case 'status':
        return {
          type: 'status',
          data: this.agent.getStatus(),
          timestamp: Date.now(),
        };

      default:
        return {
          type: 'error',
          error: `Unknown message type: ${type}`,
          timestamp: Date.now(),
        };
    }
  }
}

// ============================================================================
// SSE (Server-Sent Events) Streaming Server
// ============================================================================

export class SSEStreamServer {
  private server: ReturnType<typeof createServer> | null = null;
  private port: number;
  private agent: MidStreamAgent;
  private clients: Set<ServerResponse> = new Set();

  constructor(port: number = 3002) {
    this.port = port;
    this.agent = new MidStreamAgent();
  }

  async start(): Promise<void> {
    this.server = createServer((req: IncomingMessage, res: ServerResponse) => {
      // CORS headers
      res.setHeader('Access-Control-Allow-Origin', '*');
      res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

      if (req.method === 'OPTIONS') {
        res.writeHead(200);
        res.end();
        return;
      }

      const url = new URL(req.url || '', `http://${req.headers.host}`);

      if (url.pathname === '/stream' && req.method === 'GET') {
        this.handleSSEConnection(req, res);
      } else if (url.pathname === '/process' && req.method === 'POST') {
        this.handleProcessRequest(req, res);
      } else if (url.pathname === '/analyze' && req.method === 'POST') {
        this.handleAnalyzeRequest(req, res);
      } else if (url.pathname === '/status' && req.method === 'GET') {
        this.handleStatusRequest(req, res);
      } else {
        res.writeHead(404);
        res.end('Not Found');
      }
    });

    this.server.listen(this.port);
    console.log(`SSE server listening on port ${this.port}`);
  }

  async stop(): Promise<void> {
    if (this.server) {
      this.clients.forEach(client => client.end());
      this.server.close();
      this.server = null;
    }
  }

  broadcast(data: any): void {
    const message = `data: ${JSON.stringify(data)}\n\n`;
    this.clients.forEach(client => {
      try {
        client.write(message);
      } catch (error) {
        console.error('Error broadcasting to SSE client:', error);
      }
    });
  }

  private handleSSEConnection(req: IncomingMessage, res: ServerResponse): void {
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    });

    this.clients.add(res);
    console.log('SSE client connected');

    // Send initial connection event
    res.write(`data: ${JSON.stringify({
      type: 'connected',
      message: 'Connected to MidStream SSE server',
      timestamp: Date.now(),
    })}\n\n`);

    // Send periodic heartbeat
    const heartbeat = setInterval(() => {
      res.write(`: heartbeat\n\n`);
    }, 30000);

    req.on('close', () => {
      clearInterval(heartbeat);
      this.clients.delete(res);
      console.log('SSE client disconnected');
    });
  }

  private handleProcessRequest(req: IncomingMessage, res: ServerResponse): void {
    let body = '';

    req.on('data', chunk => {
      body += chunk.toString();
    });

    req.on('end', () => {
      try {
        const { message } = JSON.parse(body);
        const result = this.agent.processMessage(message);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          success: true,
          data: result,
          timestamp: Date.now(),
        }));

        // Broadcast to SSE clients
        this.broadcast({
          type: 'processed',
          data: result,
          timestamp: Date.now(),
        });
      } catch (error) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          success: false,
          error: error instanceof Error ? error.message : String(error),
        }));
      }
    });
  }

  private handleAnalyzeRequest(req: IncomingMessage, res: ServerResponse): void {
    let body = '';

    req.on('data', chunk => {
      body += chunk.toString();
    });

    req.on('end', () => {
      try {
        const { messages } = JSON.parse(body);
        const result = this.agent.analyzeConversation(messages);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          success: true,
          data: result,
          timestamp: Date.now(),
        }));
      } catch (error) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          success: false,
          error: error instanceof Error ? error.message : String(error),
        }));
      }
    });
  }

  private handleStatusRequest(req: IncomingMessage, res: ServerResponse): void {
    const status = this.agent.getStatus();

    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      success: true,
      data: status,
      timestamp: Date.now(),
    }));
  }
}

// ============================================================================
// HTTP Streaming Client (for use in Node.js)
// ============================================================================

export class HTTPStreamingClient {
  private baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  async stream(
    endpoint: string,
    onChunk: (chunk: Buffer) => void
  ): Promise<void> {
    const https = await import('https');
    const http = await import('http');

    const url = new URL(endpoint, this.baseUrl);
    const client = url.protocol === 'https:' ? https : http;

    return new Promise((resolve, reject) => {
      const req = client.get(url, (res) => {
        res.on('data', onChunk);
        res.on('end', resolve);
        res.on('error', reject);
      });

      req.on('error', reject);
    });
  }
}
