
let imports = {};
imports['__wbindgen_placeholder__'] = module.exports;
let wasm;
const { TextDecoder } = require(`util`);

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_export_2.set(idx, obj);
    return idx;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

cachedTextDecoder.decode();

function decodeText(ptr, len) {
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

module.exports.init_wasm = function() {
    wasm.init_wasm();
};

/**
 * @returns {string}
 */
module.exports.get_version = function() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.get_version();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} agent_count
 * @returns {string}
 */
module.exports.create_nano_swarm = function(agent_count) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_nano_swarm(agent_count);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} ticks
 * @returns {number}
 */
module.exports.run_swarm_ticks = function(ticks) {
    const ret = wasm.run_swarm_ticks(ticks);
    return ret >>> 0;
};

/**
 * @param {number} qubits
 * @returns {string}
 */
module.exports.quantum_superposition = function(qubits) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.quantum_superposition(qubits);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} qubits
 * @returns {string}
 */
module.exports.quantum_superposition_old = function(qubits) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.quantum_superposition_old(qubits);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} qubits
 * @returns {number}
 */
module.exports.measure_quantum_state = function(qubits) {
    const ret = wasm.measure_quantum_state(qubits);
    return ret >>> 0;
};

/**
 * @param {number} qubits
 * @returns {number}
 */
module.exports.measure_quantum_state_old = function(qubits) {
    const ret = wasm.measure_quantum_state_old(qubits);
    return ret >>> 0;
};

/**
 * @param {number} iterations
 * @returns {number}
 */
module.exports.evolve_consciousness = function(iterations) {
    const ret = wasm.evolve_consciousness(iterations);
    return ret;
};

/**
 * @param {number} sigma
 * @param {number} rho
 * @param {number} beta
 * @returns {string}
 */
module.exports.create_lorenz_attractor = function(sigma, rho, beta) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_lorenz_attractor(sigma, rho, beta);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} x
 * @param {number} y
 * @param {number} z
 * @param {number} dt
 * @returns {string}
 */
module.exports.step_attractor = function(x, y, z, dt) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.step_attractor(x, y, z, dt);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} size
 * @param {number} tolerance
 * @returns {string}
 */
module.exports.solve_linear_system_sublinear = function(size, tolerance) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.solve_linear_system_sublinear(size, tolerance);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} size
 * @param {number} tolerance
 * @returns {string}
 */
module.exports.solve_linear_system_sublinear_old = function(size, tolerance) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.solve_linear_system_sublinear_old(size, tolerance);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} nodes
 * @param {number} damping
 * @returns {string}
 */
module.exports.compute_pagerank = function(nodes, damping) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.compute_pagerank(nodes, damping);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} horizon
 * @returns {string}
 */
module.exports.create_retrocausal_loop = function(horizon) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_retrocausal_loop(horizon);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} current_value
 * @param {number} horizon_ms
 * @returns {number}
 */
module.exports.predict_future_state = function(current_value, horizon_ms) {
    const ret = wasm.predict_future_state(current_value, horizon_ms);
    return ret;
};

/**
 * @param {number} constant
 * @returns {string}
 */
module.exports.create_lipschitz_loop = function(constant) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_lipschitz_loop(constant);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} lipschitz_constant
 * @param {number} iterations
 * @returns {boolean}
 */
module.exports.verify_convergence = function(lipschitz_constant, iterations) {
    const ret = wasm.verify_convergence(lipschitz_constant, iterations);
    return ret !== 0;
};

/**
 * @param {number} elements
 * @param {number} connections
 * @returns {number}
 */
module.exports.calculate_phi = function(elements, connections) {
    const ret = wasm.calculate_phi(elements, connections);
    return ret;
};

/**
 * @param {number} phi
 * @param {number} emergence
 * @param {number} coherence
 * @returns {string}
 */
module.exports.verify_consciousness = function(phi, emergence, coherence) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.verify_consciousness(phi, emergence, coherence);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} window_size
 * @returns {string}
 */
module.exports.detect_temporal_patterns = function(window_size) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.detect_temporal_patterns(window_size);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} qubits
 * @param {number} classical_bits
 * @returns {string}
 */
module.exports.quantum_classical_hybrid = function(qubits, classical_bits) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.quantum_classical_hybrid(qubits, classical_bits);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} learning_rate
 * @returns {string}
 */
module.exports.create_self_modifying_loop = function(learning_rate) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_self_modifying_loop(learning_rate);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} agent_count
 * @returns {string}
 */
module.exports.benchmark_nano_agents = function(agent_count) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.benchmark_nano_agents(agent_count);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @returns {string}
 */
module.exports.get_system_info = function() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.get_system_info();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} pair_type
 * @returns {string}
 */
module.exports.create_bell_state = function(pair_type) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.create_bell_state(pair_type);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} qubits
 * @returns {number}
 */
module.exports.quantum_entanglement_entropy = function(qubits) {
    const ret = wasm.quantum_entanglement_entropy(qubits);
    return ret;
};

/**
 * @param {number} value
 * @returns {string}
 */
module.exports.quantum_gate_teleportation = function(value) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.quantum_gate_teleportation(value);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

/**
 * @param {number} qubits
 * @param {number} temperature_mk
 * @returns {number}
 */
module.exports.quantum_decoherence_time = function(qubits, temperature_mk) {
    const ret = wasm.quantum_decoherence_time(qubits, temperature_mk);
    return ret;
};

