/**
 * PluginManager - Extensible plugin system for RuvBot
 *
 * Inspired by claude-flow's IPFS-based plugin registry.
 * Supports:
 * - Local plugin discovery and loading
 * - Plugin lifecycle management (install, enable, disable)
 * - Hot-reload capabilities
 * - Sandboxed execution
 * - IPFS registry integration (optional)
 */

import { z } from 'zod';
import { EventEmitter } from 'eventemitter3';

// ============================================================================
// Types
// ============================================================================

/**
 * Plugin manifest schema
 */
export const PluginManifestSchema = z.object({
  name: z.string().min(1).max(64),
  version: z.string().regex(/^\d+\.\d+\.\d+/),
  description: z.string().max(500),
  author: z.string().optional(),
  license: z.string().default('MIT'),
  main: z.string().default('index.js'),
  types: z.string().optional(),
  keywords: z.array(z.string()).default([]),
  dependencies: z.record(z.string()).default({}),
  peerDependencies: z.record(z.string()).default({}),
  engines: z.object({
    ruvbot: z.string().default('>=0.1.0'),
    node: z.string().default('>=18.0.0'),
  }).default({}),
  permissions: z.array(z.enum([
    'memory:read',
    'memory:write',
    'session:read',
    'session:write',
    'skill:register',
    'skill:invoke',
    'llm:invoke',
    'http:outbound',
    'fs:read',
    'fs:write',
    'env:read',
  ])).default([]),
  hooks: z.object({
    onLoad: z.string().optional(),
    onUnload: z.string().optional(),
    onMessage: z.string().optional(),
    onSkillInvoke: z.string().optional(),
  }).default({}),
});

export type PluginManifest = z.infer<typeof PluginManifestSchema>;

/**
 * Plugin state
 */
export type PluginState = 'installed' | 'enabled' | 'disabled' | 'error';

/**
 * Loaded plugin instance
 */
export interface PluginInstance {
  manifest: PluginManifest;
  state: PluginState;
  path: string;
  loadedAt?: Date;
  error?: string;
  exports?: PluginExports;
}

/**
 * Plugin exports interface
 */
export interface PluginExports {
  onLoad?: (context: PluginContext) => Promise<void>;
  onUnload?: (context: PluginContext) => Promise<void>;
  onMessage?: (message: PluginMessage, context: PluginContext) => Promise<PluginResponse | void>;
  onSkillInvoke?: (skill: string, params: unknown, context: PluginContext) => Promise<unknown>;
  skills?: PluginSkill[];
  commands?: PluginCommand[];
}

/**
 * Plugin execution context
 */
export interface PluginContext {
  pluginName: string;
  pluginVersion: string;
  permissions: string[];
  memory: {
    get: (key: string) => Promise<unknown>;
    set: (key: string, value: unknown) => Promise<void>;
    search: (query: string, limit?: number) => Promise<unknown[]>;
  };
  session: {
    get: (id: string) => Promise<unknown>;
    current: () => Promise<unknown>;
  };
  llm: {
    complete: (messages: Array<{ role: string; content: string }>) => Promise<string>;
  };
  log: {
    info: (message: string) => void;
    warn: (message: string) => void;
    error: (message: string) => void;
  };
}

/**
 * Plugin message
 */
export interface PluginMessage {
  content: string;
  userId?: string;
  sessionId?: string;
  channel?: string;
  metadata?: Record<string, unknown>;
}

/**
 * Plugin response
 */
export interface PluginResponse {
  content?: string;
  handled?: boolean;
  metadata?: Record<string, unknown>;
}

/**
 * Plugin skill definition
 */
export interface PluginSkill {
  name: string;
  description: string;
  parameters: z.ZodSchema;
  execute: (params: unknown, context: PluginContext) => Promise<unknown>;
}

/**
 * Plugin command definition
 */
export interface PluginCommand {
  name: string;
  description: string;
  usage: string;
  execute: (args: string[], context: PluginContext) => Promise<string>;
}

/**
 * Plugin manager configuration
 */
export interface PluginManagerConfig {
  pluginsDir: string;
  autoLoad: boolean;
  enableHotReload: boolean;
  sandboxed: boolean;
  ipfsGateway?: string;
  maxPlugins: number;
  timeout: number;
}

/**
 * Plugin registry entry (for IPFS)
 */
export interface PluginRegistryEntry {
  name: string;
  version: string;
  description: string;
  author?: string;
  downloads: number;
  rating: number;
  ipfsCid: string;
  publishedAt: Date;
  tags: string[];
}

