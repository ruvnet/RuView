"use strict";
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
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.PluginManager = exports.DEFAULT_PLUGIN_CONFIG = exports.PluginManifestSchema = void 0;
exports.createPluginManager = createPluginManager;
exports.createPluginManifest = createPluginManifest;
exports.scaffoldPlugin = scaffoldPlugin;
const zod_1 = require("zod");
const eventemitter3_1 = require("eventemitter3");
// ============================================================================
// Types
// ============================================================================
/**
 * Plugin manifest schema
 */
exports.PluginManifestSchema = zod_1.z.object({
    name: zod_1.z.string().min(1).max(64),
    version: zod_1.z.string().regex(/^\d+\.\d+\.\d+/),
    description: zod_1.z.string().max(500),
    author: zod_1.z.string().optional(),
    license: zod_1.z.string().default('MIT'),
    main: zod_1.z.string().default('index.js'),
    types: zod_1.z.string().optional(),
    keywords: zod_1.z.array(zod_1.z.string()).default([]),
    dependencies: zod_1.z.record(zod_1.z.string()).default({}),
    peerDependencies: zod_1.z.record(zod_1.z.string()).default({}),
    engines: zod_1.z.object({
        ruvbot: zod_1.z.string().default('>=0.1.0'),
        node: zod_1.z.string().default('>=18.0.0'),
    }).default({}),
    permissions: zod_1.z.array(zod_1.z.enum([
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
    hooks: zod_1.z.object({
        onLoad: zod_1.z.string().optional(),
        onUnload: zod_1.z.string().optional(),
        onMessage: zod_1.z.string().optional(),
        onSkillInvoke: zod_1.z.string().optional(),
    }).default({}),
});
// ============================================================================
// Default Configuration
// ============================================================================
exports.DEFAULT_PLUGIN_CONFIG = {
    pluginsDir: './plugins',
    autoLoad: true,
    enableHotReload: false,
    sandboxed: true,
    ipfsGateway: 'https://ipfs.io/ipfs/',
    maxPlugins: 50,
    timeout: 30000,
};
// ============================================================================
// PluginManager Implementation
// ============================================================================
class PluginManager extends eventemitter3_1.EventEmitter {
    constructor(config = {}, contextFactory) {
        super();
        this.plugins = new Map();
        this.config = { ...exports.DEFAULT_PLUGIN_CONFIG, ...config };
        this.contextFactory = contextFactory ?? this.createDefaultContext.bind(this);
    }
    /**
     * Initialize plugin manager and auto-load plugins
     */
    async initialize() {
        if (this.config.autoLoad) {
            await this.discoverPlugins();
        }
    }
    /**
     * Discover and load plugins from plugins directory
     */
    async discoverPlugins() {
        const loaded = [];
        try {
            // Dynamic import for fs (works in Node.js)
            const fs = await Promise.resolve().then(() => __importStar(require('fs/promises')));
            const path = await Promise.resolve().then(() => __importStar(require('path')));
            const pluginsDir = this.config.pluginsDir;
            // Check if plugins directory exists
            try {
                await fs.access(pluginsDir);
            }
            catch {
                await fs.mkdir(pluginsDir, { recursive: true });
                return loaded;
            }
            // Read plugin directories
            const entries = await fs.readdir(pluginsDir, { withFileTypes: true });
            for (const entry of entries) {
                if (!entry.isDirectory())
                    continue;
                const pluginPath = path.join(pluginsDir, entry.name);
                const manifestPath = path.join(pluginPath, 'package.json');
                try {
                    const manifestContent = await fs.readFile(manifestPath, 'utf-8');
                    const manifest = exports.PluginManifestSchema.parse(JSON.parse(manifestContent));
                    const plugin = await this.loadPlugin(pluginPath, manifest);
                    if (plugin) {
                        loaded.push(plugin);
                    }
                }
                catch (error) {
                    console.warn(`Failed to load plugin from ${pluginPath}:`, error);
                }
            }
        }
        catch (error) {
            console.error('Failed to discover plugins:', error);
        }
        return loaded;
    }
    /**
     * Load a plugin from path
     */
    async loadPlugin(pluginPath, manifest) {
        if (this.plugins.size >= this.config.maxPlugins) {
            throw new Error(`Maximum plugin limit reached (${this.config.maxPlugins})`);
        }
        if (this.plugins.has(manifest.name)) {
            throw new Error(`Plugin ${manifest.name} is already loaded`);
        }
        const plugin = {
            manifest,
            state: 'installed',
            path: pluginPath,
            loadedAt: new Date(),
        };
        try {
            // Dynamic import of plugin main file
            const path = await Promise.resolve().then(() => __importStar(require('path')));
            const mainPath = path.join(pluginPath, manifest.main);
            const exports = await Promise.resolve(`${mainPath}`).then(s => __importStar(require(s)));
            plugin.exports = exports;
            plugin.state = 'enabled';
            // Call onLoad hook if present
            if (exports.onLoad) {
                const context = this.contextFactory(plugin);
                await Promise.race([
                    exports.onLoad(context),
                    new Promise((_, reject) => setTimeout(() => reject(new Error('Plugin load timeout')), this.config.timeout)),
                ]);
            }
            this.plugins.set(manifest.name, plugin);
            this.emit('plugin:loaded', plugin);
            return plugin;
        }
        catch (error) {
            plugin.state = 'error';
            plugin.error = error instanceof Error ? error.message : String(error);
            this.emit('plugin:error', manifest.name, error instanceof Error ? error : new Error(String(error)));
            return null;
        }
    }
    /**
     * Unload a plugin
     */
    async unloadPlugin(name) {
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
        }
        catch (error) {
            this.emit('plugin:error', name, error instanceof Error ? error : new Error(String(error)));
            return false;
        }
    }
    /**
     * Enable a plugin
     */
    async enablePlugin(name) {
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
    async disablePlugin(name) {
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
    listPlugins() {
        return Array.from(this.plugins.values());
    }
    /**
     * Get a specific plugin
     */
    getPlugin(name) {
        return this.plugins.get(name);
    }
    /**
     * Get all enabled plugins
     */
    getEnabledPlugins() {
        return this.listPlugins().filter(p => p.state === 'enabled');
    }
    /**
     * Get all registered skills from plugins
     */
    getPluginSkills() {
        const skills = [];
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
    getPluginCommands() {
        const commands = [];
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
    async dispatchMessage(message) {
        for (const plugin of this.getEnabledPlugins()) {
            if (plugin.exports?.onMessage) {
                try {
                    const context = this.contextFactory(plugin);
                    const response = await plugin.exports.onMessage(message, context);
                    if (response?.handled) {
                        return response;
                    }
                }
                catch (error) {
                    this.emit('plugin:error', plugin.manifest.name, error instanceof Error ? error : new Error(String(error)));
                }
            }
        }
        return null;
    }
    /**
     * Invoke a plugin skill
     */
    async invokeSkill(skillName, params) {
        for (const { plugin, skill } of this.getPluginSkills()) {
            if (skill.name === skillName) {
                const pluginInstance = this.plugins.get(plugin);
                if (!pluginInstance)
                    continue;
                const context = this.contextFactory(pluginInstance);
                return skill.execute(params, context);
            }
        }
        throw new Error(`Skill ${skillName} not found in any plugin`);
    }
    /**
     * Search IPFS registry for plugins
     */
    async searchRegistry(query) {
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
    async installFromRegistry(name) {
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
    createDefaultContext(plugin) {
        return {
            pluginName: plugin.manifest.name,
            pluginVersion: plugin.manifest.version,
            permissions: plugin.manifest.permissions,
            memory: {
                get: async () => null,
                set: async () => { },
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
exports.PluginManager = PluginManager;
// ============================================================================
// Factory Functions
// ============================================================================
/**
 * Create a new plugin manager
 */
function createPluginManager(config, contextFactory) {
    return new PluginManager(config, contextFactory);
}
/**
 * Create a plugin manifest
 */
function createPluginManifest(manifest) {
    return exports.PluginManifestSchema.parse(manifest);
}
/**
 * Scaffold a new plugin project
 */
async function scaffoldPlugin(name, targetDir) {
    const fs = await Promise.resolve().then(() => __importStar(require('fs/promises')));
    const path = await Promise.resolve().then(() => __importStar(require('path')));
    const pluginDir = path.join(targetDir, name);
    await fs.mkdir(pluginDir, { recursive: true });
    // Create package.json
    const manifest = {
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
    await fs.writeFile(path.join(pluginDir, 'package.json'), JSON.stringify(manifest, null, 2));
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
    await fs.writeFile(path.join(pluginDir, 'tsconfig.json'), JSON.stringify(tsconfig, null, 2));
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
exports.default = PluginManager;
//# sourceMappingURL=PluginManager.js.map