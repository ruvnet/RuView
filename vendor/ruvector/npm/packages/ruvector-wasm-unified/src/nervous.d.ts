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
import type { Neuron, Synapse, PlasticityRule, NervousState, PropagationResult, NervousConfig } from './types';
/**
 * Core nervous system engine for biological neural network simulation
 */
export interface NervousEngine {
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
    /**
     * Create a synapse between neurons
     * @param presynapticId Source neuron
     * @param postsynapticId Target neuron
     * @param config Synapse configuration
     * @returns Synapse ID
     */
    createSynapse(presynapticId: string, postsynapticId: string, config?: SynapseConfig): string;
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
    updateSynapse(presynapticId: string, postsynapticId: string, params: Partial<SynapseConfig>): void;
    /**
     * List synapses for a neuron
     * @param neuronId Neuron ID
     * @param direction 'incoming' | 'outgoing' | 'both'
     * @returns Array of synapses
     */
    listSynapses(neuronId: string, direction?: 'incoming' | 'outgoing' | 'both'): Synapse[];
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
export type NeuronModel = 'lif' | 'izhikevich' | 'hh' | 'adex' | 'srm' | 'if';
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
    tauPlus: number;
    tauMinus: number;
    aPlus: number;
    aMinus: number;
    wMax: number;
    wMin: number;
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
    spikes: string[];
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
    potentials: Float32Array[];
    spikeTimes: Map<string, number[]>;
    samplingRate: number;
}
/**
 * Create a nervous system engine instance
 * @param config Optional configuration
 * @returns Initialized nervous engine
 */
export declare function createNervousEngine(config?: NervousConfig): NervousEngine;
/**
 * Create default STDP configuration
 */
export declare function createStdpConfig(): StdpConfig;
/**
 * Create Izhikevich neuron parameters for different types
 */
export declare function izhikevichParams(type: 'regular' | 'bursting' | 'chattering' | 'fast'): {
    a: number;
    b: number;
    c: number;
    d: number;
};
//# sourceMappingURL=nervous.d.ts.map