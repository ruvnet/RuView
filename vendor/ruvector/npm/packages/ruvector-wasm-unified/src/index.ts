/**
 * RuVector WASM Unified API
 *
 * A unified TypeScript surface for all RuVector WASM capabilities:
 * - Attention: 14+ attention mechanisms (neural + DAG)
 * - Learning: Micro-LoRA, SONA, BTSP, RL, Meta-learning
 * - Nervous: Biological neural network simulation
 * - Economy: Compute credit management
 * - Exotic: Quantum, Hyperbolic, Topological computation
 *
 * @module @ruvector/wasm-unified
 * @version 1.0.0
 */

// ============================================================================
// Re-exports from all modules
// ============================================================================

// Types
export * from './types';

// Attention Engine
export {
  type AttentionEngine,
  type MambaConfig,
  type AttentionMechanism,
  createAttentionEngine,
  listAttentionMechanisms,
  benchmarkAttention,
} from './attention';

// Learning Engine
export {
  type LearningEngine,
  type RLAlgorithm,
  type PolicyUpdate,
  type ReplayBatch,
  type TaskDataset,
  type LearningStats,
  type LoraLayerInfo,
  createLearningEngine,
  createMicroLoraConfig,
  createBtspConfig,
  cosineAnnealingLr,
  warmupLr,
} from './learning';

// Nervous System Engine
export {
  type NervousEngine,
  type NeuronConfig,
  type NeuronModel,
  type SynapseConfig,
  type StdpConfig,
  type NeuronFilter,
  type SimulationResult,
  type PlasticityStats,
  type TopologyStats,
  type RecordedActivity,
  createNervousEngine,
  createStdpConfig,
  izhikevichParams,
} from './nervous';

// Economy Engine
export {
  type EconomyEngine,
  type TransactionFilter,
  type ContributionType,
  type ContributionRecord,
  type OperationType,
  type AccountAnalytics,
  type LeaderboardMetric,
  type LeaderboardEntry,
  createEconomyEngine,
  calculateStakingApy,
  formatCredits,
} from './economy';

// Exotic Engine
export {
  type ExoticEngine,
  type QuantumMeasurement,
  type QuantumCircuit,
  type QuantumGate,
  type VqeResult,
  type PersistencePair,
  type MapperGraph,
  type MapperNode,
  type MapperEdge,
  type ExoticStats,
  createExoticEngine,
  createCircuitBuilder,
  projectToPoincare,
  poincareToLorentz,
} from './exotic';

// ============================================================================
// Unified Engine
// ============================================================================

import { createAttentionEngine, type AttentionEngine } from './attention';
import { createLearningEngine, type LearningEngine } from './learning';
import { createNervousEngine, type NervousEngine } from './nervous';
import { createEconomyEngine, type EconomyEngine } from './economy';
import { createExoticEngine, type ExoticEngine } from './exotic';
import type { UnifiedConfig, ModuleConfig } from './types';

/**
 * Unified RuVector WASM Engine combining all capabilities
 */
export interface UnifiedEngine {
  /** Attention mechanisms (14+) */
  attention: AttentionEngine;

  /** Learning and adaptation */
  learning: LearningEngine;

  /** Biological neural simulation */
  nervous: NervousEngine;

  /** Compute credit economy */
  economy: EconomyEngine;

  /** Exotic computation paradigms */
  exotic: ExoticEngine;

  /** Get engine version */
  version(): string;

  /** Get all engine statistics */
  getStats(): UnifiedStats;

  /** Initialize WASM module */
  init(): Promise<void>;

  /** Cleanup and release resources */
  dispose(): void;
}

/** Unified statistics from all engines */
export interface UnifiedStats {
  attention: {
    operationCount: number;
    cacheHitRate: number;
  };
  learning: {
    stepsCompleted: number;
    patternsLearned: number;
  };
  nervous: {
    neuronCount: number;
    synapseCount: number;
    spikeRate: number;
  };
  economy: {
    balance: number;
    stakedAmount: number;
    transactionCount: number;
  };
  exotic: {
    quantumOps: number;
    hyperbolicOps: number;
    topologicalOps: number;
  };
  system: {
    memoryUsageBytes: number;
    wasmHeapBytes: number;
    uptime: number;
  };
}

