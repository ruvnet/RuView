#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

// Configuration
const CRATES = ['graph_reasoner', 'extractors', 'planner'];

console.log('🧪 Testing WASM modules...\n');

// Test a single WASM module
async function testWasmModule(crateName) {
    const pkgPath = path.join(__dirname, crateName, 'pkg');
    const wasmPath = path.join(pkgPath, `${crateName}.js`);
    const wasmBgPath = path.join(pkgPath, `${crateName}_bg.wasm`);

    console.log(`🔍 Testing ${crateName}...`);

    // Check if files exist
    if (!fs.existsSync(wasmPath)) {
        console.error(`❌ ${crateName}.js not found in ${pkgPath}`);
        return false;
    }

    if (!fs.existsSync(wasmBgPath)) {
        console.error(`❌ ${crateName}_bg.wasm not found in ${pkgPath}`);
        return false;
    }

    try {
        // Import the module and initialize WASM using fs for Node.js
        const wasmModule = await import(`file://${wasmPath}`);
        const wasmBuffer = fs.readFileSync(wasmBgPath);
        wasmModule.initSync(wasmBuffer);
        console.log(`📦 Module loaded and initialized: ${crateName}`);

        // Test basic functionality based on crate type
        if (crateName === 'graph_reasoner') {
            await testGraphReasoner(wasmModule);
        } else if (crateName === 'extractors') {
            await testExtractors(wasmModule);
        } else if (crateName === 'planner') {
            await testPlanner(wasmModule);
        }

        console.log(`✅ ${crateName} tests passed\n`);
        return true;
    } catch (error) {
        console.error(`❌ Error testing ${crateName}:`, error.message);
        console.error('Stack:', error.stack);
        return false;
    }
}

// Test GraphReasoner functionality
async function testGraphReasoner(module) {
    const reasoner = new module.GraphReasoner();

    // Test adding facts
    const factId = reasoner.add_fact("Alice", "knows", "Bob");
    console.log(`  📝 Added fact: ${factId}`);

    // Test query
    const queryResult = reasoner.query('{"type": "simple", "subject": "Alice"}');
    console.log(`  🔍 Query result: ${queryResult.substring(0, 100)}...`);

    // Test inference
    const inferenceResult = reasoner.infer(5);
    console.log(`  🧠 Inference result: ${inferenceResult.substring(0, 100)}...`);

    // Test stats
    const stats = reasoner.get_graph_stats();
    console.log(`  📊 Graph stats: ${stats}`);
}

// Test TextExtractor functionality
async function testExtractors(module) {
    const extractor = new module.TextExtractor();

    // Test sentiment analysis
    const sentiment = extractor.analyze_sentiment("I love this amazing product!");
    console.log(`  😊 Sentiment: ${sentiment}`);

    // Test preference extraction
    const preferences = extractor.extract_preferences("I prefer coffee over tea and like dark chocolate");
    console.log(`  ❤️ Preferences: ${preferences.substring(0, 100)}...`);

    // Test emotion detection
    const emotions = extractor.detect_emotions("I am so excited and happy about this news!");
    console.log(`  😍 Emotions: ${emotions}`);

    // Test comprehensive analysis
    const analysis = extractor.analyze_all("I absolutely love programming in Rust! It's my favorite language.");
    console.log(`  🎯 Full analysis: ${analysis.substring(0, 100)}...`);
}

// Test PlannerSystem functionality
async function testPlanner(module) {
    const planner = new module.PlannerSystem();

    // Test state management
    const stateSet = planner.set_state("has_key", '{"type": "boolean", "value": false}');
    console.log(`  🔑 Set state: ${stateSet}`);

    const stateGet = planner.get_state("has_key");
    console.log(`  📖 Get state: ${stateGet}`);

    // Test action addition
    const actionAdded = planner.add_action(JSON.stringify({
        id: "unlock_door",
        name: "Unlock Door",
        cost: { base_cost: 1.0 },
        preconditions: [{ state_key: "has_key", expected_value: { type: "boolean", value: true } }],
        effects: [{ state_key: "door_unlocked", value: { type: "boolean", value: true } }]
    }));
    console.log(`  🚪 Action added: ${actionAdded}`);

    // Test goal addition
    const goalAdded = planner.add_goal(JSON.stringify({
        id: "goal1",
        name: "Unlock the door",
        priority: "High",
        conditions: [{ state_key: "door_unlocked", target_value: { type: "boolean", value: true } }]
    }));
    console.log(`  🎯 Goal added: ${goalAdded}`);

    // Test getting world state
    const worldState = planner.get_world_state();
    console.log(`  🌍 World state: ${worldState.substring(0, 100)}...`);

    // Test available actions
    const availableActions = planner.get_available_actions();
    console.log(`  ⚡ Available actions: ${availableActions}`);
}

// Create TypeScript definitions test
function createTypeScriptTest() {
    const tsContent = `
// TypeScript integration test
import type { GraphReasoner } from './graph_reasoner/pkg/graph_reasoner';
import type { TextExtractor } from './extractors/pkg/extractors';
import type { PlannerSystem } from './planner/pkg/planner';

// Test interface compatibility
interface WasmModules {
    graphReasoner: GraphReasoner;
    textExtractor: TextExtractor;
    plannerSystem: PlannerSystem;
}

// Example usage function
export function exampleUsage(): void {
    console.log('WASM modules can be used in TypeScript!');
}
`;

    const tsTestPath = path.join(__dirname, 'test-types.ts');
    fs.writeFileSync(tsTestPath, tsContent);
    console.log('📝 Created TypeScript test file');
}

// Performance benchmark
async function performanceBenchmark() {
    console.log('⚡ Running performance benchmarks...\n');

    try {
        const graphModule = await import(`file://${path.join(__dirname, 'graph_reasoner', 'pkg', 'graph_reasoner.js')}`);
        const wasmBuffer = fs.readFileSync(path.join(__dirname, 'graph_reasoner', 'pkg', 'graph_reasoner_bg.wasm'));
        graphModule.initSync(wasmBuffer);
        const reasoner = new graphModule.GraphReasoner();

        // Benchmark fact insertion
        const startTime = process.hrtime.bigint();
        for (let i = 0; i < 1000; i++) {
            reasoner.add_fact(`entity${i}`, "type", "test");
        }
        const endTime = process.hrtime.bigint();

        const duration = Number(endTime - startTime) / 1_000_000; // Convert to milliseconds
        console.log(`📊 Added 1000 facts in ${duration.toFixed(2)}ms`);
        console.log(`🚀 Rate: ${(1000 / duration * 1000).toFixed(0)} facts/second\n`);
    } catch (error) {
        console.error('❌ Performance benchmark failed:', error.message);
    }
}

// Main test function
async function main() {
    let successCount = 0;

    // Test each module
    for (const crate of CRATES) {
        if (await testWasmModule(crate)) {
            successCount++;
        }
    }

    // Create TypeScript test
    createTypeScriptTest();

    // Run performance benchmark
    await performanceBenchmark();

    // Summary
    if (successCount === CRATES.length) {
        console.log(`🎉 All ${CRATES.length} WASM modules passed tests!`);
        console.log('\n✅ WASM compilation and exports working correctly');
        console.log('📋 Ready for integration and bundling');
    } else {
        console.error(`\n❌ Testing completed with errors. ${successCount}/${CRATES.length} modules passed.`);
        process.exit(1);
    }
}

// Run if called directly
if (require.main === module) {
    main().catch(error => {
        console.error('Fatal error:', error);
        process.exit(1);
    });
}

module.exports = {
    testWasmModule,
    performanceBenchmark
};