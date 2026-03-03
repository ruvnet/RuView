/**
 * Plugin Registry with Advanced Plugin Support
 * Manages both standard and advanced reasoning plugins
 */

import { PluginRegistry } from '../core/plugin-system.js';
import { AdvancedPluginAdapter } from '../core/advanced-types.js';

// Standard plugins
import cachePlugin from './cache-plugin.js';

// Advanced reasoning plugins (need adaptation)
import chainOfThought from './advanced-reasoning/chain-of-thought-plugin.js';
import selfConsistency from './advanced-reasoning/self-consistency-plugin.js';
import antiHallucination from './advanced-reasoning/anti-hallucination-plugin.js';
import agenticResearchFlow from './advanced-reasoning/agentic-research-flow-plugin.js';

/**
 * Create and configure the plugin registry
 */
export function createPluginRegistry(): PluginRegistry {
  const registry = new PluginRegistry();

  // Register standard plugins
  registry.register(cachePlugin);
  console.log('📦 Registered cache plugin');

  // Register advanced reasoning plugins with adapter
  const advancedPlugins = [
    chainOfThought,
    selfConsistency,
    antiHallucination,
    agenticResearchFlow
  ];

  advancedPlugins.forEach(plugin => {
    const adapted = new AdvancedPluginAdapter(plugin);
    registry.register(adapted);
    console.log(`🧠 Registered advanced plugin: ${plugin.name}`);
  });

  console.log(`✅ Plugin registry initialized with ${advancedPlugins.length + 1} plugins`);

  return registry;
}

/**
 * Get configured plugin registry singleton
 */
let registryInstance: PluginRegistry | null = null;

export function getPluginRegistry(): PluginRegistry {
  if (!registryInstance) {
    registryInstance = createPluginRegistry();
  }
  return registryInstance;
}

export default getPluginRegistry();