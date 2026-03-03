/**
 * GOAP MCP Main Entry Point
 * Exports all core components for external use
 */

export { GoapPlanner } from './goap/planner.js';
export { GoapMCPServer } from './mcp/server.js';
export { GoapMCPTools } from './mcp/tools.js';
export { PluginRegistry, PluginLoader } from './core/plugin-system.js';
export { AdvancedReasoningEngine } from './core/advanced-reasoning-engine.js';
export { perplexityActions, PerplexityClient } from './actions/perplexity-actions.js';

export * from './core/types.js';

// Built-in plugins
export {
  costTrackingPlugin,
  performanceMonitoringPlugin,
  loggingPlugin,
  queryDiversificationPlugin
} from './core/plugin-system.js';

// Default export for CLI usage
export { GoapMCPServer as default } from './mcp/server.js';