/**
 * Create a unified RuVector WASM engine
 *
 * @example
 * ```typescript
 * import { createUnifiedEngine } from '@ruvector/wasm-unified';
 *
 * const engine = await createUnifiedEngine();
 *
 * // Use attention mechanisms
 * const output = engine.attention.scaledDot(Q, K, V);
 *
 * // Use learning capabilities
 * engine.learning.btspOneShotLearn(pattern, reward);
 *
 * // Simulate nervous system
 * engine.nervous.step();
 *
 * // Manage economy
 * const balance = engine.economy.creditBalance();
 *
 * // Exotic computations
 * const qstate = engine.exotic.quantumInit(4);
 * ```
 *
 * @param config Optional configuration
 * @returns Unified engine instance
 */
export async function createUnifiedEngine(
  config?: UnifiedConfig & ModuleConfig
): Promise<UnifiedEngine> {
  const startTime = Date.now();

  // Initialize all engines
  const attention = createAttentionEngine(config?.attention);
  const learning = createLearningEngine(config?.learning);
  const nervous = createNervousEngine(config?.nervous);
  const economy = createEconomyEngine(config?.economy);
  const exotic = createExoticEngine(config?.exotic);

  // Track operation counts
  let attentionOps = 0;
  let transactionCount = 0;

  return {
    attention,
    learning,
    nervous,
    economy,
    exotic,

    version: () => '1.0.0',

    getStats: () => ({
      attention: {
        operationCount: attentionOps,
        cacheHitRate: 0,
      },
      learning: {
        stepsCompleted: learning.getStats().totalSteps,
        patternsLearned: learning.getStats().patternsLearned,
      },
      nervous: {
        neuronCount: nervous.getTopologyStats().neuronCount,
        synapseCount: nervous.getTopologyStats().synapseCount,
        spikeRate: 0,
      },
      economy: {
        balance: economy.creditBalance(),
        stakedAmount: economy.getTotalStaked(),
        transactionCount,
      },
      exotic: {
        quantumOps: exotic.getStats().quantumOperations,
        hyperbolicOps: exotic.getStats().hyperbolicOperations,
        topologicalOps: exotic.getStats().topologicalOperations,
      },
      system: {
        memoryUsageBytes: 0,
        wasmHeapBytes: 0,
        uptime: Date.now() - startTime,
      },
    }),

    init: async () => {
      // WASM initialization would happen here
      // await wasmModule.init();
      if (config?.logLevel === 'debug') {
        console.log('[ruvector-wasm-unified] Initialized');
      }
    },

    dispose: () => {
      nervous.reset(false);
      // WASM cleanup would happen here
      if (config?.logLevel === 'debug') {
        console.log('[ruvector-wasm-unified] Disposed');
      }
    },
  };
}

// ============================================================================
// Convenience exports
// ============================================================================

/** Default unified engine instance (lazy initialized) */
let defaultEngine: UnifiedEngine | null = null;

/**
 * Get or create the default unified engine
 * @returns Default engine instance
 */
export async function getDefaultEngine(): Promise<UnifiedEngine> {
  if (!defaultEngine) {
    defaultEngine = await createUnifiedEngine();
    await defaultEngine.init();
  }
  return defaultEngine;
}

/**
 * Reset the default engine
 */
export function resetDefaultEngine(): void {
  if (defaultEngine) {
    defaultEngine.dispose();
    defaultEngine = null;
  }
}

// ============================================================================
// Version and metadata
// ============================================================================

/** Package version */
export const VERSION = '1.0.0';

/** Supported features */
export const FEATURES = {
  attention: [
    'scaled-dot',
    'multi-head',
    'hyperbolic',
    'linear',
    'flash',
    'local-global',
    'moe',
    'mamba',
    'dag-topological',
    'dag-mincut',
    'dag-hierarchical',
    'dag-spectral',
    'dag-flow',
    'dag-causal',
    'dag-sparse',
  ],
  learning: [
    'micro-lora',
    'sona-pre-query',
    'btsp-one-shot',
    'ppo',
    'a2c',
    'dqn',
    'sac',
    'td3',
    'reinforce',
    'ewc',
    'progressive-nets',
    'experience-replay',
    'maml',
    'reptile',
  ],
  nervous: [
    'lif',
    'izhikevich',
    'hodgkin-huxley',
    'adex',
    'srm',
    'stdp',
    'btsp',
    'hebbian',
    'homeostasis',
  ],
  economy: [
    'credit-balance',
    'staking',
    'rewards',
    'contribution-multiplier',
    'transactions',
  ],
  exotic: [
    'quantum-superposition',
    'quantum-entanglement',
    'quantum-vqe',
    'hyperbolic-poincare',
    'hyperbolic-lorentz',
    'mobius-operations',
    'persistent-homology',
    'mapper',
    'fractal-dimension',
    'lyapunov-exponents',
  ],
} as const;
