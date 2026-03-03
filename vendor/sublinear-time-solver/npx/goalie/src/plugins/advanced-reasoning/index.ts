/**
 * Advanced Reasoning Plugins
 * Export all advanced reasoning and validation plugins
 */

export { ChainOfThoughtPlugin } from './chain-of-thought-plugin.js';
export { SelfConsistencyPlugin } from './self-consistency-plugin.js';
export { AntiHallucinationPlugin } from './anti-hallucination-plugin.js';
export { AgenticResearchFlowPlugin } from './agentic-research-flow-plugin.js';

// Default export as plugin collection
import chainOfThought from './chain-of-thought-plugin.js';
import selfConsistency from './self-consistency-plugin.js';
import antiHallucination from './anti-hallucination-plugin.js';
import agenticResearchFlow from './agentic-research-flow-plugin.js';

export const advancedReasoningPlugins = [
  chainOfThought,
  selfConsistency,
  antiHallucination,
  agenticResearchFlow
];

export default advancedReasoningPlugins;