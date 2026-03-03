#!/usr/bin/env node
/**
 * MidStream MCP (Model Context Protocol) Server
 *
 * Provides MCP interface for the Lean Agentic Learning System
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { MidStreamAgent } from './agent.js';
import { WebSocketStreamServer, SSEStreamServer } from './streaming.js';

interface MCPConfig {
  port?: number;
  wsPort?: number;
  ssePort?: number;
  maxHistory?: number;
}

class MidStreamMCPServer {
  private server: Server;
  private agent: MidStreamAgent;
  private wsServer?: WebSocketStreamServer;
  private sseServer?: SSEStreamServer;
  private config: MCPConfig;

  constructor(config: MCPConfig = {}) {
    this.config = {
      port: config.port || 3000,
      wsPort: config.wsPort || 3001,
      ssePort: config.ssePort || 3002,
      maxHistory: config.maxHistory || 1000,
    };

    this.server = new Server(
      {
        name: 'midstream-server',
        version: '0.1.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.agent = new MidStreamAgent({
      maxHistory: this.config.maxHistory,
    });

    this.setupHandlers();
  }

  private setupHandlers(): void {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: this.getTools(),
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        switch (name) {
          case 'analyze_conversation':
            return await this.analyzeConversation(args);

          case 'compare_sequences':
            return await this.compareSequences(args);

          case 'detect_patterns':
            return await this.detectPatterns(args);

          case 'analyze_behavior':
            return await this.analyzeBehavior(args);

          case 'meta_learn':
            return await this.metaLearn(args);

          case 'get_status':
            return await this.getStatus();

          case 'stream_websocket':
            return await this.setupWebSocket(args);

          case 'stream_sse':
            return await this.setupSSE(args);

          default:
            throw new Error(`Unknown tool: ${name}`);
        }
      } catch (error) {
        return {
          content: [
            {
              type: 'text' as const,
              text: `Error: ${error instanceof Error ? error.message : String(error)}`,
            },
          ],
        };
      }
    });
  }

  private getTools(): Tool[] {
    return [
      {
        name: 'analyze_conversation',
        description: 'Analyze a conversation thread using temporal analysis and meta-learning',
        inputSchema: {
          type: 'object' as const,
          properties: {
            messages: {
              type: 'array',
              items: { type: 'string' },
              description: 'Array of conversation messages',
            },
          },
          required: ['messages'],
        },
      },
      {
        name: 'compare_sequences',
        description: 'Compare two sequences using DTW, LCS, or edit distance',
        inputSchema: {
          type: 'object' as const,
          properties: {
            sequence1: {
              type: 'array',
              items: { type: 'string' },
              description: 'First sequence',
            },
            sequence2: {
              type: 'array',
              items: { type: 'string' },
              description: 'Second sequence',
            },
            algorithm: {
              type: 'string',
              enum: ['dtw', 'lcs', 'edit', 'correlation'],
              description: 'Comparison algorithm',
            },
          },
          required: ['sequence1', 'sequence2', 'algorithm'],
        },
      },
      {
        name: 'detect_patterns',
        description: 'Detect pattern occurrences in a sequence',
        inputSchema: {
          type: 'object' as const,
          properties: {
            sequence: {
              type: 'array',
              items: { type: 'string' },
              description: 'Sequence to search',
            },
            pattern: {
              type: 'array',
              items: { type: 'string' },
              description: 'Pattern to find',
            },
          },
          required: ['sequence', 'pattern'],
        },
      },
      {
        name: 'analyze_behavior',
        description: 'Analyze agent behavior for chaos/stability using attractor analysis',
        inputSchema: {
          type: 'object' as const,
          properties: {
            rewards: {
              type: 'array',
              items: { type: 'number' },
              description: 'Reward history',
            },
          },
          required: ['rewards'],
        },
      },
      {
        name: 'meta_learn',
        description: 'Perform meta-learning on a learning event',
        inputSchema: {
          type: 'object' as const,
          properties: {
            content: {
              type: 'string',
              description: 'Learning content',
            },
            reward: {
              type: 'number',
              description: 'Reward value',
            },
          },
          required: ['content', 'reward'],
        },
      },
      {
        name: 'get_status',
        description: 'Get current agent status and configuration',
        inputSchema: {
          type: 'object' as const,
          properties: {},
        },
      },
      {
        name: 'stream_websocket',
        description: 'Start WebSocket streaming server',
        inputSchema: {
          type: 'object' as const,
          properties: {
            port: {
              type: 'number',
              description: 'WebSocket port',
            },
          },
        },
      },
      {
        name: 'stream_sse',
        description: 'Start SSE streaming server',
        inputSchema: {
          type: 'object' as const,
          properties: {
            port: {
              type: 'number',
              description: 'SSE port',
            },
          },
        },
      },
    ];
  }

  private async analyzeConversation(args: any) {
    const { messages } = args;
    const result = this.agent.analyzeConversation(messages);

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify(result, null, 2),
        },
      ],
    };
  }

  private async compareSequences(args: any) {
    const { sequence1, sequence2, algorithm } = args;
    const similarity = this.agent.compareSequences(sequence1, sequence2, algorithm);

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify({
            algorithm,
            similarity,
            interpretation: similarity > 0.8 ? 'Very similar' :
                          similarity > 0.6 ? 'Moderately similar' :
                          similarity > 0.4 ? 'Somewhat similar' : 'Different',
          }, null, 2),
        },
      ],
    };
  }

  private async detectPatterns(args: any) {
    const { sequence, pattern } = args;
    const positions = this.agent.detectPattern(sequence, pattern);

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify({
            pattern_found: positions.length > 0,
            occurrences: positions.length,
            positions,
          }, null, 2),
        },
      ],
    };
  }

  private async analyzeBehavior(args: any) {
    const { rewards } = args;
    const analysis = this.agent.analyzeBehavior(rewards);

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify(analysis, null, 2),
        },
      ],
    };
  }

  private async metaLearn(args: any) {
    const { content, reward } = args;
    this.agent.learn(content, reward);

    const summary = this.agent.getMetaLearningSummary();

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify(summary, null, 2),
        },
      ],
    };
  }

  private async getStatus() {
    const status = this.agent.getStatus();

    return {
      content: [
        {
          type: 'text' as const,
          text: JSON.stringify(status, null, 2),
        },
      ],
    };
  }

  private async setupWebSocket(args: any) {
    const port = args?.port || this.config.wsPort;

    if (!this.wsServer) {
      this.wsServer = new WebSocketStreamServer(port);
      await this.wsServer.start();
    }

    return {
      content: [
        {
          type: 'text' as const,
          text: `WebSocket server started on port ${port}`,
        },
      ],
    };
  }

  private async setupSSE(args: any) {
    const port = args?.port || this.config.ssePort;

    if (!this.sseServer) {
      this.sseServer = new SSEStreamServer(port);
      await this.sseServer.start();
    }

    return {
      content: [
        {
          type: 'text' as const,
          text: `SSE server started on port ${port}`,
        },
      ],
    };
  }

  async start(): Promise<void> {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);

    console.error('MidStream MCP Server started');
    console.error('Available tools:');
    this.getTools().forEach(tool => {
      console.error(`  - ${tool.name}: ${tool.description}`);
    });
  }

  async stop(): Promise<void> {
    if (this.wsServer) {
      await this.wsServer.stop();
    }
    if (this.sseServer) {
      await this.sseServer.stop();
    }
    await this.server.close();
  }
}

// Start server if run directly
if (require.main === module) {
  const server = new MidStreamMCPServer();

  process.on('SIGINT', async () => {
    await server.stop();
    process.exit(0);
  });

  server.start().catch(console.error);
}

export { MidStreamMCPServer };
