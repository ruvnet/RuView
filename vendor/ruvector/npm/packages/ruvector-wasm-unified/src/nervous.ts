/**
 * RuVector WASM Unified - Nervous System Engine
 *
 * Provides biological neural network simulation including:
 * - Spiking neural networks (SNN)
 * - Synaptic plasticity rules (STDP, BTSP, Hebbian)
 * - Neuron dynamics (LIF, Izhikevich, Hodgkin-Huxley)
 * - Network topology management
 * - Signal propagation
 */

import type {
  Neuron,
  Synapse,
  PlasticityRule,
  NervousState,
  PropagationResult,
  NervousConfig,
} from './types';

// ============================================================================
// Nervous System Engine Interface
// ============================================================================

/**
 * Core nervous system engine for biological neural network simulation
 */
export interface NervousEngine {
  // -------------------------------------------------------------------------
  // Neuron Management
  // -------------------------------------------------------------------------

  /**
   * Create a new neuron in the network
   * @param config Neuron configuration
   * @returns Neuron ID
   */
  createNeuron(config: NeuronConfig): string;

  /**
   * Remove a neuron from the network
   * @param neuronId Neuron to remove
   */
  removeNeuron(neuronId: string): void;

  /**
   * Get neuron by ID
   * @param neuronId Neuron ID
   * @returns Neuron state
   */
  getNeuron(neuronId: string): Neuron | undefined;

  /**
   * Update neuron parameters
   * @param neuronId Neuron to update
   * @param params New parameters
   */
  updateNeuron(neuronId: string, params: Partial<NeuronConfig>): void;

  /**
   * List all neurons
   * @param filter Optional filter criteria
   * @returns Array of neurons
   */
  listNeurons(filter?: NeuronFilter): Neuron[];

  // -------------------------------------------------------------------------
  // Synapse Management
  // -------------------------------------------------------------------------

  /**
   * Create a synapse between neurons
   * @param presynapticId Source neuron
   * @param postsynapticId Target neuron
   * @param config Synapse configuration
   * @returns Synapse ID
   */
  createSynapse(
    presynapticId: string,
    postsynapticId: string,
    config?: SynapseConfig
  ): string;

  /**
   * Remove a synapse
   * @param presynapticId Source neuron
   * @param postsynapticId Target neuron
   */
  removeSynapse(presynapticId: string, postsynapticId: string): void;

  /**
   * Get synapse between neurons
   * @param presynapticId Source neuron
   * @param postsynapticId Target neuron
   * @returns Synapse or undefined
   */
  getSynapse(presynapticId: string, postsynapticId: string): Synapse | undefined;

  /**
   * Update synapse parameters
   * @param presynapticId Source neuron
   * @param postsynapticId Target neuron
   * @param params New parameters
   */
  updateSynapse(
    presynapticId: string,
    postsynapticId: string,
    params: Partial<SynapseConfig>
  ): void;

  /**
   * List synapses for a neuron
   * @param neuronId Neuron ID
   * @param direction 'incoming' | 'outgoing' | 'both'
   * @returns Array of synapses
   */
  listSynapses(neuronId: string, direction?: 'incoming' | 'outgoing' | 'both'): Synapse[];

  // -------------------------------------------------------------------------
  // Simulation
  // -------------------------------------------------------------------------

  /**
   * Step the simulation forward
   * @param dt Time step in milliseconds
   * @returns Simulation result
   */
  step(dt?: number): SimulationResult;

  /**
   * Inject current into neurons
   * @param injections Map of neuron ID to current value
   */
  injectCurrent(injections: Map<string, number>): void;

  /**
   * Propagate signal through network
   * @param sourceIds Source neuron IDs
   * @param signal Signal strength
   * @returns Propagation result
   */
  propagate(sourceIds: string[], signal: number): PropagationResult;

  /**
   * Get current network state
   * @returns Complete nervous system state
   */
  getState(): NervousState;

  /**
   * Set network state
   * @param state State to restore
   */
  setState(state: NervousState): void;

