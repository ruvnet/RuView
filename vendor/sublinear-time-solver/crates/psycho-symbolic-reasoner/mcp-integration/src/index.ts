#!/usr/bin/env node

import { Server } from '@modelcontextprotocol/sdk/server/index';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types';
import { PsychoSymbolicMcpTools } from './tools/index';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { existsSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Default WASM paths
const DEFAULT_WASM_DIR = join(__dirname, '..', 'wasm');
const DEFAULT_CONFIG = {
  graphReasonerWasmPath: join(DEFAULT_WASM_DIR, 'graph-reasoner'),
  textExtractorWasmPath: join(DEFAULT_WASM_DIR, 'extractors'),
  plannerWasmPath: join(DEFAULT_WASM_DIR, 'planner')
};

class PsychoSymbolicMcpServer {
  private server: Server;
  private tools: PsychoSymbolicMcpTools;
  private initialized = false;

  constructor() {
    this.tools = new PsychoSymbolicMcpTools();
    this.server = new Server(
      {
        name: 'psycho-symbolic-reasoner',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.setupHandlers();
  }

  private setupHandlers(): void {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: this.tools.getTools(),
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;
      return await this.tools.callTool({ name, arguments: args });
    });
  }

  public async initialize(config = DEFAULT_CONFIG): Promise<void> {
    if (this.initialized) {
      return;
    }

    // Validate WASM directories exist
    const missingDirs: string[] = [];
    if (!existsSync(config.graphReasonerWasmPath)) {
      missingDirs.push(config.graphReasonerWasmPath);
    }
    if (!existsSync(config.textExtractorWasmPath)) {
      missingDirs.push(config.textExtractorWasmPath);
    }
    if (!existsSync(config.plannerWasmPath)) {
      missingDirs.push(config.plannerWasmPath);
    }

    if (missingDirs.length > 0) {
      console.error('Missing WASM directories:', missingDirs);
      console.error('Please build the WASM modules first using:');
      console.error('  npm run build:wasm-pack');
      process.exit(1);
    }

    try {
      await this.tools.initialize(config);
      this.initialized = true;
      console.log('✓ Psycho-Symbolic Reasoner MCP server initialized successfully');
      console.log('✓ All WASM modules loaded');
      console.log('✓ Available tools:', this.tools.getTools().length);
    } catch (error) {
      console.error('Failed to initialize MCP server:', error);
      process.exit(1);
    }
  }

  public async start(): Promise<void> {
    if (!this.initialized) {
      await this.initialize();
    }

    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    
    console.log('🚀 Psycho-Symbolic Reasoner MCP server running on stdio');
  }

  public async stop(): Promise<void> {
    try {
      this.tools.cleanup();
      await this.server.close();
      console.log('✓ MCP server stopped cleanly');
    } catch (error) {
      console.error('Error stopping server:', error);
    }
  }

  public getHealthStatus() {
    return {
      initialized: this.initialized,
      toolsHealth: this.tools.getHealthStatus()
    };
  }
}

// CLI interface
if (import.meta.url === `file://${process.argv[1]}`) {
  const server = new PsychoSymbolicMcpServer();

  // Handle CLI arguments
  const args = process.argv.slice(2);
  const command = args[0];

  switch (command) {
    case 'start':
      server.start().catch((error) => {
        console.error('Failed to start server:', error);
        process.exit(1);
      });
      break;

    case 'health':
      server.initialize().then(() => {
        const health = server.getHealthStatus();
        console.log(JSON.stringify(health, null, 2));
        process.exit(0);
      }).catch((error) => {
        console.error('Health check failed:', error);
        process.exit(1);
      });
      break;

    case 'tools':
      server.initialize().then(() => {
        const tools = server['tools'].getTools();
        console.log('Available tools:');
        tools.forEach((tool, index) => {
          console.log(`${index + 1}. ${tool.name} - ${tool.description}`);
        });
        process.exit(0);
      }).catch((error) => {
        console.error('Failed to list tools:', error);
        process.exit(1);
      });
      break;

    case 'help':
    case '--help':
    case '-h':
      console.log(`
Psycho-Symbolic Reasoner MCP Server

Usage:
  npm start                 Start the MCP server
  npm run health           Check server health status
  npm run tools            List available tools
  npm run help             Show this help message

For development:
  npm run build:wasm-pack  Build WASM modules
  npm run dev              Start in development mode
  npm test                 Run tests

Environment Variables:
  WASM_DIR                 Directory containing WASM files (default: ./wasm)
  LOG_LEVEL                Logging level (debug, info, warn, error)
  MAX_INSTANCES            Maximum WASM instances (default: 100)
  INSTANCE_TIMEOUT         Instance timeout in ms (default: 300000)
`);
      process.exit(0);
      break;

    default:
      // Default behavior: start the server
      server.start().catch((error) => {
        console.error('Failed to start server:', error);
        process.exit(1);
      });
      break;
  }

  // Graceful shutdown
  process.on('SIGINT', async () => {
    console.log('\n🛑 Received SIGINT, shutting down gracefully...');
    await server.stop();
    process.exit(0);
  });

  process.on('SIGTERM', async () => {
    console.log('\n🛑 Received SIGTERM, shutting down gracefully...');
    await server.stop();
    process.exit(0);
  });

  process.on('uncaughtException', async (error) => {
    console.error('❌ Uncaught exception:', error);
    await server.stop();
    process.exit(1);
  });

  process.on('unhandledRejection', async (reason, promise) => {
    console.error('❌ Unhandled rejection at:', promise, 'reason:', reason);
    await server.stop();
    process.exit(1);
  });
}

export { PsychoSymbolicMcpServer, PsychoSymbolicMcpTools };
export * from './types/index';
export * from './schemas/index';
export * from './wrappers/graph-reasoner';
export * from './wrappers/text-extractor';
export * from './wrappers/planner';
export * from './wasm/loader';
export * from './wasm/memory-manager';