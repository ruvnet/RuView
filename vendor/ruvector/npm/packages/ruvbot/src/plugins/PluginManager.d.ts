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
/**
 * Plugin manifest schema
 */
export declare const PluginManifestSchema: z.ZodObject<{
    name: z.ZodString;
    version: z.ZodString;
    description: z.ZodString;
    author: z.ZodOptional<z.ZodString>;
    license: z.ZodDefault<z.ZodString>;
    main: z.ZodDefault<z.ZodString>;
    types: z.ZodOptional<z.ZodString>;
    keywords: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
    dependencies: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodString>>;
    peerDependencies: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodString>>;
    engines: z.ZodDefault<z.ZodObject<{
        ruvbot: z.ZodDefault<z.ZodString>;
        node: z.ZodDefault<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        node: string;
        ruvbot: string;
    }, {
        node?: string | undefined;
        ruvbot?: string | undefined;
    }>>;
    permissions: z.ZodDefault<z.ZodArray<z.ZodEnum<["memory:read", "memory:write", "session:read", "session:write", "skill:register", "skill:invoke", "llm:invoke", "http:outbound", "fs:read", "fs:write", "env:read"]>, "many">>;
    hooks: z.ZodDefault<z.ZodObject<{
        onLoad: z.ZodOptional<z.ZodString>;
        onUnload: z.ZodOptional<z.ZodString>;
        onMessage: z.ZodOptional<z.ZodString>;
        onSkillInvoke: z.ZodOptional<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        onLoad?: string | undefined;
        onUnload?: string | undefined;
        onMessage?: string | undefined;
        onSkillInvoke?: string | undefined;
    }, {
        onLoad?: string | undefined;
        onUnload?: string | undefined;
        onMessage?: string | undefined;
        onSkillInvoke?: string | undefined;
    }>>;
}, "strip", z.ZodTypeAny, {
    name: string;
    description: string;
    main: string;
    version: string;
    dependencies: Record<string, string>;
    permissions: ("skill:invoke" | "memory:read" | "memory:write" | "session:read" | "session:write" | "skill:register" | "llm:invoke" | "http:outbound" | "fs:read" | "fs:write" | "env:read")[];
    hooks: {
        onLoad?: string | undefined;
        onUnload?: string | undefined;
        onMessage?: string | undefined;
        onSkillInvoke?: string | undefined;
    };
    license: string;
    keywords: string[];
    peerDependencies: Record<string, string>;
    engines: {
        node: string;
        ruvbot: string;
    };
    author?: string | undefined;
    types?: string | undefined;
}, {
    name: string;
    description: string;
    version: string;
    author?: string | undefined;
    main?: string | undefined;
    dependencies?: Record<string, string> | undefined;
    permissions?: ("skill:invoke" | "memory:read" | "memory:write" | "session:read" | "session:write" | "skill:register" | "llm:invoke" | "http:outbound" | "fs:read" | "fs:write" | "env:read")[] | undefined;
    hooks?: {
        onLoad?: string | undefined;
        onUnload?: string | undefined;
        onMessage?: string | undefined;
        onSkillInvoke?: string | undefined;
    } | undefined;
    types?: string | undefined;
    license?: string | undefined;
    keywords?: string[] | undefined;
    peerDependencies?: Record<string, string> | undefined;
    engines?: {
        node?: string | undefined;
        ruvbot?: string | undefined;
    } | undefined;
}>;
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
        complete: (messages: Array<{
            role: string;
            content: string;
        }>) => Promise<string>;
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
export declare const DEFAULT_PLUGIN_CONFIG: PluginManagerConfig;
export interface PluginEvents {
    'plugin:loaded': (plugin: PluginInstance) => void;
    'plugin:unloaded': (name: string) => void;
    'plugin:enabled': (name: string) => void;
    'plugin:disabled': (name: string) => void;
    'plugin:error': (name: string, error: Error) => void;
    'plugin:message': (plugin: string, message: PluginMessage) => void;
}
export declare class PluginManager extends EventEmitter<PluginEvents> {
    private config;
    private plugins;
    private contextFactory;
    constructor(config?: Partial<PluginManagerConfig>, contextFactory?: (plugin: PluginInstance) => PluginContext);
    /**
     * Initialize plugin manager and auto-load plugins
     */
    initialize(): Promise<void>;
    /**
     * Discover and load plugins from plugins directory
     */
    discoverPlugins(): Promise<PluginInstance[]>;
    /**
     * Load a plugin from path
     */
    loadPlugin(pluginPath: string, manifest: PluginManifest): Promise<PluginInstance | null>;
    /**
     * Unload a plugin
     */
    unloadPlugin(name: string): Promise<boolean>;
    /**
     * Enable a plugin
     */
    enablePlugin(name: string): Promise<boolean>;
    /**
     * Disable a plugin
     */
    disablePlugin(name: string): Promise<boolean>;
    /**
     * Get all loaded plugins
     */
    listPlugins(): PluginInstance[];
    /**
     * Get a specific plugin
     */
    getPlugin(name: string): PluginInstance | undefined;
    /**
     * Get all enabled plugins
     */
    getEnabledPlugins(): PluginInstance[];
    /**
     * Get all registered skills from plugins
     */
    getPluginSkills(): Array<{
        plugin: string;
        skill: PluginSkill;
    }>;
    /**
     * Get all registered commands from plugins
     */
    getPluginCommands(): Array<{
        plugin: string;
        command: PluginCommand;
    }>;
    /**
     * Dispatch a message to all enabled plugins
     */
    dispatchMessage(message: PluginMessage): Promise<PluginResponse | null>;
    /**
     * Invoke a plugin skill
     */
    invokeSkill(skillName: string, params: unknown): Promise<unknown>;
    /**
     * Search IPFS registry for plugins
     */
    searchRegistry(query: string): Promise<PluginRegistryEntry[]>;
    /**
     * Install plugin from IPFS registry
     */
    installFromRegistry(name: string): Promise<PluginInstance | null>;
    private createDefaultContext;
}
/**
 * Create a new plugin manager
 */
export declare function createPluginManager(config?: Partial<PluginManagerConfig>, contextFactory?: (plugin: PluginInstance) => PluginContext): PluginManager;
/**
 * Create a plugin manifest
 */
export declare function createPluginManifest(manifest: Partial<PluginManifest> & {
    name: string;
    version: string;
    description: string;
}): PluginManifest;
/**
 * Scaffold a new plugin project
 */
export declare function scaffoldPlugin(name: string, targetDir: string): Promise<void>;
export default PluginManager;
//# sourceMappingURL=PluginManager.d.ts.map