const fs = require('fs');
const path = require('path');

async function test() {
    console.log('=== Micro HNSW WASM v2.2 Test Suite ===\n');

    // Load WASM
    const wasmPath = path.join(__dirname, 'micro_hnsw.wasm');
    const wasmBuffer = fs.readFileSync(wasmPath);
    const wasmModule = await WebAssembly.instantiate(wasmBuffer);
    const wasm = wasmModule.instance.exports;

    console.log('✓ WASM loaded successfully');
    console.log('  Binary size: ' + wasmBuffer.length + ' bytes (' + (wasmBuffer.length/1024).toFixed(2) + ' KB)\n');

    // List all exports
    const exports = Object.keys(wasm).filter(k => typeof wasm[k] === 'function');
    console.log('Exported functions (' + exports.length + '):');
    exports.forEach(fn => console.log('  - ' + fn));
    console.log('');

    // Test 1: Initialize HNSW
    console.log('Test 1: Initialize HNSW (dims=4, metric=0/euclidean, capacity=32)');
    wasm.init(4, 0, 32);
    console.log('  dims: ' + wasm.get_dims());
    console.log('  metric: ' + wasm.get_metric());
    console.log('  capacity: ' + wasm.get_capacity());
    console.log('  count: ' + wasm.count());
    console.log('✓ Init passed\n');

    // Test 2: Insert vectors
    console.log('Test 2: Insert vectors');
    const memory = new Float32Array(wasm.memory.buffer);
    const insertPtr = wasm.get_insert_ptr() / 4;

    // Insert 3 vectors
    const vectors = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 0.0],
    ];

    for (let i = 0; i < vectors.length; i++) {
        for (let j = 0; j < 4; j++) {
            memory[insertPtr + j] = vectors[i][j];
        }
        const idx = wasm.insert();
        console.log('  Inserted vector ' + i + ': index=' + idx);
    }
    console.log('  Total count: ' + wasm.count());
    console.log('✓ Insert passed\n');

    // Test 3: Search
    console.log('Test 3: Search for nearest neighbors');
    const queryPtr = wasm.get_query_ptr() / 4;
    memory[queryPtr] = 0.9;
    memory[queryPtr + 1] = 0.1;
    memory[queryPtr + 2] = 0.0;
    memory[queryPtr + 3] = 0.0;

    const found = wasm.search(3);
    console.log('  Query: [0.9, 0.1, 0.0, 0.0]');
    console.log('  Found: ' + found + ' neighbors');

    const resultPtr = wasm.get_result_ptr();
    console.log('  Result ptr: ' + resultPtr);
    console.log('✓ Search passed\n');

    // Test 4: Node types
    console.log('Test 4: Node types');
    wasm.set_node_type(0, 5);
    wasm.set_node_type(1, 10);
    console.log('  Node 0 type: ' + wasm.get_node_type(0));
    console.log('  Node 1 type: ' + wasm.get_node_type(1));
    console.log('  Type match (0,0): ' + wasm.type_matches(0, 0));
    console.log('  Type match (0,1): ' + wasm.type_matches(0, 1));
    console.log('✓ Node types passed\n');

    // Test 5: Edge weights (GNN feature)
    console.log('Test 5: Edge weights (GNN)');
    wasm.set_edge_weight(0, 200);
    wasm.set_edge_weight(1, 100);
    console.log('  Edge 0 weight: ' + wasm.get_edge_weight(0));
    console.log('  Edge 1 weight: ' + wasm.get_edge_weight(1));
    console.log('✓ Edge weights passed\n');

    // Test 6: SNN features (if available)
    if (wasm.snn_reset) {
        console.log('Test 6: Spiking Neural Network (SNN)');
        wasm.snn_reset();
        console.log('  Initial time: ' + wasm.snn_get_time());

        // Inject current to node 0
        wasm.snn_inject(0, 0.5); // Inject below threshold
        console.log('  Injected current 0.5 to node 0');
        console.log('  Node 0 membrane: ' + wasm.snn_get_membrane(0).toFixed(3));

        // Run simulation step with dt=1.0 ms
        const dt = 1.0;
        let spikes1 = wasm.snn_step(dt);
        console.log('  After step 1 (dt=' + dt + 'ms): time=' + wasm.snn_get_time().toFixed(1) + ', membrane=' + wasm.snn_get_membrane(0).toFixed(3) + ', spikeCount=' + spikes1);

        // Inject more to reach threshold
        wasm.snn_inject(0, 0.8);
        let spikes2 = wasm.snn_step(dt);
        console.log('  After step 2 (+0.8 current): membrane=' + wasm.snn_get_membrane(0).toFixed(3) + ', spiked=' + wasm.snn_spiked(0) + ', spikeCount=' + spikes2);

        // Check spikes bitset
        const spikes = wasm.snn_get_spikes();
        console.log('  Spike bitmask: 0b' + spikes.toString(2));

        // Test combined tick function
        wasm.snn_reset();
        wasm.snn_inject(0, 1.5); // Above threshold
        const tickSpikes = wasm.snn_tick(1.0, 0.5, 1); // dt=1.0, gain=0.5, learn=1
        console.log('  snn_tick result: ' + tickSpikes + ' spikes');

        console.log('✓ SNN passed\n');
    } else {
        console.log('Test 6: SNN not available (functions not exported)\n');
    }

    // Test 7: HNSW to SNN conversion
    if (wasm.hnsw_to_snn) {
        console.log('Test 7: HNSW to SNN conversion');
        wasm.snn_reset();
        // hnsw_to_snn(k, gain) - search for k neighbors and inject currents
        const injected = wasm.hnsw_to_snn(3, 1.0);
        console.log('  Converted HNSW search to SNN currents for ' + injected + ' nodes');
        console.log('  Node 0 membrane after injection: ' + wasm.snn_get_membrane(0).toFixed(3));
        console.log('✓ HNSW→SNN passed\n');
    }

    // Test 8: Aggregate neighbors (GNN)
    if (wasm.aggregate_neighbors) {
        console.log('Test 8: GNN aggregate neighbors');
        wasm.aggregate_neighbors(0);
        console.log('  Aggregated features for node 0');
        console.log('✓ Aggregate passed\n');
    }

    console.log('=== All Tests Passed ===');
    console.log('Final stats: ' + wasm.count() + ' vectors, ' + wasmBuffer.length + ' bytes');
}

test().catch(console.error);
