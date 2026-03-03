#!/usr/bin/env node

import { Command } from 'commander';
import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { CLIArgs } from '../types/config.js';
import { Logger } from '../utils/logger.js';
import { ConfigLoader } from '../utils/config-loader.js';
import { MCPServer } from '../mcp/server.js';

// Get package.json for version info
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const packagePath = resolve(__dirname, '../../../package.json');
const packageJson = JSON.parse(readFileSync(packagePath, 'utf-8'));

/**
 * CLI Application class
 */
class PsychoSymbolicReasonerCLI {
  private program: Command;
  private server?: MCPServer;
  private isShuttingDown: boolean = false;

  constructor() {
    this.program = new Command();
    this.setupCommands();
    this.setupGracefulShutdown();
  }

  /**
   * Setup CLI commands and options
   */
  private setupCommands(): void {
    this.program
      .name('psycho-symbolic-reasoner')
      .description('Advanced psycho-symbolic reasoning system with graph-based knowledge representation')
      .version(packageJson.version);

    // Main start command
    this.program
      .command('start', { isDefault: true })
      .description('Start the psycho-symbolic reasoner MCP server')
      .option('-k, --knowledge-base <file>', 'Load initial graph data from file')
      .option('-t, --transport <type>', 'Transport type (stdio, sse, http)', 'stdio')
      .option('-p, --port <number>', 'Port number for HTTP/SSE transport', '3000')
      .option('-H, --host <host>', 'Host address for HTTP/SSE transport', 'localhost')
      .option('-c, --config <file>', 'Configuration file path')
      .option('-l, --log-level <level>', 'Log level (error, warn, info, debug, silly)', 'info')
      .option('-f, --log-file <file>', 'Log file path')
      .option('-v, --verbose', 'Enable verbose logging (debug level)')
      .option('-q, --quiet', 'Disable console logging')
      .action(async (options) => {
        await this.startServer(options);
      });

    // Config command
    this.program
      .command('config')
      .description('Configuration management')
      .option('-g, --generate', 'Generate sample configuration file')
      .option('-v, --validate <file>', 'Validate configuration file')
      .action(async (options) => {
        await this.handleConfigCommand(options);
      });

    // Health check command
    this.program
      .command('health')
      .description('Check server health (requires running server)')
      .option('-u, --url <url>', 'Server URL', 'http://localhost:3000')
      .option('-d, --detailed', 'Show detailed health information')
      .action(async (options) => {
        await this.healthCheck(options);
      });

    // Version command
    this.program
      .command('version')
      .description('Show version information')
      .action(() => {
        console.log(`psycho-symbolic-reasoner v${packageJson.version}`);
        console.log(`Node.js ${process.version}`);
        console.log(`Platform: ${process.platform} ${process.arch}`);
      });
  }

  /**
   * Start the MCP server
   */
  private async startServer(options: any): Promise<void> {
    try {
      // Parse CLI arguments
      const cliArgs = CLIArgs.parse({
        knowledgeBase: options.knowledgeBase,
        transport: options.transport,
        port: options.port ? parseInt(options.port, 10) : undefined,
        host: options.host,
        config: options.config,
        logLevel: options.logLevel,
        logFile: options.logFile,
        verbose: options.verbose,
        quiet: options.quiet
      });

      // Load configuration
      const config = await ConfigLoader.loadConfig(cliArgs);

      // Initialize logger
      Logger.initialize(config.logging);

      Logger.info('Starting Psycho-Symbolic Reasoner', {
        version: packageJson.version,
        transport: config.server.transport,
        port: config.server.port,
        host: config.server.host
      });

      // Initialize and start MCP server
      this.server = new MCPServer(config);
      await this.server.start();

      // Keep process alive for non-stdio transports
      if (config.server.transport !== 'stdio') {
        Logger.info('Server running. Press Ctrl+C to stop.');
        // Keep process alive
        await new Promise(() => {});
      }

    } catch (error) {
      try {
        Logger.error('Failed to start server', error);
      } catch {
        console.error('Failed to start server:', error);
      }
      process.exit(1);
    }
  }

  /**
   * Handle config command
   */
  private async handleConfigCommand(options: any): Promise<void> {
    try {
      if (options.generate) {
        const sampleConfig = ConfigLoader.generateSampleConfig();
        console.log('# Sample configuration file');
        console.log('# Save as psycho-symbolic-reasoner.config.json');
        console.log(sampleConfig);
        return;
      }

      if (options.validate) {
        try {
          const config = await ConfigLoader.loadConfig(CLIArgs.parse({
            config: options.validate,
            help: false,
            version: false,
            verbose: false,
            quiet: false
          }));
          console.log('✅ Configuration is valid');
          console.log('Validated configuration:');
          console.log(JSON.stringify(config, null, 2));
        } catch (error) {
          console.error('❌ Configuration validation failed:', error instanceof Error ? error.message : error);
          process.exit(1);
        }
        return;
      }

      // Show help if no options provided
      this.program.commands.find(cmd => cmd.name() === 'config')?.help();

    } catch (error) {
      console.error('❌ Configuration error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  }

  /**
   * Perform health check
   */
  private async healthCheck(options: any): Promise<void> {
    try {
      const url = options.url.replace(/\/$/, '');
      const response = await fetch(`${url}/health`);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const health = await response.json();

      console.log('✅ Server is healthy');
      console.log('Health status:');
      console.log(JSON.stringify(health, null, 2));

    } catch (error) {
      console.error('❌ Health check failed:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  }

  /**
   * Setup graceful shutdown handling
   */
  private setupGracefulShutdown(): void {
    const shutdown = async (signal: string) => {
      if (this.isShuttingDown) {
        return;
      }

      this.isShuttingDown = true;
      Logger.info(`Received ${signal}, shutting down gracefully...`);

      try {
        // Stop the server
        if (this.server) {
          await this.server.stop();
        }

        // Close logger
        await Logger.close();

        Logger.info('Shutdown complete');
        process.exit(0);
      } catch (error) {
        Logger.error('Error during shutdown', error);
        process.exit(1);
      }
    };

    // Handle various termination signals
    process.on('SIGINT', () => shutdown('SIGINT'));
    process.on('SIGTERM', () => shutdown('SIGTERM'));
    process.on('SIGQUIT', () => shutdown('SIGQUIT'));

    // Handle uncaught exceptions
    process.on('uncaughtException', (error) => {
      Logger.error('Uncaught exception', error);
      shutdown('uncaughtException');
    });

    // Handle unhandled promise rejections
    process.on('unhandledRejection', (reason) => {
      Logger.error('Unhandled promise rejection', reason);
      shutdown('unhandledRejection');
    });
  }

  /**
   * Run the CLI application
   */
  public async run(argv: string[] = process.argv): Promise<void> {
    try {
      await this.program.parseAsync(argv);
    } catch (error) {
      console.error('CLI error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  }
}

// Export for testing
export { PsychoSymbolicReasonerCLI };

// Run if this is the main module
// Check if we're being run directly (not imported)
// This works with npx, direct execution, and symlinks
const isMainModule = process.argv[1] && (
  import.meta.url === `file://${process.argv[1]}` ||
  import.meta.url.endsWith('/cli/index.js') ||
  process.argv[1].endsWith('psycho-symbolic-reasoner') ||
  process.argv[1].endsWith('psycho-reasoner') ||
  process.argv[1].endsWith('psr')
);

if (isMainModule) {
  const cli = new PsychoSymbolicReasonerCLI();
  cli.run().catch((error) => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}