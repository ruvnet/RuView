/**
 * Plugins Command - Plugin management
 *
 * Commands:
 *   plugins list      List installed plugins
 *   plugins create    Create a new plugin scaffold
 *   plugins info      Show plugin system information
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import { PluginManifestSchema } from '../../plugins/PluginManager.js';

export function createPluginsCommand(): Command {
  const plugins = new Command('plugins');
  plugins.description('Plugin management commands');

  // List command
  plugins
    .command('list')
    .description('List installed plugins')
    .option('--json', 'Output as JSON')
    .action(async (options) => {
      try {
        const fs = await import('fs/promises');
        const path = await import('path');
        const pluginsDir = process.env.RUVBOT_PLUGINS_DIR || './plugins';

        let pluginList: Array<{ name: string; version: string; description: string; enabled: boolean }> = [];

        try {
          const files = await fs.readdir(pluginsDir);

          for (const file of files) {
            const pluginPath = path.join(pluginsDir, file);
            const stat = await fs.stat(pluginPath);

            if (stat.isDirectory()) {
              try {
                const manifestPath = path.join(pluginPath, 'plugin.json');
                const manifestContent = await fs.readFile(manifestPath, 'utf-8');
                const manifest = JSON.parse(manifestContent);

                pluginList.push({
                  name: manifest.name || file,
                  version: manifest.version || '0.0.0',
                  description: manifest.description || 'No description',
                  enabled: true, // Would need state tracking for real enabled/disabled
                });
              } catch {
                // Not a valid plugin
              }
            }
          }
        } catch {
          // Plugins directory doesn't exist
        }

        if (options.json) {
          console.log(JSON.stringify(pluginList, null, 2));
          return;
        }

        if (pluginList.length === 0) {
          console.log(chalk.yellow('No plugins installed'));
          console.log(chalk.gray('Create one with: ruvbot plugins create my-plugin'));
          return;
        }

        console.log(chalk.bold(`\n🔌 Installed Plugins (${pluginList.length})\n`));
        console.log('─'.repeat(60));

        for (const plugin of pluginList) {
          const status = plugin.enabled ? chalk.green('●') : chalk.gray('○');
          console.log(`${status} ${chalk.cyan(plugin.name.padEnd(25))} v${plugin.version.padEnd(10)}`);
          if (plugin.description) {
            console.log(chalk.gray(`  ${plugin.description}`));
          }
        }

        console.log('─'.repeat(60));
      } catch (error: any) {
        console.error(chalk.red(`Failed to list plugins: ${error.message}`));
        process.exit(1);
      }
    });

  // Create command
  plugins
    .command('create')
    .description('Create a new plugin scaffold')
    .argument('<name>', 'Plugin name')
    .option('-d, --dir <directory>', 'Target directory', './plugins')
    .option('--typescript', 'Use TypeScript')
    .action(async (name, options) => {
      const spinner = ora(`Creating plugin ${name}...`).start();

      try {
        const fs = await import('fs/promises');
        const path = await import('path');

        const pluginDir = path.join(options.dir, name);
        await fs.mkdir(pluginDir, { recursive: true });

        // Create plugin.json
        const manifest = {
          name,
          version: '1.0.0',
          description: `${name} plugin for RuvBot`,
          author: 'Your Name',
          license: 'MIT',
          main: options.typescript ? 'dist/index.js' : 'index.js',
          permissions: ['memory:read', 'memory:write'],
          hooks: {
            onLoad: 'initialize',
            onUnload: 'shutdown',
            onMessage: 'handleMessage',
          },
        };

        await fs.writeFile(path.join(pluginDir, 'plugin.json'), JSON.stringify(manifest, null, 2));

        // Create main file
        const mainContent = options.typescript
          ? `/**
 * ${name} Plugin for RuvBot
 */

import type { PluginContext, PluginMessage, PluginResponse } from '@ruvector/ruvbot';

export async function initialize(context: PluginContext): Promise<void> {
  console.log('${name} plugin initialized');

  // Access plugin memory
  await context.memory.set('initialized', true);
}

export async function handleMessage(
  message: PluginMessage,
  context: PluginContext
): Promise<PluginResponse | null> {
  // Check if message is relevant to this plugin
  if (message.content.includes('${name}')) {
    return {
      handled: true,
      response: 'Hello from ${name} plugin!',
    };
  }

  // Return null to let other handlers process
  return null;
}

export async function shutdown(context: PluginContext): Promise<void> {
  console.log('${name} plugin shutting down');
}
`
          : `/**
 * ${name} Plugin for RuvBot
 */

export async function initialize(context) {
  console.log('${name} plugin initialized');

  // Access plugin memory
  await context.memory.set('initialized', true);
}

export async function handleMessage(message, context) {
  // Check if message is relevant to this plugin
  if (message.content.includes('${name}')) {
    return {
      handled: true,
      response: 'Hello from ${name} plugin!',
    };
  }

  // Return null to let other handlers process
  return null;
}