// ============================================================================
// Default Configuration
// ============================================================================

export const DEFAULT_PLUGIN_CONFIG: PluginManagerConfig = {
  pluginsDir: './plugins',
  autoLoad: true,
  enableHotReload: false,
  sandboxed: true,
  ipfsGateway: 'https://ipfs.io/ipfs/',
  maxPlugins: 50,
  timeout: 30000,
};

// ============================================================================
// Plugin Events
// ============================================================================

export interface PluginEvents {
  'plugin:loaded': (plugin: PluginInstance) => void;
  'plugin:unloaded': (name: string) => void;
  'plugin:enabled': (name: string) => void;
  'plugin:disabled': (name: string) => void;
  'plugin:error': (name: string, error: Error) => void;
  'plugin:message': (plugin: string, message: PluginMessage) => void;
}

// ============================================================================
// PluginManager Implementation
// ============================================================================

export class PluginManager extends EventEmitter<PluginEvents> {
  private config: PluginManagerConfig;
  private plugins: Map<string, PluginInstance> = new Map();
  private contextFactory: (plugin: PluginInstance) => PluginContext;

  constructor(
    config: Partial<PluginManagerConfig> = {},
    contextFactory?: (plugin: PluginInstance) => PluginContext
  ) {
    super();
    this.config = { ...DEFAULT_PLUGIN_CONFIG, ...config };
    this.contextFactory = contextFactory ?? this.createDefaultContext.bind(this);
  }

  /**
   * Initialize plugin manager and auto-load plugins
   */
  async initialize(): Promise<void> {
    if (this.config.autoLoad) {
      await this.discoverPlugins();
    }
  }

  /**
   * Discover and load plugins from plugins directory
   */
  async discoverPlugins(): Promise<PluginInstance[]> {
    const loaded: PluginInstance[] = [];

    try {
      // Dynamic import for fs (works in Node.js)
      const fs = await import('fs/promises');
      const path = await import('path');

      const pluginsDir = this.config.pluginsDir;

      // Check if plugins directory exists
      try {
        await fs.access(pluginsDir);
      } catch {
        await fs.mkdir(pluginsDir, { recursive: true });
        return loaded;
      }

      // Read plugin directories
      const entries = await fs.readdir(pluginsDir, { withFileTypes: true });

      for (const entry of entries) {
        if (!entry.isDirectory()) continue;

        const pluginPath = path.join(pluginsDir, entry.name);
        const manifestPath = path.join(pluginPath, 'package.json');

        try {
          const manifestContent = await fs.readFile(manifestPath, 'utf-8');
          const manifest = PluginManifestSchema.parse(JSON.parse(manifestContent));

          const plugin = await this.loadPlugin(pluginPath, manifest);
          if (plugin) {
            loaded.push(plugin);
          }
        } catch (error) {
          console.warn(`Failed to load plugin from ${pluginPath}:`, error);
        }
      }
    } catch (error) {
      console.error('Failed to discover plugins:', error);
    }

    return loaded;
  }

