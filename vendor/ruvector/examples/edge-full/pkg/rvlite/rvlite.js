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

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => state.dtor(state.a, state.b));

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

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
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

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
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

function __wasm_bindgen_func_elem_690(arg0, arg1, arg2) {
    wasm.__wasm_bindgen_func_elem_690(arg0, arg1, addHeapObject(arg2));
}

function __wasm_bindgen_func_elem_1412(arg0, arg1, arg2, arg3) {
    wasm.__wasm_bindgen_func_elem_1412(arg0, arg1, addHeapObject(arg2), addHeapObject(arg3));
}

const BaseLoRAFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_baselora_free(ptr >>> 0, 1));

const EmbedderFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_embedder_free(ptr >>> 0, 1));

const EmbeddingConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_embeddingconfig_free(ptr >>> 0, 1));

const LoraConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_loraconfig_free(ptr >>> 0, 1));

const MicroLoRAFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_microlora_free(ptr >>> 0, 1));

const RvLiteFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rvlite_free(ptr >>> 0, 1));

const RvLiteConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rvliteconfig_free(ptr >>> 0, 1));

const TrmConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_trmconfig_free(ptr >>> 0, 1));

const TrmEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_trmengine_free(ptr >>> 0, 1));

const TrmResultFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_trmresult_free(ptr >>> 0, 1));

/**
 * BaseLoRA adapter for background adaptation
 */