export async function shutdown(context) {
  console.log('${name} plugin shutting down');
}
`;

        const mainFile = options.typescript ? 'src/index.ts' : 'index.js';
        if (options.typescript) {
          await fs.mkdir(path.join(pluginDir, 'src'), { recursive: true });
        }
        await fs.writeFile(path.join(pluginDir, mainFile), mainContent);

        // Create package.json
        const pkgJson = {
          name: `ruvbot-plugin-${name}`,
          version: '1.0.0',
          type: 'module',
          main: options.typescript ? 'dist/index.js' : 'index.js',
          scripts: options.typescript
            ? {
                build: 'tsc',
                dev: 'tsc -w',
              }
            : {},
          peerDependencies: {
            '@ruvector/ruvbot': '^0.1.0',
          },
        };

        await fs.writeFile(path.join(pluginDir, 'package.json'), JSON.stringify(pkgJson, null, 2));

        // Create tsconfig if typescript
        if (options.typescript) {
          const tsconfig = {
            compilerOptions: {
              target: 'ES2022',
              module: 'ESNext',
              moduleResolution: 'node',
              outDir: 'dist',
              declaration: true,
              strict: true,
              esModuleInterop: true,
              skipLibCheck: true,
            },
            include: ['src/**/*'],
          };
          await fs.writeFile(path.join(pluginDir, 'tsconfig.json'), JSON.stringify(tsconfig, null, 2));
        }

        spinner.succeed(chalk.green(`Plugin created at ${pluginDir}`));
        console.log(chalk.gray('\nNext steps:'));
        console.log(`  cd ${pluginDir}`);
        if (options.typescript) {
          console.log('  npm install');
          console.log('  npm run build');
        }
        console.log(chalk.gray('\nThe plugin will be auto-loaded when RuvBot starts.'));
      } catch (error: any) {
        spinner.fail(chalk.red(`Create failed: ${error.message}`));
        process.exit(1);
      }
    });

  // Info command
  plugins
    .command('info')
    .description('Show plugin system information')
    .action(async () => {
      console.log(chalk.bold('\n🔌 RuvBot Plugin System\n'));
      console.log('─'.repeat(50));
      console.log(chalk.cyan('Features:'));
      console.log('  • Local plugin discovery and auto-loading');
      console.log('  • Plugin lifecycle management');
      console.log('  • Permission-based sandboxing');
      console.log('  • Hot-reload support (development)');
      console.log('  • IPFS registry integration (optional)');
      console.log('');
      console.log(chalk.cyan('Available Permissions:'));
      const permissions = [
        'memory:read     - Read from memory store',
        'memory:write    - Write to memory store',
        'session:read    - Read session data',
        'session:write   - Write session data',
        'skill:register  - Register new skills',
        'skill:invoke    - Invoke existing skills',
        'llm:invoke      - Call LLM providers',
        'http:outbound   - Make HTTP requests',
        'fs:read         - Read local files',
        'fs:write        - Write local files',
        'env:read        - Read environment variables',
      ];
      for (const perm of permissions) {
        console.log(`  ${perm}`);
      }
      console.log('');
      console.log(chalk.cyan('Configuration (via .env):'));
      console.log('  RUVBOT_PLUGINS_ENABLED=true');
      console.log('  RUVBOT_PLUGINS_DIR=./plugins');
      console.log('  RUVBOT_PLUGINS_AUTOLOAD=true');
      console.log('  RUVBOT_PLUGINS_MAX=50');
      console.log('  RUVBOT_IPFS_GATEWAY=https://ipfs.io');
      console.log('─'.repeat(50));
    });

  // Validate command
  plugins
    .command('validate')
    .description('Validate a plugin manifest')
    .argument('<path>', 'Path to plugin or plugin.json')
    .action(async (pluginPath) => {
      try {
        const fs = await import('fs/promises');
        const path = await import('path');

        let manifestPath = pluginPath;
        const stat = await fs.stat(pluginPath);

        if (stat.isDirectory()) {
          manifestPath = path.join(pluginPath, 'plugin.json');
        }

        const content = await fs.readFile(manifestPath, 'utf-8');
        const manifest = JSON.parse(content);

        const result = PluginManifestSchema.safeParse(manifest);

        if (result.success) {
          console.log(chalk.green('✓ Plugin manifest is valid'));
          console.log(chalk.gray('\nManifest:'));
          console.log(`  Name: ${chalk.cyan(result.data.name)}`);
          console.log(`  Version: ${chalk.cyan(result.data.version)}`);
          console.log(`  Description: ${result.data.description || 'N/A'}`);
          console.log(`  Main: ${result.data.main}`);
          if (result.data.permissions.length > 0) {
            console.log(`  Permissions: ${result.data.permissions.join(', ')}`);
          }
        } else {
          console.log(chalk.red('✗ Plugin manifest is invalid'));
          console.log(chalk.gray('\nErrors:'));
          for (const error of result.error.errors) {
            console.log(chalk.red(`  • ${error.path.join('.')}: ${error.message}`));
          }
          process.exit(1);
        }
      } catch (error: any) {
        console.error(chalk.red(`Validation failed: ${error.message}`));
        process.exit(1);
      }
    });

  return plugins;
}

export default createPluginsCommand;
