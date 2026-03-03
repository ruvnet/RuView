/**
 * GOAP MCP Server
 * Main Model Context Protocol server for GOAP planning system
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

import { GoapMCPTools } from './tools.js';
import { PluginRegistry, PluginLoader, costTrackingPlugin, performanceMonitoringPlugin, loggingPlugin, queryDiversificationPlugin } from '../core/plugin-system.js';
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

export class GoapMCPServer {
  private server: Server;
  private goapTools: GoapMCPTools;
  private pluginRegistry: PluginRegistry;

  constructor() {
    this.server = new Server(
      {
        name: 'goalie',
        version: '1.3.1',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.goapTools = new GoapMCPTools();
    this.pluginRegistry = new PluginRegistry();

    this.setupHandlers();
  }

  async initialize(): Promise<void> {
    // Register built-in plugins
    this.pluginRegistry.register(costTrackingPlugin);
    this.pluginRegistry.register(performanceMonitoringPlugin);
    this.pluginRegistry.register(loggingPlugin);
    this.pluginRegistry.register(queryDiversificationPlugin);

    // Load external plugins if specified
    await this.loadExternalPlugins();

    // Initialize GOAP tools
    await this.goapTools.initialize();

    console.log('🚀 GOAP MCP Server initialized successfully');
    console.log(`📦 Registered plugins: ${this.pluginRegistry.getPlugins().length}`);
  }

  private async loadExternalPlugins(): Promise<void> {
    // Load plugins from environment variables
    const pluginPaths = process.env.GOAP_PLUGINS?.split(',').map(p => p.trim()) || [];
    const extensionPaths = process.env.GOAP_EXTENSIONS?.split(',').map(p => p.trim()) || [];

    try {
      if (pluginPaths.length > 0) {
        const plugins = await PluginLoader.loadFromFiles(pluginPaths);
        plugins.forEach(plugin => this.pluginRegistry.register(plugin));
        console.log(`📦 Loaded ${plugins.length} external plugins`);
      }

      if (extensionPaths.length > 0) {
        console.log(`📦 Loading ${extensionPaths.length} extensions (not implemented yet)`);
      }
    } catch (error) {
      console.warn('⚠️ Failed to load some external plugins:', error);
    }
  }

  private setupHandlers(): void {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      const tools = this.goapTools.getTools();

      return {
        tools: tools.map(tool => ({
          name: tool.name,
          description: tool.description,
          inputSchema: tool.inputSchema
        }))
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        let result;

        switch (name) {
          case 'goap.search':
            result = await this.goapTools.executeGoapSearch(args as any);
            break;

          case 'goap.plan.explain':
            result = await this.goapTools.executePlanExplain(args);
            break;

          case 'search.raw':
            result = await this.goapTools.executeRawSearch(args);
            break;

          // Plugin management tools
          case 'plugin.list':
            result = await this.handlePluginList();
            break;

          case 'plugin.enable':
            result = await this.handlePluginEnable(args);
            break;

          case 'plugin.disable':
            result = await this.handlePluginDisable(args);
            break;

          case 'plugin.info':
            result = await this.handlePluginInfo(args);
            break;

          // Advanced reasoning tools
          case 'reasoning.chain_of_thought':
            result = await this.goapTools.executeToolByName('reasoning.chain_of_thought', args);
            break;

          case 'reasoning.self_consistency':
            result = await this.goapTools.executeToolByName('reasoning.self_consistency', args);
            break;

          case 'reasoning.anti_hallucination':
            result = await this.goapTools.executeToolByName('reasoning.anti_hallucination', args);
            break;

          case 'reasoning.agentic_research':
            result = await this.goapTools.executeToolByName('reasoning.agentic_research', args);
            break;

          default:
            throw new Error(`Unknown tool: ${name}`);
        }

        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(result, null, 2)
            }
          ]
        };

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';

        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify({
                error: errorMessage,
                tool: name,
                timestamp: new Date().toISOString()
              }, null, 2)
            }
          ],
          isError: true
        };
      }
    });
  }

  // Plugin management handlers
  private async handlePluginList(): Promise<any> {
    return { plugins: this.pluginRegistry.listPlugins() };
  }

  private async handlePluginEnable(args: any): Promise<any> {
    return this.pluginRegistry.enablePlugin(args.name);
  }

  private async handlePluginDisable(args: any): Promise<any> {
    return this.pluginRegistry.disablePlugin(args.name);
  }

  private async handlePluginInfo(args: any): Promise<any> {
    return this.pluginRegistry.getPluginInfo(args.name);
  }


  async run(): Promise<void> {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);

    console.error('🎯 GOAP MCP Server running on stdio');
    console.error('🧠 Enhanced with Advanced Reasoning Engine');
    console.error('🔌 Plugin system active with 11 tools');
    console.error('📁 File output to .research/ with pagination');
    console.error('🎪 Ready for GOAP planning!');

    // Keep the process alive to handle MCP requests
    process.on('SIGINT', () => {
      console.error('🛑 GOAP MCP Server shutting down...');
      process.exit(0);
    });

    process.on('SIGTERM', () => {
      console.error('🛑 GOAP MCP Server shutting down...');
      process.exit(0);
    });

    // Keep the server running
    await new Promise<void>((resolve, reject) => {
      // Handle process termination gracefully
      process.on('SIGTERM', () => {
        console.error('🛑 Received SIGTERM, shutting down gracefully...');
        resolve();
      });

      process.on('SIGINT', () => {
        console.error('🛑 Received SIGINT, shutting down gracefully...');
        resolve();
      });

      // Keep alive indefinitely unless terminated
    });
  }
}

// Error handling
process.on('uncaughtException', (error) => {
  console.error('💥 Uncaught exception:', error);
  process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('💥 Unhandled rejection at:', promise, 'reason:', reason);
  process.exit(1);
});

// Graceful shutdown
process.on('SIGINT', () => {
  console.error('👋 Shutting down GOAP MCP Server...');
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.error('👋 Terminating GOAP MCP Server...');
  process.exit(0);
});