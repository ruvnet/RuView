/**
 * Plugins Context - Extensible plugin system
 *
 * Inspired by claude-flow's IPFS-based plugin registry.
 * Supports plugin discovery, lifecycle management, and hot-reload.
 */

export {
  PluginManager,
  createPluginManager,
  createPluginManifest,
  scaffoldPlugin,
  PluginManifestSchema,
  DEFAULT_PLUGIN_CONFIG,
  type PluginManifest,
  type PluginState,
  type PluginInstance,
  type PluginExports,
  type PluginContext,
  type PluginMessage,
  type PluginResponse,
  type PluginSkill,
  type PluginCommand,
  type PluginManagerConfig,
  type PluginRegistryEntry,
  type PluginEvents,
} from './PluginManager.js';
