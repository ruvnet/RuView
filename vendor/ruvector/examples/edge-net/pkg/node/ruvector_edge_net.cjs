
let imports = {};
imports['__wbindgen_placeholder__'] = module.exports;

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
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

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
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

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

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

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
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

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
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
    }
}

let WASM_VECTOR_LEN = 0;

function wasm_bindgen__convert__closures_____invoke__h8c81ca6cba4eba00(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h8c81ca6cba4eba00(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h9a454594a18d3e6f(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h9a454594a18d3e6f(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h094c87b54a975e5a(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen__convert__closures_____invoke__h094c87b54a975e5a(arg0, arg1, arg2, arg3);
}

const AdaptiveSecurityFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_adaptivesecurity_free(ptr >>> 0, 1));

const AdversarialSimulatorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_adversarialsimulator_free(ptr >>> 0, 1));

const AuditLogFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_auditlog_free(ptr >>> 0, 1));

const BrowserFingerprintFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_browserfingerprint_free(ptr >>> 0, 1));

const ByzantineDetectorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_byzantinedetector_free(ptr >>> 0, 1));

const CoherenceEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_coherenceengine_free(ptr >>> 0, 1));

const CollectiveMemoryFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_collectivememory_free(ptr >>> 0, 1));

const ContributionStreamFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_contributionstream_free(ptr >>> 0, 1));

const DifferentialPrivacyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_differentialprivacy_free(ptr >>> 0, 1));

const DriftTrackerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_drifttracker_free(ptr >>> 0, 1));

const EconomicEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_economicengine_free(ptr >>> 0, 1));

const EconomicHealthFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_economichealth_free(ptr >>> 0, 1));

const EdgeNetConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_edgenetconfig_free(ptr >>> 0, 1));

const EdgeNetNodeFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_edgenetnode_free(ptr >>> 0, 1));

const EntropyConsensusFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_entropyconsensus_free(ptr >>> 0, 1));

const EventLogFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_eventlog_free(ptr >>> 0, 1));

const EvolutionEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_evolutionengine_free(ptr >>> 0, 1));

const FederatedModelFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_federatedmodel_free(ptr >>> 0, 1));

const FoundingRegistryFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_foundingregistry_free(ptr >>> 0, 1));

const GenesisKeyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_genesiskey_free(ptr >>> 0, 1));

const GenesisSunsetFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_genesissunset_free(ptr >>> 0, 1));

const GradientGossipFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_gradientgossip_free(ptr >>> 0, 1));

const ModelConsensusManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_modelconsensusmanager_free(ptr >>> 0, 1));

const MultiHeadAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_multiheadattention_free(ptr >>> 0, 1));

const NetworkEventsFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_networkevents_free(ptr >>> 0, 1));

const NetworkLearningFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_networklearning_free(ptr >>> 0, 1));

const NetworkTopologyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_networktopology_free(ptr >>> 0, 1));

const NodeConfigFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_nodeconfig_free(ptr >>> 0, 1));

const NodeStatsFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_nodestats_free(ptr >>> 0, 1));

const OptimizationEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_optimizationengine_free(ptr >>> 0, 1));

const PiKeyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_pikey_free(ptr >>> 0, 1));

const QDAGLedgerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_qdagledger_free(ptr >>> 0, 1));

const QuarantineManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_quarantinemanager_free(ptr >>> 0, 1));

const RacEconomicEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_raceconomicengine_free(ptr >>> 0, 1));

const RacSemanticRouterFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_racsemanticrouter_free(ptr >>> 0, 1));

const RateLimiterFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_ratelimiter_free(ptr >>> 0, 1));

const ReasoningBankFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_reasoningbank_free(ptr >>> 0, 1));

const ReputationManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_reputationmanager_free(ptr >>> 0, 1));

const ReputationSystemFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_reputationsystem_free(ptr >>> 0, 1));

const RewardDistributionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rewarddistribution_free(ptr >>> 0, 1));

const RewardManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rewardmanager_free(ptr >>> 0, 1));

const SemanticRouterFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_semanticrouter_free(ptr >>> 0, 1));

const SessionKeyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sessionkey_free(ptr >>> 0, 1));

const SpikeDrivenAttentionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_spikedrivenattention_free(ptr >>> 0, 1));

const SpotCheckerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_spotchecker_free(ptr >>> 0, 1));

const StakeManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_stakemanager_free(ptr >>> 0, 1));

const SwarmIntelligenceFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_swarmintelligence_free(ptr >>> 0, 1));

const SybilDefenseFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sybildefense_free(ptr >>> 0, 1));

const TopKSparsifierFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_topksparsifier_free(ptr >>> 0, 1));

const TrajectoryTrackerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_trajectorytracker_free(ptr >>> 0, 1));

const WasmAdapterPoolFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmadapterpool_free(ptr >>> 0, 1));

const WasmCapabilitiesFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmcapabilities_free(ptr >>> 0, 1));

const WasmCreditLedgerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmcreditledger_free(ptr >>> 0, 1));

const WasmIdleDetectorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmidledetector_free(ptr >>> 0, 1));

const WasmMcpBroadcastFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmcpbroadcast_free(ptr >>> 0, 1));

const WasmMcpServerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmcpserver_free(ptr >>> 0, 1));

const WasmMcpTransportFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmcptransport_free(ptr >>> 0, 1));

const WasmMcpWorkerHandlerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmmcpworkerhandler_free(ptr >>> 0, 1));

const WasmNetworkManagerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmnetworkmanager_free(ptr >>> 0, 1));

const WasmNodeIdentityFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmnodeidentity_free(ptr >>> 0, 1));

const WasmStigmergyFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmstigmergy_free(ptr >>> 0, 1));

const WasmTaskExecutorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmtaskexecutor_free(ptr >>> 0, 1));

const WasmTaskQueueFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmtaskqueue_free(ptr >>> 0, 1));

const WasmWorkSchedulerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmworkscheduler_free(ptr >>> 0, 1));

const WitnessTrackerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_witnesstracker_free(ptr >>> 0, 1));

/**
 * Self-learning security system with Q-learning adaptive optimization
 */