  /**
   * Reset network to initial state
   * @param keepTopology Keep neurons and synapses, reset potentials
   */
  reset(keepTopology?: boolean): void;

  // -------------------------------------------------------------------------
  // Plasticity
  // -------------------------------------------------------------------------

  /**
   * Apply plasticity rule to all synapses
   * @param rule Plasticity rule to apply
   * @param learningRate Global learning rate modifier
   */
  applyPlasticity(rule?: PlasticityRule, learningRate?: number): void;

  /**
   * Apply STDP (Spike-Timing Dependent Plasticity)
   * @param config STDP configuration
   */
  applyStdp(config?: StdpConfig): void;

  /**
   * Apply homeostatic plasticity
   * @param targetRate Target firing rate
   */
  applyHomeostasis(targetRate?: number): void;

  /**
   * Get plasticity statistics
   * @returns Plasticity metrics
   */
  getPlasticityStats(): PlasticityStats;

  // -------------------------------------------------------------------------
  // Topology
  // -------------------------------------------------------------------------

  /**
   * Create a feedforward network
   * @param layerSizes Neurons per layer
   * @param connectivity Connection probability between layers
   */
  createFeedforward(layerSizes: number[], connectivity?: number): void;

  /**
   * Create a recurrent network
   * @param size Number of neurons
   * @param connectivity Recurrent connection probability
   */
  createRecurrent(size: number, connectivity?: number): void;

  /**
   * Create a reservoir network (Echo State Network style)
   * @param size Reservoir size
   * @param spectralRadius Target spectral radius
   * @param inputSize Number of input neurons
   */
  createReservoir(size: number, spectralRadius?: number, inputSize?: number): void;

  /**
   * Create small-world network topology
   * @param size Number of neurons
   * @param k Number of nearest neighbors
   * @param beta Rewiring probability
   */
  createSmallWorld(size: number, k?: number, beta?: number): void;

  /**
   * Get network statistics
   * @returns Topology metrics
   */
  getTopologyStats(): TopologyStats;

  // -------------------------------------------------------------------------
  // Recording
  // -------------------------------------------------------------------------

  /**
   * Start recording neuron activity
   * @param neuronIds Neurons to record (empty = all)
   */
  startRecording(neuronIds?: string[]): void;

  /**
   * Stop recording
   * @returns Recorded activity
   */
  stopRecording(): RecordedActivity;

  /**
   * Get spike raster
   * @param startTime Start time
   * @param endTime End time
   * @returns Spike times per neuron
   */
  getSpikeRaster(startTime?: number, endTime?: number): Map<string, number[]>;
}

// ============================================================================
// Supporting Types
// ============================================================================

/** Neuron configuration */
export interface NeuronConfig {
  id?: string;
  neuronType?: 'excitatory' | 'inhibitory' | 'modulatory';
  model?: NeuronModel;
  threshold?: number;
  restPotential?: number;
  resetPotential?: number;
  refractoryPeriod?: number;
  leakConductance?: number;
  capacitance?: number;
}

/** Neuron model type */
export type NeuronModel =
  | 'lif'           // Leaky Integrate-and-Fire
  | 'izhikevich'    // Izhikevich model
  | 'hh'            // Hodgkin-Huxley
  | 'adex'          // Adaptive Exponential
  | 'srm'           // Spike Response Model
  | 'if';           // Integrate-and-Fire

/** Synapse configuration */
export interface SynapseConfig {
  weight?: number;
  delay?: number;
  plasticity?: PlasticityRule;
  synapseType?: 'ampa' | 'nmda' | 'gaba_a' | 'gaba_b' | 'generic';
  timeConstant?: number;
}

/** STDP configuration */
export interface StdpConfig {
  tauPlus: number;      // Time constant for potentiation
  tauMinus: number;     // Time constant for depression
  aPlus: number;        // Amplitude for potentiation
  aMinus: number;       // Amplitude for depression
  wMax: number;         // Maximum weight
  wMin: number;         // Minimum weight
}

