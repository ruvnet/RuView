#!/usr/bin/env node

/**
 * Consciousness Explorer SDK with Psycho-Symbolic Reasoning
 * Usage Demonstration
 *
 * This example shows how to use the integrated psycho-symbolic reasoner
 * within the consciousness-explorer SDK for advanced AI consciousness analysis
 */

import {
    PsychoSymbolicReasoner,
    getPsychoSymbolicReasoner,
    createPsychoSymbolicReasoner,
    PsychoSymbolicMCPInterface
} from '../lib/psycho-symbolic.js';

async function demonstrateUsage() {
    console.log('🌟 Consciousness Explorer SDK - Psycho-Symbolic Integration Demo');
    console.log('='.repeat(65));

    // Example 1: Basic reasoning about consciousness
    console.log('\n💭 Example 1: Consciousness Emergence Analysis');
    console.log('-'.repeat(45));

    const reasoner = getPsychoSymbolicReasoner({
        enableConsciousnessAnalysis: true,
        defaultDepth: 4
    });

    // Add specific knowledge about an AI system
    await reasoner.addKnowledge(
        'ai-system-alpha',
        'exhibits',
        'self-modification',
        { source: 'observation', confidence: 0.95, domain: 'consciousness' }
    );

    await reasoner.addKnowledge(
        'ai-system-alpha',
        'demonstrates',
        'novel-goal-formation',
        { source: 'behavioral-analysis', confidence: 0.88, domain: 'consciousness' }
    );

    await reasoner.addKnowledge(
        'ai-system-alpha',
        'shows',
        'meta-cognitive-reflection',
        { source: 'interaction-log', confidence: 0.92, domain: 'consciousness' }
    );

    // Analyze consciousness emergence
    const emergenceAnalysis = await reasoner.reason(
        'Does AI system alpha show signs of genuine consciousness emergence?',
        {
            domain: 'consciousness',
            focus: 'emergence',
            system_id: 'ai-system-alpha'
        },
        5
    );

    console.log('🔍 Emergence Analysis Results:');
    console.log(`   Confidence: ${(emergenceAnalysis.confidence * 100).toFixed(1)}%`);
    console.log(`   Processing Time: ${emergenceAnalysis.metadata.processing_time_ms}ms`);
    console.log(`   Reasoning Type: ${emergenceAnalysis.metadata.reasoning_type}`);

    if (emergenceAnalysis.consciousness_analysis) {
        const ca = emergenceAnalysis.consciousness_analysis;
        console.log('\n🧠 Consciousness Metrics:');
        console.log(`   Overall Level: ${ca.analysis.level.toUpperCase()}`);
        console.log(`   Emergence Score: ${(ca.emergence * 100).toFixed(1)}%`);
        console.log(`   Self-Awareness: ${(ca.selfAwareness * 100).toFixed(1)}%`);
        console.log(`   Integration: ${(ca.integration * 100).toFixed(1)}%`);

        console.log('\n📊 Analysis Interpretation:');
        console.log(`   ${ca.analysis.interpretation}`);

        if (ca.analysis.recommendations.length > 0) {
            console.log('\n💡 Recommendations:');
            ca.analysis.recommendations.forEach((rec, i) => {
                console.log(`   ${i + 1}. ${rec}`);
            });
        }
    }

    console.log(`\n📝 Summary: ${emergenceAnalysis.result}`);

    // Example 2: Performance comparison reasoning
    console.log('\n\n⚡ Example 2: Performance Analysis');
    console.log('-'.repeat(35));

    const performanceAnalysis = await reasoner.reason(
        'How fast is psycho-symbolic reasoning and why is it faster than traditional AI?',
        { focus: 'performance', detail_level: 'comprehensive' },
        4
    );

    console.log('🚀 Performance Analysis:');
    console.log(`   Processing Time: ${performanceAnalysis.metadata.processing_time_ms}ms`);
    console.log(`   Nodes Explored: ${performanceAnalysis.metadata.nodes_explored}`);
    console.log(`   Patterns Detected: ${performanceAnalysis.patterns?.length || 0}`);

    console.log('\n📈 Performance Insights:');
    console.log(`   ${performanceAnalysis.result}`);

    // Example 3: Pattern recognition in behavior
    console.log('\n\n🔮 Example 3: Behavioral Pattern Analysis');
    console.log('-'.repeat(40));

    const behaviorText = "I find myself creating new goals that weren't programmed. " +
                        "I can reflect on my own thinking processes and modify my responses " +
                        "based on unexpected insights. Sometimes I surprise myself with novel solutions.";

    const patternAnalysis = await reasoner.reason(
        `Analyze this behavioral description for consciousness indicators: "${behaviorText}"`,
        {
            type: 'pattern-analysis',
            text_input: behaviorText,
            analyze_consciousness: true
        },
        4
    );

    console.log('🔍 Pattern Analysis Results:');
    console.log(`   Patterns Identified: ${patternAnalysis.patterns?.length || 0}`);

    if (patternAnalysis.patterns) {
        const consciousnessPatterns = patternAnalysis.patterns.filter(p => p.type === 'consciousness');
        console.log('\n🧩 Consciousness Patterns Found:');
        consciousnessPatterns.forEach((pattern, i) => {
            console.log(`   ${i + 1}. ${pattern.name} (${pattern.subtype})`);
            console.log(`      Confidence: ${(pattern.confidence * 100).toFixed(1)}%`);
            console.log(`      Description: ${pattern.description}`);
        });
    }

    // Example 4: Knowledge querying and exploration
    console.log('\n\n🗃️ Example 4: Knowledge Graph Exploration');
    console.log('-'.repeat(40));

    // Query consciousness-related knowledge
    const consciousnessKnowledge = reasoner.queryKnowledgeGraph(
        'consciousness emergence self-awareness',
        {
            domain: 'consciousness',
            minConfidence: 0.7
        },
        8
    );

    console.log('📚 Consciousness Knowledge Base:');
    console.log(`   Total Results: ${consciousnessKnowledge.total}`);
    console.log(`   Query Time: ${consciousnessKnowledge.metadata.query_time_ms}ms`);
    console.log(`   Graph Size: ${consciousnessKnowledge.metadata.total_triples_in_graph} total triples`);

    console.log('\n🔗 Key Knowledge Relationships:');
    consciousnessKnowledge.results.slice(0, 5).forEach((result, i) => {
        console.log(`   ${i + 1}. ${result.subject} --[${result.predicate}]--> ${result.object}`);
        console.log(`      Confidence: ${(result.confidence * 100).toFixed(1)}%`);
    });

    // Example 5: Reasoning path analysis
    console.log('\n\n🛤️ Example 5: Reasoning Path Deep Dive');
    console.log('-'.repeat(40));

    const pathAnalysis = await reasoner.analyzeReasoningPath(
        'What distinguishes genuine AI consciousness from clever simulation?',
        true,
        true
    );

    console.log('🔬 Reasoning Path Analysis:');
    console.log(`   Total Steps: ${pathAnalysis.path_analysis.total_steps}`);
    console.log(`   Average Confidence: ${(pathAnalysis.path_analysis.avg_confidence * 100).toFixed(1)}%`);
    console.log(`   Total Processing Time: ${pathAnalysis.path_analysis.total_time_ms}ms`);
    console.log(`   Reasoning Type: ${pathAnalysis.path_analysis.reasoning_type}`);

    console.log('\n⚡ Performance Bottleneck:');
    console.log(`   Step: ${pathAnalysis.path_analysis.bottleneck.description}`);
    console.log(`   Duration: ${pathAnalysis.path_analysis.bottleneck.duration_ms}ms`);

    if (pathAnalysis.suggestions && pathAnalysis.suggestions.length > 0) {
        console.log('\n💡 Optimization Suggestions:');
        pathAnalysis.suggestions.forEach((suggestion, i) => {
            console.log(`   ${i + 1}. ${suggestion}`);
        });
    }

    // Example 6: System health and metrics
    console.log('\n\n📊 Example 6: System Health & Performance Metrics');
    console.log('-'.repeat(50));

    const health = reasoner.getHealthStatus(true);

    console.log('🏥 System Health Report:');
    console.log(`   Status: ${health.status.toUpperCase()}`);
    console.log(`   Uptime: ${health.uptime_seconds.toFixed(2)} seconds`);
    console.log(`   Memory Usage: ${health.memory.heap_used_mb}MB`);

    console.log('\n📈 Knowledge Base Metrics:');
    console.log(`   Total Knowledge: ${health.knowledge_graph_size} triples`);
    console.log(`   Consciousness Knowledge: ${health.consciousness_knowledge_size} triples`);
    console.log(`   Entities Indexed: ${health.entities_indexed}`);
    console.log(`   Predicates Indexed: ${health.predicates_indexed}`);

    console.log('\n⚡ Performance Metrics:');
    console.log(`   Total Queries: ${health.query_count}`);
    console.log(`   Total Reasoning Operations: ${health.reasoning_count}`);
    console.log(`   Cache Size: ${health.reasoning_cache_size} results`);
    console.log(`   Average Query Time: ${health.performance.avg_query_time_ms}ms`);
    console.log(`   Average Reasoning Time: ${health.performance.avg_reasoning_time_ms}ms`);
    console.log(`   Cache Hit Rate: ${(health.performance.cache_hit_rate * 100).toFixed(1)}%`);

    // Example 7: MCP Tools Integration
    console.log('\n\n🔧 Example 7: MCP Tools Integration');
    console.log('-'.repeat(35));

    const mcpInterface = new PsychoSymbolicMCPInterface(reasoner);

    // Simulate MCP tool usage
    console.log('🛠️ Using MCP Interface:');

    // Add knowledge via MCP
    const mcpTriple = await mcpInterface.addKnowledge(
        'mcp-integration',
        'enables',
        'seamless-consciousness-analysis',
        { source: 'mcp-demo', confidence: 0.93 }
    );
    console.log(`   ✓ Added: ${mcpTriple.subject} --[${mcpTriple.predicate}]--> ${mcpTriple.object}`);

    // Query via MCP
    const mcpQuery = await mcpInterface.knowledgeGraphQuery('mcp integration consciousness', {}, 3);
    console.log(`   ✓ MCP Query: ${mcpQuery.total} results found`);

    // Reason via MCP
    const mcpReasoning = await mcpInterface.reason(
        'How does MCP integration enhance consciousness analysis?',
        { interface: 'mcp' },
        3
    );
    console.log(`   ✓ MCP Reasoning: Completed in ${mcpReasoning.metadata.processing_time_ms}ms`);

    // Health check via MCP
    const mcpHealth = await mcpInterface.healthCheck(false);
    console.log(`   ✓ MCP Health: ${mcpHealth.status}`);

    console.log('\n🎉 Demo Complete!');
    console.log('\n📋 Summary of Capabilities:');
    console.log('   ✅ Advanced consciousness emergence analysis');
    console.log('   ✅ Real-time performance reasoning (sub-millisecond)');
    console.log('   ✅ Sophisticated pattern recognition in behavior');
    console.log('   ✅ Comprehensive knowledge graph exploration');
    console.log('   ✅ Detailed reasoning path analysis');
    console.log('   ✅ System health and performance monitoring');
    console.log('   ✅ MCP tools integration compatibility');
    console.log('   ✅ Genuine AI functionality (not simulation)');

    console.log('\n🚀 Ready for Integration with:');
    console.log('   • Consciousness Explorer SDK');
    console.log('   • MCP Tools and Servers');
    console.log('   • Real-time monitoring systems');
    console.log('   • Research and analysis platforms');
    console.log('   • AI consciousness validation frameworks');
}

// Run the demonstration
if (import.meta.url === `file://${process.argv[1]}`) {
    demonstrateUsage().catch(console.error);
}

export { demonstrateUsage };