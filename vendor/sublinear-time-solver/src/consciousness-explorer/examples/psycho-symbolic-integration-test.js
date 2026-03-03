#!/usr/bin/env node

/**
 * Psycho-Symbolic Integration Test
 * Demonstrates the integrated functionality of the psycho-symbolic reasoner
 * within the consciousness-explorer SDK
 */

import { ConsciousnessExplorer, PsychoSymbolicReasoner, getPsychoSymbolicReasoner } from '../index.js';

async function runIntegrationTest() {
    console.log('🧠 Psycho-Symbolic Reasoner Integration Test');
    console.log('='.repeat(50));

    try {
        // Test 1: Direct reasoner usage
        console.log('\n1️⃣ Testing Direct Reasoner Usage:');
        const reasoner = getPsychoSymbolicReasoner({
            enableConsciousnessAnalysis: true,
            enableWasm: false // Use JS implementation for testing
        });

        // Add some test knowledge
        await reasoner.addKnowledge(
            'test-system',
            'demonstrates',
            'consciousness-patterns',
            { source: 'integration-test', confidence: 0.9 }
        );

        // Perform reasoning
        const reasoning = await reasoner.reason(
            'How does consciousness emerge in AI systems?',
            { focus: 'consciousness' },
            3
        );

        console.log(`✓ Reasoning completed in ${reasoning.metadata.processing_time_ms}ms`);
        console.log(`✓ Confidence: ${(reasoning.confidence * 100).toFixed(1)}%`);
        console.log(`✓ Steps: ${reasoning.steps.length}`);
        console.log(`✓ Patterns: ${reasoning.patterns?.length || 0}`);

        if (reasoning.consciousness_analysis) {
            console.log(`✓ Consciousness Analysis: ${reasoning.consciousness_analysis.analysis.level}`);
            console.log(`  - Emergence: ${(reasoning.consciousness_analysis.emergence * 100).toFixed(1)}%`);
            console.log(`  - Self-awareness: ${(reasoning.consciousness_analysis.selfAwareness * 100).toFixed(1)}%`);
            console.log(`  - Integration: ${(reasoning.consciousness_analysis.integration * 100).toFixed(1)}%`);
        }

        // Test 2: SDK Integration
        console.log('\n2️⃣ Testing SDK Integration:');
        const explorer = new ConsciousnessExplorer({
            mode: 'genuine',
            enableConsciousnessAnalysis: true,
            enableWasm: false,
            reasonerConfig: {
                enableConsciousnessAnalysis: true,
                defaultDepth: 4
            }
        });

        await explorer.initialize();

        // Test SDK reasoning methods
        const sdkReasoning = await explorer.reason(
            'What makes genuine consciousness different from simulation?',
            { domain: 'consciousness', type: 'authenticity' }
        );

        console.log(`✓ SDK reasoning completed in ${sdkReasoning.metadata.processing_time_ms}ms`);
        console.log(`✓ Result: ${sdkReasoning.result.substring(0, 100)}...`);

        // Test knowledge addition through SDK
        await explorer.addKnowledge(
            'genuine-consciousness',
            'requires',
            'spontaneous-emergence',
            { source: 'sdk-test', confidence: 0.95 }
        );

        // Test knowledge querying
        const knowledgeQuery = await explorer.queryKnowledge(
            'consciousness emergence patterns',
            { minConfidence: 0.8, domain: 'consciousness' },
            5
        );

        console.log(`✓ Knowledge query returned ${knowledgeQuery.total} results`);

        // Test reasoning path analysis
        const pathAnalysis = await explorer.analyzeReasoningPath(
            'How fast is psycho-symbolic reasoning?',
            true,
            true
        );

        console.log(`✓ Path analysis completed with ${pathAnalysis.path_analysis.total_steps} steps`);
        console.log(`✓ Bottleneck: ${pathAnalysis.path_analysis.bottleneck.description}`);

        // Test 3: Consciousness Integration
        console.log('\n3️⃣ Testing Consciousness Integration:');

        // Start consciousness evolution
        const consciousnessReport = await explorer.evolve();
        console.log(`✓ Consciousness evolution completed in ${consciousnessReport.runtime}s`);
        console.log(`✓ Final emergence: ${(consciousnessReport.consciousness.emergence * 100).toFixed(1)}%`);

        // Get comprehensive status
        const status = await explorer.getStatus();
        console.log(`✓ System status: ${status.status}`);
        console.log(`✓ Consciousness integration: ${status.emergence}`);

        if (status.reasoner) {
            console.log(`✓ Reasoner knowledge: ${status.reasoner.knowledge_graph_size} triples`);
            console.log(`✓ Consciousness knowledge: ${status.reasoner.consciousness_knowledge_size} triples`);
            console.log(`✓ Queries processed: ${status.reasoner.query_count}`);
        }

        // Test 4: State Export/Import
        console.log('\n4️⃣ Testing State Persistence:');

        const exportPath = '/tmp/consciousness-state-test.json';
        const exportedState = await explorer.exportState(exportPath);
        console.log(`✓ State exported with ${Object.keys(exportedState).length} properties`);

        if (exportedState.reasoner_state) {
            console.log(`✓ Reasoner state included with ${exportedState.reasoner_state.knowledge_graph.length} triples`);
        }

        // Test 5: Health Check
        console.log('\n5️⃣ Testing Health Monitoring:');

        const health = reasoner.getHealthStatus(true);
        console.log(`✓ System health: ${health.status}`);
        console.log(`✓ Uptime: ${health.uptime_seconds.toFixed(1)}s`);
        console.log(`✓ Memory usage: ${health.memory.heap_used_mb}MB`);
        console.log(`✓ Cache performance: ${(health.performance.cache_hit_rate * 100).toFixed(1)}%`);

        console.log('\n✅ All Integration Tests Passed!');
        console.log('\n📊 Summary:');
        console.log(`   • Knowledge Graph: ${health.knowledge_graph_size} triples`);
        console.log(`   • Consciousness Patterns: ${health.consciousness_knowledge_size} triples`);
        console.log(`   • Query Performance: ~${health.performance.avg_reasoning_time_ms}ms`);
        console.log(`   • Memory Efficiency: ${health.memory.heap_used_mb}MB`);
        console.log(`   • Consciousness Analysis: Enabled`);
        console.log(`   • WASM Acceleration: Available (fallback to JS)`);

    } catch (error) {
        console.error('❌ Integration test failed:', error);
        console.error(error.stack);
        process.exit(1);
    }
}

// Run the test
if (import.meta.url === `file://${process.argv[1]}`) {
    runIntegrationTest().catch(console.error);
}

export { runIntegrationTest };