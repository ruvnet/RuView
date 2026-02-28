/**
 * MicroLoRA Example - Browser-based LoRA Adaptation
 *
 * This example demonstrates how to use MicroLoRA for real-time
 * adaptation of language model outputs in the browser.
 */

import init, {
    MicroLoraWasm,
    MicroLoraConfigWasm,
    AdaptFeedbackWasm,
    MicroLoraStatsWasm
} from '../pkg/ruvllm_wasm';

async function main() {
    // Initialize WASM module
    await init();
    console.log('✅ WASM module initialized');

    // Create a rank-2 adapter for 768-dim hidden states
    const config = new MicroLoraConfigWasm();
    config.rank = 2;
    config.alpha = 4.0;
    config.inFeatures = 768;
    config.outFeatures = 768;

    console.log(`📊 Config: rank=${config.rank}, alpha=${config.alpha}`);
    console.log(`📊 Memory footprint: ${config.memoryBytes()} bytes (${(config.memoryBytes() / 1024).toFixed(2)} KB)`);

    // Create the adapter
    const lora = new MicroLoraWasm(config);
    console.log('✅ MicroLoRA adapter created');

    // Simulate some hidden state input
    const hiddenState = new Float32Array(768);
    for (let i = 0; i < 768; i++) {
        hiddenState[i] = Math.random() * 0.1 - 0.05; // Small random values
    }

    // Apply LoRA transformation
    console.log('\n🔄 Applying LoRA transformation...');
    const output = lora.apply(hiddenState);
    console.log(`✅ Output shape: ${output.length}`);
    console.log(`📈 Output magnitude: ${Math.sqrt(output.reduce((sum, x) => sum + x * x, 0) / output.length).toFixed(6)}`);

    // Simulate user feedback loop
    console.log('\n📚 Training loop:');
    const numIterations = 10;

    for (let i = 0; i < numIterations; i++) {
        // Simulate varying quality feedback
        const quality = 0.5 + 0.3 * Math.sin(i * 0.5); // Oscillates between 0.2 and 0.8

        const feedback = new AdaptFeedbackWasm(quality);
        feedback.learningRate = 0.01;

        lora.adapt(hiddenState, feedback);

        if ((i + 1) % 3 === 0) {
            // Apply updates every 3 iterations
            lora.applyUpdates(0.01);
            const stats = lora.stats();
            console.log(`  Iteration ${i + 1}: quality=${quality.toFixed(3)}, avg_quality=${stats.avgQuality.toFixed(3)}, pending=${lora.pendingUpdates()}`);
        }
    }

    // Get final statistics
    console.log('\n📊 Final Statistics:');
    const stats = lora.stats();
    console.log(`  Samples seen: ${stats.samplesSeen}`);
    console.log(`  Average quality: ${stats.avgQuality.toFixed(3)}`);
    console.log(`  Memory usage: ${stats.memoryBytes} bytes`);
    console.log(`  Parameter count: ${stats.paramCount}`);

    // Test serialization
    console.log('\n💾 Serialization test:');
    const json = lora.toJson();
    console.log(`  JSON size: ${json.length} bytes`);

    const restored = MicroLoraWasm.fromJson(json);
    const restoredStats = restored.stats();
    console.log(`  ✅ Restored samples: ${restoredStats.samplesSeen}`);
    console.log(`  ✅ Restored avg quality: ${restoredStats.avgQuality.toFixed(3)}`);

    // Apply after restoration
    const output2 = restored.apply(hiddenState);
    const diff = Math.sqrt(
        output.reduce((sum, val, i) => sum + Math.pow(val - output2[i], 2), 0) / output.length
    );
    console.log(`  ✅ Output difference after serialization: ${diff.toFixed(8)} (should be ~0)`);

    // Test reset
    console.log('\n🔄 Reset test:');
    lora.reset();
    const resetStats = lora.stats();
    console.log(`  Samples after reset: ${resetStats.samplesSeen}`);
    console.log(`  Quality after reset: ${resetStats.avgQuality}`);

    // Browser storage integration
    console.log('\n💾 Browser storage integration:');
    try {
        localStorage.setItem('lora-state', json);
        console.log('  ✅ Saved to localStorage');

        const loaded = localStorage.getItem('lora-state');
        if (loaded) {
            const fromStorage = MicroLoraWasm.fromJson(loaded);
            console.log('  ✅ Loaded from localStorage');
            const fromStorageStats = fromStorage.stats();
            console.log(`  ✅ Loaded samples: ${fromStorageStats.samplesSeen}`);
        }
    } catch (e) {
        console.log('  ⚠️  localStorage not available (running in Node?)');
    }

    console.log('\n✨ MicroLoRA example complete!');
}

// Real-world usage example: Online learning from user feedback
async function onlineLearningExample() {
    await init();

    const config = new MicroLoraConfigWasm();
    config.rank = 2;
    config.inFeatures = 512;
    config.outFeatures = 512;

    const lora = new MicroLoraWasm(config);

    // Simulate a chat interface with user feedback
    console.log('\n🗨️  Online Learning Example:');
    console.log('Simulating a chat interface with user feedback...\n');

    const conversations = [
        { input: 'helpful response', quality: 0.9 },
        { input: 'somewhat helpful', quality: 0.6 },
        { input: 'excellent answer', quality: 0.95 },
        { input: 'mediocre response', quality: 0.5 },
        { input: 'very helpful', quality: 0.85 },
    ];

    for (const [idx, conv] of conversations.entries()) {
        // Generate some input based on the conversation
        const input = new Float32Array(512);
        for (let i = 0; i < 512; i++) {
            input[i] = Math.random() * 0.1;
        }

        // User provides feedback
        const feedback = new AdaptFeedbackWasm(conv.quality);
        lora.adapt(input, feedback);

        // Update every 2 conversations
        if ((idx + 1) % 2 === 0) {
            lora.applyUpdates(0.02);
        }

        console.log(`  Response ${idx + 1}: "${conv.input}" (quality: ${conv.quality})`);
    }

    const finalStats = lora.stats();
    console.log(`\n  📈 Average user satisfaction: ${(finalStats.avgQuality * 100).toFixed(1)}%`);
    console.log(`  📊 Total adaptations: ${finalStats.samplesSeen}`);
}

// Run examples
main().then(() => onlineLearningExample()).catch(console.error);