class AdaptiveSecurity {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        AdaptiveSecurityFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_adaptivesecurity_free(ptr, 0);
    }
    /**
     * Choose action using epsilon-greedy policy
     * @param {string} state
     * @param {string} available_actions
     * @returns {string}
     */
    chooseAction(state, available_actions) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(state, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(available_actions, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            const ret = wasm.adaptivesecurity_chooseAction(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Detect if request matches known attack pattern
     * @param {Float32Array} features
     * @returns {number}
     */
    detectAttack(features) {
        const ptr0 = passArrayF32ToWasm0(features, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.adaptivesecurity_detectAttack(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Export learned patterns for persistence
     * @returns {Uint8Array}
     */
    exportPatterns() {
        const ret = wasm.adaptivesecurity_exportPatterns(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Import learned patterns
     * @param {Uint8Array} data
     */
    importPatterns(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.adaptivesecurity_importPatterns(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {number}
     */
    getMinReputation() {
        const ret = wasm.adaptivesecurity_getMinReputation(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    getRateLimitMax() {
        const ret = wasm.adaptivesecurity_getRateLimitMax(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    getSecurityLevel() {
        const ret = wasm.adaptivesecurity_getSecurityLevel(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get current adaptive thresholds
     * @returns {bigint}
     */
    getRateLimitWindow() {
        const ret = wasm.adaptivesecurity_getRateLimitWindow(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Record attack pattern for learning
     * @param {string} pattern_type
     * @param {Float32Array} features
     * @param {number} severity
     */
    recordAttackPattern(pattern_type, features, severity) {
        const ptr0 = passStringToWasm0(pattern_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(features, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.adaptivesecurity_recordAttackPattern(this.__wbg_ptr, ptr0, len0, ptr1, len1, severity);
    }
    /**
     * Update network health metrics
     * @param {number} active_nodes
     * @param {number} suspicious_nodes
     * @param {number} attacks_hour
     * @param {number} false_positives
     * @param {number} avg_response_ms
     */
    updateNetworkHealth(active_nodes, suspicious_nodes, attacks_hour, false_positives, avg_response_ms) {
        wasm.adaptivesecurity_updateNetworkHealth(this.__wbg_ptr, active_nodes, suspicious_nodes, attacks_hour, false_positives, avg_response_ms);
    }
    /**
     * @returns {number}
     */
    getSpotCheckProbability() {
        const ret = wasm.adaptivesecurity_getSpotCheckProbability(this.__wbg_ptr);
        return ret;
    }
    constructor() {
        const ret = wasm.adaptivesecurity_new();
        this.__wbg_ptr = ret >>> 0;
        AdaptiveSecurityFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Learn from security event outcome (batched for better performance)
     * @param {string} state
     * @param {string} action
     * @param {number} reward
     * @param {string} next_state
     */
    learn(state, action, reward, next_state) {
        const ptr0 = passStringToWasm0(state, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(action, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(next_state, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.adaptivesecurity_learn(this.__wbg_ptr, ptr0, len0, ptr1, len1, reward, ptr2, len2);
    }
    /**
     * Get learning statistics
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adaptivesecurity_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) AdaptiveSecurity.prototype[Symbol.dispose] = AdaptiveSecurity.prototype.free;
exports.AdaptiveSecurity = AdaptiveSecurity;

/**
 * Adversarial testing framework
 */
class AdversarialSimulator {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        AdversarialSimulatorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_adversarialsimulator_free(ptr, 0);
    }
    /**
     * Simulate DDoS attack
     * @param {number} requests_per_second
     * @param {bigint} duration_ms
     * @returns {string}
     */
    simulateDDoS(requests_per_second, duration_ms) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateDDoS(this.__wbg_ptr, requests_per_second, duration_ms);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Simulate Sybil attack
     * @param {number} fake_nodes
     * @param {boolean} same_fingerprint
     * @returns {string}
     */
    simulateSybil(fake_nodes, same_fingerprint) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateSybil(this.__wbg_ptr, fake_nodes, same_fingerprint);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Enable chaos mode for continuous testing
     * @param {boolean} enabled
     */
    enableChaosMode(enabled) {
        wasm.adversarialsimulator_enableChaosMode(this.__wbg_ptr, enabled);
    }
    /**
     * Run comprehensive security audit
     * @returns {string}
     */
    runSecurityAudit() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_runSecurityAudit(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Simulate Byzantine node behavior
     * @param {number} byzantine_nodes
     * @param {number} total_nodes
     * @returns {string}
     */
    simulateByzantine(byzantine_nodes, total_nodes) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateByzantine(this.__wbg_ptr, byzantine_nodes, total_nodes);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get defence metrics
     * @returns {string}
     */
    getDefenceMetrics() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_getDefenceMetrics(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get recommendations based on testing
     * @returns {string}
     */
    getRecommendations() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_getRecommendations(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Generate chaos event
     * @returns {string | undefined}
     */
    generateChaosEvent() {
        const ret = wasm.adversarialsimulator_generateChaosEvent(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Simulate free-riding attack
     * @param {number} consumption_rate
     * @param {number} contribution_rate
     * @returns {string}
     */
    simulateFreeRiding(consumption_rate, contribution_rate) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateFreeRiding(this.__wbg_ptr, consumption_rate, contribution_rate);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Simulate double-spend attempt
     * @param {bigint} amount
     * @param {number} concurrent_targets
     * @returns {string}
     */
    simulateDoubleSpend(amount, concurrent_targets) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateDoubleSpend(this.__wbg_ptr, amount, concurrent_targets);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Simulate result tampering
     * @param {number} tamper_percentage
     * @returns {string}
     */
    simulateResultTampering(tamper_percentage) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.adversarialsimulator_simulateResultTampering(this.__wbg_ptr, tamper_percentage);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    constructor() {
        const ret = wasm.adversarialsimulator_new();
        this.__wbg_ptr = ret >>> 0;
        AdversarialSimulatorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) AdversarialSimulator.prototype[Symbol.dispose] = AdversarialSimulator.prototype.free;
exports.AdversarialSimulator = AdversarialSimulator;

/**
 * Audit logger for security events
 */
class AuditLog {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        AuditLogFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_auditlog_free(ptr, 0);
    }
    /**
     * Export events as JSON
     * @returns {string}
     */
    exportEvents() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.auditlog_exportEvents(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get events for a node
     * @param {string} node_id
     * @returns {number}
     */
    getEventsForNode(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.auditlog_getEventsForNode(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Get events by severity
     * @param {number} min_severity
     * @returns {number}
     */
    getEventsBySeverity(min_severity) {
        const ret = wasm.auditlog_getEventsBySeverity(this.__wbg_ptr, min_severity);
        return ret >>> 0;
    }
    /**
     * Log an event
     * @param {string} event_type
     * @param {string} node_id
     * @param {string} details
     * @param {number} severity
     */
    log(event_type, node_id, details, severity) {
        const ptr0 = passStringToWasm0(event_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(details, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.auditlog_log(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, severity);
    }
    constructor() {
        const ret = wasm.auditlog_new();
        this.__wbg_ptr = ret >>> 0;
        AuditLogFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) AuditLog.prototype[Symbol.dispose] = AuditLog.prototype.free;
exports.AuditLog = AuditLog;

/**
 * Browser fingerprint generator for anti-sybil protection
 */
class BrowserFingerprint {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        BrowserFingerprintFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_browserfingerprint_free(ptr, 0);
    }
    /**
     * Generate anonymous uniqueness score
     * This doesn't track users, just ensures one node per browser
     * @returns {Promise<string>}
     */
    static generate() {
        const ret = wasm.browserfingerprint_generate();
        return ret;
    }
}
if (Symbol.dispose) BrowserFingerprint.prototype[Symbol.dispose] = BrowserFingerprint.prototype.free;
exports.BrowserFingerprint = BrowserFingerprint;

/**
 * Byzantine gradient detection using statistical methods
 */
class ByzantineDetector {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ByzantineDetectorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_byzantinedetector_free(ptr, 0);
    }
    /**
     * Get maximum allowed magnitude
     * @returns {number}
     */
    getMaxMagnitude() {
        const ret = wasm.byzantinedetector_getMaxMagnitude(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create a new Byzantine detector
     * @param {number} max_magnitude
     * @param {number} zscore_threshold
     */
    constructor(max_magnitude, zscore_threshold) {
        const ret = wasm.byzantinedetector_new(max_magnitude, zscore_threshold);
        this.__wbg_ptr = ret >>> 0;
        ByzantineDetectorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) ByzantineDetector.prototype[Symbol.dispose] = ByzantineDetector.prototype.free;
exports.ByzantineDetector = ByzantineDetector;

/**
 * The main coherence engine running the RAC protocol
 */
class CoherenceEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        CoherenceEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_coherenceengine_free(ptr, 0);
    }
    /**
     * Get event log length
     * @returns {number}
     */
    eventCount() {
        const ret = wasm.coherenceengine_eventCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if context has drifted
     * @param {string} context_hex
     * @returns {boolean}
     */
    hasDrifted(context_hex) {
        const ptr0 = passStringToWasm0(context_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_hasDrifted(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Check if a claim can be used in decisions
     * @param {string} claim_id
     * @returns {boolean}
     */
    canUseClaim(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_canUseClaim(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get witness count for a claim
     * @param {string} claim_id
     * @returns {number}
     */
    witnessCount(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_witnessCount(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Get conflict count
     * @returns {number}
     */
    conflictCount() {
        const ret = wasm.coherenceengine_conflictCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current Merkle root
     * @returns {string}
     */
    getMerkleRoot() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.coherenceengine_getMerkleRoot(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get quarantined claim count
     * @returns {number}
     */
    quarantinedCount() {
        const ret = wasm.coherenceengine_quarantinedCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check quarantine level for a claim
     * @param {string} claim_id
     * @returns {number}
     */
    getQuarantineLevel(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_getQuarantineLevel(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Check if claim has sufficient witnesses
     * @param {string} claim_id
     * @returns {boolean}
     */
    hasSufficientWitnesses(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_hasSufficientWitnesses(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new coherence engine
     */
    constructor() {
        const ret = wasm.coherenceengine_new();
        this.__wbg_ptr = ret >>> 0;
        CoherenceEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get drift for a context
     * @param {string} context_hex
     * @returns {number}
     */
    getDrift(context_hex) {
        const ptr0 = passStringToWasm0(context_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.coherenceengine_getDrift(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.coherenceengine_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) CoherenceEngine.prototype[Symbol.dispose] = CoherenceEngine.prototype.free;
exports.CoherenceEngine = CoherenceEngine;

/**
 * Collective memory system for distributed pattern learning
 */
class CollectiveMemory {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        CollectiveMemoryFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_collectivememory_free(ptr, 0);
    }
    /**
     * Get queue size
     * @returns {number}
     */
    queueSize() {
        const ret = wasm.collectivememory_queueSize(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Run consolidation (call during idle periods)
     * @returns {number}
     */
    consolidate() {
        const ret = wasm.collectivememory_consolidate(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if a pattern ID exists
     * @param {string} pattern_id
     * @returns {boolean}
     */
    hasPattern(pattern_id) {
        const ptr0 = passStringToWasm0(pattern_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.collectivememory_hasPattern(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get pattern count in shared index
     * @returns {number}
     */
    patternCount() {
        const ret = wasm.collectivememory_patternCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create new collective memory with default config
     * @param {string} node_id
     */
    constructor(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.collectivememory_new(ptr0, len0);
        this.__wbg_ptr = ret >>> 0;
        CollectiveMemoryFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Search for similar patterns
     * @param {string} query_json
     * @param {number} k
     * @returns {string}
     */
    search(query_json, k) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.collectivememory_search(this.__wbg_ptr, ptr0, len0, k);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.collectivememory_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) CollectiveMemory.prototype[Symbol.dispose] = CollectiveMemory.prototype.free;
exports.CollectiveMemory = CollectiveMemory;

/**
 * Contribution stream for sustained development
 */
class ContributionStream {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ContributionStreamFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_contributionstream_free(ptr, 0);
    }
    /**
     * Check if streams are healthy
     * @returns {boolean}
     */
    isHealthy() {
        const ret = wasm.contributionstream_isHealthy(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Process network fee distribution
     * @param {bigint} total_fees
     * @param {bigint} epoch
     * @returns {bigint}
     */
    processFees(total_fees, epoch) {
        const ret = wasm.contributionstream_processFees(this.__wbg_ptr, total_fees, epoch);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get total distributed
     * @returns {bigint}
     */
    getTotalDistributed() {
        const ret = wasm.contributionstream_getTotalDistributed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    constructor() {
        const ret = wasm.contributionstream_new();
        this.__wbg_ptr = ret >>> 0;
        ContributionStreamFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) ContributionStream.prototype[Symbol.dispose] = ContributionStream.prototype.free;
exports.ContributionStream = ContributionStream;

/**
 * Differential privacy noise generator
 */
class DifferentialPrivacy {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        DifferentialPrivacyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_differentialprivacy_free(ptr, 0);
    }
    /**
     * Check if DP is enabled
     * @returns {boolean}
     */
    isEnabled() {
        const ret = wasm.differentialprivacy_isEnabled(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get epsilon value
     * @returns {number}
     */
    getEpsilon() {
        const ret = wasm.differentialprivacy_getEpsilon(this.__wbg_ptr);
        return ret;
    }
    /**
     * Enable/disable differential privacy
     * @param {boolean} enabled
     */
    setEnabled(enabled) {
        wasm.differentialprivacy_setEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * Create a new differential privacy module
     * @param {number} epsilon
     * @param {number} sensitivity
     */
    constructor(epsilon, sensitivity) {
        const ret = wasm.differentialprivacy_new(epsilon, sensitivity);
        this.__wbg_ptr = ret >>> 0;
        DifferentialPrivacyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) DifferentialPrivacy.prototype[Symbol.dispose] = DifferentialPrivacy.prototype.free;
exports.DifferentialPrivacy = DifferentialPrivacy;

/**
 * Manages semantic drift tracking
 */
class DriftTracker {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        DriftTrackerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_drifttracker_free(ptr, 0);
    }
    /**
     * Check if context has drifted beyond threshold
     * @param {string} context_hex
     * @returns {boolean}
     */
    hasDrifted(context_hex) {
        const ptr0 = passStringToWasm0(context_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.drifttracker_hasDrifted(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get contexts with significant drift
     * @returns {string}
     */
    getDriftedContexts() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.drifttracker_getDriftedContexts(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Create a new drift tracker
     * @param {number} drift_threshold
     */
    constructor(drift_threshold) {
        const ret = wasm.drifttracker_new(drift_threshold);
        this.__wbg_ptr = ret >>> 0;
        DriftTrackerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get drift for a context
     * @param {string} context_hex
     * @returns {number}
     */
    getDrift(context_hex) {
        const ptr0 = passStringToWasm0(context_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.drifttracker_getDrift(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
}
if (Symbol.dispose) DriftTracker.prototype[Symbol.dispose] = DriftTracker.prototype.free;
exports.DriftTracker = DriftTracker;

/**
 * Economic distribution system for sustainable operations
 */
class EconomicEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EconomicEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_economicengine_free(ptr, 0);
    }
    /**
     * Get economic health status
     * @returns {EconomicHealth}
     */
    getHealth() {
        const ret = wasm.economicengine_getHealth(this.__wbg_ptr);
        return EconomicHealth.__wrap(ret);
    }
    /**
     * Get treasury balance
     * @returns {bigint}
     */
    getTreasury() {
        const ret = wasm.economicengine_getTreasury(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Advance to next epoch
     */
    advanceEpoch() {
        wasm.economicengine_advanceEpoch(this.__wbg_ptr);
    }
    /**
     * Process task completion and distribute rewards
     * @param {bigint} base_amount
     * @param {number} multiplier
     * @returns {RewardDistribution}
     */
    processReward(base_amount, multiplier) {
        const ret = wasm.economicengine_processReward(this.__wbg_ptr, base_amount, multiplier);
        return RewardDistribution.__wrap(ret);
    }
    /**
     * Get protocol fund balance (for development sustainability)
     * @returns {bigint}
     */
    getProtocolFund() {
        const ret = wasm.economicengine_getProtocolFund(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Check if network can sustain itself
     * @param {number} active_nodes
     * @param {bigint} daily_tasks
     * @returns {boolean}
     */
    isSelfSustaining(active_nodes, daily_tasks) {
        const ret = wasm.economicengine_isSelfSustaining(this.__wbg_ptr, active_nodes, daily_tasks);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.economicengine_new();
        this.__wbg_ptr = ret >>> 0;
        EconomicEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) EconomicEngine.prototype[Symbol.dispose] = EconomicEngine.prototype.free;
exports.EconomicEngine = EconomicEngine;

class EconomicHealth {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(EconomicHealth.prototype);
        obj.__wbg_ptr = ptr;
        EconomicHealthFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EconomicHealthFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_economichealth_free(ptr, 0);
    }
    /**
     * Velocity of rUv (transactions per period)
     * @returns {number}
     */
    get velocity() {
        const ret = wasm.__wbg_get_economichealth_velocity(this.__wbg_ptr);
        return ret;
    }
    /**
     * Velocity of rUv (transactions per period)
     * @param {number} arg0
     */
    set velocity(arg0) {
        wasm.__wbg_set_economichealth_velocity(this.__wbg_ptr, arg0);
    }
    /**
     * Network utilization rate
     * @returns {number}
     */
    get utilization() {
        const ret = wasm.__wbg_get_economichealth_utilization(this.__wbg_ptr);
        return ret;
    }
    /**
     * Network utilization rate
     * @param {number} arg0
     */
    set utilization(arg0) {
        wasm.__wbg_set_economichealth_utilization(this.__wbg_ptr, arg0);
    }
    /**
     * Supply growth rate
     * @returns {number}
     */
    get growth_rate() {
        const ret = wasm.__wbg_get_economichealth_growth_rate(this.__wbg_ptr);
        return ret;
    }
    /**
     * Supply growth rate
     * @param {number} arg0
     */
    set growth_rate(arg0) {
        wasm.__wbg_set_economichealth_growth_rate(this.__wbg_ptr, arg0);
    }
    /**
     * Stability index (0-1)
     * @returns {number}
     */
    get stability() {
        const ret = wasm.__wbg_get_economichealth_stability(this.__wbg_ptr);
        return ret;
    }
    /**
     * Stability index (0-1)
     * @param {number} arg0
     */
    set stability(arg0) {
        wasm.__wbg_set_economichealth_stability(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) EconomicHealth.prototype[Symbol.dispose] = EconomicHealth.prototype.free;
exports.EconomicHealth = EconomicHealth;

/**
 * Configuration builder for EdgeNet
 */
class EdgeNetConfig {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(EdgeNetConfig.prototype);
        obj.__wbg_ptr = ptr;
        EdgeNetConfigFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EdgeNetConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_edgenetconfig_free(ptr, 0);
    }
    /**
     * @param {number} bytes
     * @returns {EdgeNetConfig}
     */
    memoryLimit(bytes) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.edgenetconfig_memoryLimit(ptr, bytes);
        return EdgeNetConfig.__wrap(ret);
    }
    /**
     * @param {number} ms
     * @returns {EdgeNetConfig}
     */
    minIdleTime(ms) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.edgenetconfig_minIdleTime(ptr, ms);
        return EdgeNetConfig.__wrap(ret);
    }
    /**
     * @param {boolean} respect
     * @returns {EdgeNetConfig}
     */
    respectBattery(respect) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.edgenetconfig_respectBattery(ptr, respect);
        return EdgeNetConfig.__wrap(ret);
    }
    /**
     * @param {string} site_id
     */
    constructor(site_id) {
        const ptr0 = passStringToWasm0(site_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetconfig_new(ptr0, len0);
        this.__wbg_ptr = ret >>> 0;
        EdgeNetConfigFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {EdgeNetNode}
     */
    build() {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.edgenetconfig_build(ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return EdgeNetNode.__wrap(ret[0]);
    }
    /**
     * @param {string} url
     * @returns {EdgeNetConfig}
     */
    addRelay(url) {
        const ptr = this.__destroy_into_raw();
        const ptr0 = passStringToWasm0(url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetconfig_addRelay(ptr, ptr0, len0);
        return EdgeNetConfig.__wrap(ret);
    }
    /**
     * @param {number} limit
     * @returns {EdgeNetConfig}
     */
    cpuLimit(limit) {
        const ptr = this.__destroy_into_raw();
        const ret = wasm.edgenetconfig_cpuLimit(ptr, limit);
        return EdgeNetConfig.__wrap(ret);
    }
}
if (Symbol.dispose) EdgeNetConfig.prototype[Symbol.dispose] = EdgeNetConfig.prototype.free;
exports.EdgeNetConfig = EdgeNetConfig;

/**
 * Main EdgeNet node - the entry point for participating in the network
 */
class EdgeNetNode {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(EdgeNetNode.prototype);
        obj.__wbg_ptr = ptr;
        EdgeNetNodeFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EdgeNetNodeFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_edgenetnode_free(ptr, 0);
    }
    /**
     * Disconnect from the network
     */
    disconnect() {
        const ret = wasm.edgenetnode_disconnect(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Enable HDC for hyperdimensional computing
     * @returns {boolean}
     */
    enableHDC() {
        const ret = wasm.edgenetnode_enableHDC(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Enable Neural Autonomous Organization for governance
     * @param {number} quorum
     * @returns {boolean}
     */
    enableNAO(quorum) {
        const ret = wasm.edgenetnode_enableNAO(this.__wbg_ptr, quorum);
        return ret !== 0;
    }
    /**
     * Enable WTA for instant decisions
     * @param {number} num_neurons
     * @returns {boolean}
     */
    enableWTA(num_neurons) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, num_neurons);
        return ret !== 0;
    }
    /**
     * Enable BTSP for one-shot learning
     * @param {number} input_dim
     * @returns {boolean}
     */
    enableBTSP(input_dim) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, input_dim);
        return ret !== 0;
    }
    /**
     * Propose an action in the NAO
     * @param {string} action
     * @returns {string}
     */
    proposeNAO(action) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(action, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.edgenetnode_proposeNAO(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Alias for creditBalance - returns rUv balance
     * @returns {bigint}
     */
    ruvBalance() {
        const ret = wasm.edgenetnode_creditBalance(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Submit a task to the network
     * @param {string} task_type
     * @param {Uint8Array} payload
     * @param {bigint} max_credits
     * @returns {Promise<any>}
     */
    submitTask(task_type, payload, max_credits) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(payload, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_submitTask(this.__wbg_ptr, ptr0, len0, ptr1, len1, max_credits);
        return ret;
    }
    /**
     * Check for active celebration events
     * @returns {string}
     */
    checkEvents() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_checkEvents(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get current throttle level (0.0 - 1.0)
     * @returns {number}
     */
    getThrottle() {
        const ret = wasm.edgenetnode_getThrottle(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get treasury balance for operations
     * @returns {bigint}
     */
    getTreasury() {
        const ret = wasm.edgenetnode_getTreasury(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Check if a claim can be used (not quarantined)
     * @param {string} claim_id
     * @returns {boolean}
     */
    canUseClaim(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_canUseClaim(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Process epoch for economic distribution
     */
    processEpoch() {
        wasm.edgenetnode_processEpoch(this.__wbg_ptr);
    }
    /**
     * Store a learned pattern in the reasoning bank
     * @param {string} pattern_json
     * @returns {number}
     */
    storePattern(pattern_json) {
        const ptr0 = passStringToWasm0(pattern_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_storePattern(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get current rUv (Resource Utility Voucher) balance
     * @returns {bigint}
     */
    creditBalance() {
        const ret = wasm.edgenetnode_creditBalance(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get motivational message (subtle Easter egg)
     * @returns {string}
     */
    getMotivation() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getMotivation(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get current contribution multiplier based on network size
     * @returns {number}
     */
    getMultiplier() {
        const ret = wasm.edgenetnode_getMultiplier(this.__wbg_ptr);
        return ret;
    }
    /**
     * Prune low-quality learned patterns
     * @param {number} min_usage
     * @param {number} min_confidence
     * @returns {number}
     */
    prunePatterns(min_usage, min_confidence) {
        const ret = wasm.edgenetnode_prunePatterns(this.__wbg_ptr, min_usage, min_confidence);
        return ret >>> 0;
    }
    /**
     * Get current Merkle root for audit (Axiom 11: Equivocation detectable)
     * @returns {string}
     */
    getMerkleRoot() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getMerkleRoot(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Lookup similar patterns for task optimization
     * @param {string} query_json
     * @param {number} k
     * @returns {string}
     */
    lookupPatterns(query_json, k) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.edgenetnode_lookupPatterns(this.__wbg_ptr, ptr0, len0, k);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get all available exotic capabilities and their status
     * @returns {any}
     */
    getCapabilities() {
        const ret = wasm.edgenetnode_getCapabilities(this.__wbg_ptr);
        return ret;
    }
    /**
     * Check if this node should replicate (high performer)
     * @returns {boolean}
     */
    shouldReplicate() {
        const ret = wasm.edgenetnode_shouldReplicate(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Enable MicroLoRA for self-learning
     * @param {number} rank
     * @returns {boolean}
     */
    enableMicroLoRA(rank) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, rank);
        return ret !== 0;
    }
    /**
     * Get founding contributor count
     * @returns {number}
     */
    getFounderCount() {
        const ret = wasm.edgenetnode_getFounderCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get optimal peers for task routing
     * @param {number} count
     * @returns {string[]}
     */
    getOptimalPeers(count) {
        const ret = wasm.edgenetnode_getOptimalPeers(this.__wbg_ptr, count);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get stored pattern count
     * @returns {number}
     */
    getPatternCount() {
        const ret = wasm.edgenetnode_getPatternCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get protocol development fund balance
     * @returns {bigint}
     */
    getProtocolFund() {
        const ret = wasm.edgenetnode_getProtocolFund(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get themed network status
     * @param {number} node_count
     * @returns {string}
     */
    getThemedStatus(node_count) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getThemedStatus(this.__wbg_ptr, node_count);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get contribution stream health
     * @returns {boolean}
     */
    isStreamHealthy() {
        const ret = wasm.edgenetnode_isStreamHealthy(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Process the next available task (called by worker)
     * @returns {Promise<boolean>}
     */
    processNextTask() {
        const ret = wasm.edgenetnode_processNextTask(this.__wbg_ptr);
        return ret;
    }
    /**
     * Step all exotic capabilities forward
     * @param {number} dt
     */
    stepCapabilities(dt) {
        wasm.edgenetnode_stepCapabilities(this.__wbg_ptr, dt);
    }
    /**
     * Get active conflict count (Axiom 6: Disagreement is signal)
     * @returns {number}
     */
    getConflictCount() {
        const ret = wasm.edgenetnode_getConflictCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get learning statistics
     * @returns {string}
     */
    getLearningStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getLearningStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check if network is self-sustaining
     * @param {number} active_nodes
     * @param {bigint} daily_tasks
     * @returns {boolean}
     */
    isSelfSustaining(active_nodes, daily_tasks) {
        const ret = wasm.edgenetnode_isSelfSustaining(this.__wbg_ptr, active_nodes, daily_tasks);
        return ret !== 0;
    }
    /**
     * Record node performance for evolution
     * @param {number} success_rate
     * @param {number} throughput
     */
    recordPerformance(success_rate, throughput) {
        wasm.edgenetnode_recordPerformance(this.__wbg_ptr, success_rate, throughput);
    }
    /**
     * Run security audit (adversarial testing)
     * @returns {string}
     */
    runSecurityAudit() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_runSecurityAudit(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Enable Time Crystal for P2P synchronization
     * @param {number} oscillators
     * @returns {boolean}
     */
    enableTimeCrystal(oscillators) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, oscillators);
        return ret !== 0;
    }
    /**
     * Get coherence statistics
     * @returns {string}
     */
    getCoherenceStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getCoherenceStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get economic health metrics
     * @returns {string}
     */
    getEconomicHealth() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getEconomicHealth(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get network fitness score (0-1)
     * @returns {number}
     */
    getNetworkFitness() {
        const ret = wasm.edgenetnode_getNetworkFitness(this.__wbg_ptr);
        return ret;
    }
    /**
     * Record task routing outcome for optimization
     * @param {string} task_type
     * @param {string} node_id
     * @param {bigint} latency_ms
     * @param {boolean} success
     */
    recordTaskRouting(task_type, node_id, latency_ms, success) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.edgenetnode_recordTaskRouting(this.__wbg_ptr, ptr0, len0, ptr1, len1, latency_ms, success);
    }
    /**
     * Enable Morphogenetic Network for emergent topology
     * @param {number} size
     * @returns {boolean}
     */
    enableMorphogenetic(size) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, size);
        return ret !== 0;
    }
    /**
     * Get trajectory count for learning analysis
     * @returns {number}
     */
    getTrajectoryCount() {
        const ret = wasm.edgenetnode_getTrajectoryCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get energy efficiency ratio from spike-driven attention
     * @param {number} seq_len
     * @param {number} hidden_dim
     * @returns {number}
     */
    getEnergyEfficiency(seq_len, hidden_dim) {
        const ret = wasm.edgenetnode_getEnergyEfficiency(this.__wbg_ptr, seq_len, hidden_dim);
        return ret;
    }
    /**
     * Get quarantined claim count (Axiom 9: Quarantine is mandatory)
     * @returns {number}
     */
    getQuarantinedCount() {
        const ret = wasm.edgenetnode_getQuarantinedCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get Time Crystal synchronization level (0.0 - 1.0)
     * @returns {number}
     */
    getTimeCrystalSync() {
        const ret = wasm.edgenetnode_getTimeCrystalSync(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get optimization statistics
     * @returns {string}
     */
    getOptimizationStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getOptimizationStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get recommended configuration for new nodes
     * @returns {string}
     */
    getRecommendedConfig() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_getRecommendedConfig(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Enable Global Workspace for attention
     * @param {number} capacity
     * @returns {boolean}
     */
    enableGlobalWorkspace(capacity) {
        const ret = wasm.edgenetnode_enableBTSP(this.__wbg_ptr, capacity);
        return ret !== 0;
    }
    /**
     * Record peer interaction for topology optimization
     * @param {string} peer_id
     * @param {number} success_rate
     */
    recordPeerInteraction(peer_id, success_rate) {
        const ptr0 = passStringToWasm0(peer_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.edgenetnode_recordPeerInteraction(this.__wbg_ptr, ptr0, len0, success_rate);
    }
    /**
     * Get capabilities summary as JSON
     * @returns {any}
     */
    getCapabilitiesSummary() {
        const ret = wasm.edgenetnode_getCapabilitiesSummary(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get coherence engine event count
     * @returns {number}
     */
    getCoherenceEventCount() {
        const ret = wasm.edgenetnode_getCoherenceEventCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get quarantine level for a claim
     * @param {string} claim_id
     * @returns {number}
     */
    getClaimQuarantineLevel(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_getClaimQuarantineLevel(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Record a task execution trajectory for learning
     * @param {string} trajectory_json
     * @returns {boolean}
     */
    recordLearningTrajectory(trajectory_json) {
        const ptr0 = passStringToWasm0(trajectory_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_recordLearningTrajectory(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new EdgeNet node
     * @param {string} site_id
     * @param {NodeConfig | null} [config]
     */
    constructor(site_id, config) {
        const ptr0 = passStringToWasm0(site_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        let ptr1 = 0;
        if (!isLikeNone(config)) {
            _assertClass(config, NodeConfig);
            ptr1 = config.__destroy_into_raw();
        }
        const ret = wasm.edgenetnode_new(ptr0, len0, ptr1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        EdgeNetNodeFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Pause contribution
     */
    pause() {
        wasm.edgenetnode_pause(this.__wbg_ptr);
    }
    /**
     * Start contributing to the network
     */
    start() {
        const ret = wasm.edgenetnode_start(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Resume contribution
     */
    resume() {
        wasm.edgenetnode_resume(this.__wbg_ptr);
    }
    /**
     * Check if user is currently idle
     * @returns {boolean}
     */
    isIdle() {
        const ret = wasm.edgenetnode_isIdle(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get the node's unique identifier
     * @returns {string}
     */
    nodeId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.edgenetnode_nodeId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Vote on a NAO proposal
     * @param {string} proposal_id
     * @param {number} weight
     * @returns {boolean}
     */
    voteNAO(proposal_id, weight) {
        const ptr0 = passStringToWasm0(proposal_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.edgenetnode_voteNAO(this.__wbg_ptr, ptr0, len0, weight);
        return ret !== 0;
    }
    /**
     * Get node statistics
     * @returns {NodeStats}
     */
    getStats() {
        const ret = wasm.edgenetnode_getStats(this.__wbg_ptr);
        return NodeStats.__wrap(ret);
    }
}
if (Symbol.dispose) EdgeNetNode.prototype[Symbol.dispose] = EdgeNetNode.prototype.free;
exports.EdgeNetNode = EdgeNetNode;

/**
 * Entropy-based consensus engine for swarm decisions
 */
class EntropyConsensus {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(EntropyConsensus.prototype);
        obj.__wbg_ptr = ptr;
        EntropyConsensusFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EntropyConsensusFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_entropyconsensus_free(ptr, 0);
    }
    /**
     * Get belief probability for a decision
     * @param {bigint} decision_id
     * @returns {number}
     */
    getBelief(decision_id) {
        const ret = wasm.entropyconsensus_getBelief(this.__wbg_ptr, decision_id);
        return ret;
    }
    /**
     * Get number of negotiation rounds completed
     * @returns {number}
     */
    getRounds() {
        const ret = wasm.entropyconsensus_getRounds(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set initial belief for a decision
     * @param {bigint} decision_id
     * @param {number} probability
     */
    setBelief(decision_id, probability) {
        wasm.entropyconsensus_setBelief(this.__wbg_ptr, decision_id, probability);
    }
    /**
     * Get the winning decision (if converged)
     * @returns {bigint | undefined}
     */
    getDecision() {
        const ret = wasm.entropyconsensus_getDecision(this.__wbg_ptr);
        return ret[0] === 0 ? undefined : BigInt.asUintN(64, ret[1]);
    }
    /**
     * Get number of decision options
     * @returns {number}
     */
    optionCount() {
        const ret = wasm.entropyconsensus_optionCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if negotiation has timed out
     * @returns {boolean}
     */
    hasTimedOut() {
        const ret = wasm.entropyconsensus_hasTimedOut(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Set belief without normalizing (for batch updates)
     * Call normalize_beliefs() after all set_belief_raw calls
     * @param {bigint} decision_id
     * @param {number} probability
     */
    set_belief_raw(decision_id, probability) {
        wasm.entropyconsensus_set_belief_raw(this.__wbg_ptr, decision_id, probability);
    }
    /**
     * Create with custom entropy threshold
     * @param {number} threshold
     * @returns {EntropyConsensus}
     */
    static withThreshold(threshold) {
        const ret = wasm.entropyconsensus_withThreshold(threshold);
        return EntropyConsensus.__wrap(ret);
    }
    /**
     * Get current temperature (for annealing)
     * @returns {number}
     */
    getTemperature() {
        const ret = wasm.entropyconsensus_getTemperature(this.__wbg_ptr);
        return ret;
    }
    /**
     * Manually trigger normalization (for use after set_belief_raw)
     */
    finalize_beliefs() {
        wasm.entropyconsensus_finalize_beliefs(this.__wbg_ptr);
    }
    /**
     * Get entropy history as JSON
     * @returns {string}
     */
    getEntropyHistory() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.entropyconsensus_getEntropyHistory(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the entropy threshold for convergence
     * @returns {number}
     */
    getEntropyThreshold() {
        const ret = wasm.entropyconsensus_getEntropyThreshold(this.__wbg_ptr);
        return ret;
    }
    /**
     * Create new entropy consensus with default configuration
     */
    constructor() {
        const ret = wasm.entropyconsensus_new();
        this.__wbg_ptr = ret >>> 0;
        EntropyConsensusFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset consensus state for new decision
     */
    reset() {
        wasm.entropyconsensus_reset(this.__wbg_ptr);
    }
    /**
     * Get current entropy of belief distribution
     * @returns {number}
     */
    entropy() {
        const ret = wasm.entropyconsensus_entropy(this.__wbg_ptr);
        return ret;
    }
    /**
     * Check if consensus has been reached
     * @returns {boolean}
     */
    converged() {
        const ret = wasm.entropyconsensus_converged(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get consensus statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.entropyconsensus_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) EntropyConsensus.prototype[Symbol.dispose] = EntropyConsensus.prototype.free;
exports.EntropyConsensus = EntropyConsensus;

/**
 * Append-only Merkle log for audit (FIXED: proper event storage)
 */
class EventLog {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EventLogFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_eventlog_free(ptr, 0);
    }
    /**
     * Get total event count
     * @returns {number}
     */
    totalEvents() {
        const ret = wasm.eventlog_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current event count (includes all events)
     * @returns {number}
     */
    len() {
        const ret = wasm.eventlog_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new event log
     */
    constructor() {
        const ret = wasm.eventlog_new();
        this.__wbg_ptr = ret >>> 0;
        EventLogFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get current Merkle root as hex string
     * @returns {string}
     */
    getRoot() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.eventlog_getRoot(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check if log is empty
     * @returns {boolean}
     */
    isEmpty() {
        const ret = wasm.eventlog_isEmpty(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) EventLog.prototype[Symbol.dispose] = EventLog.prototype.free;
exports.EventLog = EventLog;

/**
 * Node replication and evolution guidance
 */
class EvolutionEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EvolutionEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_evolutionengine_free(ptr, 0);
    }
    /**
     * Check if node should replicate (spawn similar node)
     * @param {string} node_id
     * @returns {boolean}
     */
    shouldReplicate(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.evolutionengine_shouldReplicate(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Record node performance for fitness evaluation
     * @param {string} node_id
     * @param {number} success_rate
     * @param {number} throughput
     */
    recordPerformance(node_id, success_rate, throughput) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.evolutionengine_recordPerformance(this.__wbg_ptr, ptr0, len0, success_rate, throughput);
    }
    /**
     * Get network fitness score
     * @returns {number}
     */
    getNetworkFitness() {
        const ret = wasm.evolutionengine_getNetworkFitness(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get recommended configuration for new nodes
     * @returns {string}
     */
    getRecommendedConfig() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.evolutionengine_getRecommendedConfig(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    constructor() {
        const ret = wasm.evolutionengine_new();
        this.__wbg_ptr = ret >>> 0;
        EvolutionEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Evolve patterns for next generation
     */
    evolve() {
        wasm.evolutionengine_evolve(this.__wbg_ptr);
    }
}
if (Symbol.dispose) EvolutionEngine.prototype[Symbol.dispose] = EvolutionEngine.prototype.free;
exports.EvolutionEngine = EvolutionEngine;

/**
 * Federated model state for tracking learning progress
 */
class FederatedModel {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        FederatedModelFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_federatedmodel_free(ptr, 0);
    }
    /**
     * Get parameter dimension
     * @returns {number}
     */
    getDimension() {
        const ret = wasm.federatedmodel_getDimension(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get parameters as array
     * @returns {Float32Array}
     */
    getParameters() {
        const ret = wasm.federatedmodel_getParameters(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Set parameters from array
     * @param {Float32Array} params
     */
    setParameters(params) {
        const ptr0 = passArrayF32ToWasm0(params, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.federatedmodel_setParameters(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Apply aggregated gradients to update model
     * @param {Float32Array} gradients
     */
    applyGradients(gradients) {
        const ptr0 = passArrayF32ToWasm0(gradients, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.federatedmodel_applyGradients(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set local epochs per round
     * @param {number} epochs
     */
    setLocalEpochs(epochs) {
        wasm.federatedmodel_setLocalEpochs(this.__wbg_ptr, epochs);
    }
    /**
     * Set learning rate
     * @param {number} lr
     */
    setLearningRate(lr) {
        wasm.federatedmodel_setLearningRate(this.__wbg_ptr, lr);
    }
    /**
     * Create a new federated model
     * @param {number} dimension
     * @param {number} learning_rate
     * @param {number} momentum
     */
    constructor(dimension, learning_rate, momentum) {
        const ret = wasm.federatedmodel_new(dimension, learning_rate, momentum);
        this.__wbg_ptr = ret >>> 0;
        FederatedModelFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get current round
     * @returns {bigint}
     */
    getRound() {
        const ret = wasm.federatedmodel_getRound(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
}
if (Symbol.dispose) FederatedModel.prototype[Symbol.dispose] = FederatedModel.prototype.free;
exports.FederatedModel = FederatedModel;

/**
 * Founding contributor registry
 */
class FoundingRegistry {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        FoundingRegistryFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_foundingregistry_free(ptr, 0);
    }
    /**
     * Process epoch distribution
     * @param {bigint} current_epoch
     * @param {bigint} available_amount
     * @returns {any[]}
     */
    processEpoch(current_epoch, available_amount) {
        const ret = wasm.foundingregistry_processEpoch(this.__wbg_ptr, current_epoch, available_amount);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Calculate vested amount for current epoch
     * @param {bigint} current_epoch
     * @param {bigint} pool_balance
     * @returns {bigint}
     */
    calculateVested(current_epoch, pool_balance) {
        const ret = wasm.foundingregistry_calculateVested(this.__wbg_ptr, current_epoch, pool_balance);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get founding contributor count
     * @returns {number}
     */
    getFounderCount() {
        const ret = wasm.foundingregistry_getFounderCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Register additional founding contributor
     * @param {string} id
     * @param {string} category
     * @param {number} weight
     */
    registerContributor(id, category, weight) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(category, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.foundingregistry_registerContributor(this.__wbg_ptr, ptr0, len0, ptr1, len1, weight);
    }
    constructor() {
        const ret = wasm.foundingregistry_new();
        this.__wbg_ptr = ret >>> 0;
        FoundingRegistryFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) FoundingRegistry.prototype[Symbol.dispose] = FoundingRegistry.prototype.free;
exports.FoundingRegistry = FoundingRegistry;

/**
 * Genesis Key - Ultra-compact origin marker (φ-sized: 21 bytes)
 */
class GenesisKey {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        GenesisKeyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_genesiskey_free(ptr, 0);
    }
    /**
     * Get ID as hex
     * @returns {string}
     */
    getIdHex() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.genesiskey_getIdHex(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Export ultra-compact genesis key (21 bytes only)
     * @returns {Uint8Array}
     */
    exportUltraCompact() {
        const ret = wasm.genesiskey_exportUltraCompact(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Create a new genesis key
     * @param {PiKey} creator
     * @param {number} epoch
     */
    constructor(creator, epoch) {
        _assertClass(creator, PiKey);
        const ret = wasm.genesiskey_create(creator.__wbg_ptr, epoch);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        GenesisKeyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get the φ-sized genesis ID
     * @returns {Uint8Array}
     */
    getId() {
        const ret = wasm.genesiskey_getId(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Verify this genesis key was created by a specific Pi-Key
     * @param {Uint8Array} creator_public_key
     * @returns {boolean}
     */
    verify(creator_public_key) {
        const ptr0 = passArray8ToWasm0(creator_public_key, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.genesiskey_verify(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get epoch
     * @returns {number}
     */
    getEpoch() {
        const ret = wasm.genesiskey_getEpoch(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) GenesisKey.prototype[Symbol.dispose] = GenesisKey.prototype.free;
exports.GenesisKey = GenesisKey;

/**
 * Genesis node sunset orchestrator
 */
class GenesisSunset {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        GenesisSunsetFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_genesissunset_free(ptr, 0);
    }
    /**
     * Check if it's safe to retire genesis nodes
     * @returns {boolean}
     */
    canRetire() {
        const ret = wasm.genesissunset_canRetire(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get sunset status
     * @returns {string}
     */
    getStatus() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.genesissunset_getStatus(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check if genesis nodes should be read-only
     * @returns {boolean}
     */
    isReadOnly() {
        const ret = wasm.genesissunset_isReadOnly(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get current sunset phase
     * 0 = Active (genesis required)
     * 1 = Transition (stop new connections)
     * 2 = Read-only (genesis read-only)
     * 3 = Retired (genesis can be removed)
     * @returns {number}
     */
    getCurrentPhase() {
        const ret = wasm.genesissunset_getCurrentPhase(this.__wbg_ptr);
        return ret;
    }
    /**
     * Update network node count
     * @param {number} count
     * @returns {number}
     */
    updateNodeCount(count) {
        const ret = wasm.genesissunset_updateNodeCount(this.__wbg_ptr, count);
        return ret;
    }
    /**
     * Check if network is self-sustaining
     * @returns {boolean}
     */
    isSelfSustaining() {
        const ret = wasm.genesissunset_canRetire(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Register a genesis node
     * @param {string} node_id
     */
    registerGenesisNode(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.genesissunset_registerGenesisNode(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Check if genesis nodes should accept new connections
     * @returns {boolean}
     */
    shouldAcceptConnections() {
        const ret = wasm.genesissunset_shouldAcceptConnections(this.__wbg_ptr);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.genesissunset_new();
        this.__wbg_ptr = ret >>> 0;
        GenesisSunsetFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) GenesisSunset.prototype[Symbol.dispose] = GenesisSunset.prototype.free;
exports.GenesisSunset = GenesisSunset;

/**
 * P2P Gradient Gossip for decentralized federated learning
 *
 * This is the main coordinator for federated learning without a central server.
 */
class GradientGossip {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        GradientGossipFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_gradientgossip_free(ptr, 0);
    }
    /**
     * Get number of active peers
     * @returns {number}
     */
    peerCount() {
        const ret = wasm.gradientgossip_peerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Prune stale peer gradients
     * @returns {number}
     */
    pruneStale() {
        const ret = wasm.gradientgossip_pruneStale(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Configure differential privacy
     * @param {number} epsilon
     * @param {number} sensitivity
     */
    configureDifferentialPrivacy(epsilon, sensitivity) {
        wasm.gradientgossip_configureDifferentialPrivacy(this.__wbg_ptr, epsilon, sensitivity);
    }
    /**
     * Advance to next consensus round
     * @returns {bigint}
     */
    advanceRound() {
        const ret = wasm.gradientgossip_advanceRound(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get gradient dimension
     * @returns {number}
     */
    getDimension() {
        const ret = wasm.gradientgossip_getDimension(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Enable/disable differential privacy
     * @param {boolean} enabled
     */
    setDPEnabled(enabled) {
        wasm.gradientgossip_setDPEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * Set model hash for version compatibility
     * @param {Uint8Array} hash
     */
    setModelHash(hash) {
        const ptr0 = passArray8ToWasm0(hash, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.gradientgossip_setModelHash(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Get current consensus round
     * @returns {bigint}
     */
    getCurrentRound() {
        const ret = wasm.gradientgossip_getCurrentRound(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Set local gradients from JavaScript
     * @param {Float32Array} gradients
     */
    setLocalGradients(gradients) {
        const ptr0 = passArrayF32ToWasm0(gradients, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.gradientgossip_setLocalGradients(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Get compression ratio achieved
     * @returns {number}
     */
    getCompressionRatio() {
        const ret = wasm.gradientgossip_getCompressionRatio(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get aggregated gradients as JavaScript array
     * @returns {Float32Array}
     */
    getAggregatedGradients() {
        const ret = wasm.gradientgossip_getAggregatedGradients(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Create a new GradientGossip instance
     *
     * # Arguments
     * * `local_peer_id` - 32-byte peer identifier
     * * `dimension` - Gradient vector dimension
     * * `k_ratio` - TopK sparsification ratio (0.1 = keep top 10%)
     * @param {Uint8Array} local_peer_id
     * @param {number} dimension
     * @param {number} k_ratio
     */
    constructor(local_peer_id, dimension, k_ratio) {
        const ptr0 = passArray8ToWasm0(local_peer_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.gradientgossip_new(ptr0, len0, dimension, k_ratio);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        GradientGossipFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.gradientgossip_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) GradientGossip.prototype[Symbol.dispose] = GradientGossip.prototype.free;
exports.GradientGossip = GradientGossip;

/**
 * Model consensus manager for federated learning integration
 */
class ModelConsensusManager {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ModelConsensusManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_modelconsensusmanager_free(ptr, 0);
    }
    /**
     * Get number of tracked models
     * @returns {number}
     */
    modelCount() {
        const ret = wasm.modelconsensusmanager_modelCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get number of active disputes
     * @returns {number}
     */
    disputeCount() {
        const ret = wasm.modelconsensusmanager_disputeCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get number of quarantined updates
     * @returns {number}
     */
    quarantinedUpdateCount() {
        const ret = wasm.modelconsensusmanager_quarantinedUpdateCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new model consensus manager
     * @param {number} min_witnesses
     */
    constructor(min_witnesses) {
        const ret = wasm.modelconsensusmanager_new(min_witnesses);
        this.__wbg_ptr = ret >>> 0;
        ModelConsensusManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.modelconsensusmanager_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) ModelConsensusManager.prototype[Symbol.dispose] = ModelConsensusManager.prototype.free;
exports.ModelConsensusManager = ModelConsensusManager;

/**
 * Multi-head attention for distributed task routing
 */
class MultiHeadAttention {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        MultiHeadAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_multiheadattention_free(ptr, 0);
    }
    /**
     * Get embedding dimension
     * @returns {number}
     */
    dim() {
        const ret = wasm.multiheadattention_dim(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create new multi-head attention
     * @param {number} dim
     * @param {number} num_heads
     */
    constructor(dim, num_heads) {
        const ret = wasm.multiheadattention_new(dim, num_heads);
        this.__wbg_ptr = ret >>> 0;
        MultiHeadAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get number of heads
     * @returns {number}
     */
    numHeads() {
        const ret = wasm.multiheadattention_numHeads(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) MultiHeadAttention.prototype[Symbol.dispose] = MultiHeadAttention.prototype.free;
exports.MultiHeadAttention = MultiHeadAttention;

/**
 * Network lifecycle events and Easter eggs manager
 */
class NetworkEvents {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NetworkEventsFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_networkevents_free(ptr, 0);
    }
    /**
     * Get a subtle motivational message
     * @param {bigint} balance
     * @returns {string}
     */
    getMotivation(balance) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.networkevents_getMotivation(this.__wbg_ptr, balance);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check for discovery triggers (Easter eggs)
     * @param {string} action
     * @param {string} node_id
     * @returns {string | undefined}
     */
    checkDiscovery(action, node_id) {
        const ptr0 = passStringToWasm0(action, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.networkevents_checkDiscovery(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        let v3;
        if (ret[0] !== 0) {
            v3 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v3;
    }
    /**
     * Get ASCII art for special occasions
     * @returns {string | undefined}
     */
    getSpecialArt() {
        const ret = wasm.networkevents_getSpecialArt(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Check milestone achievements
     * @param {bigint} balance
     * @param {string} node_id
     * @returns {string}
     */
    checkMilestones(balance, node_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.networkevents_checkMilestones(this.__wbg_ptr, balance, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Set current time (for testing)
     * @param {bigint} timestamp
     */
    setCurrentTime(timestamp) {
        wasm.networkevents_setCurrentTime(this.__wbg_ptr, timestamp);
    }
    /**
     * Get network status with thematic flair
     * @param {number} node_count
     * @param {bigint} total_ruv
     * @returns {string}
     */
    getThemedStatus(node_count, total_ruv) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.networkevents_getThemedStatus(this.__wbg_ptr, node_count, total_ruv);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check for active special events
     * @returns {string}
     */
    checkActiveEvents() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.networkevents_checkActiveEvents(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get celebration multiplier boost
     * @returns {number}
     */
    getCelebrationBoost() {
        const ret = wasm.networkevents_getCelebrationBoost(this.__wbg_ptr);
        return ret;
    }
    constructor() {
        const ret = wasm.networkevents_new();
        this.__wbg_ptr = ret >>> 0;
        NetworkEventsFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) NetworkEvents.prototype[Symbol.dispose] = NetworkEvents.prototype.free;
exports.NetworkEvents = NetworkEvents;

/**
 * Unified learning intelligence for edge-net nodes
 */
class NetworkLearning {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NetworkLearningFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_networklearning_free(ptr, 0);
    }
    /**
     * Get pattern count
     * @returns {number}
     */
    patternCount() {
        const ret = wasm.networklearning_patternCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Store a learned pattern
     * @param {string} pattern_json
     * @returns {number}
     */
    storePattern(pattern_json) {
        const ptr0 = passStringToWasm0(pattern_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.networklearning_storePattern(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Look up similar patterns
     * @param {string} query_json
     * @param {number} k
     * @returns {string}
     */
    lookupPatterns(query_json, k) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.networklearning_lookupPatterns(this.__wbg_ptr, ptr0, len0, k);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get energy savings ratio for spike-driven attention
     * @param {number} seq_len
     * @param {number} hidden_dim
     * @returns {number}
     */
    getEnergyRatio(seq_len, hidden_dim) {
        const ret = wasm.networklearning_getEnergyRatio(this.__wbg_ptr, seq_len, hidden_dim);
        return ret;
    }
    /**
     * Get trajectory count
     * @returns {number}
     */
    trajectoryCount() {
        const ret = wasm.networklearning_trajectoryCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Record a task execution trajectory
     * @param {string} trajectory_json
     * @returns {boolean}
     */
    recordTrajectory(trajectory_json) {
        const ptr0 = passStringToWasm0(trajectory_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.networklearning_recordTrajectory(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create new network learning intelligence
     */
    constructor() {
        const ret = wasm.networklearning_new();
        this.__wbg_ptr = ret >>> 0;
        NetworkLearningFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Prune low-quality patterns
     * @param {number} min_usage
     * @param {number} min_confidence
     * @returns {number}
     */
    prune(min_usage, min_confidence) {
        const ret = wasm.networklearning_prune(this.__wbg_ptr, min_usage, min_confidence);
        return ret >>> 0;
    }
    /**
     * Get combined statistics
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.networklearning_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) NetworkLearning.prototype[Symbol.dispose] = NetworkLearning.prototype.free;
exports.NetworkLearning = NetworkLearning;

/**
 * Network topology adaptation for self-organization
 */
class NetworkTopology {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NetworkTopologyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_networktopology_free(ptr, 0);
    }
    /**
     * Register a node in the topology
     * @param {string} node_id
     * @param {Float32Array} capabilities
     */
    registerNode(node_id, capabilities) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(capabilities, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.networktopology_registerNode(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Get optimal peers for a node
     * @param {string} node_id
     * @param {number} count
     * @returns {string[]}
     */
    getOptimalPeers(node_id, count) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.networktopology_getOptimalPeers(this.__wbg_ptr, ptr0, len0, count);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Update connection strength between nodes
     * @param {string} from
     * @param {string} to
     * @param {number} success_rate
     */
    updateConnection(from, to, success_rate) {
        const ptr0 = passStringToWasm0(from, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(to, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.networktopology_updateConnection(this.__wbg_ptr, ptr0, len0, ptr1, len1, success_rate);
    }
    constructor() {
        const ret = wasm.networktopology_new();
        this.__wbg_ptr = ret >>> 0;
        NetworkTopologyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) NetworkTopology.prototype[Symbol.dispose] = NetworkTopology.prototype.free;
exports.NetworkTopology = NetworkTopology;

class NodeConfig {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NodeConfigFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_nodeconfig_free(ptr, 0);
    }
    /**
     * Maximum CPU usage when idle (0.0 - 1.0)
     * @returns {number}
     */
    get cpu_limit() {
        const ret = wasm.__wbg_get_economichealth_velocity(this.__wbg_ptr);
        return ret;
    }
    /**
     * Maximum CPU usage when idle (0.0 - 1.0)
     * @param {number} arg0
     */
    set cpu_limit(arg0) {
        wasm.__wbg_set_economichealth_velocity(this.__wbg_ptr, arg0);
    }
    /**
     * Maximum memory usage in bytes
     * @returns {number}
     */
    get memory_limit() {
        const ret = wasm.__wbg_get_nodeconfig_memory_limit(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Maximum memory usage in bytes
     * @param {number} arg0
     */
    set memory_limit(arg0) {
        wasm.__wbg_set_nodeconfig_memory_limit(this.__wbg_ptr, arg0);
    }
    /**
     * Maximum bandwidth in bytes/sec
     * @returns {number}
     */
    get bandwidth_limit() {
        const ret = wasm.__wbg_get_nodeconfig_bandwidth_limit(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Maximum bandwidth in bytes/sec
     * @param {number} arg0
     */
    set bandwidth_limit(arg0) {
        wasm.__wbg_set_nodeconfig_bandwidth_limit(this.__wbg_ptr, arg0);
    }
    /**
     * Minimum idle time before contributing (ms)
     * @returns {number}
     */
    get min_idle_time() {
        const ret = wasm.__wbg_get_nodeconfig_min_idle_time(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Minimum idle time before contributing (ms)
     * @param {number} arg0
     */
    set min_idle_time(arg0) {
        wasm.__wbg_set_nodeconfig_min_idle_time(this.__wbg_ptr, arg0);
    }
    /**
     * Whether to reduce contribution on battery
     * @returns {boolean}
     */
    get respect_battery() {
        const ret = wasm.__wbg_get_nodeconfig_respect_battery(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether to reduce contribution on battery
     * @param {boolean} arg0
     */
    set respect_battery(arg0) {
        wasm.__wbg_set_nodeconfig_respect_battery(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) NodeConfig.prototype[Symbol.dispose] = NodeConfig.prototype.free;
exports.NodeConfig = NodeConfig;

class NodeStats {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(NodeStats.prototype);
        obj.__wbg_ptr = ptr;
        NodeStatsFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NodeStatsFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_nodestats_free(ptr, 0);
    }
    /**
     * Total rUv (Resource Utility Vouchers) earned
     * @returns {bigint}
     */
    get ruv_earned() {
        const ret = wasm.__wbg_get_nodestats_ruv_earned(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Total rUv (Resource Utility Vouchers) earned
     * @param {bigint} arg0
     */
    set ruv_earned(arg0) {
        wasm.__wbg_set_nodestats_ruv_earned(this.__wbg_ptr, arg0);
    }
    /**
     * Total rUv spent
     * @returns {bigint}
     */
    get ruv_spent() {
        const ret = wasm.__wbg_get_nodestats_ruv_spent(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Total rUv spent
     * @param {bigint} arg0
     */
    set ruv_spent(arg0) {
        wasm.__wbg_set_nodestats_ruv_spent(this.__wbg_ptr, arg0);
    }
    /**
     * Tasks completed
     * @returns {bigint}
     */
    get tasks_completed() {
        const ret = wasm.__wbg_get_nodestats_tasks_completed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Tasks completed
     * @param {bigint} arg0
     */
    set tasks_completed(arg0) {
        wasm.__wbg_set_nodestats_tasks_completed(this.__wbg_ptr, arg0);
    }
    /**
     * Tasks submitted
     * @returns {bigint}
     */
    get tasks_submitted() {
        const ret = wasm.__wbg_get_nodestats_tasks_submitted(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Tasks submitted
     * @param {bigint} arg0
     */
    set tasks_submitted(arg0) {
        wasm.__wbg_set_nodestats_tasks_submitted(this.__wbg_ptr, arg0);
    }
    /**
     * Total uptime in seconds
     * @returns {bigint}
     */
    get uptime_seconds() {
        const ret = wasm.__wbg_get_nodestats_uptime_seconds(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Total uptime in seconds
     * @param {bigint} arg0
     */
    set uptime_seconds(arg0) {
        wasm.__wbg_set_nodestats_uptime_seconds(this.__wbg_ptr, arg0);
    }
    /**
     * Current reputation score (0.0 - 1.0)
     * @returns {number}
     */
    get reputation() {
        const ret = wasm.__wbg_get_nodestats_reputation(this.__wbg_ptr);
        return ret;
    }
    /**
     * Current reputation score (0.0 - 1.0)
     * @param {number} arg0
     */
    set reputation(arg0) {
        wasm.__wbg_set_nodestats_reputation(this.__wbg_ptr, arg0);
    }
    /**
     * Current contribution multiplier
     * @returns {number}
     */
    get multiplier() {
        const ret = wasm.__wbg_get_nodestats_multiplier(this.__wbg_ptr);
        return ret;
    }
    /**
     * Current contribution multiplier
     * @param {number} arg0
     */
    set multiplier(arg0) {
        wasm.__wbg_set_nodestats_multiplier(this.__wbg_ptr, arg0);
    }
    /**
     * Active lifecycle events
     * @returns {number}
     */
    get celebration_boost() {
        const ret = wasm.__wbg_get_nodestats_celebration_boost(this.__wbg_ptr);
        return ret;
    }
    /**
     * Active lifecycle events
     * @param {number} arg0
     */
    set celebration_boost(arg0) {
        wasm.__wbg_set_nodestats_celebration_boost(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) NodeStats.prototype[Symbol.dispose] = NodeStats.prototype.free;
exports.NodeStats = NodeStats;

/**
 * Network optimization for resource efficiency
 */
class OptimizationEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        OptimizationEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_optimizationengine_free(ptr, 0);
    }
    /**
     * Record task routing outcome
     * @param {string} task_type
     * @param {string} node_id
     * @param {bigint} latency_ms
     * @param {boolean} success
     */
    recordRouting(task_type, node_id, latency_ms, success) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.optimizationengine_recordRouting(this.__wbg_ptr, ptr0, len0, ptr1, len1, latency_ms, success);
    }
    /**
     * Get optimal node for a task type
     * @param {string} task_type
     * @param {string[]} candidates
     * @returns {string}
     */
    selectOptimalNode(task_type, candidates) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passArrayJsValueToWasm0(candidates, wasm.__wbindgen_malloc);
            const len1 = WASM_VECTOR_LEN;
            const ret = wasm.optimizationengine_selectOptimalNode(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            deferred3_0 = ret[0];
            deferred3_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    constructor() {
        const ret = wasm.optimizationengine_new();
        this.__wbg_ptr = ret >>> 0;
        OptimizationEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get optimization stats
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.optimizationengine_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) OptimizationEngine.prototype[Symbol.dispose] = OptimizationEngine.prototype.free;
exports.OptimizationEngine = OptimizationEngine;

/**
 * Ultra-compact Pi-Key (40 bytes identity + 21 bytes genesis signature)
 */
class PiKey {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(PiKey.prototype);
        obj.__wbg_ptr = ptr;
        PiKeyFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        PiKeyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_pikey_free(ptr, 0);
    }
    /**
     * Get the Pi-sized identity (40 bytes)
     * @returns {Uint8Array}
     */
    getIdentity() {
        const ret = wasm.pikey_getIdentity(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Get short identity (first 8 bytes as hex)
     * @returns {string}
     */
    getShortId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.pikey_getShortId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Export minimal key representation (Pi + Phi sized = 61 bytes total)
     * @returns {Uint8Array}
     */
    exportCompact() {
        const ret = wasm.pikey_exportCompact(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Get public key for verification
     * @returns {Uint8Array}
     */
    getPublicKey() {
        const ret = wasm.pikey_getPublicKey(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Verify this key has Pi magic marker
     * @returns {boolean}
     */
    verifyPiMagic() {
        const ret = wasm.pikey_verifyPiMagic(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get identity as hex string
     * @returns {string}
     */
    getIdentityHex() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.pikey_getIdentityHex(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Restore from encrypted backup (supports both v1 legacy and v2 Argon2id)
     * @param {Uint8Array} backup
     * @param {string} password
     * @returns {PiKey}
     */
    static restoreFromBackup(backup, password) {
        const ptr0 = passArray8ToWasm0(backup, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.pikey_restoreFromBackup(ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return PiKey.__wrap(ret[0]);
    }
    /**
     * Create encrypted backup of private key using Argon2id KDF
     * @param {string} password
     * @returns {Uint8Array}
     */
    createEncryptedBackup(password) {
        const ptr0 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.pikey_createEncryptedBackup(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Get the Phi-sized genesis fingerprint (21 bytes)
     * @returns {Uint8Array}
     */
    getGenesisFingerprint() {
        const ret = wasm.pikey_getGenesisFingerprint(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Sign data with this key
     * @param {Uint8Array} data
     * @returns {Uint8Array}
     */
    sign(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.pikey_sign(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Verify signature from another Pi-Key
     * @param {Uint8Array} data
     * @param {Uint8Array} signature
     * @param {Uint8Array} public_key
     * @returns {boolean}
     */
    verify(data, signature, public_key) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(signature, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArray8ToWasm0(public_key, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.pikey_verify(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret !== 0;
    }
    /**
     * Generate a new Pi-Key with genesis linking
     * @param {Uint8Array | null} [genesis_seed]
     */
    constructor(genesis_seed) {
        var ptr0 = isLikeNone(genesis_seed) ? 0 : passArray8ToWasm0(genesis_seed, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.pikey_generate(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        PiKeyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get key statistics
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.pikey_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) PiKey.prototype[Symbol.dispose] = PiKey.prototype.free;
exports.PiKey = PiKey;

/**
 * QDAG Ledger - the full transaction graph
 */
class QDAGLedger {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        QDAGLedgerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_qdagledger_free(ptr, 0);
    }
    /**
     * Export ledger state for sync
     * @returns {Uint8Array}
     */
    exportState() {
        const ret = wasm.qdagledger_exportState(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Import ledger state from sync
     * @param {Uint8Array} state_bytes
     * @returns {number}
     */
    importState(state_bytes) {
        const ptr0 = passArray8ToWasm0(state_bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.qdagledger_importState(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Get total supply
     * @returns {bigint}
     */
    totalSupply() {
        const ret = wasm.qdagledger_totalSupply(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get staked amount for a node
     * @param {string} node_id
     * @returns {bigint}
     */
    stakedAmount(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.qdagledger_stakedAmount(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Create genesis transaction (called once at network start)
     * @param {bigint} initial_supply
     * @param {Uint8Array} founder_pubkey
     * @returns {Uint8Array}
     */
    createGenesis(initial_supply, founder_pubkey) {
        const ptr0 = passArray8ToWasm0(founder_pubkey, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.qdagledger_createGenesis(this.__wbg_ptr, initial_supply, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Get transaction count
     * @returns {number}
     */
    transactionCount() {
        const ret = wasm.qdagledger_transactionCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create and validate a new transaction
     * @param {string} sender_id
     * @param {string} recipient_id
     * @param {bigint} amount
     * @param {number} tx_type
     * @param {Uint8Array} sender_privkey
     * @param {Uint8Array} sender_pubkey
     * @returns {Uint8Array}
     */
    createTransaction(sender_id, recipient_id, amount, tx_type, sender_privkey, sender_pubkey) {
        const ptr0 = passStringToWasm0(sender_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(recipient_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArray8ToWasm0(sender_privkey, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArray8ToWasm0(sender_pubkey, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ret = wasm.qdagledger_createTransaction(this.__wbg_ptr, ptr0, len0, ptr1, len1, amount, tx_type, ptr2, len2, ptr3, len3);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v5 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v5;
    }
    /**
     * Create a new QDAG ledger
     */
    constructor() {
        const ret = wasm.qdagledger_new();
        this.__wbg_ptr = ret >>> 0;
        QDAGLedgerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get balance for a node
     * @param {string} node_id
     * @returns {bigint}
     */
    balance(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.qdagledger_balance(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get tip count
     * @returns {number}
     */
    tipCount() {
        const ret = wasm.qdagledger_tipCount(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) QDAGLedger.prototype[Symbol.dispose] = QDAGLedger.prototype.free;
exports.QDAGLedger = QDAGLedger;

/**
 * Manages quarantine status of contested claims
 */
class QuarantineManager {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        QuarantineManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_quarantinemanager_free(ptr, 0);
    }
    /**
     * Get number of quarantined claims
     * @returns {number}
     */
    quarantinedCount() {
        const ret = wasm.quarantinemanager_quarantinedCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new quarantine manager
     */
    constructor() {
        const ret = wasm.quarantinemanager_new();
        this.__wbg_ptr = ret >>> 0;
        QuarantineManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Check if claim can be used in decisions
     * @param {string} claim_id
     * @returns {boolean}
     */
    canUse(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.quarantinemanager_canUse(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Check quarantine level for a claim
     * @param {string} claim_id
     * @returns {number}
     */
    getLevel(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.quarantinemanager_getLevel(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Set quarantine level
     * @param {string} claim_id
     * @param {number} level
     */
    setLevel(claim_id, level) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.quarantinemanager_setLevel(this.__wbg_ptr, ptr0, len0, level);
    }
}
if (Symbol.dispose) QuarantineManager.prototype[Symbol.dispose] = QuarantineManager.prototype.free;
exports.QuarantineManager = QuarantineManager;

/**
 * RAC-specific combined economic engine managing stakes, reputation, and rewards
 */
class RacEconomicEngine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RacEconomicEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_raceconomicengine_free(ptr, 0);
    }
    /**
     * Get summary statistics as JSON
     * @returns {string}
     */
    getSummary() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.raceconomicengine_getSummary(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check if node can participate (has stake + reputation)
     * @param {Uint8Array} node_id
     * @returns {boolean}
     */
    canParticipate(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.raceconomicengine_canParticipate(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get combined score (stake-weighted reputation)
     * @param {Uint8Array} node_id
     * @returns {number}
     */
    getCombinedScore(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.raceconomicengine_getCombinedScore(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Create a new RAC economic engine
     */
    constructor() {
        const ret = wasm.raceconomicengine_new();
        this.__wbg_ptr = ret >>> 0;
        RacEconomicEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) RacEconomicEngine.prototype[Symbol.dispose] = RacEconomicEngine.prototype.free;
exports.RacEconomicEngine = RacEconomicEngine;

/**
 * RAC-specific semantic gossip router for event propagation
 */
class RacSemanticRouter {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RacSemanticRouterFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_racsemanticrouter_free(ptr, 0);
    }
    /**
     * Get peer count
     * @returns {number}
     */
    peerCount() {
        const ret = wasm.racsemanticrouter_peerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new semantic router
     */
    constructor() {
        const ret = wasm.racsemanticrouter_new();
        this.__wbg_ptr = ret >>> 0;
        RacSemanticRouterFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) RacSemanticRouter.prototype[Symbol.dispose] = RacSemanticRouter.prototype.free;
exports.RacSemanticRouter = RacSemanticRouter;

/**
 * Rate limiter to prevent spam/DoS
 */
class RateLimiter {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RateLimiterFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_ratelimiter_free(ptr, 0);
    }
    /**
     * Check if request is allowed
     * @param {string} node_id
     * @returns {boolean}
     */
    checkAllowed(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.ratelimiter_checkAllowed(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {bigint} window_ms
     * @param {number} max_requests
     */
    constructor(window_ms, max_requests) {
        const ret = wasm.ratelimiter_new(window_ms, max_requests);
        this.__wbg_ptr = ret >>> 0;
        RateLimiterFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Reset rate limiter
     */
    reset() {
        wasm.ratelimiter_reset(this.__wbg_ptr);
    }
    /**
     * Get current count for a node
     * @param {string} node_id
     * @returns {number}
     */
    getCount(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.ratelimiter_getCount(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
}
if (Symbol.dispose) RateLimiter.prototype[Symbol.dispose] = RateLimiter.prototype.free;
exports.RateLimiter = RateLimiter;

/**
 * ReasoningBank for storing and retrieving learned patterns
 * Optimized with spatial indexing for O(1) approximate lookups
 */
class ReasoningBank {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReasoningBankFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_reasoningbank_free(ptr, 0);
    }
    /**
     * Create a new ReasoningBank
     */
    constructor() {
        const ret = wasm.reasoningbank_new();
        this.__wbg_ptr = ret >>> 0;
        ReasoningBankFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get total pattern count
     * @returns {number}
     */
    count() {
        const ret = wasm.reasoningbank_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Prune low-quality patterns
     * @param {number} min_usage
     * @param {number} min_confidence
     * @returns {number}
     */
    prune(min_usage, min_confidence) {
        const ret = wasm.reasoningbank_prune(this.__wbg_ptr, min_usage, min_confidence);
        return ret >>> 0;
    }
    /**
     * Store a new pattern (JSON format)
     * @param {string} pattern_json
     * @returns {number}
     */
    store(pattern_json) {
        const ptr0 = passStringToWasm0(pattern_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reasoningbank_store(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Lookup most similar patterns (OPTIMIZED with spatial indexing)
     * @param {string} query_json
     * @param {number} k
     * @returns {string}
     */
    lookup(query_json, k) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.reasoningbank_lookup(this.__wbg_ptr, ptr0, len0, k);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get bank statistics
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.reasoningbank_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) ReasoningBank.prototype[Symbol.dispose] = ReasoningBank.prototype.free;
exports.ReasoningBank = ReasoningBank;

/**
 * Reputation manager with decay mechanics
 */
class ReputationManager {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReputationManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_reputationmanager_free(ptr, 0);
    }
    /**
     * Get number of tracked nodes
     * @returns {number}
     */
    nodeCount() {
        const ret = wasm.reputationmanager_nodeCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get effective reputation for a node (with decay applied)
     * @param {Uint8Array} node_id
     * @returns {number}
     */
    getReputation(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reputationmanager_getReputation(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get average network reputation
     * @returns {number}
     */
    averageReputation() {
        const ret = wasm.reputationmanager_averageReputation(this.__wbg_ptr);
        return ret;
    }
    /**
     * Check if node has sufficient reputation
     * @param {Uint8Array} node_id
     * @returns {boolean}
     */
    hasSufficientReputation(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reputationmanager_hasSufficientReputation(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new reputation manager
     * @param {number} decay_rate
     * @param {bigint} decay_interval_ms
     */
    constructor(decay_rate, decay_interval_ms) {
        const ret = wasm.reputationmanager_new(decay_rate, decay_interval_ms);
        this.__wbg_ptr = ret >>> 0;
        ReputationManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) ReputationManager.prototype[Symbol.dispose] = ReputationManager.prototype.free;
exports.ReputationManager = ReputationManager;

/**
 * Reputation system for nodes
 */
class ReputationSystem {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReputationSystemFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_reputationsystem_free(ptr, 0);
    }
    /**
     * Get reputation score for a node
     * @param {string} node_id
     * @returns {number}
     */
    getReputation(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reputationsystem_getReputation(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Record failed task completion
     * @param {string} node_id
     */
    recordFailure(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.reputationsystem_recordFailure(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Record penalty (fraud, invalid result)
     * @param {string} node_id
     * @param {number} severity
     */
    recordPenalty(node_id, severity) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.reputationsystem_recordPenalty(this.__wbg_ptr, ptr0, len0, severity);
    }
    /**
     * Record successful task completion
     * @param {string} node_id
     */
    recordSuccess(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.reputationsystem_recordSuccess(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Check if node can participate
     * @param {string} node_id
     * @returns {boolean}
     */
    canParticipate(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reputationsystem_canParticipate(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.reputationsystem_new();
        this.__wbg_ptr = ret >>> 0;
        ReputationSystemFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) ReputationSystem.prototype[Symbol.dispose] = ReputationSystem.prototype.free;
exports.ReputationSystem = ReputationSystem;

class RewardDistribution {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(RewardDistribution.prototype);
        obj.__wbg_ptr = ptr;
        RewardDistributionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RewardDistributionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rewarddistribution_free(ptr, 0);
    }
    /**
     * @returns {bigint}
     */
    get total() {
        const ret = wasm.__wbg_get_nodestats_ruv_earned(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {bigint} arg0
     */
    set total(arg0) {
        wasm.__wbg_set_nodestats_ruv_earned(this.__wbg_ptr, arg0);
    }
    /**
     * @returns {bigint}
     */
    get contributor_share() {
        const ret = wasm.__wbg_get_nodestats_ruv_spent(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {bigint} arg0
     */
    set contributor_share(arg0) {
        wasm.__wbg_set_nodestats_ruv_spent(this.__wbg_ptr, arg0);
    }
    /**
     * @returns {bigint}
     */
    get treasury_share() {
        const ret = wasm.__wbg_get_nodestats_tasks_completed(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {bigint} arg0
     */
    set treasury_share(arg0) {
        wasm.__wbg_set_nodestats_tasks_completed(this.__wbg_ptr, arg0);
    }
    /**
     * @returns {bigint}
     */
    get protocol_share() {
        const ret = wasm.__wbg_get_nodestats_tasks_submitted(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {bigint} arg0
     */
    set protocol_share(arg0) {
        wasm.__wbg_set_nodestats_tasks_submitted(this.__wbg_ptr, arg0);
    }
    /**
     * @returns {bigint}
     */
    get founder_share() {
        const ret = wasm.__wbg_get_nodestats_uptime_seconds(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {bigint} arg0
     */
    set founder_share(arg0) {
        wasm.__wbg_set_nodestats_uptime_seconds(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) RewardDistribution.prototype[Symbol.dispose] = RewardDistribution.prototype.free;
exports.RewardDistribution = RewardDistribution;

/**
 * Manages time-locked rewards
 */
class RewardManager {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RewardManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rewardmanager_free(ptr, 0);
    }
    /**
     * Get number of pending rewards
     * @returns {number}
     */
    pendingCount() {
        const ret = wasm.rewardmanager_pendingCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get total pending reward amount
     * @returns {bigint}
     */
    pendingAmount() {
        const ret = wasm.rewardmanager_pendingAmount(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get claimable rewards for a node
     * @param {Uint8Array} node_id
     * @returns {bigint}
     */
    claimableAmount(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.rewardmanager_claimableAmount(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Create a new reward manager
     * @param {bigint} default_vesting_ms
     */
    constructor(default_vesting_ms) {
        const ret = wasm.rewardmanager_new(default_vesting_ms);
        this.__wbg_ptr = ret >>> 0;
        RewardManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) RewardManager.prototype[Symbol.dispose] = RewardManager.prototype.free;
exports.RewardManager = RewardManager;

/**
 * Semantic router for intelligent gossip and peer discovery
 */
class SemanticRouter {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(SemanticRouter.prototype);
        obj.__wbg_ptr = ptr;
        SemanticRouterFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SemanticRouterFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_semanticrouter_free(ptr, 0);
    }
    /**
     * Get peer count
     * @returns {number}
     */
    peerCount() {
        const ret = wasm.semanticrouter_peerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get topic count
     * @returns {number}
     */
    topicCount() {
        const ret = wasm.semanticrouter_topicCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create with custom parameters
     * @param {number} embedding_dim
     * @param {number} semantic_neighbors
     * @param {number} random_sample
     * @returns {SemanticRouter}
     */
    static withParams(embedding_dim, semantic_neighbors, random_sample) {
        const ret = wasm.semanticrouter_withParams(embedding_dim, semantic_neighbors, random_sample);
        return SemanticRouter.__wrap(ret);
    }
    /**
     * Set my peer identity
     * @param {Uint8Array} peer_id
     */
    setMyPeerId(peer_id) {
        const ptr0 = passArray8ToWasm0(peer_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.semanticrouter_setMyPeerId(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get active peer count (seen in last 60 seconds)
     * @returns {number}
     */
    activePeerCount() {
        const ret = wasm.semanticrouter_activePeerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set my capabilities and update my centroid
     * @param {string[]} capabilities
     */
    setMyCapabilities(capabilities) {
        const ptr0 = passArrayJsValueToWasm0(capabilities, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.semanticrouter_setMyCapabilities(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Create a new semantic router
     */
    constructor() {
        const ret = wasm.semanticrouter_new();
        this.__wbg_ptr = ret >>> 0;
        SemanticRouterFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.semanticrouter_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) SemanticRouter.prototype[Symbol.dispose] = SemanticRouter.prototype.free;
exports.SemanticRouter = SemanticRouter;

/**
 * Session Key - Euler-sized ephemeral key (e-sized: 34 bytes)
 */
class SessionKey {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SessionKeyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sessionkey_free(ptr, 0);
    }
    /**
     * Get ID as hex
     * @returns {string}
     */
    getIdHex() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.sessionkey_getIdHex(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Check if session is expired
     * @returns {boolean}
     */
    isExpired() {
        const ret = wasm.sessionkey_isExpired(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get parent identity fingerprint
     * @returns {Uint8Array}
     */
    getParentIdentity() {
        const ret = wasm.sessionkey_getParentIdentity(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Create a new session key linked to a Pi-Key identity
     * @param {PiKey} parent
     * @param {number} ttl_seconds
     */
    constructor(parent, ttl_seconds) {
        _assertClass(parent, PiKey);
        const ret = wasm.sessionkey_create(parent.__wbg_ptr, ttl_seconds);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        SessionKeyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get the e-sized session ID
     * @returns {Uint8Array}
     */
    getId() {
        const ret = wasm.sessionkey_getId(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Decrypt data with this session key
     * @param {Uint8Array} data
     * @returns {Uint8Array}
     */
    decrypt(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sessionkey_decrypt(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Encrypt data with this session key
     * @param {Uint8Array} plaintext
     * @returns {Uint8Array}
     */
    encrypt(plaintext) {
        const ptr0 = passArray8ToWasm0(plaintext, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sessionkey_encrypt(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
}
if (Symbol.dispose) SessionKey.prototype[Symbol.dispose] = SessionKey.prototype.free;
exports.SessionKey = SessionKey;

/**
 * Spike-driven attention for energy-efficient compute (87x savings)
 */
class SpikeDrivenAttention {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(SpikeDrivenAttention.prototype);
        obj.__wbg_ptr = ptr;
        SpikeDrivenAttentionFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SpikeDrivenAttentionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_spikedrivenattention_free(ptr, 0);
    }
    /**
     * Create with custom parameters
     * @param {number} threshold
     * @param {number} steps
     * @param {number} refractory
     * @returns {SpikeDrivenAttention}
     */
    static withConfig(threshold, steps, refractory) {
        const ret = wasm.spikedrivenattention_withConfig(threshold, steps, refractory);
        return SpikeDrivenAttention.__wrap(ret);
    }
    /**
     * Estimate energy savings ratio compared to standard attention
     * @param {number} seq_len
     * @param {number} hidden_dim
     * @returns {number}
     */
    energyRatio(seq_len, hidden_dim) {
        const ret = wasm.spikedrivenattention_energyRatio(this.__wbg_ptr, seq_len, hidden_dim);
        return ret;
    }
    /**
     * Create new spike-driven attention with default config
     */
    constructor() {
        const ret = wasm.spikedrivenattention_new();
        this.__wbg_ptr = ret >>> 0;
        SpikeDrivenAttentionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) SpikeDrivenAttention.prototype[Symbol.dispose] = SpikeDrivenAttention.prototype.free;
exports.SpikeDrivenAttention = SpikeDrivenAttention;

/**
 * Spot-check system for result verification
 */
class SpotChecker {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SpotCheckerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_spotchecker_free(ptr, 0);
    }
    /**
     * Check if a task should include a spot-check
     * @returns {boolean}
     */
    shouldCheck() {
        const ret = wasm.spotchecker_shouldCheck(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Add a known challenge-response pair
     * @param {string} task_type
     * @param {Uint8Array} input
     * @param {Uint8Array} expected_output
     */
    addChallenge(task_type, input, expected_output) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(input, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArray8ToWasm0(expected_output, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.spotchecker_addChallenge(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
    }
    /**
     * Get a random challenge for a task type
     * @param {string} task_type
     * @returns {Uint8Array | undefined}
     */
    getChallenge(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.spotchecker_getChallenge(this.__wbg_ptr, ptr0, len0);
        let v2;
        if (ret[0] !== 0) {
            v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v2;
    }
    /**
     * Verify a challenge response
     * @param {Uint8Array} input_hash
     * @param {Uint8Array} output
     * @returns {boolean}
     */
    verifyResponse(input_hash, output) {
        const ptr0 = passArray8ToWasm0(input_hash, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(output, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.spotchecker_verifyResponse(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * @param {number} check_probability
     */
    constructor(check_probability) {
        const ret = wasm.spotchecker_new(check_probability);
        this.__wbg_ptr = ret >>> 0;
        SpotCheckerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) SpotChecker.prototype[Symbol.dispose] = SpotChecker.prototype.free;
exports.SpotChecker = SpotChecker;

/**
 * Stake manager for the network
 */
class StakeManager {
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
     * Get number of stakers
     * @returns {number}
     */
    stakerCount() {
        const ret = wasm.stakemanager_stakerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get total staked amount in network
     * @returns {bigint}
     */
    totalStaked() {
        const ret = wasm.stakemanager_totalStaked(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get minimum stake requirement
     * @returns {bigint}
     */
    getMinStake() {
        const ret = wasm.stakemanager_getMinStake(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Check if node has sufficient stake
     * @param {Uint8Array} node_id
     * @returns {boolean}
     */
    hasSufficientStake(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_hasSufficientStake(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new stake manager
     * @param {bigint} min_stake
     */
    constructor(min_stake) {
        const ret = wasm.stakemanager_new(min_stake);
        this.__wbg_ptr = ret >>> 0;
        StakeManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get staked amount for a node
     * @param {Uint8Array} node_id
     * @returns {bigint}
     */
    getStake(node_id) {
        const ptr0 = passArray8ToWasm0(node_id, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.stakemanager_getStake(this.__wbg_ptr, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
}
if (Symbol.dispose) StakeManager.prototype[Symbol.dispose] = StakeManager.prototype.free;
exports.StakeManager = StakeManager;

/**
 * Unified swarm intelligence coordinator
 */
class SwarmIntelligence {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SwarmIntelligenceFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_swarmintelligence_free(ptr, 0);
    }
    /**
     * Get queue size
     * @returns {number}
     */
    queueSize() {
        const ret = wasm.swarmintelligence_queueSize(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set belief for a topic's decision
     * @param {string} topic
     * @param {bigint} decision_id
     * @param {number} probability
     */
    setBelief(topic, decision_id, probability) {
        const ptr0 = passStringToWasm0(topic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.swarmintelligence_setBelief(this.__wbg_ptr, ptr0, len0, decision_id, probability);
    }
    /**
     * Add pattern to collective memory
     * @param {string} pattern_json
     * @returns {boolean}
     */
    addPattern(pattern_json) {
        const ptr0 = passStringToWasm0(pattern_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.swarmintelligence_addPattern(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Run memory consolidation
     * @returns {number}
     */
    consolidate() {
        const ret = wasm.swarmintelligence_consolidate(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if topic has reached consensus
     * @param {string} topic
     * @returns {boolean}
     */
    hasConsensus(topic) {
        const ptr0 = passStringToWasm0(topic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.swarmintelligence_hasConsensus(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get collective memory pattern count
     * @returns {number}
     */
    patternCount() {
        const ret = wasm.swarmintelligence_patternCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Search collective memory
     * @param {string} query_json
     * @param {number} k
     * @returns {string}
     */
    searchPatterns(query_json, k) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.swarmintelligence_searchPatterns(this.__wbg_ptr, ptr0, len0, k);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Start a new consensus round for a topic
     * @param {string} topic
     * @param {number} threshold
     */
    startConsensus(topic, threshold) {
        const ptr0 = passStringToWasm0(topic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.swarmintelligence_startConsensus(this.__wbg_ptr, ptr0, len0, threshold);
    }
    /**
     * Negotiate beliefs for a topic
     * @param {string} topic
     * @param {string} beliefs_json
     * @returns {boolean}
     */
    negotiateBeliefs(topic, beliefs_json) {
        const ptr0 = passStringToWasm0(topic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(beliefs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.swarmintelligence_negotiateBeliefs(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Get consensus decision for topic
     * @param {string} topic
     * @returns {bigint | undefined}
     */
    getConsensusDecision(topic) {
        const ptr0 = passStringToWasm0(topic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.swarmintelligence_getConsensusDecision(this.__wbg_ptr, ptr0, len0);
        return ret[0] === 0 ? undefined : BigInt.asUintN(64, ret[1]);
    }
    /**
     * Create new swarm intelligence coordinator
     * @param {string} node_id
     */
    constructor(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.swarmintelligence_new(ptr0, len0);
        this.__wbg_ptr = ret >>> 0;
        SwarmIntelligenceFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Run hippocampal replay
     * @returns {number}
     */
    replay() {
        const ret = wasm.swarmintelligence_replay(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get node ID
     * @returns {string}
     */
    nodeId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.swarmintelligence_nodeId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get combined statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.swarmintelligence_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) SwarmIntelligence.prototype[Symbol.dispose] = SwarmIntelligence.prototype.free;
exports.SwarmIntelligence = SwarmIntelligence;

/**
 * Sybil resistance mechanisms
 */
class SybilDefense {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SybilDefenseFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sybildefense_free(ptr, 0);
    }
    /**
     * Register a node with its fingerprint
     * @param {string} node_id
     * @param {string} fingerprint
     * @returns {boolean}
     */
    registerNode(node_id, fingerprint) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(fingerprint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.sybildefense_registerNode(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Get sybil score (0.0 = likely unique, 1.0 = likely sybil)
     * @param {string} node_id
     * @returns {number}
     */
    getSybilScore(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sybildefense_getSybilScore(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Check if node is likely a sybil
     * @param {string} node_id
     * @returns {boolean}
     */
    isSuspectedSybil(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sybildefense_isSuspectedSybil(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.sybildefense_new();
        this.__wbg_ptr = ret >>> 0;
        SybilDefenseFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) SybilDefense.prototype[Symbol.dispose] = SybilDefense.prototype.free;
exports.SybilDefense = SybilDefense;

/**
 * Task priority levels
 * @enum {0 | 1 | 2}
 */
const TaskPriority = Object.freeze({
    Low: 0, "0": "Low",
    Normal: 1, "1": "Normal",
    High: 2, "2": "High",
});
exports.TaskPriority = TaskPriority;

/**
 * Task types supported by the network
 * @enum {0 | 1 | 2 | 3 | 4 | 5 | 6 | 7}
 */
const TaskType = Object.freeze({
    /**
     * Vector search in HNSW index
     */
    VectorSearch: 0, "0": "VectorSearch",
    /**
     * Vector insertion
     */
    VectorInsert: 1, "1": "VectorInsert",
    /**
     * Generate embeddings
     */
    Embedding: 2, "2": "Embedding",
    /**
     * Semantic task-to-agent matching
     */
    SemanticMatch: 3, "3": "SemanticMatch",
    /**
     * Neural network inference
     */
    NeuralInference: 4, "4": "NeuralInference",
    /**
     * AES encryption/decryption
     */
    Encryption: 5, "5": "Encryption",
    /**
     * Data compression
     */
    Compression: 6, "6": "Compression",
    /**
     * Custom WASM module (requires verification)
     */
    CustomWasm: 7, "7": "CustomWasm",
});
exports.TaskType = TaskType;

/**
 * TopK gradient sparsifier with error feedback for accuracy preservation
 *
 * Error feedback accumulates residuals from previous rounds to prevent
 * information loss from aggressive compression.
 */
class TopKSparsifier {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TopKSparsifierFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_topksparsifier_free(ptr, 0);
    }
    /**
     * Reset error feedback buffer
     */
    resetErrorFeedback() {
        wasm.topksparsifier_resetErrorFeedback(this.__wbg_ptr);
    }
    /**
     * Get compression ratio
     * @returns {number}
     */
    getCompressionRatio() {
        const ret = wasm.topksparsifier_getCompressionRatio(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get error feedback buffer size
     * @returns {number}
     */
    getErrorBufferSize() {
        const ret = wasm.topksparsifier_getErrorBufferSize(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new TopK sparsifier
     *
     * # Arguments
     * * `k_ratio` - Fraction of gradients to keep (0.1 = top 10%)
     * @param {number} k_ratio
     */
    constructor(k_ratio) {
        const ret = wasm.topksparsifier_new(k_ratio);
        this.__wbg_ptr = ret >>> 0;
        TopKSparsifierFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) TopKSparsifier.prototype[Symbol.dispose] = TopKSparsifier.prototype.free;
exports.TopKSparsifier = TopKSparsifier;

/**
 * Ring buffer tracker for task trajectories
 */
class TrajectoryTracker {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        TrajectoryTrackerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_trajectorytracker_free(ptr, 0);
    }
    /**
     * Create a new trajectory tracker
     * @param {number} max_size
     */
    constructor(max_size) {
        const ret = wasm.trajectorytracker_new(max_size);
        this.__wbg_ptr = ret >>> 0;
        TrajectoryTrackerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Get count of trajectories
     * @returns {number}
     */
    count() {
        const ret = wasm.trajectorytracker_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Record a new trajectory
     * @param {string} trajectory_json
     * @returns {boolean}
     */
    record(trajectory_json) {
        const ptr0 = passStringToWasm0(trajectory_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.trajectorytracker_record(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.trajectorytracker_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) TrajectoryTracker.prototype[Symbol.dispose] = TrajectoryTracker.prototype.free;
exports.TrajectoryTracker = TrajectoryTracker;

/**
 * WASM-compatible adapter pool wrapper
 */
class WasmAdapterPool {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmAdapterPoolFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmadapterpool_free(ptr, 0);
    }
    /**
     * Get or create an adapter for a task type
     * @param {string} task_type
     * @returns {any}
     */
    getAdapter(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmadapterpool_getAdapter(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get adapter count
     * @returns {number}
     */
    adapterCount() {
        const ret = wasm.wasmadapterpool_adapterCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Export adapter to bytes for P2P sharing
     * @param {string} task_type
     * @returns {Uint8Array}
     */
    exportAdapter(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmadapterpool_exportAdapter(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Import adapter from bytes
     * @param {string} task_type
     * @param {Uint8Array} bytes
     * @returns {boolean}
     */
    importAdapter(task_type, bytes) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmadapterpool_importAdapter(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Route to best adapter by task embedding
     * @param {Float32Array} task_embedding
     * @returns {any}
     */
    routeToAdapter(task_embedding) {
        const ptr0 = passArrayF32ToWasm0(task_embedding, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmadapterpool_routeToAdapter(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Create a new adapter pool
     * @param {number} hidden_dim
     * @param {number} max_slots
     */
    constructor(hidden_dim, max_slots) {
        const ret = wasm.wasmadapterpool_new(hidden_dim, max_slots);
        this.__wbg_ptr = ret >>> 0;
        WasmAdapterPoolFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Apply adapter to input
     * @param {string} task_type
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    forward(task_type, input) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(input, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmadapterpool_forward(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        var v3 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v3;
    }
    /**
     * Get pool statistics
     * @returns {any}
     */
    getStats() {
        const ret = wasm.wasmadapterpool_getStats(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) WasmAdapterPool.prototype[Symbol.dispose] = WasmAdapterPool.prototype.free;
exports.WasmAdapterPool = WasmAdapterPool;

/**
 * Unified interface for all exotic WASM capabilities
 */
class WasmCapabilities {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmCapabilitiesFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmcapabilities_free(ptr, 0);
    }
    /**
     * @returns {boolean}
     */
    enableHDC() {
        const ret = wasm.wasmcapabilities_enableHDC(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} _quorum
     * @returns {boolean}
     */
    enableNAO(_quorum) {
        const ret = wasm.wasmcapabilities_enableNAO(this.__wbg_ptr, _quorum);
        return ret !== 0;
    }
    /**
     * @param {number} _num_neurons
     * @param {number} _inhibition
     * @param {number} _threshold
     * @returns {boolean}
     */
    enableWTA(_num_neurons, _inhibition, _threshold) {
        const ret = wasm.wasmcapabilities_enableWTA(this.__wbg_ptr, _num_neurons, _inhibition, _threshold);
        return ret !== 0;
    }
    /**
     * @param {Float32Array} _activations
     * @returns {number}
     */
    competeWTA(_activations) {
        const ptr0 = passArrayF32ToWasm0(_activations, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_competeWTA(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} _input_dim
     * @param {number} _time_constant
     * @returns {boolean}
     */
    enableBTSP(_input_dim, _time_constant) {
        const ret = wasm.wasmcapabilities_enableBTSP(this.__wbg_ptr, _input_dim, _time_constant);
        return ret !== 0;
    }
    /**
     * @param {string} _proposal_id
     * @returns {boolean}
     */
    executeNAO(_proposal_id) {
        const ptr0 = passStringToWasm0(_proposal_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_executeNAO(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get a summary of all enabled capabilities
     * @returns {any}
     */
    getSummary() {
        const ret = wasm.wasmcapabilities_getSummary(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} _action
     * @returns {string}
     */
    proposeNAO(_action) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(_action, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmcapabilities_proposeNAO(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {Float32Array} _input
     * @returns {number}
     */
    forwardBTSP(_input) {
        const ptr0 = passArrayF32ToWasm0(_input, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_forwardBTSP(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * @returns {number}
     */
    getNAOSync() {
        const ret = wasm.wasmcapabilities_getNAOSync(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {string} _key
     * @param {number} _threshold
     * @returns {any}
     */
    retrieveHDC(_key, _threshold) {
        const ptr0 = passStringToWasm0(_key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_retrieveHDC(this.__wbg_ptr, ptr0, len0, _threshold);
        return ret;
    }
    /**
     * @param {string} _member_id
     * @param {bigint} _stake
     * @returns {boolean}
     */
    addNAOMember(_member_id, _stake) {
        const ptr0 = passStringToWasm0(_member_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_addNAOMember(this.__wbg_ptr, ptr0, len0, _stake);
        return ret !== 0;
    }
    /**
     * @param {string} _operator_type
     * @param {Float32Array} _gradient
     * @returns {boolean}
     */
    adaptMicroLoRA(_operator_type, _gradient) {
        const ptr0 = passStringToWasm0(_operator_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(_gradient, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_adaptMicroLoRA(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * @param {string} _operator_type
     * @param {Float32Array} input
     * @returns {Float32Array}
     */
    applyMicroLoRA(_operator_type, input) {
        const ptr0 = passStringToWasm0(_operator_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF32ToWasm0(input, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_applyMicroLoRA(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        var v3 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v3;
    }
    /**
     * List all available exotic capabilities
     * @returns {any}
     */
    getCapabilities() {
        const ret = wasm.wasmcapabilities_getCapabilities(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} _dim
     * @param {number} _rank
     * @returns {boolean}
     */
    enableMicroLoRA(_dim, _rank) {
        const ret = wasm.wasmcapabilities_enableMicroLoRA(this.__wbg_ptr, _dim, _rank);
        return ret !== 0;
    }
    /**
     * @returns {any}
     */
    tickTimeCrystal() {
        const ret = wasm.wasmcapabilities_tickTimeCrystal(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} _rate
     */
    growMorphogenetic(_rate) {
        wasm.wasmcapabilities_growMorphogenetic(this.__wbg_ptr, _rate);
    }
    /**
     * @param {Float32Array} _pattern
     * @param {number} _target
     * @returns {boolean}
     */
    oneShotAssociate(_pattern, _target) {
        const ptr0 = passArrayF32ToWasm0(_pattern, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_oneShotAssociate(this.__wbg_ptr, ptr0, len0, _target);
        return ret !== 0;
    }
    /**
     * @param {number} _oscillators
     * @param {number} _period_ms
     * @returns {boolean}
     */
    enableTimeCrystal(_oscillators, _period_ms) {
        const ret = wasm.wasmcapabilities_enableMicroLoRA(this.__wbg_ptr, _oscillators, _period_ms);
        return ret !== 0;
    }
    /**
     * @param {number} _threshold
     */
    pruneMorphogenetic(_threshold) {
        wasm.wasmcapabilities_growMorphogenetic(this.__wbg_ptr, _threshold);
    }
    /**
     * @param {number} _width
     * @param {number} _height
     * @returns {boolean}
     */
    enableMorphogenetic(_width, _height) {
        const ret = wasm.wasmcapabilities_enableMicroLoRA(this.__wbg_ptr, _width, _height);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    getTimeCrystalSync() {
        const ret = wasm.wasmcapabilities_getNAOSync(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {Float32Array} _content
     * @param {number} _salience
     * @param {number} _source_module
     * @returns {boolean}
     */
    broadcastToWorkspace(_content, _salience, _source_module) {
        const ptr0 = passArrayF32ToWasm0(_content, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_broadcastToWorkspace(this.__wbg_ptr, ptr0, len0, _salience, _source_module);
        return ret !== 0;
    }
    /**
     * @returns {any}
     */
    getWorkspaceContents() {
        const ret = wasm.wasmcapabilities_getWorkspaceContents(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    isTimeCrystalStable() {
        const ret = wasm.wasmcapabilities_getMorphogeneticCellCount(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} _capacity
     * @returns {boolean}
     */
    enableGlobalWorkspace(_capacity) {
        const ret = wasm.wasmcapabilities_enableGlobalWorkspace(this.__wbg_ptr, _capacity);
        return ret !== 0;
    }
    /**
     * @returns {any}
     */
    getMorphogeneticStats() {
        const ret = wasm.wasmcapabilities_getMorphogeneticStats(this.__wbg_ptr);
        return ret;
    }
    differentiateMorphogenetic() {
        wasm.wasmcapabilities_differentiateMorphogenetic(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    getMorphogeneticCellCount() {
        const ret = wasm.wasmcapabilities_getMorphogeneticCellCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a new capabilities manager for a node
     * @param {string} node_id
     */
    constructor(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_new(ptr0, len0);
        this.__wbg_ptr = ret >>> 0;
        WasmCapabilitiesFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Step all enabled capabilities forward (for main loop integration)
     * @param {number} dt
     */
    step(dt) {
        wasm.wasmcapabilities_growMorphogenetic(this.__wbg_ptr, dt);
    }
    /**
     * @param {number} _dt
     */
    tickNAO(_dt) {
        wasm.wasmcapabilities_growMorphogenetic(this.__wbg_ptr, _dt);
    }
    /**
     * @param {string} _proposal_id
     * @param {number} _weight
     * @returns {boolean}
     */
    voteNAO(_proposal_id, _weight) {
        const ptr0 = passStringToWasm0(_proposal_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_voteNAO(this.__wbg_ptr, ptr0, len0, _weight);
        return ret !== 0;
    }
    /**
     * @param {string} _key
     * @returns {boolean}
     */
    storeHDC(_key) {
        const ptr0 = passStringToWasm0(_key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcapabilities_executeNAO(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
}
if (Symbol.dispose) WasmCapabilities.prototype[Symbol.dispose] = WasmCapabilities.prototype.free;
exports.WasmCapabilities = WasmCapabilities;

/**
 * CRDT-based credit ledger for P2P consistency
 */
class WasmCreditLedger {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmCreditLedgerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmcreditledger_free(ptr, 0);
    }
    /**
     * Get total spent
     * @returns {bigint}
     */
    totalSpent() {
        const ret = wasm.wasmcreditledger_totalSpent(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Export spent counter for sync
     * @returns {Uint8Array}
     */
    exportSpent() {
        const ret = wasm.wasmcreditledger_exportSpent(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Get total earned (before spending)
     * @returns {bigint}
     */
    totalEarned() {
        const ret = wasm.wasmcreditledger_totalEarned(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Export earned counter for sync
     * @returns {Uint8Array}
     */
    exportEarned() {
        const ret = wasm.wasmcreditledger_exportEarned(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Get staked amount
     * @returns {bigint}
     */
    stakedAmount() {
        const ret = wasm.wasmcreditledger_stakedAmount(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Get network compute hours (for multiplier)
     * @returns {number}
     */
    networkCompute() {
        const ret = wasm.wasmcreditledger_networkCompute(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get current multiplier
     * @returns {number}
     */
    currentMultiplier() {
        const ret = wasm.wasmcreditledger_currentMultiplier(this.__wbg_ptr);
        return ret;
    }
    /**
     * Update network compute (from P2P sync)
     * @param {number} hours
     */
    updateNetworkCompute(hours) {
        wasm.wasmcreditledger_updateNetworkCompute(this.__wbg_ptr, hours);
    }
    /**
     * Create a new credit ledger
     * @param {string} node_id
     */
    constructor(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcreditledger_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmCreditLedgerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Merge with another ledger (CRDT merge) - optimized batch processing
     * @param {Uint8Array} other_earned
     * @param {Uint8Array} other_spent
     */
    merge(other_earned, other_spent) {
        const ptr0 = passArray8ToWasm0(other_earned, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(other_spent, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcreditledger_merge(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Slash staked credits (penalty for bad behavior)
     * @param {bigint} amount
     * @returns {bigint}
     */
    slash(amount) {
        const ret = wasm.wasmcreditledger_slash(this.__wbg_ptr, amount);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return BigInt.asUintN(64, ret[0]);
    }
    /**
     * Stake credits for participation
     * @param {bigint} amount
     */
    stake(amount) {
        const ret = wasm.wasmcreditledger_stake(this.__wbg_ptr, amount);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Credit the ledger (earn credits)
     * @param {bigint} amount
     * @param {string} reason
     */
    credit(amount, reason) {
        const ptr0 = passStringToWasm0(reason, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmcreditledger_credit(this.__wbg_ptr, amount, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Deduct from the ledger (spend credits)
     * @param {bigint} amount
     */
    deduct(amount) {
        const ret = wasm.wasmcreditledger_deduct(this.__wbg_ptr, amount);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Get current balance
     * @returns {bigint}
     */
    balance() {
        const ret = wasm.wasmcreditledger_balance(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * Unstake credits
     * @param {bigint} amount
     */
    unstake(amount) {
        const ret = wasm.wasmcreditledger_unstake(this.__wbg_ptr, amount);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) WasmCreditLedger.prototype[Symbol.dispose] = WasmCreditLedger.prototype.free;
exports.WasmCreditLedger = WasmCreditLedger;

/**
 * Idle detection and throttling
 */
class WasmIdleDetector {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmIdleDetectorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmidledetector_free(ptr, 0);
    }
    /**
     * Get status summary
     * @returns {any}
     */
    getStatus() {
        const ret = wasm.wasmidledetector_getStatus(this.__wbg_ptr);
        return ret;
    }
    /**
     * Update FPS measurement
     * @param {number} fps
     */
    updateFps(fps) {
        wasm.wasmidledetector_updateFps(this.__wbg_ptr, fps);
    }
    /**
     * Check if we should be working
     * @returns {boolean}
     */
    shouldWork() {
        const ret = wasm.wasmidledetector_shouldWork(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get current throttle level (0.0 - max_cpu)
     * @returns {number}
     */
    getThrottle() {
        const ret = wasm.wasmidledetector_getThrottle(this.__wbg_ptr);
        return ret;
    }
    /**
     * Record user interaction
     */
    recordInteraction() {
        wasm.wasmidledetector_recordInteraction(this.__wbg_ptr);
    }
    /**
     * Set battery status (called from JS)
     * @param {boolean} on_battery
     */
    setBatteryStatus(on_battery) {
        wasm.wasmidledetector_setBatteryStatus(this.__wbg_ptr, on_battery);
    }
    /**
     * Create a new idle detector
     * @param {number} max_cpu
     * @param {number} min_idle_time
     */
    constructor(max_cpu, min_idle_time) {
        const ret = wasm.wasmidledetector_new(max_cpu, min_idle_time);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmIdleDetectorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Stop monitoring
     */
    stop() {
        wasm.wasmidledetector_stop(this.__wbg_ptr);
    }
    /**
     * Pause contribution (user-initiated)
     */
    pause() {
        wasm.wasmidledetector_pause(this.__wbg_ptr);
    }
    /**
     * Start monitoring
     */
    start() {
        const ret = wasm.wasmidledetector_start(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Resume contribution
     */
    resume() {
        wasm.wasmidledetector_resume(this.__wbg_ptr);
    }
    /**
     * Check if user is idle
     * @returns {boolean}
     */
    isIdle() {
        const ret = wasm.wasmidledetector_isIdle(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) WasmIdleDetector.prototype[Symbol.dispose] = WasmIdleDetector.prototype.free;
exports.WasmIdleDetector = WasmIdleDetector;

/**
 * BroadcastChannel-based transport for multi-tab communication
 */
class WasmMcpBroadcast {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMcpBroadcastFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmcpbroadcast_free(ptr, 0);
    }
    /**
     * Set as server mode (responds to requests)
     * @param {WasmMcpServer} server
     */
    setServer(server) {
        _assertClass(server, WasmMcpServer);
        var ptr0 = server.__destroy_into_raw();
        wasm.wasmmcpbroadcast_setServer(this.__wbg_ptr, ptr0);
    }
    /**
     * Create a broadcast transport
     * @param {string} channel_name
     */
    constructor(channel_name) {
        const ptr0 = passStringToWasm0(channel_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmmcpbroadcast_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmMcpBroadcastFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Send a request (client mode)
     * @param {string} request_json
     */
    send(request_json) {
        const ptr0 = passStringToWasm0(request_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmmcpbroadcast_send(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Close the channel
     */
    close() {
        wasm.wasmmcpbroadcast_close(this.__wbg_ptr);
    }
    /**
     * Start listening for requests (server mode)
     */
    listen() {
        const ret = wasm.wasmmcpbroadcast_listen(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) WasmMcpBroadcast.prototype[Symbol.dispose] = WasmMcpBroadcast.prototype.free;
exports.WasmMcpBroadcast = WasmMcpBroadcast;

/**
 * Browser-based MCP server for edge-net
 *
 * Provides Model Context Protocol interface over MessagePort or direct calls.
 * All edge-net capabilities are exposed as MCP tools.
 */
class WasmMcpServer {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmMcpServer.prototype);
        obj.__wbg_ptr = ptr;
        WasmMcpServerFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMcpServerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmcpserver_free(ptr, 0);
    }
    /**
     * Create with custom configuration
     * @param {any} config
     * @returns {WasmMcpServer}
     */
    static withConfig(config) {
        const ret = wasm.wasmmcpserver_withConfig(config);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return WasmMcpServer.__wrap(ret[0]);
    }
    /**
     * Set identity for authenticated operations
     * @param {WasmNodeIdentity} identity
     */
    setIdentity(identity) {
        _assertClass(identity, WasmNodeIdentity);
        var ptr0 = identity.__destroy_into_raw();
        wasm.wasmmcpserver_setIdentity(this.__wbg_ptr, ptr0);
    }
    /**
     * Initialize learning engine
     */
    initLearning() {
        const ret = wasm.wasmmcpserver_initLearning(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Handle an MCP request (JSON string)
     * @param {string} request_json
     * @returns {Promise<string>}
     */
    handleRequest(request_json) {
        const ptr0 = passStringToWasm0(request_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmmcpserver_handleRequest(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get server info
     * @returns {any}
     */
    getServerInfo() {
        const ret = wasm.wasmmcpserver_getServerInfo(this.__wbg_ptr);
        return ret;
    }
    /**
     * Handle MCP request from JsValue (for direct JS calls)
     * @param {any} request
     * @returns {Promise<any>}
     */
    handleRequestJs(request) {
        const ret = wasm.wasmmcpserver_handleRequestJs(this.__wbg_ptr, request);
        return ret;
    }
    /**
     * Create a new MCP server with default configuration
     */
    constructor() {
        const ret = wasm.wasmmcpserver_new();
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmMcpServerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmMcpServer.prototype[Symbol.dispose] = WasmMcpServer.prototype.free;
exports.WasmMcpServer = WasmMcpServer;

/**
 * Browser-based MCP transport using MessagePort
 */
class WasmMcpTransport {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmMcpTransport.prototype);
        obj.__wbg_ptr = ptr;
        WasmMcpTransportFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMcpTransportFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmcptransport_free(ptr, 0);
    }
    /**
     * Create transport from a Worker
     * @param {Worker} worker
     */
    constructor(worker) {
        const ret = wasm.wasmmcptransport_new(worker);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmMcpTransportFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Initialize transport (set up message handler)
     */
    init() {
        const ret = wasm.wasmmcptransport_init(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Send an MCP request and get a Promise for the response
     * @param {any} request
     * @returns {Promise<any>}
     */
    send(request) {
        const ret = wasm.wasmmcptransport_send(this.__wbg_ptr, request);
        return ret;
    }
    /**
     * Close the transport
     */
    close() {
        wasm.wasmmcptransport_close(this.__wbg_ptr);
    }
    /**
     * Create transport from existing MessagePort
     * @param {MessagePort} port
     * @returns {WasmMcpTransport}
     */
    static fromPort(port) {
        const ret = wasm.wasmmcptransport_fromPort(port);
        return WasmMcpTransport.__wrap(ret);
    }
}
if (Symbol.dispose) WasmMcpTransport.prototype[Symbol.dispose] = WasmMcpTransport.prototype.free;
exports.WasmMcpTransport = WasmMcpTransport;

/**
 * Worker-side handler for MCP requests
 */
class WasmMcpWorkerHandler {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmMcpWorkerHandlerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmmcpworkerhandler_free(ptr, 0);
    }
    /**
     * Create handler with MCP server
     * @param {WasmMcpServer} server
     */
    constructor(server) {
        _assertClass(server, WasmMcpServer);
        var ptr0 = server.__destroy_into_raw();
        const ret = wasm.wasmmcpworkerhandler_new(ptr0);
        this.__wbg_ptr = ret >>> 0;
        WasmMcpWorkerHandlerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Start handling messages (call in worker)
     */
    start() {
        const ret = wasm.wasmmcpworkerhandler_start(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) WasmMcpWorkerHandler.prototype[Symbol.dispose] = WasmMcpWorkerHandler.prototype.free;
exports.WasmMcpWorkerHandler = WasmMcpWorkerHandler;

/**
 * P2P network manager
 */
class WasmNetworkManager {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmNetworkManagerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmnetworkmanager_free(ptr, 0);
    }
    /**
     * Get peer count
     * @returns {number}
     */
    peerCount() {
        const ret = wasm.wasmnetworkmanager_peerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if connected
     * @returns {boolean}
     */
    isConnected() {
        const ret = wasm.wasmnetworkmanager_isConnected(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Register a peer
     * @param {string} node_id
     * @param {Uint8Array} pubkey
     * @param {string[]} capabilities
     * @param {bigint} stake
     */
    registerPeer(node_id, pubkey, capabilities, stake) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(pubkey, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayJsValueToWasm0(capabilities, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.wasmnetworkmanager_registerPeer(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, stake);
    }
    /**
     * Select workers for task execution (reputation-weighted random)
     * @param {string} capability
     * @param {number} count
     * @returns {string[]}
     */
    selectWorkers(capability, count) {
        const ptr0 = passStringToWasm0(capability, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnetworkmanager_selectWorkers(this.__wbg_ptr, ptr0, len0, count);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Get active peer count (seen in last 60s)
     * @returns {number}
     */
    activePeerCount() {
        const ret = wasm.wasmnetworkmanager_activePeerCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Update peer reputation
     * @param {string} node_id
     * @param {number} delta
     */
    updateReputation(node_id, delta) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmnetworkmanager_updateReputation(this.__wbg_ptr, ptr0, len0, delta);
    }
    /**
     * Get peers with specific capability
     * @param {string} capability
     * @returns {string[]}
     */
    getPeersWithCapability(capability) {
        const ptr0 = passStringToWasm0(capability, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnetworkmanager_getPeersWithCapability(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * @param {string} node_id
     */
    constructor(node_id) {
        const ptr0 = passStringToWasm0(node_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnetworkmanager_new(ptr0, len0);
        this.__wbg_ptr = ret >>> 0;
        WasmNetworkManagerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Add a relay URL
     * @param {string} url
     */
    addRelay(url) {
        const ptr0 = passStringToWasm0(url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmnetworkmanager_addRelay(this.__wbg_ptr, ptr0, len0);
    }
}
if (Symbol.dispose) WasmNetworkManager.prototype[Symbol.dispose] = WasmNetworkManager.prototype.free;
exports.WasmNetworkManager = WasmNetworkManager;

/**
 * Node identity with Ed25519 keypair
 */
class WasmNodeIdentity {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmNodeIdentity.prototype);
        obj.__wbg_ptr = ptr;
        WasmNodeIdentityFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmNodeIdentityFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmnodeidentity_free(ptr, 0);
    }
    /**
     * Verify a signature from another node
     * @param {Uint8Array} public_key
     * @param {Uint8Array} message
     * @param {Uint8Array} signature
     * @returns {boolean}
     */
    static verifyFrom(public_key, message, signature) {
        const ptr0 = passArray8ToWasm0(public_key, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArray8ToWasm0(signature, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_verifyFrom(ptr0, len0, ptr1, len1, ptr2, len2);
        return ret !== 0;
    }
    /**
     * Get the public key as hex string
     * @returns {string}
     */
    publicKeyHex() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmnodeidentity_publicKeyHex(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Restore identity from secret key bytes
     * @param {Uint8Array} secret_key
     * @param {string} site_id
     * @returns {WasmNodeIdentity}
     */
    static fromSecretKey(secret_key, site_id) {
        const ptr0 = passArray8ToWasm0(secret_key, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(site_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_fromSecretKey(ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return WasmNodeIdentity.__wrap(ret[0]);
    }
    /**
     * Get browser fingerprint
     * @returns {string | undefined}
     */
    getFingerprint() {
        const ret = wasm.wasmnodeidentity_getFingerprint(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Set browser fingerprint for anti-sybil
     * @param {string} fingerprint
     */
    setFingerprint(fingerprint) {
        const ptr0 = passStringToWasm0(fingerprint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmnodeidentity_setFingerprint(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get the public key as bytes
     * @returns {Uint8Array}
     */
    publicKeyBytes() {
        const ret = wasm.wasmnodeidentity_publicKeyBytes(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Export secret key encrypted with password (secure backup)
     * Uses Argon2id for key derivation and AES-256-GCM for encryption
     * @param {string} password
     * @returns {Uint8Array}
     */
    exportSecretKey(password) {
        const ptr0 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_exportSecretKey(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Import secret key from encrypted backup
     * @param {Uint8Array} encrypted
     * @param {string} password
     * @param {string} site_id
     * @returns {WasmNodeIdentity}
     */
    static importSecretKey(encrypted, password, site_id) {
        const ptr0 = passArray8ToWasm0(encrypted, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(password, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(site_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_importSecretKey(ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return WasmNodeIdentity.__wrap(ret[0]);
    }
    /**
     * Sign a message
     * @param {Uint8Array} message
     * @returns {Uint8Array}
     */
    sign(message) {
        const ptr0 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_sign(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Verify a signature
     * @param {Uint8Array} message
     * @param {Uint8Array} signature
     * @returns {boolean}
     */
    verify(message, signature) {
        const ptr0 = passArray8ToWasm0(message, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(signature, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_verify(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Get the node's unique identifier
     * @returns {string}
     */
    nodeId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmnodeidentity_nodeId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the site ID
     * @returns {string}
     */
    siteId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmnodeidentity_siteId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Generate a new node identity
     * @param {string} site_id
     * @returns {WasmNodeIdentity}
     */
    static generate(site_id) {
        const ptr0 = passStringToWasm0(site_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmnodeidentity_generate(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return WasmNodeIdentity.__wrap(ret[0]);
    }
}
if (Symbol.dispose) WasmNodeIdentity.prototype[Symbol.dispose] = WasmNodeIdentity.prototype.free;
exports.WasmNodeIdentity = WasmNodeIdentity;

/**
 * WASM-bindgen wrapper for stigmergy coordination
 */
class WasmStigmergy {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmStigmergy.prototype);
        obj.__wbg_ptr = ptr;
        WasmStigmergyFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmStigmergyFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmstigmergy_free(ptr, 0);
    }
    /**
     * Create with custom parameters
     * @param {number} decay_rate
     * @param {number} deposit_rate
     * @param {number} evaporation_hours
     * @returns {WasmStigmergy}
     */
    static withParams(decay_rate, deposit_rate, evaporation_hours) {
        const ret = wasm.wasmstigmergy_withParams(decay_rate, deposit_rate, evaporation_hours);
        return WasmStigmergy.__wrap(ret);
    }
    /**
     * Export current state for P2P sharing
     * @returns {string}
     */
    exportState() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmstigmergy_exportState(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get raw pheromone intensity
     * @param {string} task_type
     * @returns {number}
     */
    getIntensity(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_getIntensity(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Set minimum stake for anti-sybil
     * @param {bigint} min_stake
     */
    setMinStake(min_stake) {
        wasm.wasmstigmergy_setMinStake(this.__wbg_ptr, min_stake);
    }
    /**
     * Should this node accept a task? (combined decision)
     * @param {string} task_type
     * @returns {number}
     */
    shouldAccept(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_shouldAccept(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Check and run evaporation if due
     * @returns {boolean}
     */
    maybeEvaporate() {
        const ret = wasm.wasmstigmergy_maybeEvaporate(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get all task types ranked by attractiveness
     * @returns {string}
     */
    getRankedTasks() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmstigmergy_getRankedTasks(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get success rate for a task type
     * @param {string} task_type
     * @returns {number}
     */
    getSuccessRate(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_getSuccessRate(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get node's specialization score
     * @param {string} task_type
     * @returns {number}
     */
    getSpecialization(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_getSpecialization(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Deposit with success/failure outcome
     * @param {string} task_type
     * @param {string} peer_id
     * @param {boolean} success
     * @param {bigint} stake
     */
    depositWithOutcome(task_type, peer_id, success, stake) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(peer_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.wasmstigmergy_depositWithOutcome(this.__wbg_ptr, ptr0, len0, ptr1, len1, success, stake);
    }
    /**
     * Update node specialization based on outcome
     * @param {string} task_type
     * @param {boolean} success
     */
    updateSpecialization(task_type, success) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmstigmergy_updateSpecialization(this.__wbg_ptr, ptr0, len0, success);
    }
    /**
     * Get best specialization recommendation
     * @returns {string | undefined}
     */
    getBestSpecialization() {
        const ret = wasm.wasmstigmergy_getBestSpecialization(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Create a new stigmergy engine
     */
    constructor() {
        const ret = wasm.wasmstigmergy_new();
        this.__wbg_ptr = ret >>> 0;
        WasmStigmergyFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Merge peer pheromone state (JSON format)
     * @param {string} peer_state_json
     * @returns {boolean}
     */
    merge(peer_state_json) {
        const ptr0 = passStringToWasm0(peer_state_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_merge(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Get acceptance probability for a task type
     * @param {string} task_type
     * @returns {number}
     */
    follow(task_type) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstigmergy_follow(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Deposit pheromone after task completion
     * @param {string} task_type
     * @param {string} peer_id
     * @param {number} success_rate
     * @param {bigint} stake
     */
    deposit(task_type, peer_id, success_rate, stake) {
        const ptr0 = passStringToWasm0(task_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(peer_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.wasmstigmergy_deposit(this.__wbg_ptr, ptr0, len0, ptr1, len1, success_rate, stake);
    }
    /**
     * Run evaporation (call periodically)
     */
    evaporate() {
        wasm.wasmstigmergy_evaporate(this.__wbg_ptr);
    }
    /**
     * Get statistics as JSON
     * @returns {string}
     */
    getStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmstigmergy_getStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) WasmStigmergy.prototype[Symbol.dispose] = WasmStigmergy.prototype.free;
exports.WasmStigmergy = WasmStigmergy;

/**
 * Sandboxed task executor
 */
class WasmTaskExecutor {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmTaskExecutorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmtaskexecutor_free(ptr, 0);
    }
    /**
     * Set encryption key for payload decryption
     * @param {Uint8Array} key
     */
    setTaskKey(key) {
        const ptr0 = passArray8ToWasm0(key, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmtaskexecutor_setTaskKey(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Create a new task executor
     * @param {number} max_memory
     */
    constructor(max_memory) {
        const ret = wasm.wasmtaskexecutor_new(max_memory);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        WasmTaskExecutorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmTaskExecutor.prototype[Symbol.dispose] = WasmTaskExecutor.prototype.free;
exports.WasmTaskExecutor = WasmTaskExecutor;

/**
 * Task queue for P2P distribution - optimized with priority heap
 */
class WasmTaskQueue {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmTaskQueueFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmtaskqueue_free(ptr, 0);
    }
}
if (Symbol.dispose) WasmTaskQueue.prototype[Symbol.dispose] = WasmTaskQueue.prototype.free;
exports.WasmTaskQueue = WasmTaskQueue;

/**
 * Work scheduler for distributing compute across frames
 */
class WasmWorkScheduler {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmWorkSchedulerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmworkscheduler_free(ptr, 0);
    }
    /**
     * Calculate how many tasks to run this frame
     * @param {number} throttle
     * @returns {number}
     */
    tasksThisFrame(throttle) {
        const ret = wasm.wasmworkscheduler_tasksThisFrame(this.__wbg_ptr, throttle);
        return ret >>> 0;
    }
    /**
     * Set pending task count
     * @param {number} count
     */
    setPendingTasks(count) {
        wasm.wasmworkscheduler_setPendingTasks(this.__wbg_ptr, count);
    }
    /**
     * Record task completion for averaging
     * @param {number} duration_ms
     */
    recordTaskDuration(duration_ms) {
        wasm.wasmworkscheduler_recordTaskDuration(this.__wbg_ptr, duration_ms);
    }
    constructor() {
        const ret = wasm.wasmworkscheduler_new();
        this.__wbg_ptr = ret >>> 0;
        WasmWorkSchedulerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WasmWorkScheduler.prototype[Symbol.dispose] = WasmWorkScheduler.prototype.free;
exports.WasmWorkScheduler = WasmWorkScheduler;

/**
 * Manages witness tracking for claims
 */
class WitnessTracker {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WitnessTrackerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_witnesstracker_free(ptr, 0);
    }
    /**
     * Get witness count for a claim
     * @param {string} claim_id
     * @returns {number}
     */
    witnessCount(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.witnesstracker_witnessCount(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Get confidence score based on witness diversity
     * @param {string} claim_id
     * @returns {number}
     */
    witnessConfidence(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.witnesstracker_witnessConfidence(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Check if claim has sufficient independent witnesses
     * @param {string} claim_id
     * @returns {boolean}
     */
    hasSufficientWitnesses(claim_id) {
        const ptr0 = passStringToWasm0(claim_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.witnesstracker_hasSufficientWitnesses(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Create a new witness tracker
     * @param {number} min_witnesses
     */
    constructor(min_witnesses) {
        const ret = wasm.witnesstracker_new(min_witnesses);
        this.__wbg_ptr = ret >>> 0;
        WitnessTrackerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) WitnessTracker.prototype[Symbol.dispose] = WitnessTracker.prototype.free;
exports.WitnessTracker = WitnessTracker;

/**
 * Initialize panic hook for better error messages in console
 */
function init_panic_hook() {
    wasm.init_panic_hook();
}
exports.init_panic_hook = init_panic_hook;

exports.__wbg_Error_52673b7de5a0ca89 = function(arg0, arg1) {
    const ret = Error(getStringFromWasm0(arg0, arg1));
    return ret;
};

exports.__wbg_Number_2d1dcfcf4ec51736 = function(arg0) {
    const ret = Number(arg0);
    return ret;
};

exports.__wbg_String_8f0eb39a4a4c2f66 = function(arg0, arg1) {
    const ret = String(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
};

exports.__wbg___wbindgen_bigint_get_as_i64_6e32f5e6aff02e1d = function(arg0, arg1) {
    const v = arg1;
    const ret = typeof(v) === 'bigint' ? v : undefined;
    getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
};

exports.__wbg___wbindgen_boolean_get_dea25b33882b895b = function(arg0) {
    const v = arg0;
    const ret = typeof(v) === 'boolean' ? v : undefined;
    return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
};

exports.__wbg___wbindgen_debug_string_adfb662ae34724b6 = function(arg0, arg1) {
    const ret = debugString(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
};

exports.__wbg___wbindgen_in_0d3e1e8f0c669317 = function(arg0, arg1) {
    const ret = arg0 in arg1;
    return ret;
};

exports.__wbg___wbindgen_is_bigint_0e1a2e3f55cfae27 = function(arg0) {
    const ret = typeof(arg0) === 'bigint';
    return ret;
};

exports.__wbg___wbindgen_is_function_8d400b8b1af978cd = function(arg0) {
    const ret = typeof(arg0) === 'function';
    return ret;
};

exports.__wbg___wbindgen_is_object_ce774f3490692386 = function(arg0) {
    const val = arg0;
    const ret = typeof(val) === 'object' && val !== null;
    return ret;
};

exports.__wbg___wbindgen_is_string_704ef9c8fc131030 = function(arg0) {
    const ret = typeof(arg0) === 'string';
    return ret;
};

exports.__wbg___wbindgen_is_undefined_f6b95eab589e0269 = function(arg0) {
    const ret = arg0 === undefined;
    return ret;
};

exports.__wbg___wbindgen_jsval_eq_b6101cc9cef1fe36 = function(arg0, arg1) {
    const ret = arg0 === arg1;
    return ret;
};

exports.__wbg___wbindgen_jsval_loose_eq_766057600fdd1b0d = function(arg0, arg1) {
    const ret = arg0 == arg1;
    return ret;
};

exports.__wbg___wbindgen_number_get_9619185a74197f95 = function(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'number' ? obj : undefined;
    getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
};

exports.__wbg___wbindgen_string_get_a2a31e16edf96e42 = function(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'string' ? obj : undefined;
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
};

exports.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
    throw new Error(getStringFromWasm0(arg0, arg1));
};

exports.__wbg__wbg_cb_unref_87dfb5aaa0cbcea7 = function(arg0) {
    arg0._wbg_cb_unref();
};

exports.__wbg_call_3020136f7a2d6e44 = function() { return handleError(function (arg0, arg1, arg2) {
    const ret = arg0.call(arg1, arg2);
    return ret;
}, arguments) };

exports.__wbg_call_abb4ff46ce38be40 = function() { return handleError(function (arg0, arg1) {
    const ret = arg0.call(arg1);
    return ret;
}, arguments) };

exports.__wbg_close_8158530fc398ee2f = function(arg0) {
    arg0.close();
};

exports.__wbg_close_c956ddbf0426a990 = function(arg0) {
    arg0.close();
};

exports.__wbg_crypto_574e78ad8b13b65f = function(arg0) {
    const ret = arg0.crypto;
    return ret;
};

exports.__wbg_data_8bf4ae669a78a688 = function(arg0) {
    const ret = arg0.data;
    return ret;
};

exports.__wbg_done_62ea16af4ce34b24 = function(arg0) {
    const ret = arg0.done;
    return ret;
};

exports.__wbg_entries_83c79938054e065f = function(arg0) {
    const ret = Object.entries(arg0);
    return ret;
};

exports.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
    let deferred0_0;
    let deferred0_1;
    try {
        deferred0_0 = arg0;
        deferred0_1 = arg1;
        console.error(getStringFromWasm0(arg0, arg1));
    } finally {
        wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
    }
};

exports.__wbg_getDate_b8071ea9fc4f6838 = function(arg0) {
    const ret = arg0.getDate();
    return ret;
};

exports.__wbg_getDay_c13a50561112f77a = function(arg0) {
    const ret = arg0.getDay();
    return ret;
};

exports.__wbg_getMonth_48a392071f9e5017 = function(arg0) {
    const ret = arg0.getMonth();
    return ret;
};

exports.__wbg_getRandomValues_9b655bdd369112f2 = function() { return handleError(function (arg0, arg1) {
    globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
}, arguments) };

exports.__wbg_getRandomValues_b8f5dbd5f3995a9e = function() { return handleError(function (arg0, arg1) {
    arg0.getRandomValues(arg1);
}, arguments) };

exports.__wbg_getTimezoneOffset_45389e26d6f46823 = function(arg0) {
    const ret = arg0.getTimezoneOffset();
    return ret;
};

exports.__wbg_get_6b7bd52aca3f9671 = function(arg0, arg1) {
    const ret = arg0[arg1 >>> 0];
    return ret;
};

exports.__wbg_get_af9dab7e9603ea93 = function() { return handleError(function (arg0, arg1) {
    const ret = Reflect.get(arg0, arg1);
    return ret;
}, arguments) };

exports.__wbg_get_with_ref_key_1dc361bd10053bfe = function(arg0, arg1) {
    const ret = arg0[arg1];
    return ret;
};

exports.__wbg_hardwareConcurrency_11023a850a093b20 = function(arg0) {
    const ret = arg0.hardwareConcurrency;
    return ret;
};

exports.__wbg_height_5405e57b18dddece = function() { return handleError(function (arg0) {
    const ret = arg0.height;
    return ret;
}, arguments) };

exports.__wbg_instanceof_ArrayBuffer_f3320d2419cd0355 = function(arg0) {
    let result;
    try {
        result = arg0 instanceof ArrayBuffer;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
};

exports.__wbg_instanceof_Map_084be8da74364158 = function(arg0) {
    let result;
    try {
        result = arg0 instanceof Map;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
};

exports.__wbg_instanceof_MessagePort_c6d647a8cffdd1a6 = function(arg0) {
    let result;
    try {
        result = arg0 instanceof MessagePort;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
};

exports.__wbg_instanceof_Uint8Array_da54ccc9d3e09434 = function(arg0) {
    let result;
    try {
        result = arg0 instanceof Uint8Array;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
};

exports.__wbg_instanceof_Window_b5cf7783caa68180 = function(arg0) {
    let result;
    try {
        result = arg0 instanceof Window;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
};

exports.__wbg_isArray_51fd9e6422c0a395 = function(arg0) {
    const ret = Array.isArray(arg0);
    return ret;
};

exports.__wbg_isSafeInteger_ae7d3f054d55fa16 = function(arg0) {
    const ret = Number.isSafeInteger(arg0);
    return ret;
};

exports.__wbg_iterator_27b7c8b35ab3e86b = function() {
    const ret = Symbol.iterator;
    return ret;
};

exports.__wbg_language_763ea76470ed849b = function(arg0, arg1) {
    const ret = arg1.language;
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
};

exports.__wbg_length_22ac23eaec9d8053 = function(arg0) {
    const ret = arg0.length;
    return ret;
};

exports.__wbg_length_d45040a40c570362 = function(arg0) {
    const ret = arg0.length;
    return ret;
};

exports.__wbg_msCrypto_a61aeb35a24c1329 = function(arg0) {
    const ret = arg0.msCrypto;
    return ret;
};

exports.__wbg_navigator_b49edef831236138 = function(arg0) {
    const ret = arg0.navigator;
    return ret;
};

exports.__wbg_new_0_23cedd11d9b40c9d = function() {
    const ret = new Date();
    return ret;
};

exports.__wbg_new_137453588c393c59 = function() { return handleError(function () {
    const ret = new MessageChannel();
    return ret;
}, arguments) };

exports.__wbg_new_1ba21ce319a06297 = function() {
    const ret = new Object();
    return ret;
};

exports.__wbg_new_25f239778d6112b9 = function() {
    const ret = new Array();
    return ret;
};

exports.__wbg_new_6421f6084cc5bc5a = function(arg0) {
    const ret = new Uint8Array(arg0);
    return ret;
};

exports.__wbg_new_8a6f238a6ece86ea = function() {
    const ret = new Error();
    return ret;
};

exports.__wbg_new_b2db8aa2650f793a = function(arg0) {
    const ret = new Date(arg0);
    return ret;
};

exports.__wbg_new_b3dd747604c3c93e = function() { return handleError(function (arg0, arg1) {
    const ret = new BroadcastChannel(getStringFromWasm0(arg0, arg1));
    return ret;
}, arguments) };

exports.__wbg_new_b546ae120718850e = function() {
    const ret = new Map();
    return ret;
};

exports.__wbg_new_ff12d2b041fb48f1 = function(arg0, arg1) {
    try {
        var state0 = {a: arg0, b: arg1};
        var cb0 = (arg0, arg1) => {
            const a = state0.a;
            state0.a = 0;
            try {
                return wasm_bindgen__convert__closures_____invoke__h094c87b54a975e5a(a, state0.b, arg0, arg1);
            } finally {
                state0.a = a;
            }
        };
        const ret = new Promise(cb0);
        return ret;
    } finally {
        state0.a = state0.b = 0;
    }
};

exports.__wbg_new_no_args_cb138f77cf6151ee = function(arg0, arg1) {
    const ret = new Function(getStringFromWasm0(arg0, arg1));
    return ret;
};

exports.__wbg_new_with_length_aa5eaf41d35235e5 = function(arg0) {
    const ret = new Uint8Array(arg0 >>> 0);
    return ret;
};

exports.__wbg_next_138a17bbf04e926c = function(arg0) {
    const ret = arg0.next;
    return ret;
};

exports.__wbg_next_3cfe5c0fe2a4cc53 = function() { return handleError(function (arg0) {
    const ret = arg0.next();
    return ret;
}, arguments) };

exports.__wbg_node_905d3e251edff8a2 = function(arg0) {
    const ret = arg0.node;
    return ret;
};

exports.__wbg_now_69d776cd24f5215b = function() {
    const ret = Date.now();
    return ret;
};

exports.__wbg_platform_c9dd29375c0e6694 = function() { return handleError(function (arg0, arg1) {
    const ret = arg1.platform;
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}, arguments) };

exports.__wbg_port1_75dce9d0d8087125 = function(arg0) {
    const ret = arg0.port1;
    return ret;
};

exports.__wbg_port2_3cffa4119380f41d = function(arg0) {
    const ret = arg0.port2;
    return ret;
};

exports.__wbg_postMessage_79f844174f56304f = function() { return handleError(function (arg0, arg1) {
    arg0.postMessage(arg1);
}, arguments) };

exports.__wbg_postMessage_e0309b53c7ad30e6 = function() { return handleError(function (arg0, arg1, arg2) {
    arg0.postMessage(arg1, arg2);
}, arguments) };

exports.__wbg_postMessage_ee7b4e76cd1ed685 = function() { return handleError(function (arg0, arg1) {
    arg0.postMessage(arg1);
}, arguments) };

exports.__wbg_process_dc0fbacc7c1c06f7 = function(arg0) {
    const ret = arg0.process;
    return ret;
};

exports.__wbg_prototypesetcall_dfe9b766cdc1f1fd = function(arg0, arg1, arg2) {
    Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
};

exports.__wbg_push_7d9be8f38fc13975 = function(arg0, arg1) {
    const ret = arg0.push(arg1);
    return ret;
};

exports.__wbg_queueMicrotask_9b549dfce8865860 = function(arg0) {
    const ret = arg0.queueMicrotask;
    return ret;
};

exports.__wbg_queueMicrotask_fca69f5bfad613a5 = function(arg0) {
    queueMicrotask(arg0);
};

exports.__wbg_randomFillSync_ac0988aba3254290 = function() { return handleError(function (arg0, arg1) {
    arg0.randomFillSync(arg1);
}, arguments) };

exports.__wbg_random_cc1f9237d866d212 = function() {
    const ret = Math.random();
    return ret;
};

exports.__wbg_require_60cc747a6bc5215a = function() { return handleError(function () {
    const ret = module.require;
    return ret;
}, arguments) };

exports.__wbg_resolve_fd5bfbaa4ce36e1e = function(arg0) {
    const ret = Promise.resolve(arg0);
    return ret;
};

exports.__wbg_screen_7c5162a9a6fa46ee = function() { return handleError(function (arg0) {
    const ret = arg0.screen;
    return ret;
}, arguments) };

exports.__wbg_set_3f1d0b984ed272ed = function(arg0, arg1, arg2) {
    arg0[arg1] = arg2;
};

exports.__wbg_set_781438a03c0c3c81 = function() { return handleError(function (arg0, arg1, arg2) {
    const ret = Reflect.set(arg0, arg1, arg2);
    return ret;
}, arguments) };

exports.__wbg_set_7df433eea03a5c14 = function(arg0, arg1, arg2) {
    arg0[arg1 >>> 0] = arg2;
};

exports.__wbg_set_efaaf145b9377369 = function(arg0, arg1, arg2) {
    const ret = arg0.set(arg1, arg2);
    return ret;
};

exports.__wbg_set_onmessage_6fa00f5d8f1c055a = function(arg0, arg1) {
    arg0.onmessage = arg1;
};

exports.__wbg_set_onmessage_f0d5bf805190d1d8 = function(arg0, arg1) {
    arg0.onmessage = arg1;
};

exports.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
    const ret = arg1.stack;
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
};

exports.__wbg_start_dd05b3be5674e9f3 = function(arg0) {
    arg0.start();
};

exports.__wbg_static_accessor_GLOBAL_769e6b65d6557335 = function() {
    const ret = typeof global === 'undefined' ? null : global;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

exports.__wbg_static_accessor_GLOBAL_THIS_60cf02db4de8e1c1 = function() {
    const ret = typeof globalThis === 'undefined' ? null : globalThis;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

exports.__wbg_static_accessor_SELF_08f5a74c69739274 = function() {
    const ret = typeof self === 'undefined' ? null : self;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

exports.__wbg_static_accessor_WINDOW_a8924b26aa92d024 = function() {
    const ret = typeof window === 'undefined' ? null : window;
    return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
};

exports.__wbg_subarray_845f2f5bce7d061a = function(arg0, arg1, arg2) {
    const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
    return ret;
};

exports.__wbg_then_4f95312d68691235 = function(arg0, arg1) {
    const ret = arg0.then(arg1);
    return ret;
};

exports.__wbg_value_57b7b035e117f7ee = function(arg0) {
    const ret = arg0.value;
    return ret;
};

exports.__wbg_versions_c01dfd4722a88165 = function(arg0) {
    const ret = arg0.versions;
    return ret;
};

exports.__wbg_width_b8c97f5d3a7f759c = function() { return handleError(function (arg0) {
    const ret = arg0.width;
    return ret;
}, arguments) };

exports.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
    // Cast intrinsic for `Ref(String) -> Externref`.
    const ret = getStringFromWasm0(arg0, arg1);
    return ret;
};

exports.__wbindgen_cast_4625c577ab2ec9ee = function(arg0) {
    // Cast intrinsic for `U64 -> Externref`.
    const ret = BigInt.asUintN(64, arg0);
    return ret;
};

exports.__wbindgen_cast_46d6ccd6e2a13afa = function(arg0, arg1) {
    // Cast intrinsic for `Closure(Closure { dtor_idx: 1, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 2, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
    const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h16844f6554aa4052, wasm_bindgen__convert__closures_____invoke__h8c81ca6cba4eba00);
    return ret;
};

exports.__wbindgen_cast_6ad6aa2864ac3163 = function(arg0, arg1) {
    // Cast intrinsic for `Closure(Closure { dtor_idx: 185, function: Function { arguments: [Externref], shim_idx: 186, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
    const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h5a0fd3a052925ed0, wasm_bindgen__convert__closures_____invoke__h9a454594a18d3e6f);
    return ret;
};

exports.__wbindgen_cast_9ae0607507abb057 = function(arg0) {
    // Cast intrinsic for `I64 -> Externref`.
    const ret = arg0;
    return ret;
};

exports.__wbindgen_cast_cb9088102bce6b30 = function(arg0, arg1) {
    // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
    const ret = getArrayU8FromWasm0(arg0, arg1);
    return ret;
};

exports.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
    // Cast intrinsic for `F64 -> Externref`.
    const ret = arg0;
    return ret;
};

exports.__wbindgen_init_externref_table = function() {
    const table = wasm.__wbindgen_externrefs;
    const offset = table.grow(4);
    table.set(0, undefined);
    table.set(offset + 0, undefined);
    table.set(offset + 1, null);
    table.set(offset + 2, true);
    table.set(offset + 3, false);
};

const wasmPath = `${__dirname}/ruvector_edge_net_bg.wasm`;
const wasmBytes = require('fs').readFileSync(wasmPath);
const wasmModule = new WebAssembly.Module(wasmBytes);
const wasm = exports.__wasm = new WebAssembly.Instance(wasmModule, imports).exports;

wasm.__wbindgen_start();
