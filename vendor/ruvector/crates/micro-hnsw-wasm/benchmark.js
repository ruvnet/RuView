const fs = require('fs');
const path = require('path');

// High-resolution timer
const now = () => {
    const [s, ns] = process.hrtime();
    return s * 1e9 + ns;
};

async function benchmark() {
    console.log('╔══════════════════════════════════════════════════════════════╗');
    console.log('║     MICRO HNSW WASM v2.2 - DEEP BENCHMARK & ANALYSIS         ║');
    console.log('╚══════════════════════════════════════════════════════════════╝\n');

    // Load WASM
    const wasmPath = path.join(__dirname, 'micro_hnsw.wasm');
    const wasmBuffer = fs.readFileSync(wasmPath);
    const wasmModule = await WebAssembly.instantiate(wasmBuffer);
    const wasm = wasmModule.instance.exports;
    const memory = new Float32Array(wasm.memory.buffer);

    console.log('=== BINARY ANALYSIS ===');
    console.log('Size: ' + wasmBuffer.length + ' bytes (' + (wasmBuffer.length/1024).toFixed(2) + ' KB)');
    console.log('Target: 8192 bytes (8 KB)');
    console.log('Headroom: ' + (8192 - wasmBuffer.length) + ' bytes (' + ((8192 - wasmBuffer.length)/8192*100).toFixed(1) + '%)');
    console.log('Functions exported: ' + Object.keys(wasm).filter(k => typeof wasm[k] === 'function').length);
    console.log('');

    // ========== HNSW BENCHMARKS ==========
    console.log('=== HNSW BENCHMARKS ===');

    const DIMS = 16;
    const ITERATIONS = 1000;

    // Benchmark: Init
    let t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.init(DIMS, 0, 0);
    }
    let initTime = (now() - t0) / ITERATIONS;
    console.log('init():           ' + initTime.toFixed(0) + ' ns/op');

    // Prepare insert buffer
    wasm.init(DIMS, 0, 0);
    const insertPtr = wasm.get_insert_ptr() / 4;

    // Benchmark: Single insert (empty index)
    t0 = now();
    for (let iter = 0; iter < 100; iter++) {
        wasm.init(DIMS, 0, 0);
        for (let j = 0; j < DIMS; j++) memory[insertPtr + j] = Math.random();
        wasm.insert();
    }
    let insertFirstTime = (now() - t0) / 100;
    console.log('insert() first:   ' + insertFirstTime.toFixed(0) + ' ns/op');

    // Benchmark: Insert with connections (fill to 16 vectors)
    wasm.init(DIMS, 0, 0);
    for (let i = 0; i < 16; i++) {
        for (let j = 0; j < DIMS; j++) memory[insertPtr + j] = Math.random();
        wasm.insert();
    }

    t0 = now();
    for (let iter = 0; iter < 100; iter++) {
        wasm.init(DIMS, 0, 0);
        for (let i = 0; i < 16; i++) {
            for (let j = 0; j < DIMS; j++) memory[insertPtr + j] = Math.random();
            wasm.insert();
        }
    }
    let insert16Time = (now() - t0) / 100;
    console.log('insert() x16:     ' + (insert16Time/1000).toFixed(1) + ' µs total (' + (insert16Time/16).toFixed(0) + ' ns avg/vector)');

    // Fill to 32 vectors for search benchmark
    wasm.init(DIMS, 0, 0);
    for (let i = 0; i < 32; i++) {
        for (let j = 0; j < DIMS; j++) memory[insertPtr + j] = Math.random();
        wasm.insert();
    }
    console.log('Indexed: ' + wasm.count() + ' vectors');

    // Benchmark: Search k=1
    const queryPtr = wasm.get_query_ptr() / 4;
    for (let j = 0; j < DIMS; j++) memory[queryPtr + j] = Math.random();

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.search(1);
    }
    let search1Time = (now() - t0) / ITERATIONS;
    console.log('search(k=1):      ' + search1Time.toFixed(0) + ' ns/op');

    // Benchmark: Search k=6
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.search(6);
    }
    let search6Time = (now() - t0) / ITERATIONS;
    console.log('search(k=6):      ' + search6Time.toFixed(0) + ' ns/op');

    // Benchmark: Search k=16
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.search(16);
    }
    let search16Time = (now() - t0) / ITERATIONS;
    console.log('search(k=16):     ' + search16Time.toFixed(0) + ' ns/op');

    console.log('');

    // ========== GNN BENCHMARKS ==========
    console.log('=== GNN BENCHMARKS ===');

    // Benchmark: Node type operations
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.set_node_type(i % 32, i % 16);
    }
    let setTypeTime = (now() - t0) / ITERATIONS;
    console.log('set_node_type():  ' + setTypeTime.toFixed(0) + ' ns/op');

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.get_node_type(i % 32);
    }
    let getTypeTime = (now() - t0) / ITERATIONS;
    console.log('get_node_type():  ' + getTypeTime.toFixed(0) + ' ns/op');

    // Benchmark: Edge weight operations
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.set_edge_weight(i % 32, i % 256);
    }
    let setWeightTime = (now() - t0) / ITERATIONS;
    console.log('set_edge_weight(): ' + setWeightTime.toFixed(0) + ' ns/op');

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.get_edge_weight(i % 32);
    }
    let getWeightTime = (now() - t0) / ITERATIONS;
    console.log('get_edge_weight(): ' + getWeightTime.toFixed(0) + ' ns/op');

    // Benchmark: Aggregate neighbors
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.aggregate_neighbors(i % 32);
    }
    let aggregateTime = (now() - t0) / ITERATIONS;
    console.log('aggregate():      ' + aggregateTime.toFixed(0) + ' ns/op');

    // Benchmark: Update vector
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.update_vector(i % 32, 0.01);
    }
    let updateTime = (now() - t0) / ITERATIONS;
    console.log('update_vector():  ' + updateTime.toFixed(0) + ' ns/op');

    console.log('');

    // ========== SNN BENCHMARKS ==========
    console.log('=== SNN BENCHMARKS ===');

    wasm.snn_reset();

    // Benchmark: snn_inject
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_inject(i % 32, 0.1);
    }
    let injectTime = (now() - t0) / ITERATIONS;
    console.log('snn_inject():     ' + injectTime.toFixed(0) + ' ns/op');

    // Benchmark: snn_step
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_step(1.0);
    }
    let stepTime = (now() - t0) / ITERATIONS;
    console.log('snn_step():       ' + stepTime.toFixed(0) + ' ns/op');

    // Benchmark: snn_propagate
    // First make some neurons spike
    wasm.snn_reset();
    for (let i = 0; i < 8; i++) wasm.snn_inject(i, 2.0);
    wasm.snn_step(1.0);

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_propagate(0.5);
    }
    let propagateTime = (now() - t0) / ITERATIONS;
    console.log('snn_propagate():  ' + propagateTime.toFixed(0) + ' ns/op');

    // Benchmark: snn_stdp
    wasm.snn_reset();
    for (let i = 0; i < 8; i++) wasm.snn_inject(i, 2.0);
    wasm.snn_step(1.0);

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_stdp();
    }
    let stdpTime = (now() - t0) / ITERATIONS;
    console.log('snn_stdp():       ' + stdpTime.toFixed(0) + ' ns/op');

    // Benchmark: snn_tick (combined)
    wasm.snn_reset();
    for (let i = 0; i < 8; i++) wasm.snn_inject(i, 0.5);

    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_tick(1.0, 0.5, 1);
    }
    let tickTime = (now() - t0) / ITERATIONS;
    console.log('snn_tick():       ' + tickTime.toFixed(0) + ' ns/op');

    // Benchmark: snn_get_spikes
    t0 = now();
    for (let i = 0; i < ITERATIONS; i++) {
        wasm.snn_get_spikes();
    }
    let getSpikesTime = (now() - t0) / ITERATIONS;
    console.log('snn_get_spikes(): ' + getSpikesTime.toFixed(0) + ' ns/op');

    // Benchmark: hnsw_to_snn
    wasm.snn_reset();
    t0 = now();
    for (let i = 0; i < 100; i++) {
        wasm.hnsw_to_snn(6, 1.0);
    }
    let hnswToSnnTime = (now() - t0) / 100;
    console.log('hnsw_to_snn():    ' + hnswToSnnTime.toFixed(0) + ' ns/op');

    console.log('');

    // ========== MEMORY ANALYSIS ==========
    console.log('=== MEMORY LAYOUT ANALYSIS ===');

    const memoryBytes = wasm.memory.buffer.byteLength;
    console.log('Linear memory:    ' + memoryBytes + ' bytes (' + (memoryBytes/1024) + ' KB)');
    console.log('Insert ptr:       ' + wasm.get_insert_ptr());
    console.log('Query ptr:        ' + wasm.get_query_ptr());
    console.log('Result ptr:       ' + wasm.get_result_ptr());
    console.log('Global ptr:       ' + wasm.get_global_ptr());
    console.log('Delta ptr:        ' + wasm.get_delta_ptr());

    // Calculate static data size from WASM
    const dataEnd = wasm.__data_end;
    const heapBase = wasm.__heap_base;
    console.log('Data end:         ' + dataEnd);
    console.log('Heap base:        ' + heapBase);
    console.log('Static data:      ' + (heapBase - 0) + ' bytes');

    console.log('');

    // ========== THROUGHPUT ANALYSIS ==========
    console.log('=== THROUGHPUT ANALYSIS ===');

    const searchOpsPerSec = 1e9 / search6Time;
    const insertOpsPerSec = 1e9 / (insert16Time / 16);
    const tickOpsPerSec = 1e9 / tickTime;

    console.log('Search (k=6):     ' + (searchOpsPerSec/1e6).toFixed(2) + ' M ops/sec');
    console.log('Insert:           ' + (insertOpsPerSec/1e6).toFixed(2) + ' M ops/sec');
    console.log('SNN tick:         ' + (tickOpsPerSec/1e6).toFixed(2) + ' M ops/sec');

    // ASIC projection (256 cores)
    console.log('\n--- 256-Core ASIC Projection ---');
    console.log('Search:           ' + (searchOpsPerSec * 256 / 1e9).toFixed(2) + ' B ops/sec');
    console.log('SNN tick:         ' + (tickOpsPerSec * 256 / 1e6).toFixed(0) + ' M neurons/sec');
    console.log('Total vectors:    ' + (32 * 256) + ' (32/core × 256 cores)');

    console.log('');

    // ========== ACCURACY TEST ==========
    console.log('=== ACCURACY VALIDATION ===');

    // Test search accuracy with known vectors
    wasm.init(4, 0, 0); // L2 metric, 4 dims
    const testVectors = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1],
        [0.5, 0.5, 0, 0],
    ];

    for (const v of testVectors) {
        for (let j = 0; j < 4; j++) memory[insertPtr + j] = v[j];
        wasm.insert();
    }

    // Query closest to [1,0,0,0]
    memory[queryPtr] = 0.9;
    memory[queryPtr + 1] = 0.1;
    memory[queryPtr + 2] = 0;
    memory[queryPtr + 3] = 0;

    const found = wasm.search(3);
    const resultPtr = wasm.get_result_ptr();
    const resultU8 = new Uint8Array(wasm.memory.buffer);
    const resultF32 = new Float32Array(wasm.memory.buffer);

    console.log('Query: [0.9, 0.1, 0, 0], Expected nearest: idx=0 [1,0,0,0]');
    console.log('Found ' + found + ' neighbors:');
    for (let i = 0; i < found; i++) {
        const idx = resultU8[resultPtr + i * 8];
        const dist = resultF32[(resultPtr + i * 8 + 4) / 4];
        console.log('  #' + (i+1) + ': idx=' + idx + ' dist=' + dist.toFixed(4) + ' vec=[' + testVectors[idx].join(',') + ']');
    }

    // Verify correct ordering
    const firstIdx = resultU8[resultPtr];
    if (firstIdx === 0) {
        console.log('✓ Accuracy: PASS (nearest neighbor correct)');
    } else {
        console.log('✗ Accuracy: FAIL (expected idx=0, got idx=' + firstIdx + ')');
    }

    console.log('');

    // ========== SNN DYNAMICS VALIDATION ==========
    console.log('=== SNN DYNAMICS VALIDATION ===');

    wasm.init(4, 0, 0);
    for (const v of testVectors) {
        for (let j = 0; j < 4; j++) memory[insertPtr + j] = v[j];
        wasm.insert();
    }

    wasm.snn_reset();

    // Test LIF dynamics
    console.log('LIF Neuron Test (τ=20ms, threshold=1.0):');
    wasm.snn_inject(0, 0.8);
    console.log('  t=0: inject 0.8, membrane=' + wasm.snn_get_membrane(0).toFixed(3));

    wasm.snn_step(5.0);
    console.log('  t=5: decay, membrane=' + wasm.snn_get_membrane(0).toFixed(3) + ' (expected ~0.6)');

    wasm.snn_inject(0, 0.5);
    console.log('  t=5: inject +0.5, membrane=' + wasm.snn_get_membrane(0).toFixed(3));

    const spiked = wasm.snn_step(1.0);
    console.log('  t=6: step, spiked=' + spiked + ', membrane=' + wasm.snn_get_membrane(0).toFixed(3));

    if (spiked > 0) {
        console.log('✓ LIF dynamics: PASS (spike generated above threshold)');
    } else {
        console.log('✗ LIF dynamics: membrane should have spiked');
    }

    console.log('');
    console.log('═══════════════════════════════════════════════════════════════');
    console.log('                    BENCHMARK COMPLETE');
    console.log('═══════════════════════════════════════════════════════════════');
}

benchmark().catch(console.error);
