let wasm;

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
}

function dropObject(idx) {
    if (idx < 132) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
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

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
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

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
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

const BTSPAssociativeMemoryFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_btspassociativememory_free(ptr >>> 0, 1));

const BTSPLayerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_btsplayer_free(ptr >>> 0, 1));

const BTSPSynapseFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_btspsynapse_free(ptr >>> 0, 1));

const GlobalWorkspaceFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_globalworkspace_free(ptr >>> 0, 1));

const HdcMemoryFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_hdcmemory_free(ptr >>> 0, 1));

const HypervectorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_hypervector_free(ptr >>> 0, 1));

const KWTALayerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_kwtalayer_free(ptr >>> 0, 1));

const WTALayerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wtalayer_free(ptr >>> 0, 1));

const WorkspaceItemFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_workspaceitem_free(ptr >>> 0, 1));

/**
 * Associative memory using BTSP for key-value storage
 */
export class BTSPAssociativeMemory {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BTSPAssociativeMemoryFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_btspassociativememory_free(ptr, 0);
    }
    /**
     * Get memory dimensions
     * @returns {any}
     */
    dimensions() {
        const ret = wasm.btspassociativememory_dimensions(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Store key-value association in one shot
     * @param {Float32Array} key
     * @param {Float32Array} value
     */
    store_one_shot(key, value) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(key, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayF32ToWasm0(value, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.btspassociativememory_store_one_shot(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create new associative memory
     *
     * # Arguments
     * * `input_size` - Dimension of key vectors
     * * `output_size` - Dimension of value vectors
     * @param {number} input_size
     * @param {number} output_size
     */
    constructor(input_size, output_size) {
        const ret = wasm.btspassociativememory_new(input_size, output_size);
        this.__wbg_ptr = ret >>> 0;
        BTSPAssociativeMemoryFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Retrieve value from key
     * @param {Float32Array} query
     * @returns {Float32Array}
     */
    retrieve(query) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.btspassociativememory_retrieve(retptr, this.__wbg_ptr, ptr0, len0);
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
if (Symbol.dispose) BTSPAssociativeMemory.prototype[Symbol.dispose] = BTSPAssociativeMemory.prototype.free;

/**
 * BTSP Layer for one-shot learning
 *
 * # Performance
 * - One-shot learning: immediate, no iteration
 * - Forward pass: <10us for 10K synapses
 */
export class BTSPLayer {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BTSPLayerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_btsplayer_free(ptr, 0);
    }
    /**
     * Get weights as Float32Array
     * @returns {Float32Array}
     */
    get_weights() {
        const ret = wasm.btsplayer_get_weights(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * One-shot association: learn pattern -> target in single step
     *
     * This is the key BTSP capability: immediate learning without iteration.
     * Uses gradient normalization for single-step convergence.
     * @param {Float32Array} pattern
     * @param {number} target
     */
    one_shot_associate(pattern, target) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(pattern, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.btsplayer_one_shot_associate(retptr, this.__wbg_ptr, ptr0, len0, target);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a new BTSP layer
     *
     * # Arguments
     * * `size` - Number of synapses (input dimension)
     * * `tau` - Time constant in milliseconds (2000ms default)
     * @param {number} size
     * @param {number} tau
     */
    constructor(size, tau) {
        const ret = wasm.btsplayer_new(size, tau);
        this.__wbg_ptr = ret >>> 0;
        BTSPLayerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get number of synapses
     * @returns {number}
     */
    get size() {
        const ret = wasm.btsplayer_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Reset layer to initial state
     */
    reset() {
        wasm.btsplayer_reset(this.__wbg_ptr);
    }
    /**
     * Forward pass: compute layer output
     * @param {Float32Array} input
     * @returns {number}
     */
    forward(input) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.btsplayer_forward(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getFloat32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) BTSPLayer.prototype[Symbol.dispose] = BTSPLayer.prototype.free;

/**
 * BTSP synapse with eligibility trace and bidirectional plasticity
 */
export class BTSPSynapse {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BTSPSynapseFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_btspsynapse_free(ptr, 0);
    }
    /**
     * Get eligibility trace
     * @returns {number}
     */
    get eligibility_trace() {
        const ret = wasm.btspsynapse_eligibility_trace(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new BTSP synapse
     *
     * # Arguments
     * * `initial_weight` - Starting weight (0.0 to 1.0)
     * * `tau_btsp` - Time constant in milliseconds (1000-3000ms recommended)
     * @param {number} initial_weight
     * @param {number} tau_btsp
     */
    constructor(initial_weight, tau_btsp) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.btspsynapse_new(retptr, initial_weight, tau_btsp);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            BTSPSynapseFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Update synapse based on activity and plateau signal
     *
     * # Arguments
     * * `presynaptic_active` - Is presynaptic neuron firing?
     * * `plateau_signal` - Dendritic plateau potential detected?
     * * `dt` - Time step in milliseconds
     * @param {boolean} presynaptic_active
     * @param {boolean} plateau_signal
     * @param {number} dt
     */
    update(presynaptic_active, plateau_signal, dt) {
        wasm.btspsynapse_update(this.__wbg_ptr, presynaptic_active, plateau_signal, dt);
    }
    /**
     * Get current weight
     * @returns {number}
     */
    get weight() {
        const ret = wasm.btspsynapse_weight(this.__wbg_ptr);
        return ret;
    }
    /**
     * Compute synaptic output
     * @param {number} input
     * @returns {number}
     */
    forward(input) {
        const ret = wasm.btspsynapse_forward(this.__wbg_ptr, input);
        return ret;
    }
}
if (Symbol.dispose) BTSPSynapse.prototype[Symbol.dispose] = BTSPSynapse.prototype.free;

/**
 * Global workspace with limited capacity and competitive dynamics
 *
 * Implements attention and conscious access mechanisms based on
 * Global Workspace Theory.
 */
export class GlobalWorkspace {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(GlobalWorkspace.prototype);
        obj.__wbg_ptr = ptr;
        GlobalWorkspaceFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        GlobalWorkspaceFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_globalworkspace_free(ptr, 0);
    }
    /**
     * Get current load (0.0 to 1.0)
     * @returns {number}
     */
    current_load() {
        const ret = wasm.globalworkspace_current_load(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get most salient item
     * @returns {WorkspaceItem | undefined}
     */
    most_salient() {
        const ret = wasm.globalworkspace_most_salient(this.__wbg_ptr);
        return ret === 0 ? undefined : WorkspaceItem.__wrap(ret);
    }
    /**
     * Retrieve top-k most salient representations
     * @param {number} k
     * @returns {any}
     */
    retrieve_top_k(k) {
        const ret = wasm.globalworkspace_retrieve_top_k(this.__wbg_ptr, k);
        return takeObject(ret);
    }
    /**
     * Set salience decay rate
     * @param {number} decay
     */
    set_decay_rate(decay) {
        wasm.globalworkspace_set_decay_rate(this.__wbg_ptr, decay);
    }
    /**
     * Create with custom threshold
     * @param {number} capacity
     * @param {number} threshold
     * @returns {GlobalWorkspace}
     */
    static with_threshold(capacity, threshold) {
        const ret = wasm.globalworkspace_with_threshold(capacity, threshold);
        return GlobalWorkspace.__wrap(ret);
    }
    /**
     * Get available slots
     * @returns {number}
     */
    available_slots() {
        const ret = wasm.globalworkspace_available_slots(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get average salience
     * @returns {number}
     */
    average_salience() {
        const ret = wasm.globalworkspace_average_salience(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get current number of representations
     * @returns {number}
     */
    get len() {
        const ret = wasm.globalworkspace_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new global workspace
     *
     * # Arguments
     * * `capacity` - Maximum number of representations (typically 4-7)
     * @param {number} capacity
     */
    constructor(capacity) {
        const ret = wasm.globalworkspace_new(capacity);
        this.__wbg_ptr = ret >>> 0;
        GlobalWorkspaceFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Clear all representations
     */
    clear() {
        wasm.globalworkspace_clear(this.__wbg_ptr);
    }
    /**
     * Run competitive dynamics (salience decay and pruning)
     */
    compete() {
        wasm.globalworkspace_compete(this.__wbg_ptr);
    }
    /**
     * Check if workspace is at capacity
     * @returns {boolean}
     */
    is_full() {
        const ret = wasm.globalworkspace_is_full(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get workspace capacity
     * @returns {number}
     */
    get capacity() {
        const ret = wasm.globalworkspace_capacity(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if workspace is empty
     * @returns {boolean}
     */
    is_empty() {
        const ret = wasm.globalworkspace_is_empty(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Retrieve all current representations as JSON
     * @returns {any}
     */
    retrieve() {
        const ret = wasm.globalworkspace_retrieve(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Broadcast a representation to the workspace
     *
     * Returns true if accepted, false if rejected.
     * @param {WorkspaceItem} item
     * @returns {boolean}
     */
    broadcast(item) {
        _assertClass(item, WorkspaceItem);
        var ptr0 = item.__destroy_into_raw();
        const ret = wasm.globalworkspace_broadcast(this.__wbg_ptr, ptr0);
        return ret !== 0;
    }
}
if (Symbol.dispose) GlobalWorkspace.prototype[Symbol.dispose] = GlobalWorkspace.prototype.free;

/**
 * HDC Memory for storing and retrieving hypervectors by label
 */
export class HdcMemory {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        HdcMemoryFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_hdcmemory_free(ptr, 0);
    }
    /**
     * Get a vector by label
     * @param {string} label
     * @returns {Hypervector | undefined}
     */
    get(label) {
        const ptr0 = passStringToWasm0(label, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.hdcmemory_get(this.__wbg_ptr, ptr0, len0);
        return ret === 0 ? undefined : Hypervector.__wrap(ret);
    }
    /**
     * Check if a label exists
     * @param {string} label
     * @returns {boolean}
     */
    has(label) {
        const ptr0 = passStringToWasm0(label, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.hdcmemory_has(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new empty HDC memory
     */
    constructor() {
        const ret = wasm.hdcmemory_new();
        this.__wbg_ptr = ret >>> 0;
        HdcMemoryFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get number of stored vectors
     * @returns {number}
     */
    get size() {
        const ret = wasm.hdcmemory_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Clear all stored vectors
     */
    clear() {
        wasm.hdcmemory_clear(this.__wbg_ptr);
    }
    /**
     * Store a hypervector with a label
     * @param {string} label
     * @param {Hypervector} vector
     */
    store(label, vector) {
        const ptr0 = passStringToWasm0(label, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        _assertClass(vector, Hypervector);
        var ptr1 = vector.__destroy_into_raw();
        wasm.hdcmemory_store(this.__wbg_ptr, ptr0, len0, ptr1);
    }
    /**
     * Find the k most similar vectors to query
     * @param {Hypervector} query
     * @param {number} k
     * @returns {any}
     */
    top_k(query, k) {
        _assertClass(query, Hypervector);
        const ret = wasm.hdcmemory_top_k(this.__wbg_ptr, query.__wbg_ptr, k);
        return takeObject(ret);
    }
    /**
     * Retrieve vectors similar to query above threshold
     *
     * Returns array of [label, similarity] pairs
     * @param {Hypervector} query
     * @param {number} threshold
     * @returns {any}
     */
    retrieve(query, threshold) {
        _assertClass(query, Hypervector);
        const ret = wasm.hdcmemory_retrieve(this.__wbg_ptr, query.__wbg_ptr, threshold);
        return takeObject(ret);
    }
}
if (Symbol.dispose) HdcMemory.prototype[Symbol.dispose] = HdcMemory.prototype.free;

/**
 * A binary hypervector with 10,000 bits
 *
 * # Performance
 * - Memory: 1,248 bytes per vector
 * - XOR binding: <50ns
 * - Similarity: <100ns with SIMD popcount
 */
export class Hypervector {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Hypervector.prototype);
        obj.__wbg_ptr = ptr;
        HypervectorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        HypervectorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_hypervector_free(ptr, 0);
    }
    /**
     * Create from raw bytes
     * @param {Uint8Array} bytes
     * @returns {Hypervector}
     */
    static from_bytes(bytes) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.hypervector_from_bytes(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return Hypervector.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Compute similarity between two hypervectors
     *
     * Returns a value in [-1.0, 1.0] where:
     * - 1.0 = identical vectors
     * - 0.0 = random/orthogonal vectors
     * - -1.0 = completely opposite vectors
     * @param {Hypervector} other
     * @returns {number}
     */
    similarity(other) {
        _assertClass(other, Hypervector);
        const ret = wasm.hypervector_similarity(this.__wbg_ptr, other.__wbg_ptr);
        return ret;
    }
    /**
     * Compute Hamming distance (number of differing bits)
     * @param {Hypervector} other
     * @returns {number}
     */
    hamming_distance(other) {
        _assertClass(other, Hypervector);
        const ret = wasm.hypervector_hamming_distance(this.__wbg_ptr, other.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a zero hypervector
     */
    constructor() {
        const ret = wasm.hypervector_new();
        this.__wbg_ptr = ret >>> 0;
        HypervectorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Bind two hypervectors using XOR
     *
     * Binding is associative, commutative, and self-inverse:
     * - a.bind(b) == b.bind(a)
     * - a.bind(b).bind(b) == a
     * @param {Hypervector} other
     * @returns {Hypervector}
     */
    bind(other) {
        _assertClass(other, Hypervector);
        const ret = wasm.hypervector_bind(this.__wbg_ptr, other.__wbg_ptr);
        return Hypervector.__wrap(ret);
    }
    /**
     * Create a random hypervector with ~50% bits set
     * @returns {Hypervector}
     */
    static random() {
        const ret = wasm.hypervector_random();
        return Hypervector.__wrap(ret);
    }
    /**
     * Bundle multiple vectors by majority voting on each bit
     * @param {Hypervector} a
     * @param {Hypervector} b
     * @param {Hypervector} c
     * @returns {Hypervector}
     */
    static bundle_3(a, b, c) {
        _assertClass(a, Hypervector);
        _assertClass(b, Hypervector);
        _assertClass(c, Hypervector);
        const ret = wasm.hypervector_bundle_3(a.__wbg_ptr, b.__wbg_ptr, c.__wbg_ptr);
        return Hypervector.__wrap(ret);
    }
    /**
     * Count the number of set bits (population count)
     * @returns {number}
     */
    popcount() {
        const ret = wasm.hypervector_popcount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the raw bits as Uint8Array (for serialization)
     * @returns {Uint8Array}
     */
    to_bytes() {
        const ret = wasm.hypervector_to_bytes(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Get number of bits
     * @returns {number}
     */
    get dimension() {
        const ret = wasm.hypervector_dimension(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a hypervector from a seed for reproducibility
     * @param {bigint} seed
     * @returns {Hypervector}
     */
    static from_seed(seed) {
        const ret = wasm.hypervector_from_seed(seed);
        return Hypervector.__wrap(ret);
    }
}
if (Symbol.dispose) Hypervector.prototype[Symbol.dispose] = Hypervector.prototype.free;

/**
 * K-Winner-Take-All layer for sparse distributed coding
 *
 * Selects top-k neurons with highest activations.
 *
 * # Performance
 * - O(n + k log k) using partial sorting
 * - <10us for 1000 neurons, k=50
 */
export class KWTALayer {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        KWTALayerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_kwtalayer_free(ptr, 0);
    }
    /**
     * Set activation threshold
     * @param {number} threshold
     */
    with_threshold(threshold) {
        wasm.kwtalayer_with_threshold(this.__wbg_ptr, threshold);
    }
    /**
     * Select top-k neurons with their activation values
     *
     * Returns array of [index, value] pairs.
     * @param {Float32Array} inputs
     * @returns {any}
     */
    select_with_values(inputs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(inputs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.kwtalayer_select_with_values(retptr, this.__wbg_ptr, ptr0, len0);
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
     * Create sparse activation vector (only top-k preserved)
     * @param {Float32Array} inputs
     * @returns {Float32Array}
     */
    sparse_activations(inputs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(inputs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.kwtalayer_sparse_activations(retptr, this.__wbg_ptr, ptr0, len0);
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
     * Get number of winners
     * @returns {number}
     */
    get k() {
        const ret = wasm.kwtalayer_k(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new K-WTA layer
     *
     * # Arguments
     * * `size` - Total number of neurons
     * * `k` - Number of winners to select
     * @param {number} size
     * @param {number} k
     */
    constructor(size, k) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.kwtalayer_new(retptr, size, k);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            KWTALayerFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get layer size
     * @returns {number}
     */
    get size() {
        const ret = wasm.kwtalayer_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Select top-k neurons
     *
     * Returns indices of k neurons with highest activations, sorted descending.
     * @param {Float32Array} inputs
     * @returns {Uint32Array}
     */
    select(inputs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(inputs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.kwtalayer_select(retptr, this.__wbg_ptr, ptr0, len0);
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
if (Symbol.dispose) KWTALayer.prototype[Symbol.dispose] = KWTALayer.prototype.free;

/**
 * Winner-Take-All competition layer
 *
 * Implements neural competition where the highest-activation neuron
 * wins and suppresses others through lateral inhibition.
 *
 * # Performance
 * - <1us winner selection for 1000 neurons
 */
export class WTALayer {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WTALayerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wtalayer_free(ptr, 0);
    }
    /**
     * Soft competition with normalized activations
     *
     * Returns activation levels for all neurons after softmax-like normalization.
     * @param {Float32Array} inputs
     * @returns {Float32Array}
     */
    compete_soft(inputs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(inputs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wtalayer_compete_soft(retptr, this.__wbg_ptr, ptr0, len0);
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
     * Get current membrane potentials
     * @returns {Float32Array}
     */
    get_membranes() {
        const ret = wasm.wtalayer_get_membranes(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Set refractory period
     * @param {number} period
     */
    set_refractory_period(period) {
        wasm.wtalayer_set_refractory_period(this.__wbg_ptr, period);
    }
    /**
     * Create a new WTA layer
     *
     * # Arguments
     * * `size` - Number of competing neurons
     * * `threshold` - Activation threshold for firing
     * * `inhibition` - Lateral inhibition strength (0.0-1.0)
     * @param {number} size
     * @param {number} threshold
     * @param {number} inhibition
     */
    constructor(size, threshold, inhibition) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wtalayer_new(retptr, size, threshold, inhibition);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            WTALayerFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get layer size
     * @returns {number}
     */
    get size() {
        const ret = wasm.btsplayer_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Reset layer state
     */
    reset() {
        wasm.wtalayer_reset(this.__wbg_ptr);
    }
    /**
     * Run winner-take-all competition
     *
     * Returns the index of the winning neuron, or -1 if no neuron exceeds threshold.
     * @param {Float32Array} inputs
     * @returns {number}
     */
    compete(inputs) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(inputs, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wtalayer_compete(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) WTALayer.prototype[Symbol.dispose] = WTALayer.prototype.free;

/**
 * Item in the global workspace
 */
export class WorkspaceItem {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WorkspaceItem.prototype);
        obj.__wbg_ptr = ptr;
        WorkspaceItemFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WorkspaceItemFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_workspaceitem_free(ptr, 0);
    }
    /**
     * Check if expired
     * @param {bigint} current_time
     * @returns {boolean}
     */
    is_expired(current_time) {
        const ret = wasm.workspaceitem_is_expired(this.__wbg_ptr, current_time);
        return ret !== 0;
    }
    /**
     * Create with custom decay and lifetime
     * @param {Float32Array} content
     * @param {number} salience
     * @param {number} source_module
     * @param {bigint} timestamp
     * @param {number} decay_rate
     * @param {bigint} lifetime
     * @returns {WorkspaceItem}
     */
    static with_decay(content, salience, source_module, timestamp, decay_rate, lifetime) {
        const ptr0 = passArrayF32ToWasm0(content, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.workspaceitem_with_decay(ptr0, len0, salience, source_module, timestamp, decay_rate, lifetime);
        return WorkspaceItem.__wrap(ret);
    }
    /**
     * Apply temporal decay
     * @param {number} dt
     */
    apply_decay(dt) {
        wasm.workspaceitem_apply_decay(this.__wbg_ptr, dt);
    }
    /**
     * Get content as Float32Array
     * @returns {Float32Array}
     */
    get_content() {
        const ret = wasm.workspaceitem_get_content(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Get source module
     * @returns {number}
     */
    get source_module() {
        const ret = wasm.workspaceitem_source_module(this.__wbg_ptr);
        return ret;
    }
    /**
     * Update salience
     * @param {number} new_salience
     */
    update_salience(new_salience) {
        wasm.workspaceitem_update_salience(this.__wbg_ptr, new_salience);
    }
    /**
     * Get ID
     * @returns {bigint}
     */
    get id() {
        const ret = wasm.workspaceitem_id(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Create a new workspace item
     * @param {Float32Array} content
     * @param {number} salience
     * @param {number} source_module
     * @param {bigint} timestamp
     */
    constructor(content, salience, source_module, timestamp) {
        const ptr0 = passArrayF32ToWasm0(content, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.workspaceitem_new(ptr0, len0, salience, source_module, timestamp);
        this.__wbg_ptr = ret >>> 0;
        WorkspaceItemFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get salience
     * @returns {number}
     */
    get salience() {
        const ret = wasm.workspaceitem_salience(this.__wbg_ptr);
        return ret;
    }
    /**
     * Compute content magnitude (L2 norm)
     * @returns {number}
     */
    magnitude() {
        const ret = wasm.workspaceitem_magnitude(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get timestamp
     * @returns {bigint}
     */
    get timestamp() {
        const ret = wasm.workspaceitem_timestamp(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
}
if (Symbol.dispose) WorkspaceItem.prototype[Symbol.dispose] = WorkspaceItem.prototype.free;

/**
 * Get information about available bio-inspired mechanisms
 * @returns {any}
 */
export function available_mechanisms() {
    const ret = wasm.available_mechanisms();
    return takeObject(ret);
}

/**
 * Get biological references for the mechanisms
 * @returns {any}
 */
export function biological_references() {
    const ret = wasm.biological_references();
    return takeObject(ret);
}

/**
 * Initialize the WASM module with panic hook
 */
export function init() {
    wasm.init();
}

/**
 * Get performance targets for each mechanism
 * @returns {any}
 */
export function performance_targets() {
    const ret = wasm.performance_targets();
    return takeObject(ret);
}

/**
 * Get the version of the crate
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
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
        }
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
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_b546ae120718850e = function() {
        const ret = new Map();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_from_slice_41e2764a343e3cb1 = function(arg0, arg1) {
        const ret = new Float32Array(getArrayF32FromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_from_slice_db0691b69e9d3891 = function(arg0, arg1) {
        const ret = new Uint32Array(getArrayU32FromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_from_slice_f9c22b9153b26992 = function(arg0, arg1) {
        const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_no_args_cb138f77cf6151ee = function(arg0, arg1) {
        const ret = new Function(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_with_length_202b3db94ba5fc86 = function(arg0) {
        const ret = new Uint32Array(arg0 >>> 0);
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
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = getObject(arg1).stack;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
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
    cachedFloat32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
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
        module_or_path = new URL('ruvector_nervous_system_wasm_bg.wasm', import.meta.url);
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
