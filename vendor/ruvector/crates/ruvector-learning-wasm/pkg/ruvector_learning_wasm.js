let wasm;

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
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

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
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

let WASM_VECTOR_LEN = 0;

const WasmMicroLoRAFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmicrolora_free(ptr >>> 0, 1));

const WasmScopedLoRAFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmscopedlora_free(ptr >>> 0, 1));

const WasmTrajectoryBufferFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmtrajectorybuffer_free(ptr >>> 0, 1));

/**
 * WASM-exposed MicroLoRA engine
 */
export class WasmMicroLoRA {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMicroLoRAFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmicrolora_free(ptr, 0);
    }
    /**
     * Get delta norm (weight change magnitude)
     * @returns {number}
     */
    delta_norm() {
        const ret = wasm.wasmmicrolora_delta_norm(this.__wbg_ptr);
        return ret;
    }
    /**
     * Adapt with typed array gradient
     * @param {Float32Array} gradient
     */
    adapt_array(gradient) {
        const ptr0 = passArrayF32ToWasm0(gradient, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmmicrolora_adapt_array(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get adaptation count
     * @returns {bigint}
     */
    adapt_count() {
        const ret = wasm.wasmmicrolora_adapt_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get parameter count
     * @returns {number}
     */
    param_count() {
        const ret = wasm.wasmmicrolora_param_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Forward pass with typed array input (allocates output)
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    forward_array(input) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmmicrolora_forward_array(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v2 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export2(r0, r1 * 4, 4);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get forward pass count
     * @returns {bigint}
     */
    forward_count() {
        const ret = wasm.wasmmicrolora_forward_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get pointer to input buffer for direct memory access
     * @returns {number}
     */
    get_input_ptr() {
        const ret = wasm.wasmmicrolora_get_input_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get pointer to output buffer for direct memory access
     * @returns {number}
     */
    get_output_ptr() {
        const ret = wasm.wasmmicrolora_get_output_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Adapt with improvement reward using input buffer as gradient
     * @param {number} improvement
     */
    adapt_with_reward(improvement) {
        wasm.wasmmicrolora_adapt_with_reward(this.__wbg_ptr, improvement);
    }
    /**
     * Get embedding dimension
     * @returns {number}
     */
    dim() {
        const ret = wasm.wasmmicrolora_dim(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new MicroLoRA engine
     *
     * @param dim - Embedding dimension (default 256, max 256)
     * @param alpha - Scaling factor (default 0.1)
     * @param learning_rate - Learning rate (default 0.01)
     * @param {number | null} [dim]
     * @param {number | null} [alpha]
     * @param {number | null} [learning_rate]
     */
    constructor(dim, alpha, learning_rate) {
        const ret = wasm.wasmmicrolora_new(isLikeNone(dim) ? 0x100000001 : (dim) >>> 0, isLikeNone(alpha) ? 0x100000001 : Math.fround(alpha), isLikeNone(learning_rate) ? 0x100000001 : Math.fround(learning_rate));
        this.__wbg_ptr = ret >>> 0;
        WasmMicroLoRAFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Adapt using input buffer as gradient
     */
    adapt() {
        wasm.wasmmicrolora_adapt(this.__wbg_ptr);
    }
    /**
     * Reset the engine
     */
    reset() {
        wasm.wasmmicrolora_reset(this.__wbg_ptr);
    }
    /**
     * Forward pass using internal buffers (zero-allocation)
     *
     * Write input to get_input_ptr(), call forward(), read from get_output_ptr()
     */
    forward() {
        wasm.wasmmicrolora_forward(this.__wbg_ptr);
    }
}
if (Symbol.dispose) WasmMicroLoRA.prototype[Symbol.dispose] = WasmMicroLoRA.prototype.free;

/**
 * WASM-exposed Scoped LoRA manager
 */
export class WasmScopedLoRA {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmScopedLoRAFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmscopedlora_free(ptr, 0);
    }
    /**
     * Get delta norm for operator
     * @param {number} op_type
     * @returns {number}
     */
    delta_norm(op_type) {
        const ret = wasm.wasmscopedlora_delta_norm(this.__wbg_ptr, op_type);
        return ret;
    }
    /**
     * Get operator scope name
     * @param {number} op_type
     * @returns {string}
     */
    static scope_name(op_type) {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmscopedlora_scope_name(retptr, op_type);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export2(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Adapt with typed array
     * @param {number} op_type
     * @param {Float32Array} gradient
     */
    adapt_array(op_type, gradient) {
        const ptr0 = passArrayF32ToWasm0(gradient, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmscopedlora_adapt_array(this.__wbg_ptr, op_type, ptr0, len0);
    }
    /**
     * Get adapt count for operator
     * @param {number} op_type
     * @returns {bigint}
     */
    adapt_count(op_type) {
        const ret = wasm.wasmscopedlora_adapt_count(this.__wbg_ptr, op_type);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Reset specific operator adapter
     * @param {number} op_type
     */
    reset_scope(op_type) {
        wasm.wasmscopedlora_reset_scope(this.__wbg_ptr, op_type);
    }
    /**
     * Forward pass with typed array
     * @param {number} op_type
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    forward_array(op_type, input) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(input, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmscopedlora_forward_array(retptr, this.__wbg_ptr, op_type, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v2 = getArrayF32FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export2(r0, r1 * 4, 4);
            return v2;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get forward count for operator
     * @param {number} op_type
     * @returns {bigint}
     */
    forward_count(op_type) {
        const ret = wasm.wasmscopedlora_forward_count(this.__wbg_ptr, op_type);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get input buffer pointer
     * @returns {number}
     */
    get_input_ptr() {
        const ret = wasm.wasmscopedlora_get_input_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get output buffer pointer
     * @returns {number}
     */
    get_output_ptr() {
        const ret = wasm.wasmscopedlora_get_output_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Adapt with improvement reward
     * @param {number} op_type
     * @param {number} improvement
     */
    adapt_with_reward(op_type, improvement) {
        wasm.wasmscopedlora_adapt_with_reward(this.__wbg_ptr, op_type, improvement);
    }
    /**
     * Get total adapt count
     * @returns {bigint}
     */
    total_adapt_count() {
        const ret = wasm.wasmscopedlora_total_adapt_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get total forward count
     * @returns {bigint}
     */
    total_forward_count() {
        const ret = wasm.wasmscopedlora_total_forward_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Enable/disable category fallback
     * @param {boolean} enabled
     */
    set_category_fallback(enabled) {
        wasm.wasmscopedlora_set_category_fallback(this.__wbg_ptr, enabled);
    }
    /**
     * Create a new scoped LoRA manager
     *
     * @param dim - Embedding dimension (max 256)
     * @param alpha - Scaling factor (default 0.1)
     * @param learning_rate - Learning rate (default 0.01)
     * @param {number | null} [dim]
     * @param {number | null} [alpha]
     * @param {number | null} [learning_rate]
     */
    constructor(dim, alpha, learning_rate) {
        const ret = wasm.wasmscopedlora_new(isLikeNone(dim) ? 0x100000001 : (dim) >>> 0, isLikeNone(alpha) ? 0x100000001 : Math.fround(alpha), isLikeNone(learning_rate) ? 0x100000001 : Math.fround(learning_rate));
        this.__wbg_ptr = ret >>> 0;
        WasmScopedLoRAFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Adapt for operator type using input buffer as gradient
     * @param {number} op_type
     */
    adapt(op_type) {
        wasm.wasmscopedlora_adapt(this.__wbg_ptr, op_type);
    }
    /**
     * Forward pass for operator type (uses internal buffers)
     *
     * @param op_type - Operator type (0-16)
     * @param {number} op_type
     */
    forward(op_type) {
        wasm.wasmscopedlora_forward(this.__wbg_ptr, op_type);
    }
    /**
     * Reset all adapters
     */
    reset_all() {
        wasm.wasmscopedlora_reset_all(this.__wbg_ptr);
    }
}
if (Symbol.dispose) WasmScopedLoRA.prototype[Symbol.dispose] = WasmScopedLoRA.prototype.free;

/**
 * WASM-exposed trajectory buffer
 */
export class WasmTrajectoryBuffer {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmTrajectoryBufferFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmtrajectorybuffer_free(ptr, 0);
    }
    /**
     * Get total count
     * @returns {bigint}
     */
    total_count() {
        const ret = wasm.wasmtrajectorybuffer_total_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get success rate
     * @returns {number}
     */
    success_rate() {
        const ret = wasm.wasmtrajectorybuffer_success_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get best attention type
     * @returns {number}
     */
    best_attention() {
        const ret = wasm.wasmtrajectorybuffer_best_attention(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get best improvement
     * @returns {number}
     */
    best_improvement() {
        const ret = wasm.wasmtrajectorybuffer_best_improvement(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get mean improvement
     * @returns {number}
     */
    mean_improvement() {
        const ret = wasm.wasmtrajectorybuffer_mean_improvement(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get trajectory count for operator
     * @param {number} op_type
     * @returns {number}
     */
    count_by_operator(op_type) {
        const ret = wasm.wasmtrajectorybuffer_count_by_operator(this.__wbg_ptr, op_type);
        return ret >>> 0;
    }
    /**
     * Get high quality trajectory count
     * @param {number} threshold
     * @returns {number}
     */
    high_quality_count(threshold) {
        const ret = wasm.wasmtrajectorybuffer_high_quality_count(this.__wbg_ptr, threshold);
        return ret >>> 0;
    }
    /**
     * Get buffer length
     * @returns {number}
     */
    len() {
        const ret = wasm.wasmtrajectorybuffer_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new trajectory buffer
     *
     * @param capacity - Maximum number of trajectories to store
     * @param embedding_dim - Dimension of embeddings (default 256)
     * @param {number | null} [capacity]
     * @param {number | null} [embedding_dim]
     */
    constructor(capacity, embedding_dim) {
        const ret = wasm.wasmtrajectorybuffer_new(isLikeNone(capacity) ? 0x100000001 : (capacity) >>> 0, isLikeNone(embedding_dim) ? 0x100000001 : (embedding_dim) >>> 0);
        this.__wbg_ptr = ret >>> 0;
        WasmTrajectoryBufferFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset buffer
     */
    reset() {
        wasm.wasmtrajectorybuffer_reset(this.__wbg_ptr);
    }
    /**
     * Record a trajectory
     *
     * @param embedding - Embedding vector (Float32Array)
     * @param op_type - Operator type (0-16)
     * @param attention_type - Attention mechanism used
     * @param execution_ms - Actual execution time
     * @param baseline_ms - Baseline execution time
     * @param {Float32Array} embedding
     * @param {number} op_type
     * @param {number} attention_type
     * @param {number} execution_ms
     * @param {number} baseline_ms
     */
    record(embedding, op_type, attention_type, execution_ms, baseline_ms) {
        const ptr0 = passArrayF32ToWasm0(embedding, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmtrajectorybuffer_record(this.__wbg_ptr, ptr0, len0, op_type, attention_type, execution_ms, baseline_ms);
    }
    /**
     * Check if empty
     * @returns {boolean}
     */
    is_empty() {
        const ret = wasm.wasmtrajectorybuffer_is_empty(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get variance
     * @returns {number}
     */
    variance() {
        const ret = wasm.wasmtrajectorybuffer_variance(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) WasmTrajectoryBuffer.prototype[Symbol.dispose] = WasmTrajectoryBuffer.prototype.free;

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
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;



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
        module_or_path = new URL('ruvector_learning_wasm_bg.wasm', import.meta.url);
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
