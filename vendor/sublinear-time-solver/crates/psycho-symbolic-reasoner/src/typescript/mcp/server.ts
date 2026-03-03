import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { AppConfig } from '../types/config.js';
import { Logger } from '../utils/logger.js';
import { createServer } from 'http';
import express from 'express';
import cors from 'cors';
import WebSocket from 'ws';
import { getReasoner } from '../reasoner/psycho-symbolic-reasoner.js';

/**
 * MCP Server implementation with multiple transport support
 */
export class MCPServer {
  private server: Server;
  private config: AppConfig;
  private httpServer?: any;
  private wsServer?: WebSocket.Server;
  private isRunning: boolean = false;

  constructor(config: AppConfig) {
    this.config = config;
    this.server = new Server(
      {
        name: 'psycho-symbolic-reasoner',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
          resources: {},
          prompts: {},
        },
      }
    );

    this.setupHandlers();
  }

  /**
   * Setup MCP handlers
   */
  private setupHandlers(): void {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: [
          {
            name: 'reason',
            description: 'Perform psycho-symbolic reasoning on a given query',
            inputSchema: {
              type: 'object',
              properties: {
                query: { type: 'string', description: 'The reasoning query' },
                context: { type: 'object', description: 'Additional context for reasoning' },
                depth: { type: 'number', description: 'Maximum reasoning depth', default: 5 }
              },
              required: ['query']
            }
          },
          {
            name: 'knowledge_graph_query',
            description: 'Query the knowledge graph',
            inputSchema: {
              type: 'object',
              properties: {
                query: { type: 'string', description: 'Graph query in natural language' },
                filters: { type: 'object', description: 'Query filters' },
                limit: { type: 'number', description: 'Maximum results', default: 10 }
              },
              required: ['query']
            }
          },
          {
            name: 'add_knowledge',
            description: 'Add new knowledge to the graph',
            inputSchema: {
              type: 'object',
              properties: {
                subject: { type: 'string', description: 'Subject entity' },
                predicate: { type: 'string', description: 'Relationship type' },
                object: { type: 'string', description: 'Object entity' },
                metadata: { type: 'object', description: 'Additional metadata' }
              },
              required: ['subject', 'predicate', 'object']
            }
          },
          {
            name: 'analyze_reasoning_path',
            description: 'Analyze and explain a reasoning path',
            inputSchema: {
              type: 'object',
              properties: {
                query: { type: 'string', description: 'Original query' },
                showSteps: { type: 'boolean', description: 'Show detailed steps', default: true },
                includeConfidence: { type: 'boolean', description: 'Include confidence scores', default: true }
              },
              required: ['query']
            }
          },
          {
            name: 'health_check',
            description: 'Check server health and status',
            inputSchema: {
              type: 'object',
              properties: {
                detailed: { type: 'boolean', description: 'Include detailed metrics', default: false }
              }
            }
          }
        ]
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        switch (name) {
          case 'reason':
            return await this.handleReason(args);
          case 'knowledge_graph_query':
            return await this.handleKnowledgeGraphQuery(args);
          case 'add_knowledge':
            return await this.handleAddKnowledge(args);
          case 'analyze_reasoning_path':
            return await this.handleAnalyzeReasoningPath(args);
          case 'health_check':
            return await this.handleHealthCheck(args);
          default:
            throw new Error(`Unknown tool: ${name}`);
        }
      } catch (error) {
        Logger.error(`Tool execution failed: ${name}`, error);
        return {
          content: [
            {
              type: 'text',
              text: `Error executing tool '${name}': ${error instanceof Error ? error.message : 'Unknown error'}`
            }
          ]
        };
      }
    });
  }

  /**
   * Handle reasoning tool
   */
  private async handleReason(args: any): Promise<any> {
    const { query, context, depth = 5 } = args;

    Logger.info('Processing reasoning request', { query, depth });

    try {
      const reasoner = getReasoner();
      const reasoningResult = await reasoner.reason(query, context, depth);

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(reasoningResult, null, 2)
          }
        ]
      };
    } catch (error) {
      Logger.error('Reasoning failed', error);
      throw error;
    }
  }

  /**
   * Handle knowledge graph query
   */
  private async handleKnowledgeGraphQuery(args: any): Promise<any> {
    const { query, filters = {}, limit = 10 } = args;

    Logger.info('Processing knowledge graph query', { query, limit });

    try {
      const reasoner = getReasoner();
      const queryResult = reasoner.queryKnowledgeGraph(query, filters, limit);

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(queryResult, null, 2)
          }
        ]
      };
    } catch (error) {
      Logger.error('Knowledge graph query failed', error);
      throw error;
    }
  }

  /**
   * Handle add knowledge
   */
  private async handleAddKnowledge(args: any): Promise<any> {
    const { subject, predicate, object, metadata = {} } = args;

    Logger.info('Adding knowledge to graph', { subject, predicate, object });

    try {
      const reasoner = getReasoner();
      const triple = reasoner.addKnowledge(subject, predicate, object, metadata);

      const addResult = {
        success: true,
        triple: {
          subject: triple.subject,
          predicate: triple.predicate,
          object: triple.object
        },
        id: triple.id,
        confidence: triple.confidence,
        metadata: triple.metadata
      };

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(addResult, null, 2)
          }
        ]
      };
    } catch (error) {
      Logger.error('Failed to add knowledge', error);
      throw error;
    }
  }

  /**
   * Handle analyze reasoning path
   */
  private async handleAnalyzeReasoningPath(args: any): Promise<any> {
    const { query, showSteps = true, includeConfidence = true } = args;

    Logger.info('Analyzing reasoning path', { query });

    try {
      const reasoner = getReasoner();
      const analysisResult = await reasoner.analyzeReasoningPath(query, showSteps, includeConfidence);

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(analysisResult, null, 2)
          }
        ]
      };
    } catch (error) {
      Logger.error('Reasoning path analysis failed', error);
      throw error;
    }
  }

  /**
   * Handle health check
   */
  private async handleHealthCheck(args: any): Promise<any> {
    const { detailed = false } = args || {};

    Logger.info('Processing health check', { detailed });

    try {
      const reasoner = getReasoner();
      const healthStatus = reasoner.getHealthStatus(detailed);

      // Add server info
      healthStatus.server = {
        name: 'psycho-symbolic-reasoner',
        version: '1.0.5',
        transport: this.config.server.transport,
        host: this.config.server.host,
        port: this.config.server.port
      };

      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify(healthStatus, null, 2)
          }
        ]
      };
    } catch (error) {
      Logger.error('Health check failed', error);
      throw error;
    }
  }

  /**
   * Start the MCP server with configured transport
   */
  public async start(): Promise<void> {
    try {
      Logger.info(`Starting MCP server with ${this.config.server.transport} transport`);

      switch (this.config.server.transport) {
        case 'stdio':
          await this.startStdioTransport();
          break;
        case 'http':
          await this.startHttpTransport();
          break;
        case 'sse':
          await this.startSSETransport();
          break;
        default:
          throw new Error(`Unsupported transport: ${this.config.server.transport}`);
      }

      this.isRunning = true;
      Logger.info('MCP server started successfully');
    } catch (error) {
      Logger.error('Failed to start MCP server', error);
      throw error;
    }
  }

  /**
   * Start STDIO transport
   */
  private async startStdioTransport(): Promise<void> {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
  }

  /**
   * Start HTTP transport
   */
  private async startHttpTransport(): Promise<void> {
    const app = express();

    if (this.config.server.cors) {
      app.use(cors({
        origin: this.config.security.allowedOrigins,
        credentials: true
      }));
    }

    app.use(express.json({ limit: '10mb' }));

    // Health check endpoint
    app.get('/health', (_req, res) => {
      res.json({
        status: 'healthy',
        timestamp: new Date().toISOString(),
        transport: 'http'
      });
    });

    // MCP endpoint
    app.post('/mcp', async (_req, res) => {
      try {
        // TODO: Implement HTTP MCP protocol handling
        res.json({ message: 'MCP HTTP transport not fully implemented yet' });
      } catch (error) {
        Logger.error('HTTP MCP request failed', error);
        res.status(500).json({ error: 'Internal server error' });
      }
    });

    this.httpServer = createServer(app);

    return new Promise((resolve, reject) => {
      this.httpServer.listen(this.config.server.port, this.config.server.host, () => {
        Logger.info(`HTTP server listening on ${this.config.server.host}:${this.config.server.port}`);
        resolve();
      });

      this.httpServer.on('error', reject);
    });
  }

  /**
   * Start SSE transport
   */
  private async startSSETransport(): Promise<void> {
    await this.startHttpTransport(); // SSE builds on HTTP

    // TODO: Implement SSE-specific MCP protocol handling
    Logger.info('SSE transport started (built on HTTP)');
  }

  /**
   * Stop the server gracefully
   */
  public async stop(): Promise<void> {
    if (!this.isRunning) {
      return;
    }

    Logger.info('Stopping MCP server...');

    try {
      // Close WebSocket server
      if (this.wsServer) {
        this.wsServer.close();
      }

      // Close HTTP server
      if (this.httpServer) {
        await new Promise<void>((resolve) => {
          this.httpServer.close(() => resolve());
        });
      }

      this.isRunning = false;
      Logger.info('MCP server stopped successfully');
    } catch (error) {
      Logger.error('Error stopping MCP server', error);
      throw error;
    }
  }

  /**
   * Check if server is running
   */
  public isServerRunning(): boolean {
    return this.isRunning;
  }
}