export class BaseLoRA {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BaseLoRAFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_baselora_free(ptr, 0);
    }
    /**
     * @param {MicroLoRA} micro
     * @param {number} blend_factor
     */
    distillFrom(micro, blend_factor) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(micro, MicroLoRA);
            wasm.baselora_distillFrom(retptr, this.__wbg_ptr, micro.__wbg_ptr, blend_factor);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    applyGradients() {
        wasm.baselora_applyGradients(this.__wbg_ptr);
    }
    /**
     * @param {LoraConfig} config
     */
    constructor(config) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(config, LoraConfig);
            var ptr0 = config.__destroy_into_raw();
            wasm.baselora_new(retptr, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            BaseLoRAFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @returns {any}
     */
    stats() {
        const ret = wasm.baselora_stats(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    forward(input) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.baselora_forward(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v2 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) BaseLoRA.prototype[Symbol.dispose] = BaseLoRA.prototype.free;

/**
 * Lightweight embedding engine using mean pooling
 * This is a simplified implementation that works without model weights
 * for basic semantic similarity using TF-IDF-like approach
 */
export class Embedder {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Embedder.prototype);
        obj.__wbg_ptr = ptr;
        EmbedderFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EmbedderFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_embedder_free(ptr, 0);
    }
    /**
     * Get embedding dimensions
     * @returns {number}
     */
    dimensions() {
        const ret = wasm.embedder_dimensions(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Compute similarity between two texts
     * @param {string} text_a
     * @param {string} text_b
     * @returns {number}
     */
    similarity(text_a, text_b) {
        const ptr0 = passStringToWasm0(text_a, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(text_b, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.embedder_similarity(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Batch embed multiple texts
     * Takes JsValue array of strings, returns JsValue array of Float32Arrays
     * @param {any} texts
     * @returns {any}
     */
    embed_batch(texts) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.embedder_embed_batch(retptr, this.__wbg_ptr, addHeapObject(texts));
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
     * Create embedder with custom config
     * @param {EmbeddingConfig} config
     * @returns {Embedder}
     */
    static with_config(config) {
        _assertClass(config, EmbeddingConfig);
        var ptr0 = config.__destroy_into_raw();
        const ret = wasm.embedder_with_config(ptr0);
        return Embedder.__wrap(ret);
    }
    /**
     * Compute cosine similarity between two embeddings (JS arrays)
     * @param {Float32Array} a
     * @param {Float32Array} b
     * @returns {number}
     */
    static cosine_similarity(a, b) {
        const ret = wasm.embedder_cosine_similarity(addHeapObject(a), addHeapObject(b));
        return ret;
    }
    /**
     * Create a new embedder with default config
     */
    constructor() {
        const ret = wasm.embedder_new();
        this.__wbg_ptr = ret >>> 0;
        EmbedderFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Generate embeddings for a single text
     * Uses hash-based projection for lightweight WASM operation
     * Returns Float32Array for direct JS consumption
     * @param {string} text
     * @returns {Float32Array}
     */
    embed(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.embedder_embed(this.__wbg_ptr, ptr0, len0);
        return takeObject(ret);
    }
}
if (Symbol.dispose) Embedder.prototype[Symbol.dispose] = Embedder.prototype.free;

/**
 * Embedding model configuration
 */
export class EmbeddingConfig {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(EmbeddingConfig.prototype);
        obj.__wbg_ptr = ptr;
        EmbeddingConfigFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EmbeddingConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_embeddingconfig_free(ptr, 0);
    }
    /**
     * Create config for larger models
     * @param {number} dimensions
     * @returns {EmbeddingConfig}
     */
    static with_dimensions(dimensions) {
        const ret = wasm.embeddingconfig_with_dimensions(dimensions);
        return EmbeddingConfig.__wrap(ret);
    }
    constructor() {
        const ret = wasm.embeddingconfig_minilm();
        this.__wbg_ptr = ret >>> 0;
        EmbeddingConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create config for all-MiniLM-L6-v2 (default)
     * @returns {EmbeddingConfig}
     */
    static minilm() {
        const ret = wasm.embeddingconfig_minilm();
        return EmbeddingConfig.__wrap(ret);
    }
    /**
     * Model dimensions (384 for all-MiniLM-L6-v2)
     * @returns {number}
     */
    get dimensions() {
        const ret = wasm.__wbg_get_embeddingconfig_dimensions(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Model dimensions (384 for all-MiniLM-L6-v2)
     * @param {number} arg0
     */
    set dimensions(arg0) {
        wasm.__wbg_set_embeddingconfig_dimensions(this.__wbg_ptr, arg0);
    }
    /**
     * Normalize output vectors
     * @returns {boolean}
     */
    get normalize() {
        const ret = wasm.__wbg_get_embeddingconfig_normalize(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Normalize output vectors
     * @param {boolean} arg0
     */
    set normalize(arg0) {
        wasm.__wbg_set_embeddingconfig_normalize(this.__wbg_ptr, arg0);
    }
    /**
     * Max sequence length
     * @returns {number}
     */
    get max_length() {
        const ret = wasm.__wbg_get_embeddingconfig_max_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Max sequence length
     * @param {number} arg0
     */
    set max_length(arg0) {
        wasm.__wbg_set_embeddingconfig_max_length(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) EmbeddingConfig.prototype[Symbol.dispose] = EmbeddingConfig.prototype.free;

/**
 * LoRA Configuration for WASM
 */
export class LoraConfig {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(LoraConfig.prototype);
        obj.__wbg_ptr = ptr;
        LoraConfigFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        LoraConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_loraconfig_free(ptr, 0);
    }
    /**
     * Create custom LoRA configuration
     * @param {number} hidden_dim
     * @param {number} rank
     * @param {number} alpha
     * @param {number} learning_rate
     */
    constructor(hidden_dim, rank, alpha, learning_rate) {
        const ret = wasm.loraconfig_new(hidden_dim, rank, alpha, learning_rate);
        this.__wbg_ptr = ret >>> 0;
        LoraConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create BaseLoRA configuration (background training)
     * @param {number} hidden_dim
     * @returns {LoraConfig}
     */
    static base(hidden_dim) {
        const ret = wasm.loraconfig_base(hidden_dim);
        return LoraConfig.__wrap(ret);
    }
    /**
     * Create MicroLoRA configuration (per-request, <100μs)
     * @param {number} hidden_dim
     * @returns {LoraConfig}
     */
    static micro(hidden_dim) {
        const ret = wasm.loraconfig_micro(hidden_dim);
        return LoraConfig.__wrap(ret);
    }
    /**
     * Export as JSON
     * @returns {any}
     */
    toJSON() {
        const ret = wasm.loraconfig_toJSON(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Validate configuration
     */
    validate() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.loraconfig_validate(retptr, this.__wbg_ptr);
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
     * LoRA rank (1-2 for micro, 4-16 for base)
     * @returns {number}
     */
    get rank() {
        const ret = wasm.__wbg_get_embeddingconfig_dimensions(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Alpha scaling factor
     * @returns {number}
     */
    get alpha() {
        const ret = wasm.__wbg_get_loraconfig_alpha(this.__wbg_ptr);
        return ret;
    }
    /**
     * Learning rate for adaptation
     * @returns {number}
     */
    get learning_rate() {
        const ret = wasm.__wbg_get_loraconfig_learning_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Hidden dimension
     * @returns {number}
     */
    get hidden_dim() {
        const ret = wasm.__wbg_get_loraconfig_hidden_dim(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) LoraConfig.prototype[Symbol.dispose] = LoraConfig.prototype.free;

/**
 * MicroLoRA adapter for per-request adaptation
 */
export class MicroLoRA {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        MicroLoRAFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_microlora_free(ptr, 0);
    }
    /**
     * @returns {any}
     */
    exportWeights() {
        const ret = wasm.microlora_exportWeights(this.__wbg_ptr);
        return takeObject(ret);
    }
    applyGradients() {
        wasm.microlora_applyGradients(this.__wbg_ptr);
    }
    /**
     * @param {Float32Array} input
     * @param {number} feedback
     */
    accumulateGradient(input, feedback) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.microlora_accumulateGradient(retptr, this.__wbg_ptr, ptr0, len0, feedback);
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
     * @param {LoraConfig} config
     */
    constructor(config) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(config, LoraConfig);
            var ptr0 = config.__destroy_into_raw();
            wasm.microlora_new(retptr, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            MicroLoRAFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    reset() {
        wasm.microlora_reset(this.__wbg_ptr);
    }
    /**
     * @returns {any}
     */
    stats() {
        const ret = wasm.microlora_stats(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    forward(input) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.microlora_forward(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v2 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) MicroLoRA.prototype[Symbol.dispose] = MicroLoRA.prototype.free;

/**
 * Main RvLite database
 */
export class RvLite {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(RvLite.prototype);
        obj.__wbg_ptr = ptr;
        RvLiteFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RvLiteFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rvlite_free(ptr, 0);
    }
    /**
     * Get configuration
     * @returns {any}
     */
    get_config() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_get_config(retptr, this.__wbg_ptr);
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
     * Get version string
     * @returns {string}
     */
    get_version() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_get_version(retptr, this.__wbg_ptr);
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
     * Get enabled features
     * @returns {any}
     */
    get_features() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_get_features(retptr, this.__wbg_ptr);
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
     * Insert a vector with a specific ID
     * @param {string} id
     * @param {Float32Array} vector
     * @param {any | null} [metadata]
     */
    insert_with_id(id, vector, metadata) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayF32ToWasm0(vector, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.rvlite_insert_with_id(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, isLikeNone(metadata) ? 0 : addHeapObject(metadata));
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
     * Search with metadata filter
     * @param {Float32Array} query_vector
     * @param {number} k
     * @param {any} filter
     * @returns {any}
     */
    search_with_filter(query_vector, k, filter) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query_vector, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.rvlite_search_with_filter(retptr, this.__wbg_ptr, ptr0, len0, k, addHeapObject(filter));
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
     * Get a vector by ID
     * @param {string} id
     * @returns {any}
     */
    get(id) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.rvlite_get(retptr, this.__wbg_ptr, ptr0, len0);
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
     * Get the number of vectors in the database
     * @returns {number}
     */
    len() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_len(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 >>> 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Create a new RvLite database
     * @param {RvLiteConfig} config
     */
    constructor(config) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(config, RvLiteConfig);
            var ptr0 = config.__destroy_into_raw();
            wasm.rvlite_new(retptr, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            RvLiteFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Execute SQL query (not yet implemented)
     * @param {string} _query
     * @returns {Promise<any>}
     */
    sql(_query) {
        const ptr0 = passStringToWasm0(_query, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rvlite_sql(this.__wbg_ptr, ptr0, len0);
        return takeObject(ret);
    }
    /**
     * Execute Cypher query (not yet implemented)
     * @param {string} _query
     * @returns {Promise<any>}
     */
    cypher(_query) {
        const ptr0 = passStringToWasm0(_query, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rvlite_cypher(this.__wbg_ptr, ptr0, len0);
        return takeObject(ret);
    }
    /**
     * Delete a vector by ID
     * @param {string} id
     * @returns {boolean}
     */
    delete(id) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.rvlite_delete(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Insert a vector with optional metadata
     * Returns the vector ID
     * @param {Float32Array} vector
     * @param {any | null} [metadata]
     * @returns {string}
     */
    insert(vector, metadata) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(vector, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.rvlite_insert(retptr, this.__wbg_ptr, ptr0, len0, isLikeNone(metadata) ? 0 : addHeapObject(metadata));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr2 = r0;
            var len2 = r1;
            if (r3) {
                ptr2 = 0; len2 = 0;
                throw takeObject(r2);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export4(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Search for similar vectors
     * Returns a JavaScript array of search results
     * @param {Float32Array} query_vector
     * @param {number} k
     * @returns {any}
     */
    search(query_vector, k) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query_vector, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.rvlite_search(retptr, this.__wbg_ptr, ptr0, len0, k);
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
     * Execute SPARQL query (not yet implemented)
     * @param {string} _query
     * @returns {Promise<any>}
     */
    sparql(_query) {
        const ptr0 = passStringToWasm0(_query, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rvlite_sparql(this.__wbg_ptr, ptr0, len0);
        return takeObject(ret);
    }
    /**
     * Create with default configuration (384 dimensions, cosine similarity)
     * @returns {RvLite}
     */
    static default() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_default(retptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return RvLite.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Check if database is empty
     * @returns {boolean}
     */
    is_empty() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.rvlite_is_empty(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return r0 !== 0;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Check if database is ready
     * @returns {boolean}
     */
    is_ready() {
        const ret = wasm.rvlite_is_ready(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) RvLite.prototype[Symbol.dispose] = RvLite.prototype.free;

/**
 * Configuration for RvLite database
 */
export class RvLiteConfig {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(RvLiteConfig.prototype);
        obj.__wbg_ptr = ptr;
        RvLiteConfigFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RvLiteConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rvliteconfig_free(ptr, 0);
    }
    /**
     * Set distance metric (euclidean, cosine, dotproduct, manhattan)
     * @param {string} metric
     * @returns {RvLiteConfig}
     */
    with_distance_metric(metric) {
        const ptr = this.__destroy_into_raw();
        const ptr0 = passStringToWasm0(metric, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rvliteconfig_with_distance_metric(ptr, ptr0, len0);
        return RvLiteConfig.__wrap(ret);
    }
    /**
     * @param {number} dimensions
     */
    constructor(dimensions) {
        const ret = wasm.rvliteconfig_new(dimensions);
        this.__wbg_ptr = ret >>> 0;
        RvLiteConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) RvLiteConfig.prototype[Symbol.dispose] = RvLiteConfig.prototype.free;

/**
 * TRM Configuration for WASM
 *
 * Optimized defaults for browser-based inference with small models.
 */
export class TrmConfig {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(TrmConfig.prototype);
        obj.__wbg_ptr = ptr;
        TrmConfigFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TrmConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_trmconfig_free(ptr, 0);
    }
    /**
     * Embedding dimension (input/output size)
     * @returns {number}
     */
    get embedding_dim() {
        const ret = wasm.__wbg_get_embeddingconfig_dimensions(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Hidden dimension for latent state
     * @returns {number}
     */
    get hidden_dim() {
        const ret = wasm.__wbg_get_embeddingconfig_max_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Maximum K iterations
     * @returns {number}
     */
    get max_k() {
        const ret = wasm.__wbg_get_trmconfig_max_k(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Default K iterations
     * @returns {number}
     */
    get default_k() {
        const ret = wasm.__wbg_get_loraconfig_hidden_dim(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Latent updates per K iteration
     * @returns {number}
     */
    get latent_iterations() {
        const ret = wasm.__wbg_get_trmconfig_latent_iterations(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Use attention variant (more expressive, slower)
     * @returns {boolean}
     */
    get use_attention() {
        const ret = wasm.__wbg_get_trmconfig_use_attention(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Number of attention heads
     * @returns {number}
     */
    get num_heads() {
        const ret = wasm.__wbg_get_trmconfig_num_heads(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Confidence threshold for early stopping
     * @returns {number}
     */
    get confidence_threshold() {
        const ret = wasm.__wbg_get_trmconfig_confidence_threshold(this.__wbg_ptr);
        return ret;
    }
    /**
     * Enable early stopping
     * @returns {boolean}
     */
    get early_stopping() {
        const ret = wasm.__wbg_get_trmconfig_early_stopping(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Minimum iterations before early stopping
     * @returns {number}
     */
    get min_iterations() {
        const ret = wasm.__wbg_get_trmconfig_min_iterations(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Convergence threshold for plateau detection
     * @returns {number}
     */
    get convergence_threshold() {
        const ret = wasm.__wbg_get_trmconfig_convergence_threshold(this.__wbg_ptr);
        return ret;
    }
    /**
     * Residual scale for answer refinement
     * @returns {number}
     */
    get residual_scale() {
        const ret = wasm.__wbg_get_trmconfig_residual_scale(this.__wbg_ptr);
        return ret;
    }
    /**
     * Enable/disable attention variant
     * @param {boolean} use_attention
     * @returns {TrmConfig}
     */
    withAttention(use_attention) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.trmconfig_withAttention(ptr, use_attention);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Set default K iterations
     * @param {number} k
     * @returns {TrmConfig}
     */
    withDefaultK(k) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.trmconfig_withDefaultK(ptr, k);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Enable/disable early stopping
     * @param {boolean} enabled
     * @returns {TrmConfig}
     */
    withEarlyStopping(enabled) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.trmconfig_withEarlyStopping(ptr, enabled);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Set latent iterations per K step
     * @param {number} n
     * @returns {TrmConfig}
     */
    withLatentIterations(n) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.trmconfig_withLatentIterations(ptr, n);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Set confidence threshold
     * @param {number} threshold
     * @returns {TrmConfig}
     */
    withConfidenceThreshold(threshold) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.trmconfig_withConfidenceThreshold(ptr, threshold);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Create a new TRM configuration
     * @param {number} embedding_dim
     * @param {number} hidden_dim
     * @param {number} max_k
     */
    constructor(embedding_dim, hidden_dim, max_k) {
        const ret = wasm.trmconfig_new(embedding_dim, hidden_dim, max_k);
        this.__wbg_ptr = ret >>> 0;
        TrmConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create configuration optimized for speed
     * @param {number} embedding_dim
     * @returns {TrmConfig}
     */
    static fast(embedding_dim) {
        const ret = wasm.trmconfig_fast(embedding_dim);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Create configuration optimized for quality
     * @param {number} embedding_dim
     * @returns {TrmConfig}
     */
    static quality(embedding_dim) {
        const ret = wasm.trmconfig_quality(embedding_dim);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Export configuration as JSON
     * @returns {any}
     */
    toJSON() {
        const ret = wasm.trmconfig_toJSON(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * Create balanced configuration (recommended)
     * @param {number} embedding_dim
     * @returns {TrmConfig}
     */
    static balanced(embedding_dim) {
        const ret = wasm.trmconfig_balanced(embedding_dim);
        return TrmConfig.__wrap(ret);
    }
    /**
     * Validate configuration
     */
    validate() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.trmconfig_validate(retptr, this.__wbg_ptr);
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
     * Import configuration from JSON
     * @param {any} value
     * @returns {TrmConfig}
     */
    static fromJSON(value) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.trmconfig_fromJSON(retptr, addHeapObject(value));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return TrmConfig.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) TrmConfig.prototype[Symbol.dispose] = TrmConfig.prototype.free;

/**
 * TRM Recursive Reasoning Engine
 */
export class TrmEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TrmEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_trmengine_free(ptr, 0);
    }
    /**
     * @returns {any}
     */
    getConfig() {
        const ret = wasm.trmengine_getConfig(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * @param {Float32Array} question
     * @param {Float32Array} answer
     * @param {number} k
     * @returns {TrmResult}
     */
    reasonWithK(question, answer, k) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(question, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayF32ToWasm0(answer, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.trmengine_reasonWithK(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, k);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return TrmResult.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {TrmConfig} config
     */
    constructor(config) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            _assertClass(config, TrmConfig);
            var ptr0 = config.__destroy_into_raw();
            wasm.trmengine_new(retptr, ptr0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            TrmEngineFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    reset() {
        wasm.trmengine_reset(this.__wbg_ptr);
    }
    /**
     * @returns {any}
     */
    stats() {
        const ret = wasm.trmengine_stats(this.__wbg_ptr);
        return takeObject(ret);
    }
    /**
     * @param {Float32Array} question
     * @param {Float32Array} answer
     * @returns {TrmResult}
     */
    reason(question, answer) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(question, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayF32ToWasm0(answer, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.trmengine_reason(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return TrmResult.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) TrmEngine.prototype[Symbol.dispose] = TrmEngine.prototype.free;

/**
 * Result of TRM recursive reasoning
 */
export class TrmResult {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(TrmResult.prototype);
        obj.__wbg_ptr = ptr;
        TrmResultFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TrmResultFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_trmresult_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    get confidence() {
        const ret = wasm.__wbg_get_trmresult_confidence(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get iterations_used() {
        const ret = wasm.__wbg_get_trmresult_iterations_used(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    get early_stopped() {
        const ret = wasm.__wbg_get_trmresult_early_stopped(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    get latency_ms() {
        const ret = wasm.__wbg_get_trmresult_latency_ms(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    getAnswer() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.trmresult_getAnswer(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export4(r0, r1 * 4, 4);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @returns {any}
     */
    toJSON() {
        const ret = wasm.trmresult_toJSON(this.__wbg_ptr);
        return takeObject(ret);
    }
}
if (Symbol.dispose) TrmResult.prototype[Symbol.dispose] = TrmResult.prototype.free;

/**
 * Quick benchmark for embeddings
 * @param {number} iterations
 * @returns {any}
 */
export function benchmark_embeddings(iterations) {
    const ret = wasm.benchmark_embeddings(iterations);
    return takeObject(ret);
}

/**
 * Quick benchmark function
 * @param {number} iterations
 * @param {number} hidden_dim
 * @returns {any}
 */
export function benchmark_trm(iterations, hidden_dim) {
    const ret = wasm.benchmark_trm(iterations, hidden_dim);
    return takeObject(ret);
}

/**
 * Compute cosine similarity
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @returns {number}
 */
export function cosineSimilarity(a, b) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(a, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(b, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.cosineSimilarity(retptr, ptr0, len0, ptr1, len1);
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

/**
 * Dot product
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @returns {number}
 */
export function dotProduct(a, b) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(a, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(b, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.dotProduct(retptr, ptr0, len0, ptr1, len1);
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

/**
 * Get feature info
 * @returns {any}
 */
export function features() {
    const ret = wasm.features();
    return takeObject(ret);
}

export function init() {
    wasm.init();
}

/**
 * Compute L2 distance
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @returns {number}
 */
export function l2Distance(a, b) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(a, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(b, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.l2Distance(retptr, ptr0, len0, ptr1, len1);
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

/**
 * Linear interpolation
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @param {number} t
 * @returns {Float32Array}
 */
export function lerp(a, b, t) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(a, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(b, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.lerp(retptr, ptr0, len0, ptr1, len1, t);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v3 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v3;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Mean pooling for token embeddings
 * @param {any} embeddings
 * @param {Float32Array | null} [attention_mask]
 * @returns {Float32Array}
 */
export function meanPooling(embeddings, attention_mask) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        var ptr0 = isLikeNone(attention_mask) ? 0 : passArrayF32ToWasm0(attention_mask, wasm.__wbindgen_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.meanPooling(retptr, addHeapObject(embeddings), ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v2 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Normalize a vector to unit length
 * @param {Float32Array} vec
 * @returns {Float32Array}
 */
export function normalizeVector(vec) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(vec, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.normalizeVector(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v2 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Create a random vector
 * @param {number} dim
 * @param {number | null} [seed]
 * @returns {Float32Array}
 */
export function randomVector(dim, seed) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.randomVector(retptr, dim, isLikeNone(seed) ? 0x100000001 : (seed) >>> 0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v1 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Softmax function
 * @param {Float32Array} vec
 * @returns {Float32Array}
 */
export function softmax(vec) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(vec, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.softmax(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v2 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * Get version string
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

/**
 * Create a zero vector
 * @param {number} dim
 * @returns {Float32Array}
 */
export function zeros(dim) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.zeros(retptr, dim);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v1 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
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
    imports.wbg.__wbg_Number_2d1dcfcf4ec51736 = function(arg0) {
        const ret = Number(getObject(arg0));
        return ret;
    };
    imports.wbg.__wbg_String_8f0eb39a4a4c2f66 = function(arg0, arg1) {
        const ret = String(getObject(arg1));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_bigint_get_as_i64_6e32f5e6aff02e1d = function(arg0, arg1) {
        const v = getObject(arg1);
        const ret = typeof(v) === 'bigint' ? v : undefined;
        getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    };
    imports.wbg.__wbg___wbindgen_boolean_get_dea25b33882b895b = function(arg0) {
        const v = getObject(arg0);
        const ret = typeof(v) === 'boolean' ? v : undefined;
        return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
    };
    imports.wbg.__wbg___wbindgen_debug_string_adfb662ae34724b6 = function(arg0, arg1) {
        const ret = debugString(getObject(arg1));
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_in_0d3e1e8f0c669317 = function(arg0, arg1) {
        const ret = getObject(arg0) in getObject(arg1);
        return ret;
    };
    imports.wbg.__wbg___wbindgen_is_bigint_0e1a2e3f55cfae27 = function(arg0) {
        const ret = typeof(getObject(arg0)) === 'bigint';
        return ret;
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
    imports.wbg.__wbg___wbindgen_jsval_eq_b6101cc9cef1fe36 = function(arg0, arg1) {
        const ret = getObject(arg0) === getObject(arg1);
        return ret;
    };
    imports.wbg.__wbg___wbindgen_jsval_loose_eq_766057600fdd1b0d = function(arg0, arg1) {
        const ret = getObject(arg0) == getObject(arg1);
        return ret;
    };
    imports.wbg.__wbg___wbindgen_number_get_9619185a74197f95 = function(arg0, arg1) {
        const obj = getObject(arg1);
        const ret = typeof(obj) === 'number' ? obj : undefined;
        getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    };
    imports.wbg.__wbg___wbindgen_string_get_a2a31e16edf96e42 = function(arg0, arg1) {
        const obj = getObject(arg1);
        const ret = typeof(obj) === 'string' ? obj : undefined;
        var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        var len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg__wbg_cb_unref_87dfb5aaa0cbcea7 = function(arg0) {
        getObject(arg0)._wbg_cb_unref();
    };
    imports.wbg.__wbg_call_3020136f7a2d6e44 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_call_abb4ff46ce38be40 = function() { return handleError(function (arg0, arg1) {
        const ret = getObject(arg0).call(getObject(arg1));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_done_62ea16af4ce34b24 = function(arg0) {
        const ret = getObject(arg0).done;
        return ret;
    };
    imports.wbg.__wbg_entries_83c79938054e065f = function(arg0) {
        const ret = Object.entries(getObject(arg0));
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
    imports.wbg.__wbg_get_6b7bd52aca3f9671 = function(arg0, arg1) {
        const ret = getObject(arg0)[arg1 >>> 0];
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_get_af9dab7e9603ea93 = function() { return handleError(function (arg0, arg1) {
        const ret = Reflect.get(getObject(arg0), getObject(arg1));
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_get_with_ref_key_1dc361bd10053bfe = function(arg0, arg1) {
        const ret = getObject(arg0)[getObject(arg1)];
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_instanceof_ArrayBuffer_f3320d2419cd0355 = function(arg0) {
        let result;
        try {
            result = getObject(arg0) instanceof ArrayBuffer;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Map_084be8da74364158 = function(arg0) {
        let result;
        try {
            result = getObject(arg0) instanceof Map;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Uint8Array_da54ccc9d3e09434 = function(arg0) {
        let result;
        try {
            result = getObject(arg0) instanceof Uint8Array;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_instanceof_Window_b5cf7783caa68180 = function(arg0) {
        let result;
        try {
            result = getObject(arg0) instanceof Window;
        } catch (_) {
            result = false;
        }
        const ret = result;
        return ret;
    };
    imports.wbg.__wbg_isArray_51fd9e6422c0a395 = function(arg0) {
        const ret = Array.isArray(getObject(arg0));
        return ret;
    };
    imports.wbg.__wbg_isSafeInteger_ae7d3f054d55fa16 = function(arg0) {
        const ret = Number.isSafeInteger(getObject(arg0));
        return ret;
    };
    imports.wbg.__wbg_iterator_27b7c8b35ab3e86b = function() {
        const ret = Symbol.iterator;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_length_22ac23eaec9d8053 = function(arg0) {
        const ret = getObject(arg0).length;
        return ret;
    };
    imports.wbg.__wbg_length_86ce4877baf913bb = function(arg0) {
        const ret = getObject(arg0).length;
        return ret;
    };
    imports.wbg.__wbg_length_d45040a40c570362 = function(arg0) {
        const ret = getObject(arg0).length;
        return ret;
    };
    imports.wbg.__wbg_log_1d990106d99dacb7 = function(arg0) {
        console.log(getObject(arg0));
    };
    imports.wbg.__wbg_new_1ba21ce319a06297 = function() {
        const ret = new Object();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_25f239778d6112b9 = function() {
        const ret = new Array();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_6421f6084cc5bc5a = function(arg0) {
        const ret = new Uint8Array(getObject(arg0));
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
    imports.wbg.__wbg_new_ff12d2b041fb48f1 = function(arg0, arg1) {
        try {
            var state0 = {a: arg0, b: arg1};
            var cb0 = (arg0, arg1) => {
                const a = state0.a;
                state0.a = 0;
                try {
                    return __wasm_bindgen_func_elem_1412(a, state0.b, arg0, arg1);
                } finally {
                    state0.a = a;
                }
            };
            const ret = new Promise(cb0);
            return addHeapObject(ret);
        } finally {
            state0.a = state0.b = 0;
        }
    };
    imports.wbg.__wbg_new_from_slice_41e2764a343e3cb1 = function(arg0, arg1) {
        const ret = new Float32Array(getArrayF32FromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_new_no_args_cb138f77cf6151ee = function(arg0, arg1) {
        const ret = new Function(getStringFromWasm0(arg0, arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_next_138a17bbf04e926c = function(arg0) {
        const ret = getObject(arg0).next;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_next_3cfe5c0fe2a4cc53 = function() { return handleError(function (arg0) {
        const ret = getObject(arg0).next();
        return addHeapObject(ret);
    }, arguments) };
    imports.wbg.__wbg_now_8cf15d6e317793e1 = function(arg0) {
        const ret = getObject(arg0).now();
        return ret;
    };
    imports.wbg.__wbg_performance_c77a440eff2efd9b = function(arg0) {
        const ret = getObject(arg0).performance;
        return isLikeNone(ret) ? 0 : addHeapObject(ret);
    };
    imports.wbg.__wbg_prototypesetcall_96cc7097487b926d = function(arg0, arg1, arg2) {
        Float32Array.prototype.set.call(getArrayF32FromWasm0(arg0, arg1), getObject(arg2));
    };
    imports.wbg.__wbg_prototypesetcall_dfe9b766cdc1f1fd = function(arg0, arg1, arg2) {
        Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
    };
    imports.wbg.__wbg_queueMicrotask_9b549dfce8865860 = function(arg0) {
        const ret = getObject(arg0).queueMicrotask;
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_queueMicrotask_fca69f5bfad613a5 = function(arg0) {
        queueMicrotask(getObject(arg0));
    };
    imports.wbg.__wbg_resolve_fd5bfbaa4ce36e1e = function(arg0) {
        const ret = Promise.resolve(getObject(arg0));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_set_3f1d0b984ed272ed = function(arg0, arg1, arg2) {
        getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
    };
    imports.wbg.__wbg_set_781438a03c0c3c81 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
        return ret;
    }, arguments) };
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
    imports.wbg.__wbg_then_4f95312d68691235 = function(arg0, arg1) {
        const ret = getObject(arg0).then(getObject(arg1));
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_value_57b7b035e117f7ee = function(arg0) {
        const ret = getObject(arg0).value;
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
    imports.wbg.__wbindgen_cast_90fb09e08acc4555 = function(arg0, arg1) {
        // Cast intrinsic for `Closure(Closure { dtor_idx: 39, function: Function { arguments: [Externref], shim_idx: 40, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
        const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_684, __wasm_bindgen_func_elem_690);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_cast_9ae0607507abb057 = function(arg0) {
        // Cast intrinsic for `I64 -> Externref`.
        const ret = arg0;
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
    imports.wbg.__wbindgen_object_is_undefined = function(arg0) {
        const ret = getObject(arg0) === undefined;
        return ret;
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
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
        module_or_path = new URL('rvlite_bg.wasm', import.meta.url);
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