/** Neuron filter criteria */
export interface NeuronFilter {
  type?: 'excitatory' | 'inhibitory' | 'modulatory';
  model?: NeuronModel;
  minPotential?: number;
  maxPotential?: number;
  isActive?: boolean;
}

/** Simulation result */
export interface SimulationResult {
  timestep: number;
  spikes: string[];           // IDs of neurons that spiked
  averagePotential: number;
  averageFiringRate: number;
  energyConsumed: number;
}

/** Plasticity statistics */
export interface PlasticityStats {
  averageWeightChange: number;
  potentiationCount: number;
  depressionCount: number;
  synapsesPruned: number;
  synapsesCreated: number;
}

/** Topology statistics */
export interface TopologyStats {
  neuronCount: number;
  synapseCount: number;
  averageConnectivity: number;
  clusteringCoefficient: number;
  averagePathLength: number;
  spectralRadius: number;
}

/** Recorded neural activity */
export interface RecordedActivity {
  duration: number;
  neuronIds: string[];
  potentials: Float32Array[];   // Time series per neuron
  spikeTimes: Map<string, number[]>;
  samplingRate: number;
}

// ============================================================================
// Factory and Utilities
// ============================================================================

/**
 * Create a nervous system engine instance
 * @param config Optional configuration
 * @returns Initialized nervous engine
 */