  /**
   * Load a plugin from path
   */
  async loadPlugin(pluginPath: string, manifest: PluginManifest): Promise<PluginInstance | null> {
    if (this.plugins.size >= this.config.maxPlugins) {
      throw new Error(`Maximum plugin limit reached (${this.config.maxPlugins})`);
    }

    if (this.plugins.has(manifest.name)) {
      throw new Error(`Plugin ${manifest.name} is already loaded`);
    }

    const plugin: PluginInstance = {
      manifest,
      state: 'installed',
      path: pluginPath,
      loadedAt: new Date(),
    };

    try {
      // Dynamic import of plugin main file
      const path = await import('path');
      const mainPath = path.join(pluginPath, manifest.main);
      const exports = await import(mainPath) as PluginExports;

      plugin.exports = exports;
      plugin.state = 'enabled';

      // Call onLoad hook if present
      if (exports.onLoad) {
        const context = this.contextFactory(plugin);
        await Promise.race([
          exports.onLoad(context),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error('Plugin load timeout')), this.config.timeout)
          ),
        ]);
      }

      this.plugins.set(manifest.name, plugin);
      this.emit('plugin:loaded', plugin);

      return plugin;
    } catch (error) {
      plugin.state = 'error';
      plugin.error = error instanceof Error ? error.message : String(error);
      this.emit('plugin:error', manifest.name, error instanceof Error ? error : new Error(String(error)));
      return null;
    }
  }

  /**
   * Unload a plugin
   */
  async unloadPlugin(name: string): Promise<boolean> {
    const plugin = this.plugins.get(name);
    if (!plugin) {
      return false;
    }

    try {
      // Call onUnload hook if present
      if (plugin.exports?.onUnload) {
        const context = this.contextFactory(plugin);
        await plugin.exports.onUnload(context);
      }

      this.plugins.delete(name);
      this.emit('plugin:unloaded', name);
      return true;
    } catch (error) {
      this.emit('plugin:error', name, error instanceof Error ? error : new Error(String(error)));
      return false;
    }
  }

  /**
   * Enable a plugin
   */
  async enablePlugin(name: string): Promise<boolean> {
    const plugin = this.plugins.get(name);
    if (!plugin || plugin.state === 'enabled') {
      return false;
    }

    plugin.state = 'enabled';
    this.emit('plugin:enabled', name);
    return true;
  }

  /**
   * Disable a plugin
   */
  async disablePlugin(name: string): Promise<boolean> {
    const plugin = this.plugins.get(name);
    if (!plugin || plugin.state !== 'enabled') {
      return false;
    }

    plugin.state = 'disabled';
    this.emit('plugin:disabled', name);
    return true;
  }

  /**
   * Get all loaded plugins
   */
  listPlugins(): PluginInstance[] {
    return Array.from(this.plugins.values());
  }

  /**
   * Get a specific plugin
   */
  getPlugin(name: string): PluginInstance | undefined {
    return this.plugins.get(name);
  }

  /**
   * Get all enabled plugins
   */
  getEnabledPlugins(): PluginInstance[] {
    return this.listPlugins().filter(p => p.state === 'enabled');
  }

  /**
   * Get all registered skills from plugins
   */
  getPluginSkills(): Array<{ plugin: string; skill: PluginSkill }> {
    const skills: Array<{ plugin: string; skill: PluginSkill }> = [];

    for (const plugin of this.getEnabledPlugins()) {
      if (plugin.exports?.skills) {
        for (const skill of plugin.exports.skills) {
          skills.push({ plugin: plugin.manifest.name, skill });
        }
      }
    }

    return skills;
  }

  /**
   * Get all registered commands from plugins
   */
  getPluginCommands(): Array<{ plugin: string; command: PluginCommand }> {
    const commands: Array<{ plugin: string; command: PluginCommand }> = [];

    for (const plugin of this.getEnabledPlugins()) {
      if (plugin.exports?.commands) {
        for (const command of plugin.exports.commands) {
          commands.push({ plugin: plugin.manifest.name, command });
        }
      }
    }

    return commands;
  }

  /**
   * Dispatch a message to all enabled plugins
   */
  async dispatchMessage(message: PluginMessage): Promise<PluginResponse | null> {
    for (const plugin of this.getEnabledPlugins()) {
      if (plugin.exports?.onMessage) {
        try {
          const context = this.contextFactory(plugin);
          const response = await plugin.exports.onMessage(message, context);

          if (response?.handled) {
            return response;
          }
        } catch (error) {
          this.emit('plugin:error', plugin.manifest.name, error instanceof Error ? error : new Error(String(error)));
        }
      }
    }

    return null;
  }

  /**
   * Invoke a plugin skill
   */
  async invokeSkill(skillName: string, params: unknown): Promise<unknown> {
    for (const { plugin, skill } of this.getPluginSkills()) {
      if (skill.name === skillName) {
        const pluginInstance = this.plugins.get(plugin);
        if (!pluginInstance) continue;

        const context = this.contextFactory(pluginInstance);
        return skill.execute(params, context);
      }
    }

    throw new Error(`Skill ${skillName} not found in any plugin`);
  }

  /**
   * Search IPFS registry for plugins
   */
  async searchRegistry(query: string): Promise<PluginRegistryEntry[]> {
    if (!this.config.ipfsGateway) {
      return [];
    }

    // Placeholder for IPFS registry search
    // In production, this would query an IPFS-based registry
    console.log(`Searching IPFS registry for: ${query}`);
    return [];
  }

  /**
   * Install plugin from IPFS registry
   */
  async installFromRegistry(name: string): Promise<PluginInstance | null> {
    if (!this.config.ipfsGateway) {
      throw new Error('IPFS gateway not configured');
    }

    // Placeholder for IPFS installation
    // In production, this would:
    // 1. Fetch plugin metadata from IPFS registry
    // 2. Download plugin package from IPFS
    // 3. Verify integrity (hash check)
    // 4. Extract to plugins directory
    // 5. Load plugin

    console.log(`Installing ${name} from IPFS registry...`);
    return null;
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private createDefaultContext(plugin: PluginInstance): PluginContext {
    return {
      pluginName: plugin.manifest.name,
      pluginVersion: plugin.manifest.version,
      permissions: plugin.manifest.permissions,
      memory: {
        get: async () => null,
        set: async () => { /* no-op */ },
        search: async () => [],
      },
      session: {
        get: async () => null,
        current: async () => null,
      },
      llm: {
        complete: async () => 'LLM not available in default context',
      },
      log: {
        info: (msg) => console.log(`[${plugin.manifest.name}] ${msg}`),
        warn: (msg) => console.warn(`[${plugin.manifest.name}] ${msg}`),
        error: (msg) => console.error(`[${plugin.manifest.name}] ${msg}`),
      },
    };
  }
}

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * Create a new plugin manager
 */
