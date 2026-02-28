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

let heap = new Array(128).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
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

const CreditLedgerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_creditledger_free(ptr >>> 0, 1));

const ReputationScoreFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_reputationscore_free(ptr >>> 0, 1));

const StakeManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_stakemanager_free(ptr >>> 0, 1));

/**
 * CRDT-based credit ledger for P2P consistency
 *
 * The ledger uses two types of counters:
 * - G-Counter (grow-only) for credits earned - safe for concurrent updates
 * - PN-Counter (positive-negative) for credits spent - supports disputes
 *
 * ```text
 * Earned (G-Counter):     Spent (PN-Counter):
 * +----------------+      +--------------------+
 * | event_1: 100   |      | event_a: (50, 0)   |  <- (positive, negative)
 * | event_2: 200   |      | event_b: (30, 10)  |  <- disputed 10 returned
 * | event_3: 150   |      +--------------------+
 * +----------------+
 *
 * Balance = sum(earned) - sum(spent.positive - spent.negative) - staked
 * ```
 */
export class CreditLedger {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        CreditLedgerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_creditledger_free(ptr, 0);
    }
    /**
     * Get the state root (Merkle root of ledger state)
     * @returns {Uint8Array}
     */
    stateRoot() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_stateRoot(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get event count
     * @returns {number}
     */
    eventCount() {
        const ret = wasm.creditledger_eventCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get total credits spent
     * @returns {bigint}
     */
    totalSpent() {
        const ret = wasm.creditledger_totalSpent(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Export spent counter for P2P sync
     * @returns {Uint8Array}
     */
    exportSpent() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_exportSpent(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get total credits ever earned (before spending)
     * @returns {bigint}
     */
    totalEarned() {
        const ret = wasm.creditledger_totalEarned(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Export earned counter for P2P sync
     * @returns {Uint8Array}
     */
    exportEarned() {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_exportEarned(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            var v1 = getArrayU8FromWasm0(r0, r1).slice();
            wasm.__wbindgen_export(r0, r1 * 1, 1);
            return v1;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get staked amount
     * @returns {bigint}
     */
    stakedAmount() {
        const ret = wasm.creditledger_stakedAmount(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get state root as hex string
     * @returns {string}
     */
    stateRootHex() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_stateRootHex(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get network compute hours
     * @returns {number}
     */
    networkCompute() {
        const ret = wasm.creditledger_networkCompute(this.__wbg_ptr);
        return ret;
    }
    /**
     * Verify state root matches current state
     * @param {Uint8Array} expected_root
     * @returns {boolean}
     */
    verifyStateRoot(expected_root) {
        const ptr0 = passArray8ToWasm0(expected_root, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.creditledger_verifyStateRoot(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get current contribution multiplier
     * @returns {number}
     */
    currentMultiplier() {
        const ret = wasm.creditledger_currentMultiplier(this.__wbg_ptr);
        return ret;
    }
    /**
     * Credit with multiplier applied (for task rewards)
     * @param {bigint} base_amount
     * @param {string} reason
     * @returns {string}
     */
    creditWithMultiplier(base_amount, reason) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(reason, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.creditledger_creditWithMultiplier(retptr, this.__wbg_ptr, base_amount, ptr0, len0);
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
            wasm.__wbindgen_export(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Update network compute hours (from P2P sync)
     * @param {number} hours
     */
    updateNetworkCompute(hours) {
        wasm.creditledger_updateNetworkCompute(this.__wbg_ptr, hours);
    }
    /**
     * Create a new credit ledger for a node
     * @param {string} node_id
     */
    constructor(node_id) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.creditledger_new(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            this.__wbg_ptr = r0 >>> 0;
            CreditLedgerFinalization.register(this, this.__wbg_ptr, this);
            return this;
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Merge with another ledger (CRDT merge operation)
     *
     * This is the core CRDT operation - associative, commutative, and idempotent.
     * Safe to apply in any order with any number of concurrent updates.
     * @param {Uint8Array} other_earned
     * @param {Uint8Array} other_spent
     * @returns {number}
     */
    merge(other_earned, other_spent) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(other_earned, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArray8ToWasm0(other_spent, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.creditledger_merge(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1);
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
     * Slash staked credits (penalty for bad behavior)
     *
     * Returns the actual amount slashed (may be less if stake is insufficient)
     * @param {bigint} amount
     * @returns {bigint}
     */
    slash(amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_slash(retptr, this.__wbg_ptr, amount);
            var r0 = getDataViewMemory0().getBigInt64(retptr + 8 * 0, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            return BigInt.asUintN(64, r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Stake credits for participation
     * @param {bigint} amount
     */
    stake(amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_stake(retptr, this.__wbg_ptr, amount);
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
     * Credit the ledger (earn credits)
     *
     * This updates the G-Counter which is monotonically increasing.
     * Safe for concurrent P2P updates.
     * @param {bigint} amount
     * @param {string} _reason
     * @returns {string}
     */
    credit(amount, _reason) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(_reason, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.creditledger_credit(retptr, this.__wbg_ptr, amount, ptr0, len0);
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
            wasm.__wbindgen_export(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Deduct from the ledger (spend credits)
     *
     * This updates the PN-Counter positive side.
     * Spending can be disputed/refunded by updating the negative side.
     * @param {bigint} amount
     * @returns {string}
     */
    deduct(amount) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_deduct(retptr, this.__wbg_ptr, amount);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeObject(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Refund a previous deduction (dispute resolution)
     *
     * This updates the PN-Counter negative side for the given event.
     * @param {string} event_id
     * @param {bigint} amount
     */
    refund(event_id, amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(event_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.creditledger_refund(retptr, this.__wbg_ptr, ptr0, len0, amount);
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
     * Get current available balance (earned - spent - staked)
     * @returns {bigint}
     */
    balance() {
        const ret = wasm.creditledger_balance(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get the node ID
     * @returns {string}
     */
    nodeId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_nodeId(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Unstake credits
     * @param {bigint} amount
     */
    unstake(amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.creditledger_unstake(retptr, this.__wbg_ptr, amount);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
}
if (Symbol.dispose) CreditLedger.prototype[Symbol.dispose] = CreditLedger.prototype.free;

/**
 * Reputation score for a network participant
 *
 * Combines multiple factors into a single trust score:
 * - accuracy: 0.0 to 1.0 (success rate of verified tasks)
 * - uptime: 0.0 to 1.0 (availability ratio)
 * - stake: absolute stake amount (economic commitment)
 *
 * The composite score is weighted:
 * ```text
 * composite = accuracy^2 * uptime * stake_weight
 *
 * where stake_weight = min(1.0, log10(stake + 1) / 6)
 * ```
 *
 * This ensures:
 * - Accuracy is most important (squared)
 * - Uptime provides linear scaling
 * - Stake has diminishing returns (log scale)
 */
export class ReputationScore {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(ReputationScore.prototype);
        obj.__wbg_ptr = ptr;
        ReputationScoreFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReputationScoreFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_reputationscore_free(ptr, 0);
    }
    /**
     * Get total tasks
     * @returns {bigint}
     */
    totalTasks() {
        const ret = wasm.reputationscore_totalTasks(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Calculate stake weight using logarithmic scaling
     *
     * Uses log10(stake + 1) / 6 capped at 1.0
     * This means:
     * - 0 stake = 0.0 weight
     * - 100 stake = ~0.33 weight
     * - 10,000 stake = ~0.67 weight
     * - 1,000,000 stake = 1.0 weight (capped)
     * @returns {number}
     */
    stakeWeight() {
        const ret = wasm.reputationscore_stakeWeight(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get tasks failed
     * @returns {bigint}
     */
    tasksFailed() {
        const ret = wasm.reputationscore_tasksFailed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Update stake amount
     * @param {bigint} new_stake
     */
    updateStake(new_stake) {
        wasm.reputationscore_updateStake(this.__wbg_ptr, new_stake);
    }
    /**
     * Check if node meets minimum reputation for participation
     * @param {number} min_accuracy
     * @param {number} min_uptime
     * @param {bigint} min_stake
     * @returns {boolean}
     */
    meetsMinimum(min_accuracy, min_uptime, min_stake) {
        const ret = wasm.reputationscore_meetsMinimum(this.__wbg_ptr, min_accuracy, min_uptime, min_stake);
        return ret !== 0;
    }
    /**
     * Update uptime tracking
     * @param {bigint} online_seconds
     * @param {bigint} total_seconds
     */
    updateUptime(online_seconds, total_seconds) {
        wasm.reputationscore_updateUptime(this.__wbg_ptr, online_seconds, total_seconds);
    }
    /**
     * Check if this reputation is better than another
     * @param {ReputationScore} other
     * @returns {boolean}
     */
    isBetterThan(other) {
        _assertClass(other, ReputationScore);
        const ret = wasm.reputationscore_isBetterThan(this.__wbg_ptr, other.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Record a failed/disputed task
     */
    recordFailure() {
        wasm.reputationscore_recordFailure(this.__wbg_ptr);
    }
    /**
     * Record a successful task completion
     */
    recordSuccess() {
        wasm.reputationscore_recordSuccess(this.__wbg_ptr);
    }
    /**
     * Calculate composite reputation score
     *
     * Formula: accuracy^2 * uptime * stake_weight
     *
     * Returns a value between 0.0 and 1.0
     * @returns {number}
     */
    compositeScore() {
        const ret = wasm.reputationscore_compositeScore(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get tasks completed
     * @returns {bigint}
     */
    tasksCompleted() {
        const ret = wasm.reputationscore_tasksCompleted(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Create with detailed tracking
     * @param {bigint} tasks_completed
     * @param {bigint} tasks_failed
     * @param {bigint} uptime_seconds
     * @param {bigint} total_seconds
     * @param {bigint} stake
     * @returns {ReputationScore}
     */
    static newWithTracking(tasks_completed, tasks_failed, uptime_seconds, total_seconds, stake) {
        const ret = wasm.reputationscore_newWithTracking(tasks_completed, tasks_failed, uptime_seconds, total_seconds, stake);
        return ReputationScore.__wrap(ret);
    }
    /**
     * Create a new reputation score
     * @param {number} accuracy
     * @param {number} uptime
     * @param {bigint} stake
     */
    constructor(accuracy, uptime, stake) {
        const ret = wasm.reputationscore_new(accuracy, uptime, stake);
        this.__wbg_ptr = ret >>> 0;
        ReputationScoreFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get stake amount
     * @returns {bigint}
     */
    get stake() {
        const ret = wasm.reputationscore_stake(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get uptime score (0.0 - 1.0)
     * @returns {number}
     */
    get uptime() {
        const ret = wasm.reputationscore_uptime(this.__wbg_ptr);
        return ret;
    }
    /**
     * Serialize to JSON
     * @returns {string}
     */
    toJson() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.reputationscore_toJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get accuracy score (0.0 - 1.0)
     * @returns {number}
     */
    get accuracy() {
        const ret = wasm.reputationscore_accuracy(this.__wbg_ptr);
        return ret;
    }
    /**
     * Deserialize from JSON
     * @param {string} json
     * @returns {ReputationScore}
     */
    static fromJson(json) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(json, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.reputationscore_fromJson(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return ReputationScore.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Get reputation tier based on composite score
     * @returns {string}
     */
    tierName() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.reputationscore_tierName(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) ReputationScore.prototype[Symbol.dispose] = ReputationScore.prototype.free;

/**
 * Reasons for slashing stake
 * @enum {0 | 1 | 2 | 3 | 4 | 5}
 */
export const SlashReason = Object.freeze({
    /**
     * Invalid task result
     */
    InvalidResult: 0, "0": "InvalidResult",
    /**
     * Double-spending attempt
     */
    DoubleSpend: 1, "1": "DoubleSpend",
    /**
     * Sybil attack detected
     */
    SybilAttack: 2, "2": "SybilAttack",
    /**
     * Excessive downtime
     */
    Downtime: 3, "3": "Downtime",
    /**
     * Spam/flooding
     */
    Spam: 4, "4": "Spam",
    /**
     * Malicious behavior
     */
    Malicious: 5, "5": "Malicious",
});

/**
 * Stake manager for the network
 */
export class StakeManager {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(StakeManager.prototype);
        obj.__wbg_ptr = ptr;
        StakeManagerFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        StakeManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_stakemanager_free(ptr, 0);
    }
    /**
     * Undelegate stake
     * @param {string} from_node
     * @param {string} to_node
     * @param {bigint} amount
     */
    undelegate(from_node, to_node, amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(from_node, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(to_node, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len1 = WASM_VECTOR_LEN;
            wasm.stakemanager_undelegate(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, amount);
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
     * Export stake data as JSON
     * @returns {string}
     */
    exportJson() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.stakemanager_exportJson(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get number of stakers
     * @returns {number}
     */
    stakerCount() {
        const ret = wasm.stakemanager_stakerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get total network staked
     * @returns {bigint}
     */
    totalStaked() {
        const ret = wasm.stakemanager_totalStaked(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Check if node meets minimum stake
     * @param {string} node_id
     * @returns {boolean}
     */
    meetsMinimum(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_meetsMinimum(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get total slashed
     * @returns {bigint}
     */
    totalSlashed() {
        const ret = wasm.stakemanager_totalSlashed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get slash count for a node
     * @param {string} node_id
     * @returns {number}
     */
    getSlashCount(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getSlashCount(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Create with custom parameters
     * @param {bigint} min_stake
     * @param {bigint} lock_period_ms
     * @returns {StakeManager}
     */
    static newWithParams(min_stake, lock_period_ms) {
        const ret = wasm.stakemanager_newWithParams(min_stake, lock_period_ms);
        return StakeManager.__wrap(ret);
    }
    /**
     * Get lock timestamp for a node
     * @param {string} node_id
     * @returns {bigint}
     */
    getLockTimestamp(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getLockTimestamp(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get delegator count
     * @param {string} node_id
     * @returns {number}
     */
    getDelegatorCount(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getDelegatorCount(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Get effective stake (own + delegated)
     * @param {string} node_id
     * @returns {bigint}
     */
    getEffectiveStake(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getEffectiveStake(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get total amount slashed from a node
     * @param {string} node_id
     * @returns {bigint}
     */
    getNodeTotalSlashed(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getNodeTotalSlashed(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Create a new stake manager
     */
    constructor() {
        const ret = wasm.stakemanager_new();
        this.__wbg_ptr = ret >>> 0;
        StakeManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Slash stake for bad behavior
     * @param {string} node_id
     * @param {SlashReason} reason
     * @param {string} evidence
     * @returns {bigint}
     */
    slash(node_id, reason, evidence) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(evidence, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len1 = WASM_VECTOR_LEN;
            wasm.stakemanager_slash(retptr, this.__wbg_ptr, ptr0, len0, reason, ptr1, len1);
            var r0 = getDataViewMemory0().getBigInt64(retptr + 8 * 0, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            return BigInt.asUintN(64, r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Stake credits for a node
     * @param {string} node_id
     * @param {bigint} amount
     */
    stake(node_id, amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.stakemanager_stake(retptr, this.__wbg_ptr, ptr0, len0, amount);
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
     * Unstake credits (if lock period has passed)
     * @param {string} node_id
     * @param {bigint} amount
     * @returns {bigint}
     */
    unstake(node_id, amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            wasm.stakemanager_unstake(retptr, this.__wbg_ptr, ptr0, len0, amount);
            var r0 = getDataViewMemory0().getBigInt64(retptr + 8 * 0, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            if (r3) {
                throw takeObject(r2);
            }
            return BigInt.asUintN(64, r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Delegate stake to another node
     * @param {string} from_node
     * @param {string} to_node
     * @param {bigint} amount
     */
    delegate(from_node, to_node, amount) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(from_node, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(to_node, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
            const len1 = WASM_VECTOR_LEN;
            wasm.stakemanager_delegate(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, amount);
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
     * Get stake for a node
     * @param {string} node_id
     * @returns {bigint}
     */
    getStake(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getStake(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Check if stake is locked
     * @param {string} node_id
     * @returns {boolean}
     */
    isLocked(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_isLocked(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get minimum stake requirement
     * @returns {bigint}
     */
    minStake() {
        const ret = wasm.reputationscore_tasksFailed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
}
if (Symbol.dispose) StakeManager.prototype[Symbol.dispose] = StakeManager.prototype.free;

/**
 * Calculate reward with multiplier (WASM export)
 * @param {bigint} base_reward
 * @param {number} network_compute_hours
 * @returns {bigint}
 */
export function calculate_reward(base_reward, network_compute_hours) {
    const ret = wasm.calculate_reward(base_reward, network_compute_hours);
    return BigInt.asUintN(64, ret);
}

/**
 * Calculate composite reputation score (WASM export)
 * @param {number} accuracy
 * @param {number} uptime
 * @param {bigint} stake
 * @returns {number}
 */
export function composite_reputation(accuracy, uptime, stake) {
    const ret = wasm.composite_reputation(accuracy, uptime, stake);
    return ret;
}

/**
 * Calculate contribution multiplier (WASM export)
 *
 * Returns the reward multiplier based on total network compute hours.
 * Early adopters get up to 10x rewards, decaying to 1x as network grows.
 * @param {number} network_compute_hours
 * @returns {number}
 */
export function contribution_multiplier(network_compute_hours) {
    const ret = wasm.contribution_multiplier(network_compute_hours);
    return ret;
}

/**
 * Get tier name based on compute level (WASM export)
 * @param {number} network_compute_hours
 * @returns {string}
 */
export function get_tier_name(network_compute_hours) {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.get_tier_name(retptr, network_compute_hours);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Get tier information as JSON (WASM export)
 * @returns {string}
 */
export function get_tiers_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.get_tiers_json(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Initialize panic hook for better error messages in console
 */
export function init_panic_hook() {
    wasm.init_panic_hook();
}

/**
 * Calculate stake weight (WASM export)
 * @param {bigint} stake
 * @returns {number}
 */
export function stake_weight(stake) {
    const ret = wasm.stake_weight(stake);
    return ret;
}

/**
 * Get the current version of the economy module
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
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
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
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_export(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_now_69d776cd24f5215b = function() {
        const ret = Date.now();
        return ret;
    };
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = getObject(arg1).stack;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
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
        module_or_path = new URL('ruvector_economy_wasm_bg.wasm', import.meta.url);
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
