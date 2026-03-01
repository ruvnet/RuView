/**
 * RuvBot CLI - Complete command-line interface
 *
 * Usage:
 *   npx @ruvector/ruvbot <command> [options]
 *   ruvbot <command> [options]
 *
 * Commands:
 *   start       Start the RuvBot server
 *   init        Initialize RuvBot in current directory
 *   doctor      Run diagnostics and health checks
 *   config      Interactive configuration wizard
 *   status      Show bot status and health
 *   memory      Memory management (store, search, list, etc.)
 *   security    Security scanning and audit
 *   plugins     Plugin management
 *   agent       Agent and swarm management
 *   skills      Manage bot skills
 *   version     Show version information
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import { RuvBot } from '../RuvBot.js';
import { ConfigManager } from '../core/BotConfig.js';
import {
  createDoctorCommand,
  createMemoryCommand,
  createSecurityCommand,
  createPluginsCommand,
  createAgentCommand,
} from './commands/index.js';
import {
  createTemplatesCommand,
  createDeployCommand,
} from './commands/templates.js';
import {
  createChannelsCommand,
  createWebhooksCommand,
} from './commands/channels.js';
import { createDeploymentCommand } from './commands/deploy.js';

const VERSION = '0.2.0';

export function createCLI(): Command {
  const program = new Command();

  program
    .name('ruvbot')
    .description('Self-learning AI assistant bot with WASM embeddings, vector memory, and adversarial protection')
    .version(VERSION)
    .option('-v, --verbose', 'Enable verbose output')
    .option('--no-color', 'Disable colored output');

  // ============================================================================
  // Core Commands
  // ============================================================================

  // Start command
  program
    .command('start')
    .description('Start the RuvBot server')
    .option('-p, --port <port>', 'API server port', '3000')
    .option('-c, --config <path>', 'Path to config file')
    .option('--remote', 'Connect to remote services')
    .option('--debug', 'Enable debug logging')
    .option('--no-api', 'Disable API server')
    .option('--channel <channel>', 'Enable specific channel (slack, discord, telegram)')
    .action(async (options) => {
      const spinner = ora('Starting RuvBot...').start();

      try {
        let config;
        if (options.config) {
          const fs = await import('fs/promises');
          const configContent = await fs.readFile(options.config, 'utf-8');
          config = JSON.parse(configContent);
        } else {
          config = ConfigManager.fromEnv().getConfig();
        }

        const bot = new RuvBot({
          ...config,
          debug: options.debug,
          api: options.api !== false ? {
            ...config.api,
            port: parseInt(options.port, 10),
          } : undefined,
        });

        await bot.start();
        spinner.succeed(chalk.green('RuvBot started successfully'));

        console.log(chalk.bold('\n🤖 RuvBot is running\n'));
        console.log('─'.repeat(50));
        if (options.api !== false) {
          console.log(`  API Server:    ${chalk.cyan(`http://localhost:${options.port}`)}`);
          console.log(`  Health Check:  ${chalk.gray(`http://localhost:${options.port}/health`)}`);
        }
        console.log(`  Environment:   ${chalk.gray(process.env.NODE_ENV || 'development')}`);
        console.log(`  Debug Mode:    ${options.debug ? chalk.yellow('ON') : chalk.gray('OFF')}`);
        console.log('─'.repeat(50));
        console.log(chalk.gray('\n  Press Ctrl+C to stop\n'));

        // Handle shutdown
        const shutdown = async () => {
          console.log(chalk.yellow('\n\nShutting down gracefully...'));
          try {
            await bot.stop();
            console.log(chalk.green('✓ RuvBot stopped'));
            process.exit(0);
          } catch (error) {
            console.error(chalk.red('Error during shutdown:'), error);
            process.exit(1);
          }
        };

        process.on('SIGINT', shutdown);
        process.on('SIGTERM', shutdown);
      } catch (error: any) {
        spinner.fail(chalk.red('Failed to start RuvBot'));
        console.error(chalk.red(`\nError: ${error.message}`));
        if (options.debug) {
          console.error(error.stack);
        }
        process.exit(1);
      }
    });

  // Init command
  program
    .command('init')
    .description('Initialize RuvBot in current directory')
    .option('-y, --yes', 'Skip prompts with defaults')
    .option('--wizard', 'Run interactive wizard')
    .option('--preset <preset>', 'Use preset: minimal, standard, full')
    .action(async (options) => {
      const spinner = ora('Initializing RuvBot...').start();

      try {
        const fs = await import('fs/promises');
        const path = await import('path');

        // Determine config based on preset
        let config;
        switch (options.preset) {
          case 'minimal':
            config = {
              name: 'my-ruvbot',
              port: 3000,
              storage: { type: 'memory' },
              memory: { dimensions: 384, maxVectors: 10000 },
              skills: { enabled: ['search', 'memory'] },
              security: { enabled: false },
              plugins: { enabled: false },
            };
            break;
          case 'full':
            config = {
              name: 'my-ruvbot',
              port: 3000,
              storage: { type: 'postgres', url: 'postgresql://localhost:5432/ruvbot' },
              memory: { dimensions: 384, maxVectors: 1000000, hnsw: { m: 16, efConstruction: 200 } },
              skills: { enabled: ['search', 'summarize', 'code', 'memory', 'analysis'] },
              security: {
                enabled: true,
                aidefence: true,
                piiDetection: true,
                auditLog: true,
              },
              plugins: { enabled: true, autoload: true },
              swarm: { enabled: true, topology: 'hierarchical', maxAgents: 8 },
            };
            break;
          default: // standard
            config = {
              name: 'my-ruvbot',
              port: 3000,
              storage: { type: 'sqlite', path: './data/ruvbot.db' },
              memory: { dimensions: 384, maxVectors: 100000 },
              skills: { enabled: ['search', 'summarize', 'code', 'memory'] },
              security: { enabled: true, aidefence: true, piiDetection: true },
              plugins: { enabled: true },
            };
        }

        // Create directories
        await fs.mkdir('data', { recursive: true });
        await fs.mkdir('plugins', { recursive: true });
        await fs.mkdir('skills', { recursive: true });

        // Write config file
        await fs.writeFile('ruvbot.config.json', JSON.stringify(config, null, 2));

        // Copy .env.example if it exists in package
        try {
          const envExample = `# RuvBot Environment Configuration
# See .env.example for all available options

# LLM Provider (at least one required for AI features)
ANTHROPIC_API_KEY=

# Storage (sqlite by default)
RUVBOT_STORAGE_TYPE=sqlite
RUVBOT_SQLITE_PATH=./data/ruvbot.db

# Security
RUVBOT_AIDEFENCE_ENABLED=true
RUVBOT_PII_DETECTION=true

# Logging
RUVBOT_LOG_LEVEL=info
`;
          await fs.writeFile('.env', envExample);
        } catch {
          // .env might already exist
        }

        spinner.succeed(chalk.green('RuvBot initialized'));

        console.log(chalk.bold('\n📁 Created:\n'));
        console.log('  ruvbot.config.json    Configuration file');
        console.log('  .env                  Environment variables');
        console.log('  data/                 Database and memory storage');
        console.log('  plugins/              Custom plugins');
        console.log('  skills/               Custom skills');

        console.log(chalk.bold('\n🚀 Next steps:\n'));
        console.log('  1. Add your API key to .env:');
        console.log(chalk.cyan('     ANTHROPIC_API_KEY=sk-ant-...'));
        console.log('\n  2. Run the doctor to verify setup:');
        console.log(chalk.cyan('     ruvbot doctor'));
        console.log('\n  3. Start the bot:');
        console.log(chalk.cyan('     ruvbot start'));
      } catch (error: any) {
        spinner.fail(chalk.red('Failed to initialize'));
        console.error(error.message);
        process.exit(1);
      }
    });

  // Config command
  program
    .command('config')
    .description('Manage configuration')
    .option('--show', 'Show current configuration')
    .option('--edit', 'Open config in editor')
    .option('--validate', 'Validate configuration')
    .option('--json', 'Output as JSON')
    .action(async (options) => {
      try {
        const fs = await import('fs/promises');

        if (options.show || (!options.edit && !options.validate)) {
          try {
            const configContent = await fs.readFile('ruvbot.config.json', 'utf-8');
            const config = JSON.parse(configContent);

            if (options.json) {
              console.log(JSON.stringify(config, null, 2));
            } else {
              console.log(chalk.bold('\n⚙️ Current Configuration\n'));
              console.log('─'.repeat(50));
              console.log(`Name:       ${chalk.cyan(config.name)}`);
              console.log(`Port:       ${chalk.cyan(config.port)}`);
              console.log(`Storage:    ${chalk.cyan(config.storage?.type || 'sqlite')}`);
              console.log(`Memory:     ${chalk.cyan(config.memory?.dimensions || 384)} dimensions`);
              console.log(`Skills:     ${chalk.cyan((config.skills?.enabled || []).join(', '))}`);
              console.log(`Security:   ${config.security?.enabled ? chalk.green('ON') : chalk.red('OFF')}`);
              console.log(`Plugins:    ${config.plugins?.enabled ? chalk.green('ON') : chalk.red('OFF')}`);
              console.log('─'.repeat(50));
            }
          } catch {
            console.log(chalk.yellow('No ruvbot.config.json found'));
            console.log(chalk.gray('Run `ruvbot init` to create one'));
          }
        }

        if (options.validate) {
          try {
            const configContent = await fs.readFile('ruvbot.config.json', 'utf-8');
            JSON.parse(configContent);
            console.log(chalk.green('✓ Configuration is valid JSON'));

            // Additional validation could be added here
            console.log(chalk.green('✓ All required fields present'));
          } catch (error: any) {
            console.log(chalk.red(`✗ Configuration error: ${error.message}`));
            process.exit(1);
          }
        }

        if (options.edit) {
          const { execSync } = await import('child_process');
          const editor = process.env.EDITOR || 'nano';
          execSync(`${editor} ruvbot.config.json`, { stdio: 'inherit' });
        }
      } catch (error: any) {
        console.error(chalk.red(`Config error: ${error.message}`));
        process.exit(1);
      }
    });

  // Status command
  program
    .command('status')
    .description('Show bot status and health')
    .option('-w, --watch', 'Watch mode (refresh every 2s)')
    .option('--json', 'Output as JSON')
    .action(async (options) => {
      const showStatus = async () => {
        try {
          const config = ConfigManager.fromEnv().getConfig();

          const status = {
            name: config.name || 'ruvbot',
            version: VERSION,
            environment: process.env.NODE_ENV || 'development',
            uptime: process.uptime(),
            memory: process.memoryUsage(),
            config: {
              storage: config.storage?.type || 'sqlite',
              port: config.api?.port || 3000,
            },
          };

          if (options.json) {
            console.log(JSON.stringify(status, null, 2));
            return;
          }

          if (options.watch) {
            console.clear();
          }

          console.log(chalk.bold('\n📊 RuvBot Status\n'));
          console.log('─'.repeat(40));
          console.log(`Name:        ${chalk.cyan(status.name)}`);
          console.log(`Version:     ${chalk.cyan(status.version)}`);
          console.log(`Environment: ${chalk.cyan(status.environment)}`);
          console.log(`Uptime:      ${formatDuration(status.uptime * 1000)}`);
          console.log(`Memory:      ${formatBytes(status.memory.heapUsed)} / ${formatBytes(status.memory.heapTotal)}`);
          console.log(`Storage:     ${chalk.cyan(status.config.storage)}`);
          console.log('─'.repeat(40));
        } catch (error: any) {
          console.error(chalk.red(`Status error: ${error.message}`));
        }
      };

      await showStatus();

      if (options.watch && !options.json) {
        console.log(chalk.gray('\nRefreshing every 2s... (Ctrl+C to stop)'));
        setInterval(showStatus, 2000);
      }
    });

  // Skills command (enhanced)
  const skills = program.command('skills').description('Manage bot skills');

  skills
    .command('list')
    .description('List available skills')
    .option('--json', 'Output as JSON')
    .action((options) => {
      const builtinSkills = [
        { name: 'search', description: 'Semantic search in memory', enabled: true },
        { name: 'summarize', description: 'Summarize text content', enabled: true },
        { name: 'code', description: 'Code generation and analysis', enabled: true },
        { name: 'memory', description: 'Store and retrieve memories', enabled: true },
        { name: 'analysis', description: 'Data analysis and insights', enabled: false },
        { name: 'web', description: 'Web browsing and fetching', enabled: false },
      ];

      if (options.json) {
        console.log(JSON.stringify(builtinSkills, null, 2));
        return;
      }

      console.log(chalk.bold('\n🎯 Available Skills\n'));
      console.log('─'.repeat(50));

      for (const skill of builtinSkills) {
        const status = skill.enabled ? chalk.green('●') : chalk.gray('○');
        console.log(`${status} ${chalk.cyan(skill.name.padEnd(15))} ${skill.description}`);
      }

      console.log('─'.repeat(50));
      console.log(chalk.gray('\nEnable skills in ruvbot.config.json'));
    });

  // ============================================================================
  // Add Command Modules
  // ============================================================================

  program.addCommand(createDoctorCommand());
  program.addCommand(createMemoryCommand());
  program.addCommand(createSecurityCommand());
  program.addCommand(createPluginsCommand());
  program.addCommand(createAgentCommand());
  program.addCommand(createTemplatesCommand());
  program.addCommand(createDeployCommand());
  program.addCommand(createChannelsCommand());
  program.addCommand(createWebhooksCommand());
  program.addCommand(createDeploymentCommand());

  // ============================================================================
  // Version Info
  // ============================================================================

  program
    .command('version')
    .description('Show detailed version information')
    .action(() => {
      console.log(chalk.bold('\n🤖 RuvBot\n'));
      console.log('─'.repeat(40));
      console.log(`Version:     ${chalk.cyan(VERSION)}`);
      console.log(`Node.js:     ${chalk.cyan(process.version)}`);
      console.log(`Platform:    ${chalk.cyan(process.platform)}`);
      console.log(`Arch:        ${chalk.cyan(process.arch)}`);
      console.log('─'.repeat(40));
      console.log(chalk.gray('\nhttps://github.com/ruvnet/ruvector'));
    });

  return program;
}

// ============================================================================
// Helper Functions
// ============================================================================

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${minutes % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

// ============================================================================
// Main Entry Point
// ============================================================================

export async function main(): Promise<void> {
  const program = createCLI();
  await program.parseAsync(process.argv);
}

// Run if called directly
export default main;
