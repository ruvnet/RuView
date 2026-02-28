#!/usr/bin/env node
/**
 * Benchmark: Sequential vs Parallel ONNX Embeddings
 */
import { cpus } from 'os';
import { ParallelEmbedder } from './parallel-embedder.mjs';
import { createEmbedder } from './loader.js';

console.log('🧪 Parallel vs Sequential ONNX Embeddings Benchmark\n');
console.log(`CPU Cores: ${cpus().length}`);
console.log('='.repeat(60));

// Test data - various batch sizes
const testTexts = [
    "Machine learning is transforming technology",
    "Deep learning uses neural networks",
    "Natural language processing understands text",
    "Computer vision analyzes images",
    "Reinforcement learning learns from rewards",
    "Generative AI creates new content",
    "Vector databases enable semantic search",
    "Embeddings capture semantic meaning",
    "Transformers revolutionized NLP",
    "BERT is a popular language model",
    "GPT generates human-like text",
    "RAG combines retrieval and generation",
];

async function benchmarkSequential(embedder, texts, iterations = 3) {
    const times = [];
    for (let i = 0; i < iterations; i++) {
        const start = performance.now();
        for (const text of texts) {
            embedder.embedOne(text);
        }
        times.push(performance.now() - start);
    }
    return times.reduce((a, b) => a + b) / times.length;
}

async function benchmarkParallel(embedder, texts, iterations = 3) {
    const times = [];
    for (let i = 0; i < iterations; i++) {
        const start = performance.now();
        await embedder.embedBatch(texts);
        times.push(performance.now() - start);
    }
    return times.reduce((a, b) => a + b) / times.length;
}

async function main() {
    try {
        // Initialize sequential embedder
        console.log('\n📦 Loading model for sequential test...');
        const seqEmbedder = await createEmbedder();
        console.log('✅ Sequential embedder ready\n');

        // Warm up
        seqEmbedder.embedOne("warmup");

        // Initialize parallel embedder
        console.log('📦 Initializing parallel embedder...');
        const parEmbedder = new ParallelEmbedder({ numWorkers: Math.min(4, cpus().length) });
        await parEmbedder.init();

        // Benchmark different batch sizes
        for (const batchSize of [4, 8, 12]) {
            const texts = testTexts.slice(0, batchSize);

            console.log(`\n${'='.repeat(60)}`);
            console.log(`📊 Batch Size: ${batchSize} texts`);
            console.log('='.repeat(60));

            // Sequential benchmark
            console.log('\n⏱️  Sequential (single-threaded)...');
            const seqTime = await benchmarkSequential(seqEmbedder, texts);
            console.log(`   Time: ${seqTime.toFixed(1)}ms`);
            console.log(`   Per text: ${(seqTime / batchSize).toFixed(1)}ms`);

            // Parallel benchmark
            console.log('\n⏱️  Parallel (worker threads)...');
            const parTime = await benchmarkParallel(parEmbedder, texts);
            console.log(`   Time: ${parTime.toFixed(1)}ms`);
            console.log(`   Per text: ${(parTime / batchSize).toFixed(1)}ms`);

            // Speedup
            const speedup = seqTime / parTime;
            const icon = speedup > 1.2 ? '🚀' : speedup > 1 ? '✅' : '⚠️';
            console.log(`\n${icon} Speedup: ${speedup.toFixed(2)}x`);
        }

        // Verify correctness
        console.log(`\n${'='.repeat(60)}`);
        console.log('🔍 Verifying correctness...');
        console.log('='.repeat(60));

        const testText = "Vector databases are awesome";
        const seqEmb = seqEmbedder.embedOne(testText);
        const parEmb = await parEmbedder.embedOne(testText);

        // Compare embeddings
        let diff = 0;
        for (let i = 0; i < seqEmb.length; i++) {
            diff += Math.abs(seqEmb[i] - parEmb[i]);
        }
        const avgDiff = diff / seqEmb.length;
        console.log(`\nEmbedding difference: ${avgDiff.toExponential(4)}`);
        console.log(avgDiff < 1e-6 ? '✅ Embeddings match!' : '⚠️ Embeddings differ');

        // Cleanup
        await parEmbedder.shutdown();
        console.log('\n✅ Benchmark complete!');

    } catch (error) {
        console.error('❌ Error:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

main();