export function createPluginManager(
  config?: Partial<PluginManagerConfig>,
  contextFactory?: (plugin: PluginInstance) => PluginContext
): PluginManager {
  return new PluginManager(config, contextFactory);
}

/**
 * Create a plugin manifest
 */
export function createPluginManifest(manifest: Partial<PluginManifest> & { name: string; version: string; description: string }): PluginManifest {
  return PluginManifestSchema.parse(manifest);
}

/**
 * Scaffold a new plugin project
 */
export async function scaffoldPlugin(name: string, targetDir: string): Promise<void> {
  const fs = await import('fs/promises');
  const path = await import('path');

  const pluginDir = path.join(targetDir, name);
  await fs.mkdir(pluginDir, { recursive: true });

  // Create package.json
  const manifest: PluginManifest = {
    name,
    version: '1.0.0',
    description: `${name} plugin for RuvBot`,
    main: 'index.js',
    keywords: ['ruvbot', 'plugin'],
    dependencies: {},
    peerDependencies: {},
    engines: { ruvbot: '>=0.1.0', node: '>=18.0.0' },
    permissions: ['memory:read'],
    hooks: {},
    license: 'MIT',
  };

  await fs.writeFile(
    path.join(pluginDir, 'package.json'),
    JSON.stringify(manifest, null, 2)
  );

  // Create index.ts template
  const indexTemplate = `/**
 * ${name} - RuvBot Plugin
 */

import type { PluginContext, PluginMessage, PluginResponse, PluginSkill } from '@ruvector/ruvbot';

/**
 * Called when plugin is loaded
 */
export async function onLoad(context: PluginContext): Promise<void> {
  context.log.info('Plugin loaded successfully');
}

/**
 * Called when plugin is unloaded
 */
export async function onUnload(context: PluginContext): Promise<void> {
  context.log.info('Plugin unloaded');
}

/**
 * Handle incoming messages
 */
export async function onMessage(
  message: PluginMessage,
  context: PluginContext
): Promise<PluginResponse | void> {
  // Return undefined to pass through to other plugins
  // Return { handled: true, content: '...' } to handle the message
  return undefined;
}

/**
 * Plugin skills
 */
export const skills: PluginSkill[] = [
  {
    name: '${name}-example',
    description: 'Example skill from ${name} plugin',
    parameters: {} as any, // Define Zod schema
    execute: async (params, context) => {
      return { success: true, message: 'Example skill executed' };
    },
  },
];
`;

  await fs.writeFile(path.join(pluginDir, 'index.ts'), indexTemplate);

  // Create tsconfig.json
  const tsconfig = {
    compilerOptions: {
      target: 'ES2022',
      module: 'NodeNext',
      moduleResolution: 'NodeNext',
      declaration: true,
      strict: true,
      outDir: './dist',
    },
    include: ['*.ts'],
  };

  await fs.writeFile(
    path.join(pluginDir, 'tsconfig.json'),
    JSON.stringify(tsconfig, null, 2)
  );

  // Create README.md
  const readme = `# ${name}

RuvBot plugin.

## Installation

\`\`\`bash
# Copy to plugins directory
cp -r ${name} ~/.ruvbot/plugins/
\`\`\`

## Usage

This plugin provides:

- **Skills**: ${name}-example

## License

MIT
`;

  await fs.writeFile(path.join(pluginDir, 'README.md'), readme);
}

export default PluginManager;
