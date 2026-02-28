let wasm;

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function dropObject(idx) {
    if (idx < 132) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        wasm.__wbindgen_export3(addHeapObject(e));
    }
}

let heap = new Array(128).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    }
}

let WASM_VECTOR_LEN = 0;

const ExoticEcosystemFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_exoticecosystem_free(ptr >>> 0, 1));

const WasmMorphogeneticNetworkFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmorphogeneticnetwork_free(ptr >>> 0, 1));

const WasmNAOFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmnao_free(ptr >>> 0, 1));

const WasmTimeCrystalFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmtimecrystal_free(ptr >>> 0, 1));

/**
 * Create a demonstration of all three exotic mechanisms working together
 */
export class ExoticEcosystem {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ExoticEcosystemFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_exoticecosystem_free(ptr, 0);
    }
    /**
     * Get current cell count (from morphogenetic network)
     * @returns {number}
     */
    cellCount() {
        const ret = wasm.exoticecosystem_cellCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Crystallize the time crystal
     */
    crystallize() {
        wasm.exoticecosystem_crystallize(this.__wbg_ptr);
    }
    /**
     * Get current step
     * @returns {number}
     */
    currentStep() {
        const ret = wasm.exoticecosystem_currentStep(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current member count (from NAO)
     * @returns {number}
     */
    memberCount() {
        const ret = wasm.exoticecosystem_memberCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get ecosystem summary as JSON
     * @returns {any}
     */
    summaryJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.exoticecosystem_summaryJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get current synchronization level (from time crystal)
     * @returns {number}
     */
    synchronization() {
        const ret = wasm.exoticecosystem_synchronization(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new exotic ecosystem with interconnected mechanisms
     * @param {number} agents
     * @param {number} grid_size
     * @param {number} oscillators
     */
    constructor(agents, grid_size, oscillators) {
        const ret = wasm.exoticecosystem_new(agents, grid_size, oscillators);
        this.__wbg_ptr = ret >>> 0;
        ExoticEcosystemFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Advance all systems by one step
     */
    step() {
        wasm.exoticecosystem_step(this.__wbg_ptr);
    }
    /**
     * Vote on a proposal
     * @param {string} proposal_id
     * @param {string} agent_id
     * @param {number} weight
     * @returns {boolean}
     */
    vote(proposal_id, agent_id, weight) {
        const ptr0 = passStringToWasm0(proposal_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(agent_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.exoticecosystem_vote(this.__wbg_ptr, ptr0, len0, ptr1, len1, weight);
        return ret !== 0;
    }
    /**
     * Execute a proposal
     * @param {string} proposal_id
     * @returns {boolean}
     */
    execute(proposal_id) {
        const ptr0 = passStringToWasm0(proposal_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.exoticecosystem_execute(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Propose an action in the NAO
     * @param {string} action
     * @returns {string}
     */
    propose(action) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(action, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.exoticecosystem_propose(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
}
if (Symbol.dispose) ExoticEcosystem.prototype[Symbol.dispose] = ExoticEcosystem.prototype.free;

/**
 * WASM-bindgen wrapper for MorphogeneticNetwork
 */
export class WasmMorphogeneticNetwork {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMorphogeneticNetworkFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmorphogeneticnetwork_free(ptr, 0);
    }
    /**
     * Get cell count
     * @returns {number}
     */
    cellCount() {
        const ret = wasm.wasmmorphogeneticnetwork_cellCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get all cells as JSON
     * @returns {any}
     */
    cellsJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmmorphogeneticnetwork_cellsJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get statistics as JSON
     * @returns {any}
     */
    statsJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmmorphogeneticnetwork_statsJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get stem cell count
     * @returns {number}
     */
    stemCount() {
        const ret = wasm.wasmmorphogeneticnetwork_stemCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current tick
     * @returns {number}
     */
    currentTick() {
        const ret = wasm.wasmmorphogeneticnetwork_currentTick(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get compute cell count
     * @returns {number}
     */
    computeCount() {
        const ret = wasm.wasmmorphogeneticnetwork_computeCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Differentiate stem cells
     */
    differentiate() {
        wasm.wasmmorphogeneticnetwork_differentiate(this.__wbg_ptr);
    }
    /**
     * Seed a signaling cell at position
     * @param {number} x
     * @param {number} y
     * @returns {number}
     */
    seedSignaling(x, y) {
        const ret = wasm.wasmmorphogeneticnetwork_seedSignaling(this.__wbg_ptr, x, y);
        return ret >>> 0;
    }
    /**
     * Get signaling cell count
     * @returns {number}
     */
    signalingCount() {
        const ret = wasm.wasmmorphogeneticnetwork_signalingCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Add a growth factor source
     * @param {number} x
     * @param {number} y
     * @param {string} name
     * @param {number} concentration
     */
    addGrowthSource(x, y, name, concentration) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmmorphogeneticnetwork_addGrowthSource(this.__wbg_ptr, x, y, ptr0, len0, concentration);
    }
    /**
     * Create a new morphogenetic network
     * @param {number} width
     * @param {number} height
     */
    constructor(width, height) {
        const ret = wasm.wasmmorphogeneticnetwork_new(width, height);
        this.__wbg_ptr = ret >>> 0;
        WasmMorphogeneticNetworkFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Grow the network
     * @param {number} dt
     */
    grow(dt) {
        wasm.wasmmorphogeneticnetwork_grow(this.__wbg_ptr, dt);
    }
    /**
     * Prune weak connections and dead cells
     * @param {number} threshold
     */
    prune(threshold) {
        wasm.wasmmorphogeneticnetwork_prune(this.__wbg_ptr, threshold);
    }
    /**
     * Seed a stem cell at position
     * @param {number} x
     * @param {number} y
     * @returns {number}
     */
    seedStem(x, y) {
        const ret = wasm.wasmmorphogeneticnetwork_seedStem(this.__wbg_ptr, x, y);
        return ret >>> 0;
    }
}
if (Symbol.dispose) WasmMorphogeneticNetwork.prototype[Symbol.dispose] = WasmMorphogeneticNetwork.prototype.free;

/**
 * WASM-bindgen wrapper for NeuralAutonomousOrg
 */
export class WasmNAO {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmNAOFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmnao_free(ptr, 0);
    }
    /**
     * Add a member agent with initial stake
     * @param {string} agent_id
     * @param {number} stake
     */
    addMember(agent_id, stake) {
        const ptr0 = passStringToWasm0(agent_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmnao_addMember(this.__wbg_ptr, ptr0, len0, stake);
    }
    /**
     * Get current tick
     * @returns {number}
     */
    currentTick() {
        const ret = wasm.wasmnao_currentTick(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get member count
     * @returns {number}
     */
    memberCount() {
        const ret = wasm.wasmnao_memberCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Remove a member agent
     * @param {string} agent_id
     */
    removeMember(agent_id) {
        const ptr0 = passStringToWasm0(agent_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmnao_removeMember(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get coherence between two agents (0-1)
     * @param {string} agent_a
     * @param {string} agent_b
     * @returns {number}
     */
    agentCoherence(agent_a, agent_b) {
        const ptr0 = passStringToWasm0(agent_a, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(agent_b, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnao_agentCoherence(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Get current synchronization level (0-1)
     * @returns {number}
     */
    synchronization() {
        const ret = wasm.wasmnao_synchronization(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get total voting power
     * @returns {number}
     */
    totalVotingPower() {
        const ret = wasm.wasmnao_totalVotingPower(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get active proposal count
     * @returns {number}
     */
    activeProposalCount() {
        const ret = wasm.wasmnao_activeProposalCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new NAO with the given quorum threshold (0.0 - 1.0)
     * @param {number} quorum_threshold
     */
    constructor(quorum_threshold) {
        const ret = wasm.wasmnao_new(quorum_threshold);
        this.__wbg_ptr = ret >>> 0;
        WasmNAOFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Advance simulation by one tick
     * @param {number} dt
     */
    tick(dt) {
        wasm.wasmnao_tick(this.__wbg_ptr, dt);
    }
    /**
     * Vote on a proposal
     * @param {string} proposal_id
     * @param {string} agent_id
     * @param {number} weight
     * @returns {boolean}
     */
    vote(proposal_id, agent_id, weight) {
        const ptr0 = passStringToWasm0(proposal_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(agent_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnao_vote(this.__wbg_ptr, ptr0, len0, ptr1, len1, weight);
        return ret !== 0;
    }
    /**
     * Execute a proposal if consensus reached
     * @param {string} proposal_id
     * @returns {boolean}
     */
    execute(proposal_id) {
        const ptr0 = passStringToWasm0(proposal_id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnao_execute(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new proposal, returns proposal ID
     * @param {string} action
     * @returns {string}
     */
    propose(action) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(action, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmnao_propose(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get all data as JSON
     * @returns {any}
     */
    toJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmnao_toJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WasmNAO.prototype[Symbol.dispose] = WasmNAO.prototype.free;

/**
 * WASM-bindgen wrapper for TimeCrystal
 */
export class WasmTimeCrystal {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmTimeCrystal.prototype);
        obj.__wbg_ptr = ptr;
        WasmTimeCrystalFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmTimeCrystalFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmtimecrystal_free(ptr, 0);
    }
    /**
     * Get robustness measure
     * @returns {number}
     */
    robustness() {
        const ret = wasm.wasmtimecrystal_robustness(this.__wbg_ptr);
        return ret;
    }
    /**
     * Crystallize to establish periodic order
     */
    crystallize() {
        wasm.wasmtimecrystal_crystallize(this.__wbg_ptr);
    }
    /**
     * Get phases as JSON array
     * @returns {any}
     */
    phasesJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimecrystal_phasesJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Set driving strength
     * @param {number} strength
     */
    setDriving(strength) {
        wasm.wasmtimecrystal_setDriving(this.__wbg_ptr, strength);
    }
    /**
     * Get current step
     * @returns {number}
     */
    currentStep() {
        const ret = wasm.wasmtimecrystal_currentStep(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current pattern type as string
     * @returns {string}
     */
    patternType() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimecrystal_patternType(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Set coupling strength
     * @param {number} coupling
     */
    setCoupling(coupling) {
        wasm.wasmtimecrystal_setCoupling(this.__wbg_ptr, coupling);
    }
    /**
     * Set disorder level
     * @param {number} disorder
     */
    setDisorder(disorder) {
        wasm.wasmtimecrystal_setDisorder(this.__wbg_ptr, disorder);
    }
    /**
     * Get signals as JSON array
     * @returns {any}
     */
    signalsJson() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimecrystal_signalsJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return takeObject(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a synchronized crystal
     * @param {number} n
     * @param {number} period_ms
     * @returns {WasmTimeCrystal}
     */
    static synchronized(n, period_ms) {
        const ret = wasm.wasmtimecrystal_synchronized(n, period_ms);
        return WasmTimeCrystal.__wrap(ret);
    }
    /**
     * Get collective spin
     * @returns {number}
     */
    collectiveSpin() {
        const ret = wasm.wasmtimecrystal_collectiveSpin(this.__wbg_ptr);
        return ret;
    }
    /**
     * Check if crystallized
     * @returns {boolean}
     */
    isCrystallized() {
        const ret = wasm.wasmtimecrystal_isCrystallized(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get order parameter (synchronization level)
     * @returns {number}
     */
    orderParameter() {
        const ret = wasm.exoticecosystem_synchronization(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get number of oscillators
     * @returns {number}
     */
    oscillatorCount() {
        const ret = wasm.wasmtimecrystal_oscillatorCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new time crystal with n oscillators
     * @param {number} n
     * @param {number} period_ms
     */
    constructor(n, period_ms) {
        const ret = wasm.wasmtimecrystal_new(n, period_ms);
        this.__wbg_ptr = ret >>> 0;
        WasmTimeCrystalFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Advance one tick, returns coordination pattern as Uint8Array
     * @returns {Uint8Array}
     */
    tick() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmtimecrystal_tick(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Apply perturbation
     * @param {number} strength
     */
    perturb(strength) {
        wasm.wasmtimecrystal_perturb(this.__wbg_ptr, strength);
    }
    /**
     * Get period in milliseconds
     * @returns {number}
     */
    periodMs() {
        const ret = wasm.wasmtimecrystal_periodMs(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) WasmTimeCrystal.prototype[Symbol.dispose] = WasmTimeCrystal.prototype.free;

/**
 * Get information about available exotic mechanisms
 * @returns {any}
 */
export function available_mechanisms() {
    const ret = wasm.available_mechanisms();
    return takeObject(ret);
}

/**
 * Initialize the WASM module with panic hook
 */
export function init() {
    wasm.init();
}

/**
 * Get the version of the ruvector-exotic-wasm crate
 * @returns {string}
 */
export function version() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.version(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
    }
}

const EXPECTED_RESPONSE_TYPES = new Set(['basic', 'cors', 'default']);

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_Error_52673b7de5a0ca89 = function(arg0, arg1) {
        const ret = Error(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_String_8f0eb39a4a4c2f66 = function(arg0, arg1) {
        const ret = String(getObject(arg1));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_debug_string_adfb662ae34724b6 = function(arg0, arg1) {
        const ret = debugString(getObject(arg1));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_is_function_8d400b8b1af978cd = function(arg0) {
        const ret = typeof(getObject(arg0)) === 'function';
        return ret;
    };
    imports.wbg.__wbg___wbindgen_is_object_ce774f3490692386 = function(arg0) {
        const val = getObject(arg0);
        const ret = typeof(val) === 'object' && val !== null;
        return ret;
    };
    imports.wbg.__wbg___wbindgen_is_string_704ef9c8fc131030 = function(arg0) {
        const ret = typeof(getObject(arg0)) === 'string';
        return ret;
    };
    imports.wbg.__wbg___wbindgen_is_undefined_f6b95eab589e0269 = function(arg0) {
        const ret = getObject(arg0) === undefined;
        return ret;
    };
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_call_3020136f7a2d6e44 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_call_abb4ff46ce38be40 = function() { return handleError(function (arg0, arg1) {
        const ret = getObject(arg0).call(getObject(arg1));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_crypto_574e78ad8b13b65f = function(arg0) {
        const ret = getObject(arg0).crypto;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_getRandomValues_b8f5dbd5f3995a9e = function() { return handleError(function (arg0, arg1) {
        getObject(arg0).getRandomValues(getObject(arg1));
    }, arguments) };
    imports.wbg.__wbg_length_22ac23eaec9d8053 = function(arg0) {
        const ret = getObject(arg0).length;
        return ret;
    };
    imports.wbg.__wbg_msCrypto_a61aeb35a24c1329 = function(arg0) {
        const ret = getObject(arg0).msCrypto;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_1ba21ce319a06297 = function() {
        const ret = new Object();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_25f239778d6112b9 = function() {
        const ret = new Array();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_b546ae120718850e = function() {
        const ret = new Map();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_no_args_cb138f77cf6151ee = function(arg0, arg1) {
        const ret = new Function(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_with_length_aa5eaf41d35235e5 = function(arg0) {
        const ret = new Uint8Array(arg0 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_node_905d3e251edff8a2 = function(arg0) {
        const ret = getObject(arg0).node;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_process_dc0fbacc7c1c06f7 = function(arg0) {
        const ret = getObject(arg0).process;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_prototypesetcall_dfe9b766cdc1f1fd = function(arg0, arg1, arg2) {
        Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
    };
    imports.wbg.__wbg_randomFillSync_ac0988aba3254290 = function() { return handleError(function (arg0, arg1) {
        getObject(arg0).randomFillSync(takeObject(arg1));
    }, arguments) };
    imports.wbg.__wbg_require_60cc747a6bc5215a = function() { return handleError(function () {
        const ret = module.require;
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_set_3f1d0b984ed272ed = function(arg0, arg1, arg2) {
        getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
    };
    imports.wbg.__wbg_set_7df433eea03a5c14 = function(arg0, arg1, arg2) {
        getObject(arg0)[arg1 >>> 0] = takeObject(arg2);
    };
    imports.wbg.__wbg_set_efaaf145b9377369 = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).set(getObject(arg1), getObject(arg2));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_static_accessor_GLOBAL_769e6b65d6557335 = function() {
        const ret = typeof global === 'undefined' ? null : global;
        return isLikeNone(ret) ? 0 : addHeapObject(ret);
    };
    imports.wbg.__wbg_static_accessor_GLOBAL_THIS_60cf02db4de8e1c1 = function() {
        const ret = typeof globalThis === 'undefined' ? null : globalThis;
        return isLikeNone(ret) ? 0 : addHeapObject(ret);
    };
    imports.wbg.__wbg_static_accessor_SELF_08f5a74c69739274 = function() {
        const ret = typeof self === 'undefined' ? null : self;
        return isLikeNone(ret) ? 0 : addHeapObject(ret);
    };
    imports.wbg.__wbg_static_accessor_WINDOW_a8924b26aa92d024 = function() {
        const ret = typeof window === 'undefined' ? null : window;
        return isLikeNone(ret) ? 0 : addHeapObject(ret);
    };
    imports.wbg.__wbg_subarray_845f2f5bce7d061a = function(arg0, arg1, arg2) {
        const ret = getObject(arg0).subarray(arg1 >>> 0, arg2 >>> 0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_versions_c01dfd4722a88165 = function(arg0) {
        const ret = getObject(arg0).versions;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_4625c577ab2ec9ee = function(arg0) {
        // Cast intrinsic for `U64 -> Externref`.
        const ret = BigInt.asUintN(64, arg0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_9ae0607507abb057 = function(arg0) {
        // Cast intrinsic for `I64 -> Externref`.
        const ret = arg0;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_cb9088102bce6b30 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
        const ret = getArrayU8FromWasm0(arg0, arg1);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
        // Cast intrinsic for `F64 -> Externref`.
        const ret = arg0;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_object_clone_ref = function(arg0) {
        const ret = getObject(arg0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
        takeObject(arg0);
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined') {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('ruvector_exotic_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
