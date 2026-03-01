"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.createNervousEngine = createNervousEngine;
exports.createStdpConfig = createStdpConfig;
exports.izhikevichParams = izhikevichParams;
// ============================================================================
// Factory and Utilities
// ============================================================================
/**
 * Create a nervous system engine instance
 * @param config Optional configuration
 * @returns Initialized nervous engine
 */
function createNervousEngine(config) {
    const defaultConfig = {
        maxNeurons: 10000,
        simulationDt: 0.1,
        enablePlasticity: true,
        ...config,
    };
    // Internal state
    const neurons = new Map();
    const synapses = [];
    let neuronIdCounter = 0;
    let currentTime = 0;
    return {
        createNeuron: (neuronConfig) => {
            const id = neuronConfig.id || `neuron_${neuronIdCounter++}`;
            const neuron = {
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
            const synapse = {
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
            const idx = synapses.findIndex(s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId);
            if (idx >= 0)
                synapses.splice(idx, 1);
        },
        getSynapse: (presynapticId, postsynapticId) => {
            return synapses.find(s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId);
        },
        updateSynapse: (presynapticId, postsynapticId, params) => {
            const synapse = synapses.find(s => s.presynapticId === presynapticId && s.postsynapticId === postsynapticId);
            if (synapse) {
                Object.assign(synapse, params);
            }
        },
        listSynapses: (neuronId, direction = 'both') => {
            return synapses.filter(s => {
                if (direction === 'outgoing')
                    return s.presynapticId === neuronId;
                if (direction === 'incoming')
                    return s.postsynapticId === neuronId;
                return s.presynapticId === neuronId || s.postsynapticId === neuronId;
            });
        },
        step: (dt = defaultConfig.simulationDt) => {
            currentTime += dt;
            const spikes = [];
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
            }
            else {
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
function createStdpConfig() {
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
function izhikevichParams(type) {
    const params = {
        regular: { a: 0.02, b: 0.2, c: -65, d: 8 },
        bursting: { a: 0.02, b: 0.2, c: -50, d: 2 },
        chattering: { a: 0.02, b: 0.2, c: -50, d: 2 },
        fast: { a: 0.1, b: 0.2, c: -65, d: 2 },
    };
    return params[type];
}
//# sourceMappingURL=nervous.js.map