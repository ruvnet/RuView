/* @ts-self-types="./ruvector_attention_wasm.d.ts" */

/**
 * Adam optimizer
 */
class WasmAdam {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmAdamFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmadam_free(ptr, 0);
    }
    /**
     * Get current learning rate
     * @returns {number}
     */
    get learning_rate() {
        const ret = wasm.wasmadam_learning_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new Adam optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     * @param {number} param_count
     * @param {number} learning_rate
     */
    constructor(param_count, learning_rate) {
        const ret = wasm.wasmadam_new(param_count, learning_rate);
        this.__wbg_ptr = ret >>> 0;
        WasmAdamFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset optimizer state
     */
    reset() {
        wasm.wasmadam_reset(this.__wbg_ptr);
    }
    /**
     * Set learning rate
     * @param {number} lr
     */
    set learning_rate(lr) {
        wasm.wasmadam_set_learning_rate(this.__wbg_ptr, lr);
    }
    /**
     * Perform optimization step
     *
     * # Arguments
     * * `params` - Current parameter values (will be updated in-place)
     * * `gradients` - Gradient values
     * @param {Float32Array} params
     * @param {Float32Array} gradients
     */
    step(params, gradients) {
        var ptr0 = passArrayF32ToWasm0(params, wasm.__wbindgen_export);
        var len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(gradients, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.wasmadam_step(this.__wbg_ptr, ptr0, len0, addHeapObject(params), ptr1, len1);
    }
}
if (Symbol.dispose) WasmAdam.prototype[Symbol.dispose] = WasmAdam.prototype.free;
exports.WasmAdam = WasmAdam;

/**
 * AdamW optimizer (Adam with decoupled weight decay)
 */
class WasmAdamW {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmAdamWFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmadamw_free(ptr, 0);
    }
    /**
     * Get current learning rate
     * @returns {number}
     */
    get learning_rate() {
        const ret = wasm.wasmadamw_learning_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new AdamW optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     * * `weight_decay` - Weight decay coefficient
     * @param {number} param_count
     * @param {number} learning_rate
     * @param {number} weight_decay
     */
    constructor(param_count, learning_rate, weight_decay) {
        const ret = wasm.wasmadamw_new(param_count, learning_rate, weight_decay);
        this.__wbg_ptr = ret >>> 0;
        WasmAdamWFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset optimizer state
     */
    reset() {
        wasm.wasmadamw_reset(this.__wbg_ptr);
    }
    /**
     * Set learning rate
     * @param {number} lr
     */
    set learning_rate(lr) {
        wasm.wasmadamw_set_learning_rate(this.__wbg_ptr, lr);
    }
    /**
     * Perform optimization step with weight decay
     * @param {Float32Array} params
     * @param {Float32Array} gradients
     */
    step(params, gradients) {
        var ptr0 = passArrayF32ToWasm0(params, wasm.__wbindgen_export);
        var len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(gradients, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.wasmadamw_step(this.__wbg_ptr, ptr0, len0, addHeapObject(params), ptr1, len1);
    }
    /**
     * Get weight decay
     * @returns {number}
     */
    get weight_decay() {
        const ret = wasm.wasmadamw_weight_decay(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) WasmAdamW.prototype[Symbol.dispose] = WasmAdamW.prototype.free;
exports.WasmAdamW = WasmAdamW;

/**
 * Flash attention mechanism
 */
class WasmFlashAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmFlashAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmflashattention_free(ptr, 0);
    }
    /**
     * Compute flash attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmflashattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Create a new flash attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `block_size` - Block size for tiling
     * @param {number} dim
     * @param {number} block_size
     */
    constructor(dim, block_size) {
        const ret = wasm.wasmflashattention_new(dim, block_size);
        this.__wbg_ptr = ret >>> 0;
        WasmFlashAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmFlashAttention.prototype[Symbol.dispose] = WasmFlashAttention.prototype.free;
exports.WasmFlashAttention = WasmFlashAttention;

/**
 * Hyperbolic attention mechanism
 */
class WasmHyperbolicAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmHyperbolicAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmhyperbolicattention_free(ptr, 0);
    }
    /**
     * Compute hyperbolic attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmhyperbolicattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Get the curvature
     * @returns {number}
     */
    get curvature() {
        const ret = wasm.wasmhyperbolicattention_curvature(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new hyperbolic attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `curvature` - Hyperbolic curvature parameter
     * @param {number} dim
     * @param {number} curvature
     */
    constructor(dim, curvature) {
        const ret = wasm.wasmhyperbolicattention_new(dim, curvature);
        this.__wbg_ptr = ret >>> 0;
        WasmHyperbolicAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmHyperbolicAttention.prototype[Symbol.dispose] = WasmHyperbolicAttention.prototype.free;
exports.WasmHyperbolicAttention = WasmHyperbolicAttention;

/**
 * InfoNCE contrastive loss for training
 */
class WasmInfoNCELoss {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmInfoNCELossFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasminfonceloss_free(ptr, 0);
    }
    /**
     * Compute InfoNCE loss
     *
     * # Arguments
     * * `anchor` - Anchor embedding
     * * `positive` - Positive example embedding
     * * `negatives` - Array of negative example embeddings
     * @param {Float32Array} anchor
     * @param {Float32Array} positive
     * @param {any} negatives
     * @returns {number}
     */
    compute(anchor, positive, negatives) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(anchor, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayF32ToWasm0(positive, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            wasm.wasminfonceloss_compute(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, addHeapObject(negatives));
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
     * Create a new InfoNCE loss instance
     *
     * # Arguments
     * * `temperature` - Temperature parameter for softmax
     * @param {number} temperature
     */
    constructor(temperature) {
        const ret = wasm.wasminfonceloss_new(temperature);
        this.__wbg_ptr = ret >>> 0;
        WasmInfoNCELossFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmInfoNCELoss.prototype[Symbol.dispose] = WasmInfoNCELoss.prototype.free;
exports.WasmInfoNCELoss = WasmInfoNCELoss;

/**
 * Learning rate scheduler
 */
class WasmLRScheduler {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmLRSchedulerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmlrscheduler_free(ptr, 0);
    }
    /**
     * Get learning rate for current step
     * @returns {number}
     */
    get_lr() {
        const ret = wasm.wasmlrscheduler_get_lr(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new learning rate scheduler with warmup and cosine decay
     *
     * # Arguments
     * * `initial_lr` - Initial learning rate
     * * `warmup_steps` - Number of warmup steps
     * * `total_steps` - Total training steps
     * @param {number} initial_lr
     * @param {number} warmup_steps
     * @param {number} total_steps
     */
    constructor(initial_lr, warmup_steps, total_steps) {
        const ret = wasm.wasmlrscheduler_new(initial_lr, warmup_steps, total_steps);
        this.__wbg_ptr = ret >>> 0;
        WasmLRSchedulerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset scheduler
     */
    reset() {
        wasm.wasmlrscheduler_reset(this.__wbg_ptr);
    }
    /**
     * Advance to next step
     */
    step() {
        wasm.wasmlrscheduler_step(this.__wbg_ptr);
    }
}
if (Symbol.dispose) WasmLRScheduler.prototype[Symbol.dispose] = WasmLRScheduler.prototype.free;
exports.WasmLRScheduler = WasmLRScheduler;

/**
 * Linear attention (Performer-style)
 */
class WasmLinearAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmLinearAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmlinearattention_free(ptr, 0);
    }
    /**
     * Compute linear attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmlinearattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Create a new linear attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_features` - Number of random features
     * @param {number} dim
     * @param {number} num_features
     */
    constructor(dim, num_features) {
        const ret = wasm.wasmlinearattention_new(dim, num_features);
        this.__wbg_ptr = ret >>> 0;
        WasmLinearAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmLinearAttention.prototype[Symbol.dispose] = WasmLinearAttention.prototype.free;
exports.WasmLinearAttention = WasmLinearAttention;

/**
 * Local-global attention mechanism
 */
class WasmLocalGlobalAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmLocalGlobalAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmlocalglobalattention_free(ptr, 0);
    }
    /**
     * Compute local-global attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmlocalglobalattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Create a new local-global attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `local_window` - Size of local attention window
     * * `global_tokens` - Number of global attention tokens
     * @param {number} dim
     * @param {number} local_window
     * @param {number} global_tokens
     */
    constructor(dim, local_window, global_tokens) {
        const ret = wasm.wasmlocalglobalattention_new(dim, local_window, global_tokens);
        this.__wbg_ptr = ret >>> 0;
        WasmLocalGlobalAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmLocalGlobalAttention.prototype[Symbol.dispose] = WasmLocalGlobalAttention.prototype.free;
exports.WasmLocalGlobalAttention = WasmLocalGlobalAttention;

/**
 * Mixture of Experts (MoE) attention
 */
class WasmMoEAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMoEAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmoeattention_free(ptr, 0);
    }
    /**
     * Compute MoE attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmmoeattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Create a new MoE attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_experts` - Number of expert attention mechanisms
     * * `top_k` - Number of experts to use per query
     * @param {number} dim
     * @param {number} num_experts
     * @param {number} top_k
     */
    constructor(dim, num_experts, top_k) {
        const ret = wasm.wasmmoeattention_new(dim, num_experts, top_k);
        this.__wbg_ptr = ret >>> 0;
        WasmMoEAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmMoEAttention.prototype[Symbol.dispose] = WasmMoEAttention.prototype.free;
exports.WasmMoEAttention = WasmMoEAttention;

/**
 * Multi-head attention mechanism
 */
class WasmMultiHeadAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMultiHeadAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmultiheadattention_free(ptr, 0);
    }
    /**
     * Compute multi-head attention
     * @param {Float32Array} query
     * @param {any} keys
     * @param {any} values
     * @returns {Float32Array}
     */
    compute(query, keys, values) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.wasmmultiheadattention_compute(retptr, this.__wbg_ptr, ptr0, len0, addHeapObject(keys), addHeapObject(values));
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
     * Get the dimension
     * @returns {number}
     */
    get dim() {
        const ret = wasm.wasmmultiheadattention_dim(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new multi-head attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_heads` - Number of attention heads
     * @param {number} dim
     * @param {number} num_heads
     */
    constructor(dim, num_heads) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.wasmmultiheadattention_new(retptr, dim, num_heads);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            WasmMultiHeadAttentionFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get the number of heads
     * @returns {number}
     */
    get num_heads() {
        const ret = wasm.wasmmultiheadattention_num_heads(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) WasmMultiHeadAttention.prototype[Symbol.dispose] = WasmMultiHeadAttention.prototype.free;
exports.WasmMultiHeadAttention = WasmMultiHeadAttention;

/**
 * SGD optimizer with momentum
 */
class WasmSGD {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmSGDFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmsgd_free(ptr, 0);
    }
    /**
     * Get current learning rate
     * @returns {number}
     */
    get learning_rate() {
        const ret = wasm.wasmsgd_learning_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new SGD optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     * * `momentum` - Momentum coefficient (default: 0)
     * @param {number} param_count
     * @param {number} learning_rate
     * @param {number | null} [momentum]
     */
    constructor(param_count, learning_rate, momentum) {
        const ret = wasm.wasmsgd_new(param_count, learning_rate, isLikeNone(momentum) ? 0x100000001 : Math.fround(momentum));
        this.__wbg_ptr = ret >>> 0;
        WasmSGDFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset optimizer state
     */
    reset() {
        wasm.wasmsgd_reset(this.__wbg_ptr);
    }
    /**
     * Set learning rate
     * @param {number} lr
     */
    set learning_rate(lr) {
        wasm.wasmsgd_set_learning_rate(this.__wbg_ptr, lr);
    }
    /**
     * Perform optimization step
     * @param {Float32Array} params
     * @param {Float32Array} gradients
     */
    step(params, gradients) {
        var ptr0 = passArrayF32ToWasm0(params, wasm.__wbindgen_export);
        var len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(gradients, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.wasmsgd_step(this.__wbg_ptr, ptr0, len0, addHeapObject(params), ptr1, len1);
    }
}
if (Symbol.dispose) WasmSGD.prototype[Symbol.dispose] = WasmSGD.prototype.free;
exports.WasmSGD = WasmSGD;

/**
 * Compute attention weights from scores
 * @param {Float32Array} scores
 * @param {number | null} [temperature]
 */
function attention_weights(scores, temperature) {
    var ptr0 = passArrayF32ToWasm0(scores, wasm.__wbindgen_export);
    var len0 = WASM_VECTOR_LEN;
    wasm.attention_weights(ptr0, len0, addHeapObject(scores), isLikeNone(temperature) ? 0x100000001 : Math.fround(temperature));
}
exports.attention_weights = attention_weights;

/**
 * Get information about available attention mechanisms
 * @returns {any}
 */
function available_mechanisms() {
    const ret = wasm.available_mechanisms();
    return takeObject(ret);
}
exports.available_mechanisms = available_mechanisms;

/**
 * Batch normalize vectors
 * @param {any} vectors
 * @param {number | null} [epsilon]
 * @returns {Float32Array}
 */
function batch_normalize(vectors, epsilon) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.batch_normalize(retptr, addHeapObject(vectors), isLikeNone(epsilon) ? 0x100000001 : Math.fround(epsilon));
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v1 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}
exports.batch_normalize = batch_normalize;

/**
 * Compute cosine similarity between two vectors
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @returns {number}
 */
function cosine_similarity(a, b) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(a, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(b, wasm.__wbindgen_export);
        const len1 = WASM_VECTOR_LEN;
        wasm.cosine_similarity(retptr, ptr0, len0, ptr1, len1);
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
exports.cosine_similarity = cosine_similarity;

/**
 * Initialize the WASM module with panic hook
 */
function init() {
    wasm.init();
}
exports.init = init;

/**
 * Compute L2 norm of a vector
 * @param {Float32Array} vec
 * @returns {number}
 */
function l2_norm(vec) {
    const ptr0 = passArrayF32ToWasm0(vec, wasm.__wbindgen_export);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.l2_norm(ptr0, len0);
    return ret;
}
exports.l2_norm = l2_norm;

/**
 * Log a message to the browser console
 * @param {string} message
 */
function log(message) {
    const ptr0 = passStringToWasm0(message, wasm.__wbindgen_export, wasm.__wbindgen_export2);
    const len0 = WASM_VECTOR_LEN;
    wasm.log(ptr0, len0);
}
exports.log = log;

/**
 * Log an error to the browser console
 * @param {string} message
 */
function log_error(message) {
    const ptr0 = passStringToWasm0(message, wasm.__wbindgen_export, wasm.__wbindgen_export2);
    const len0 = WASM_VECTOR_LEN;
    wasm.log_error(ptr0, len0);
}
exports.log_error = log_error;

/**
 * Normalize a vector to unit length
 * @param {Float32Array} vec
 */
function normalize(vec) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        var ptr0 = passArrayF32ToWasm0(vec, wasm.__wbindgen_export);
        var len0 = WASM_VECTOR_LEN;
        wasm.normalize(retptr, ptr0, len0, addHeapObject(vec));
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        if (r1) {
            throw takeObject(r0);
        }
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}
exports.normalize = normalize;

/**
 * Compute pairwise distances between vectors
 * @param {any} vectors
 * @returns {Float32Array}
 */
function pairwise_distances(vectors) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.pairwise_distances(retptr, addHeapObject(vectors));
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v1 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}
exports.pairwise_distances = pairwise_distances;

/**
 * Generate random orthogonal matrix (for initialization)
 * @param {number} dim
 * @returns {Float32Array}
 */
function random_orthogonal_matrix(dim) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.random_orthogonal_matrix(retptr, dim);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v1 = getArrayF32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export4(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}
exports.random_orthogonal_matrix = random_orthogonal_matrix;

/**
 * Compute scaled dot-product attention
 *
 * # Arguments
 * * `query` - Query vector as Float32Array
 * * `keys` - Array of key vectors
 * * `values` - Array of value vectors
 * * `scale` - Optional scaling factor (defaults to 1/sqrt(dim))
 * @param {Float32Array} query
 * @param {any} keys
 * @param {any} values
 * @param {number | null} [scale]
 * @returns {Float32Array}
 */
function scaled_dot_attention(query, keys, values, scale) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArrayF32ToWasm0(query, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.scaled_dot_attention(retptr, ptr0, len0, addHeapObject(keys), addHeapObject(values), isLikeNone(scale) ? 0x100000001 : Math.fround(scale));
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
exports.scaled_dot_attention = scaled_dot_attention;

/**
 * Compute softmax of a vector
 * @param {Float32Array} vec
 */
function softmax(vec) {
    var ptr0 = passArrayF32ToWasm0(vec, wasm.__wbindgen_export);
    var len0 = WASM_VECTOR_LEN;
    wasm.softmax(ptr0, len0, addHeapObject(vec));
}
exports.softmax = softmax;

/**
 * Get the version of the ruvector-attention-wasm crate
 * @returns {string}
 */
function version() {
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
exports.version = version;

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_4577686b3a6d9b3a: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_String_8564e559799eccda: function(arg0, arg1) {
            const ret = String(getObject(arg1));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_boolean_get_18c4ed9422296fff: function(arg0) {
            const v = getObject(arg0);
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_copy_to_typed_array_5294f8e46aecc086: function(arg0, arg1, arg2) {
            new Uint8Array(getObject(arg2).buffer, getObject(arg2).byteOffset, getObject(arg2).byteLength).set(getArrayU8FromWasm0(arg0, arg1));
        },
        __wbg___wbindgen_debug_string_ddde1867f49c2442: function(arg0, arg1) {
            const ret = debugString(getObject(arg1));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_d633e708baf0d146: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_object_4b3de556756ee8a8: function(arg0) {
            const val = getObject(arg0);
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_jsval_loose_eq_1562ceb9af84e990: function(arg0, arg1) {
            const ret = getObject(arg0) == getObject(arg1);
            return ret;
        },
        __wbg___wbindgen_number_get_5854912275df1894: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_3e5751597f39a112: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_39bc967c0e5a9b58: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_73af281463ec8b58: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg0).call(getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_done_5aad55ec6b1954b1: function(arg0) {
            const ret = getObject(arg0).done;
            return ret;
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_error_ad28debb48b5c6bb: function(arg0) {
            console.error(getObject(arg0));
        },
        __wbg_get_4920fefd3451364b: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(getObject(arg0), getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_get_unchecked_3d0f4b91c8eca4f0: function(arg0, arg1) {
            const ret = getObject(arg0)[arg1 >>> 0];
            return addHeapObject(ret);
        },
        __wbg_instanceof_ArrayBuffer_15859862b80b732d: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Uint8Array_2240b7046ac16f05: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_isArray_fad08a0d12828686: function(arg0) {
            const ret = Array.isArray(getObject(arg0));
            return ret;
        },
        __wbg_iterator_fc7ad8d33bab9e26: function() {
            const ret = Symbol.iterator;
            return addHeapObject(ret);
        },
        __wbg_length_5855c1f289dfffc1: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_length_a31e05262e09b7f8: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_log_3c5e4b64af29e724: function(arg0) {
            console.log(getObject(arg0));
        },
        __wbg_new_09959f7b4c92c246: function(arg0) {
            const ret = new Uint8Array(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return addHeapObject(ret);
        },
        __wbg_new_cbee8c0d5c479eac: function() {
            const ret = new Array();
            return addHeapObject(ret);
        },
        __wbg_next_a5fe6f328f7affc2: function(arg0) {
            const ret = getObject(arg0).next;
            return addHeapObject(ret);
        },
        __wbg_next_e592122bb4ed4c67: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).next();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_prototypesetcall_f034d444741426c3: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
        },
        __wbg_random_2b7bed8995d680fb: function() {
            const ret = Math.random();
            return ret;
        },
        __wbg_set_4c81cfb5dc3a333c: function(arg0, arg1, arg2) {
            getObject(arg0)[arg1 >>> 0] = takeObject(arg2);
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = getObject(arg1).stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_value_667dcb90597486a6: function(arg0) {
            const ret = getObject(arg0).value;
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        },
        __wbindgen_object_drop_ref: function(arg0) {
            takeObject(arg0);
        },
    };
    return {
        __proto__: null,
        "./ruvector_attention_wasm_bg.js": import0,
    };
}

const WasmAdamFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmadam_free(ptr >>> 0, 1));
const WasmAdamWFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmadamw_free(ptr >>> 0, 1));
const WasmFlashAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmflashattention_free(ptr >>> 0, 1));
const WasmHyperbolicAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmhyperbolicattention_free(ptr >>> 0, 1));
const WasmInfoNCELossFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasminfonceloss_free(ptr >>> 0, 1));
const WasmLRSchedulerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmlrscheduler_free(ptr >>> 0, 1));
const WasmLinearAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmlinearattention_free(ptr >>> 0, 1));
const WasmLocalGlobalAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmlocalglobalattention_free(ptr >>> 0, 1));
const WasmMoEAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmoeattention_free(ptr >>> 0, 1));
const WasmMultiHeadAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmultiheadattention_free(ptr >>> 0, 1));
const WasmSGDFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmsgd_free(ptr >>> 0, 1));

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
    if (idx < 1028) return;
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

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
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
function decodeText(ptr, len) {
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
    };
}

let WASM_VECTOR_LEN = 0;

const wasmPath = `${__dirname}/ruvector_attention_wasm_bg.wasm`;
const wasmBytes = require('fs').readFileSync(wasmPath);
const wasmModule = new WebAssembly.Module(wasmBytes);
let wasm = new WebAssembly.Instance(wasmModule, __wbg_get_imports()).exports;
wasm.__wbindgen_start();