export function createNervousEngine(config?: NervousConfig): NervousEngine {
  const defaultConfig: NervousConfig = {
    maxNeurons: 10000,
    simulationDt: 0.1,
    enablePlasticity: true,
    ...config,
  };

  // Internal state
  const neurons = new Map<string, Neuron>();
  const synapses: Synapse[] = [];
  let neuronIdCounter = 0;
  let currentTime = 0;

  return {
    createNeuron: (neuronConfig) => {
      const id = neuronConfig.id || `neuron_${neuronIdCounter++}`;
      const neuron: Neuron = {
        id,
        potential: neuronConfig.restPotential ?? -70,
        threshold: neuronConfig.threshold ?? -55,
        refractory: 0,
        neuronType: neuronConfig.neuronType ?? 'excitatory',
      };
      neurons.set(id, neuron);
      return id;
    },
    removeNeuron: (neuronId) => {
      neurons.delete(neuronId);
    },
    getNeuron: (neuronId) => neurons.get(neuronId),
    updateNeuron: (neuronId, params) => {
      const neuron = neurons.get(neuronId);
      if (neuron) {
        Object.assign(neuron, params);
      }
    },
    listNeurons: (filter) => {
      let result = Array.from(neurons.values());
      if (filter) {
        if (filter.type) {
          result = result.filter(n => n.neuronType === filter.type);
        }
      }
      return result;
    },
    createSynapse: (presynapticId, postsynapticId, synapseConfig) => {
      const synapse: Synapse = {
        presynapticId,
        postsynapticId,
        weight: synapseConfig?.weight ?? 1.0,
        delay: synapseConfig?.delay ?? 1.0,
        plasticity: synapseConfig?.plasticity ?? { type: 'stdp', params: {} },
      };
      synapses.push(synapse);
      return `${presynapticId}->${postsynapticId}`;
    },
    removeSynapse: (presynapticId, postsynapticId) => {
      const idx = synapses.findIndex(
        s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId
      );
      if (idx >= 0) synapses.splice(idx, 1);
    },
    getSynapse: (presynapticId, postsynapticId) => {
      return synapses.find(
        s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId
      );
    },
    updateSynapse: (presynapticId, postsynapticId, params) => {
      const synapse = synapses.find(
        s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId
      );
      if (synapse) {
        Object.assign(synapse, params);
      }
    },
    listSynapses: (neuronId, direction = 'both') => {
      return synapses.filter(s => {
        if (direction === 'outgoing') return s.presynapticId === neuronId;
        if (direction === 'incoming') return s.postsynapticId === neuronId;
        return s.presynapticId === neuronId || s.postsynapticId === neuronId;
      });
    },
    step: (dt = defaultConfig.simulationDt!) => {
      currentTime += dt;
      const spikes: string[] = [];
      // Placeholder: actual simulation delegated to WASM
      return {
        timestep: currentTime,
        spikes,
        averagePotential: 0,
        averageFiringRate: 0,
        energyConsumed: 0,
      };
    },
    injectCurrent: (injections) => {
      // WASM call: ruvector_nervous_inject(injections)
    },
    propagate: (sourceIds, signal) => {
      // WASM call: ruvector_nervous_propagate(sourceIds, signal)
      return {
        activatedNeurons: [],
        spikeTimings: new Map(),
        totalActivity: 0,
      };
    },
    getState: () => ({
      neurons,
      synapses,
      globalModulation: 1.0,
      timestamp: currentTime,
    }),
    setState: (state) => {
      neurons.clear();
      state.neurons.forEach((v, k) => neurons.set(k, v));
      synapses.length = 0;
      synapses.push(...state.synapses);
      currentTime = state.timestamp;
    },
    reset: (keepTopology = false) => {
      if (!keepTopology) {
        neurons.clear();
        synapses.length = 0;
      } else {
        neurons.forEach(n => {
          n.potential = -70;
          n.refractory = 0;
        });
      }
      currentTime = 0;
    },
    applyPlasticity: (rule, learningRate = 1.0) => {
      // WASM call: ruvector_nervous_plasticity(rule, learningRate)
    },
    applyStdp: (stdpConfig) => {
      // WASM call: ruvector_nervous_stdp(config)
    },
    applyHomeostasis: (targetRate = 10) => {
      // WASM call: ruvector_nervous_homeostasis(targetRate)
    },
    getPlasticityStats: () => ({
      averageWeightChange: 0,
      potentiationCount: 0,
      depressionCount: 0,
      synapsesPruned: 0,
      synapsesCreated: 0,
    }),
    createFeedforward: (layerSizes, connectivity = 1.0) => {
      // WASM call: ruvector_nervous_create_feedforward(layerSizes, connectivity)
    },
    createRecurrent: (size, connectivity = 0.1) => {
      // WASM call: ruvector_nervous_create_recurrent(size, connectivity)
    },
    createReservoir: (size, spectralRadius = 0.9, inputSize = 10) => {
      // WASM call: ruvector_nervous_create_reservoir(size, spectralRadius, inputSize)
    },
    createSmallWorld: (size, k = 4, beta = 0.1) => {
      // WASM call: ruvector_nervous_create_small_world(size, k, beta)
    },
    getTopologyStats: () => ({
      neuronCount: neurons.size,
      synapseCount: synapses.length,
      averageConnectivity: neurons.size > 0 ? synapses.length / neurons.size : 0,
      clusteringCoefficient: 0,
      averagePathLength: 0,
      spectralRadius: 0,
    }),
    startRecording: (neuronIds) => {
      // WASM call: ruvector_nervous_start_recording(neuronIds)
    },
    stopRecording: () => ({
      duration: 0,
      neuronIds: [],
      potentials: [],
      spikeTimes: new Map(),
      samplingRate: 1000,
    }),
    getSpikeRaster: (startTime = 0, endTime = currentTime) => {
      // WASM call: ruvector_nervous_get_raster(startTime, endTime)
      return new Map();
    },
  };
}

/**
 * Create default STDP configuration
 */
export function createStdpConfig(): StdpConfig {
  return {
    tauPlus: 20,
    tauMinus: 20,
    aPlus: 0.01,
    aMinus: 0.012,
    wMax: 1.0,
    wMin: 0.0,
  };
}

/**
 * Create Izhikevich neuron parameters for different types
 */
export function izhikevichParams(type: 'regular' | 'bursting' | 'chattering' | 'fast'): {
  a: number;
  b: number;
  c: number;
  d: number;
} {
  const params = {
    regular: { a: 0.02, b: 0.2, c: -65, d: 8 },
    bursting: { a: 0.02, b: 0.2, c: -50, d: 2 },
    chattering: { a: 0.02, b: 0.2, c: -50, d: 2 },
    fast: { a: 0.1, b: 0.2, c: -65, d: 2 },
  };
  return params[type];
}
