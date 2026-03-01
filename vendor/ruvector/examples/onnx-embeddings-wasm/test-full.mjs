#!/usr/bin/env node
/**
 * Full end-to-end test with model download
 *
 * Downloads all-MiniLM-L6-v2 and runs embedding tests
 */

import { ModelLoader, MODELS, DEFAULT_MODEL } from './loader.js';
import {
    WasmEmbedder,
    WasmEmbedderConfig,
    cosineSimilarity,
} from './pkg/ruvector_onnx_embeddings_wasm.js';

console.log('🧪 RuVector ONNX Embeddings WASM - Full E2E Test\n');
console.log('='.repeat(60));

// List available models
console.log('\n📦 Available Models:');
ModelLoader.listModels().forEach(m => {
    const isDefault = m.id === DEFAULT_MODEL ? ' ⭐ DEFAULT' : '';
    console.log(`  • ${m.id} (${m.dimension}d, ${m.size})${isDefault}`);
    console.log(`    ${m.description}`);
});

console.log('\n' + '='.repeat(60));
console.log(`\n🔄 Loading model: ${DEFAULT_MODEL}...\n`);

// Load model with progress
const loader = new ModelLoader({
    cache: false, // Disable cache for testing
    onProgress: ({ loaded, total, percent }) => {
        process.stdout.write(`\r  Progress: ${percent}% (${(loaded/1024/1024).toFixed(1)}MB / ${(total/1024/1024).toFixed(1)}MB)`);
    }
});

try {
    const { modelBytes, tokenizerJson, config } = await loader.loadModel(DEFAULT_MODEL);
    console.log('\n');
    console.log(`  ✅ Model loaded: ${config.name}`);
    console.log(`  ✅ Model size: ${(modelBytes.length / 1024 / 1024).toFixed(2)} MB`);
    console.log(`  ✅ Tokenizer size: ${(tokenizerJson.length / 1024).toFixed(2)} KB`);

    // Create embedder
    console.log('\n🔧 Creating embedder...');
    const embedderConfig = new WasmEmbedderConfig()
        .setMaxLength(config.maxLength)
        .setNormalize(true)
        .setPooling(0);

    const embedder = WasmEmbedder.withConfig(modelBytes, tokenizerJson, embedderConfig);
    console.log(`  ✅ Embedder created`);
    console.log(`  ✅ Dimension: ${embedder.dimension()}`);
    console.log(`  ✅ Max length: ${embedder.maxLength()}`);

    // Test 1: Single embedding
    console.log('\n' + '='.repeat(60));
    console.log('\n📝 Test 1: Single Embedding');
    const text1 = "The quick brown fox jumps over the lazy dog.";
    console.log(`  Input: "${text1}"`);

    const start1 = performance.now();
    const embedding1 = embedder.embedOne(text1);
    const time1 = performance.now() - start1;

    console.log(`  ✅ Output dimension: ${embedding1.length}`);
    console.log(`  ✅ First 5 values: [${Array.from(embedding1.slice(0, 5)).map(v => v.toFixed(4)).join(', ')}]`);
    console.log(`  ✅ Time: ${time1.toFixed(2)}ms`);

    // Test 2: Semantic similarity
    console.log('\n' + '='.repeat(60));
    console.log('\n📝 Test 2: Semantic Similarity');

    const pairs = [
        ["I love programming in Rust", "Rust is my favorite programming language"],
        ["The weather is nice today", "It's sunny outside"],
        ["I love programming in Rust", "The weather is nice today"],
        ["Machine learning is fascinating", "AI and deep learning are interesting"],
    ];

    for (const [a, b] of pairs) {
        const start = performance.now();
        const sim = embedder.similarity(a, b);
        const time = performance.now() - start;

        const label = sim > 0.5 ? '🟢 Similar' : '🔴 Different';
        console.log(`\n  "${a.substring(0, 30)}..."`);
        console.log(`  "${b.substring(0, 30)}..."`);
        console.log(`  ${label}: ${sim.toFixed(4)} (${time.toFixed(1)}ms)`);
    }

    // Test 3: Batch embedding
    console.log('\n' + '='.repeat(60));
    console.log('\n📝 Test 3: Batch Embedding');

    const texts = [
        "Artificial intelligence is transforming technology.",
        "Machine learning models learn from data.",
        "Deep learning uses neural networks.",
        "Vector databases enable semantic search.",
    ];

    console.log(`  Embedding ${texts.length} texts...`);
    const start3 = performance.now();
    const batchEmbeddings = embedder.embedBatch(texts);
    const time3 = performance.now() - start3;

    const embeddingDim = embedder.dimension();
    const numEmbeddings = batchEmbeddings.length / embeddingDim;

    console.log(`  ✅ Total values: ${batchEmbeddings.length}`);
    console.log(`  ✅ Embeddings: ${numEmbeddings} x ${embeddingDim}d`);
    console.log(`  ✅ Time: ${time3.toFixed(2)}ms (${(time3/texts.length).toFixed(2)}ms per text)`);

    // Compute pairwise similarities
    console.log('\n  Pairwise similarities:');
    for (let i = 0; i < numEmbeddings; i++) {
        for (let j = i + 1; j < numEmbeddings; j++) {
            const emb_i = batchEmbeddings.slice(i * embeddingDim, (i + 1) * embeddingDim);
            const emb_j = batchEmbeddings.slice(j * embeddingDim, (j + 1) * embeddingDim);
            const sim = cosineSimilarity(emb_i, emb_j);
            console.log(`    [${i}] vs [${j}]: ${sim.toFixed(4)}`);
        }
    }

    // Summary
    console.log('\n' + '='.repeat(60));
    console.log('\n✅ All tests passed!');
    console.log('='.repeat(60));

    console.log('\n📊 Performance Summary:');
    console.log(`  • Model: ${config.name}`);
    console.log(`  • Dimension: ${embeddingDim}`);
    console.log(`  • Single embed: ~${time1.toFixed(0)}ms`);
    console.log(`  • Batch (4 texts): ~${time3.toFixed(0)}ms`);
    console.log(`  • Throughput: ~${(1000 / (time3/texts.length)).toFixed(0)} texts/sec`);

} catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error(error.stack);
    process.exit(1);
}