/**
 * @param {number} database_size
 * @returns {number}
 */
module.exports.quantum_grover_iterations = function(database_size) {
    const ret = wasm.quantum_grover_iterations(database_size);
    return ret >>> 0;
};

/**
 * @param {number} theta
 * @returns {string}
 */
module.exports.quantum_phase_estimation = function(theta) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.quantum_phase_estimation(theta);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
};

module.exports.__wbg_call_2f8d426a20a307fe = function() { return handleError(function (arg0, arg1) {
    const ret = arg0.call(arg1);
    return ret;
}, arguments) };

module.exports.__wbg_call_f53f0647ceb9c567 = function() { return handleError(function (arg0, arg1, arg2) {
    const ret = arg0.call(arg1, arg2);
    return ret;
}, arguments) };

module.exports.__wbg_crypto_574e78ad8b13b65f = function(arg0) {
    const ret = arg0.crypto;
    return ret;
};

module.exports.__wbg_getRandomValues_b8f5dbd5f3995a9e = function() { return handleError(function (arg0, arg1) {
    arg0.getRandomValues(arg1);
}, arguments) };

module.exports.__wbg_length_904c0910ed998bf3 = function(arg0) {
    const ret = arg0.length;
    return ret;
};

module.exports.__wbg_msCrypto_a61aeb35a24c1329 = function(arg0) {
    const ret = arg0.msCrypto;
    return ret;
};

module.exports.__wbg_newnoargs_a81330f6e05d8aca = function(arg0, arg1) {
    const ret = new Function(getStringFromWasm0(arg0, arg1));
    return ret;
};

module.exports.__wbg_newwithlength_ed0ee6c1edca86fc = function(arg0) {
    const ret = new Uint8Array(arg0 >>> 0);
    return ret;
};

module.exports.__wbg_node_905d3e251edff8a2 = function(arg0) {
    const ret = arg0.node;
    return ret;
};

module.exports.__wbg_process_dc0fbacc7c1c06f7 = function(arg0) {
    const ret = arg0.process;
    return ret;
};

module.exports.__wbg_prototypesetcall_c5f74efd31aea86b = function(arg0, arg1, arg2) {
    Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
};

module.exports.__wbg_randomFillSync_ac0988aba3254290 = function() { return handleError(function (arg0, arg1) {
    arg0.randomFillSync(arg1);
}, arguments) };

module.exports.__wbg_require_60cc747a6bc5215a = function() { return handleError(function () {
    const ret = module.require;
    return ret;
}, arguments) };

module.exports.__wbg_static_accessor_GLOBAL_1f13249cc3acc96d = function() {
    const ret = typeof global === 'undefined' ? null : global;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

module.exports.__wbg_static_accessor_GLOBAL_THIS_df7ae94b1e0ed6a3 = function() {
    const ret = typeof globalThis === 'undefined' ? null : globalThis;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

module.exports.__wbg_static_accessor_SELF_6265471db3b3c228 = function() {
    const ret = typeof self === 'undefined' ? null : self;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

module.exports.__wbg_static_accessor_WINDOW_16fb482f8ec52863 = function() {
    const ret = typeof window === 'undefined' ? null : window;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

module.exports.__wbg_subarray_a219824899e59712 = function(arg0, arg1, arg2) {
    const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
    return ret;
};

module.exports.__wbg_versions_c01dfd4722a88165 = function(arg0) {
    const ret = arg0.versions;
    return ret;
};

module.exports.__wbg_wbindgenisfunction_ea72b9d66a0e1705 = function(arg0) {
    const ret = typeof(arg0) === 'function';
    return ret;
};

module.exports.__wbg_wbindgenisobject_dfe064a121d87553 = function(arg0) {
    const val = arg0;
    const ret = typeof(val) === 'object' && val !== null;
    return ret;
};

module.exports.__wbg_wbindgenisstring_4b74e4111ba029e6 = function(arg0) {
    const ret = typeof(arg0) === 'string';
    return ret;
};

module.exports.__wbg_wbindgenisundefined_71f08a6ade4354e7 = function(arg0) {
    const ret = arg0 === undefined;
    return ret;
};

module.exports.__wbg_wbindgenthrow_4c11a24fca429ccf = function(arg0, arg1) {
    throw new Error(getStringFromWasm0(arg0, arg1));
};

module.exports.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
    // Cast intrinsic for `Ref(String) -> Externref`.
    const ret = getStringFromWasm0(arg0, arg1);
    return ret;
};

module.exports.__wbindgen_cast_cb9088102bce6b30 = function(arg0, arg1) {
    // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
    const ret = getArrayU8FromWasm0(arg0, arg1);
    return ret;
};

module.exports.__wbindgen_init_externref_table = function() {
    const table = wasm.__wbindgen_export_2;
    const offset = table.grow(4);
    table.set(0, undefined);
    table.set(offset + 0, undefined);
    table.set(offset + 1, null);
    table.set(offset + 2, true);
    table.set(offset + 3, false);
    ;
};

const path = require('path').join(__dirname, 'strange_loop_bg.wasm');
const bytes = require('fs').readFileSync(path);

const wasmModule = new WebAssembly.Module(bytes);
const wasmInstance = new WebAssembly.Instance(wasmModule, imports);
wasm = wasmInstance.exports;
module.exports.__wasm = wasm;

wasm.__wbindgen_start();

