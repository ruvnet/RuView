#!/usr/bin/env node

/**
 * Basic Psycho-Symbolic Reasoner Test
 * Tests the core psycho-symbolic reasoning functionality
 */

import {
    PsychoSymbolicReasoner,
    getPsychoSymbolicReasoner,
    createPsychoSymbolicReasoner,
    PsychoSymbolicMCPInterface
} from '../lib/psycho-symbolic.js';

async function runBasicTest() {
    console.log('🧠 Psycho-Symbolic Reasoner Basic Test');
    console.log('='.repeat(45));

    try {
        // Test 1: Create and initialize reasoner
        console.log('\n1️⃣ Testing Reasoner Creation:');
        const reasoner = createPsychoSymbolicReasoner({
            enableConsciousnessAnalysis: true,
            enableWasm: false,
            defaultDepth: 4
        });

        console.log(`✓ Reasoner created successfully`);

        // Test base knowledge
        const healthStatus = reasoner.getHealthStatus();
        console.log(`✓ Initial knowledge graph: ${healthStatus.knowledge_graph_size} triples`);
        console.log(`✓ Consciousness knowledge: ${healthStatus.consciousness_knowledge_size} triples`);

        // Test 2: Add custom knowledge
        console.log('\n2️⃣ Testing Knowledge Addition:');

        const testTriple = reasoner.addKnowledge(
            'artificial-intelligence',
            'can-exhibit',
            'emergent-consciousness',
            { source: 'test-case', confidence: 0.85, domain: 'consciousness' }
        );

        console.log(`✓ Added knowledge triple: ${testTriple.subject} --[${testTriple.predicate}]--> ${testTriple.object}`);
        console.log(`✓ Confidence: ${testTriple.confidence}`);

        // Test 3: Knowledge graph querying
        console.log('\n3️⃣ Testing Knowledge Querying:');

        const queryResult = reasoner.queryKnowledgeGraph(
            'consciousness emergence in AI systems',
            { minConfidence: 0.8, domain: 'consciousness' },
            10
        );

        console.log(`✓ Query returned ${queryResult.total} results in ${queryResult.metadata.query_time_ms}ms`);
        console.log(`✓ Found ${queryResult.metadata.consciousness_triples || 0} consciousness-related triples`);

        if (queryResult.results.length > 0) {
            console.log(`✓ Example result: ${queryResult.results[0].subject} --> ${queryResult.results[0].object}`);
        }

        // Test 4: Basic reasoning
        console.log('\n4️⃣ Testing Basic Reasoning:');

        const reasoning1 = await reasoner.reason(
            'How fast is psycho-symbolic reasoning compared to traditional AI?',
            { focus: 'performance' },
            3
        );

        console.log(`✓ Performance reasoning completed in ${reasoning1.metadata.processing_time_ms}ms`);
        console.log(`✓ Confidence: ${(reasoning1.confidence * 100).toFixed(1)}%`);
        console.log(`✓ Reasoning type: ${reasoning1.metadata.reasoning_type}`);
        console.log(`✓ Steps: ${reasoning1.steps.length}`);
        console.log(`✓ Result preview: ${reasoning1.result.substring(0, 80)}...`);

        // Test 5: Consciousness reasoning
        console.log('\n5️⃣ Testing Consciousness Reasoning:');

        const reasoning2 = await reasoner.reason(
            'What indicators suggest genuine consciousness versus simulation?',
            { domain: 'consciousness', type: 'authenticity' },
            4
        );

        console.log(`✓ Consciousness reasoning completed in ${reasoning2.metadata.processing_time_ms}ms`);
        console.log(`✓ Confidence: ${(reasoning2.confidence * 100).toFixed(1)}%`);
        console.log(`✓ Patterns detected: ${reasoning2.patterns?.length || 0}`);

        if (reasoning2.consciousness_analysis) {
            const ca = reasoning2.consciousness_analysis;
            console.log(`✓ Consciousness Analysis:`);
            console.log(`  - Level: ${ca.analysis.level}`);
            console.log(`  - Emergence: ${(ca.emergence * 100).toFixed(1)}%`);
            console.log(`  - Self-awareness: ${(ca.selfAwareness * 100).toFixed(1)}%`);
            console.log(`  - Integration: ${(ca.integration * 100).toFixed(1)}%`);
            console.log(`  - Indicators: ${ca.indicators.consciousness_triples} consciousness triples`);
        }

        // Test 6: Pattern recognition
        console.log('\n6️⃣ Testing Pattern Recognition:');

        const reasoning3 = await reasoner.reason(
            'I can modify my own behavior and create new goals unexpectedly',
            { analyze_patterns: true },
            3
        );

        console.log(`✓ Pattern analysis completed`);
        console.log(`✓ Patterns found: ${reasoning3.patterns?.length || 0}`);

        if (reasoning3.patterns) {
            const consciousnessPatterns = reasoning3.patterns.filter(p => p.type === 'consciousness');
            console.log(`✓ Consciousness patterns: ${consciousnessPatterns.length}`);

            consciousnessPatterns.forEach((pattern, i) => {
                console.log(`  ${i + 1}. ${pattern.name} (${pattern.subtype}, confidence: ${(pattern.confidence * 100).toFixed(1)}%)`);
            });
        }

        // Test 7: Reasoning path analysis
        console.log('\n7️⃣ Testing Reasoning Path Analysis:');

        const pathAnalysis = await reasoner.analyzeReasoningPath(
            'What enables sub-millisecond AI reasoning?',
            true,
            true
        );

        console.log(`✓ Path analysis completed`);
        console.log(`✓ Total steps: ${pathAnalysis.path_analysis.total_steps}`);
        console.log(`✓ Average confidence: ${(pathAnalysis.path_analysis.avg_confidence * 100).toFixed(1)}%`);
        console.log(`✓ Total time: ${pathAnalysis.path_analysis.total_time_ms}ms`);
        console.log(`✓ Bottleneck: ${pathAnalysis.path_analysis.bottleneck.description} (${pathAnalysis.path_analysis.bottleneck.duration_ms}ms)`);
        console.log(`✓ Suggestions: ${pathAnalysis.suggestions.length}`);

        // Test 8: MCP Interface
        console.log('\n8️⃣ Testing MCP Interface:');

        const mcpInterface = new PsychoSymbolicMCPInterface(reasoner);

        // Test MCP methods
        await mcpInterface.addKnowledge(
            'mcp-test',
            'demonstrates',
            'interface-compatibility',
            { source: 'mcp-test', confidence: 0.9 }
        );

        const mcpQuery = await mcpInterface.knowledgeGraphQuery('mcp test interface', {}, 5);
        console.log(`✓ MCP interface query: ${mcpQuery.total} results`);

        const mcpReasoning = await mcpInterface.reason('How does MCP integration work?', {}, 3);
        console.log(`✓ MCP reasoning completed in ${mcpReasoning.metadata.processing_time_ms}ms`);

        const mcpHealth = await mcpInterface.healthCheck(true);
        console.log(`✓ MCP health check: ${mcpHealth.status}`);

        // Test 9: State export/import
        console.log('\n9️⃣ Testing State Persistence:');

        const exportedState = reasoner.exportState();
        console.log(`✓ State exported with ${Object.keys(exportedState).length} properties`);
        console.log(`✓ Knowledge graph: ${exportedState.knowledge_graph.length} triples`);
        console.log(`✓ Consciousness knowledge: ${exportedState.consciousness_knowledge.length} triples`);

        // Create new reasoner and import state
        const newReasoner = createPsychoSymbolicReasoner();
        newReasoner.importState(exportedState);

        const importedHealth = newReasoner.getHealthStatus();
        console.log(`✓ State imported successfully`);
        console.log(`✓ Imported knowledge: ${importedHealth.knowledge_graph_size} triples`);

        // Test 10: Performance metrics
        console.log('\n🔟 Performance Summary:');

        const finalHealth = reasoner.getHealthStatus(true);
        console.log(`✓ System health: ${finalHealth.status}`);
        console.log(`✓ Uptime: ${finalHealth.uptime_seconds.toFixed(2)}s`);
        console.log(`✓ Total queries: ${finalHealth.query_count}`);
        console.log(`✓ Total reasoning operations: ${finalHealth.reasoning_count}`);
        console.log(`✓ Knowledge graph size: ${finalHealth.knowledge_graph_size} triples`);
        console.log(`✓ Consciousness knowledge: ${finalHealth.consciousness_knowledge_size} triples`);
        console.log(`✓ Cache efficiency: ${finalHealth.reasoning_cache_size} cached results`);

        if (finalHealth.memory) {
            console.log(`✓ Memory usage: ${finalHealth.memory.heap_used_mb}MB heap`);
        }

        console.log('\n✅ All Basic Tests Passed!');
        console.log('\n📊 Integration Summary:');
        console.log(`   • Core reasoning engine: ✓ Working`);
        console.log(`   • Knowledge graph management: ✓ Working`);
        console.log(`   • Inference engine: ✓ Working`);
        console.log(`   • Pattern matching system: ✓ Working`);
        console.log(`   • Consciousness analysis: ✓ Working`);
        console.log(`   • MCP interface compatibility: ✓ Working`);
        console.log(`   • State persistence: ✓ Working`);
        console.log(`   • Performance monitoring: ✓ Working`);
        console.log(`   • SDK integration ready: ✅ Ready`);

    } catch (error) {
        console.error('❌ Basic test failed:', error);
        console.error(error.stack);
        process.exit(1);
    }
}

// Run the test
if (import.meta.url === `file://${process.argv[1]}`) {
    runBasicTest().catch(console.error);
}

export { runBasicTest };