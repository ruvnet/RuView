"use strict";
/**
 * Complete Integration Example for RuVector Extensions
 *
 * This example demonstrates all 5 major features:
 * 1. Real Embeddings (OpenAI/Cohere/Anthropic/HuggingFace)
 * 2. Database Persistence (save/load/snapshots)
 * 3. Graph Exports (GraphML, GEXF, Neo4j, D3.js, NetworkX)
 * 4. Temporal Tracking (version control, time-travel)
 * 5. Interactive Web UI (D3.js visualization)
 */
Object.defineProperty(exports, "__esModule", { value: true });
const ruvector_1 = require("ruvector");
const index_js_1 = require("../dist/index.js");
async function main() {
    console.log('🚀 RuVector Extensions - Complete Integration Example\n');
    console.log('='.repeat(60));
    // ========== 1. Initialize Database ==========
    console.log('\n📊 Step 1: Initialize VectorDB');
    const db = new ruvector_1.VectorDB({
        dimensions: 1536,
        distanceMetric: 'Cosine',
        storagePath: './data/example.db'
    });
    console.log('✅ Database initialized (1536 dimensions, Cosine similarity)');
    // ========== 2. Real Embeddings Integration ==========
    console.log('\n🔤 Step 2: Generate Real Embeddings with OpenAI');
    const openai = new index_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'demo-key',
        model: 'text-embedding-3-small'
    });
    const documents = [
        { id: '1', text: 'Machine learning is a subset of artificial intelligence', category: 'AI' },
        { id: '2', text: 'Deep learning uses neural networks with multiple layers', category: 'AI' },
        { id: '3', text: 'Natural language processing enables computers to understand text', category: 'NLP' },
        { id: '4', text: 'Computer vision allows machines to interpret visual information', category: 'CV' },
        { id: '5', text: 'Reinforcement learning trains agents through rewards and penalties', category: 'RL' }
    ];
    console.log(`Embedding ${documents.length} documents...`);
    await (0, index_js_1.embedAndInsert)(db, openai, documents.map(d => ({
        id: d.id,
        text: d.text,
        metadata: { category: d.category }
    })), {
        onProgress: (progress) => {
            console.log(`  Progress: ${progress.percentage}% - ${progress.message}`);
        }
    });
    console.log('✅ Documents embedded and inserted');
    // ========== 3. Database Persistence ==========
    console.log('\n💾 Step 3: Database Persistence');
    const persistence = new index_js_1.DatabasePersistence(db, {
        baseDir: './data/backups',
        format: 'json',
        compression: 'gzip',
        autoSaveInterval: 60000 // Auto-save every minute
    });
    // Save database
    console.log('Saving database...');
    await persistence.save({
        onProgress: (p) => console.log(`  ${p.percentage}% - ${p.message}`)
    });
    console.log('✅ Database saved');
    // Create snapshot
    console.log('Creating snapshot...');
    const snapshot = await persistence.createSnapshot('initial-state', {
        description: 'Initial state with 5 documents',
        tags: ['demo', 'v1.0']
    });
    console.log(`✅ Snapshot created: ${snapshot.id}`);
    // ========== 4. Temporal Tracking ==========
    console.log('\n⏰ Step 4: Temporal Tracking & Version Control');
    const temporal = new index_js_1.TemporalTracker();
    // Track initial state
    temporal.trackChange({
        type: index_js_1.ChangeType.ADDITION,
        path: 'documents',
        before: null,
        after: { count: 5, categories: ['AI', 'NLP', 'CV', 'RL'] },
        timestamp: Date.now(),
        metadata: { operation: 'initial_load' }
    });
    // Create version
    const v1 = await temporal.createVersion({
        description: 'Initial dataset with 5 AI/ML documents',
        tags: ['v1.0', 'baseline'],
        author: 'demo-user'
    });
    console.log(`✅ Version created: ${v1.id}`);
    // Simulate a change
    temporal.trackChange({
        type: index_js_1.ChangeType.ADDITION,
        path: 'documents.6',
        before: null,
        after: { id: '6', text: 'Transformer models revolutionized NLP', category: 'NLP' },
        timestamp: Date.now()
    });
    const v2 = await temporal.createVersion({
        description: 'Added transformer document',
        tags: ['v1.1']
    });
    console.log(`✅ Version updated: ${v2.id}`);
    // Compare versions
    const diff = await temporal.compareVersions(v1.id, v2.id);
    console.log(`📊 Changes: ${diff.changes.length} modifications`);
    console.log(`   Added: ${diff.summary.added}, Modified: ${diff.summary.modified}`);
    // ========== 5. Graph Exports ==========
    console.log('\n📈 Step 5: Export Similarity Graphs');
    // Build graph from vectors
    console.log('Building similarity graph...');
    const entries = await Promise.all(documents.map(async (d) => {
        const vector = await db.get(d.id);
        return vector;
    }));
    const graph = await (0, index_js_1.buildGraphFromEntries)(entries.filter(e => e !== null), {
        threshold: 0.7, // Only edges with >70% similarity
        maxNeighbors: 3
    });
    console.log(`✅ Graph built: ${graph.nodes.length} nodes, ${graph.edges.length} edges`);
    // Export to multiple formats
    console.log('Exporting to formats...');
    // GraphML (for Gephi, yEd)
    const graphml = (0, index_js_1.exportToGraphML)(graph, {
        graphName: 'AI Concepts Network',
        includeVectors: false
    });
    console.log('  ✅ GraphML export ready (for Gephi/yEd)');
    // GEXF (for Gephi)
    const gexf = (0, index_js_1.exportToGEXF)(graph, {
        graphName: 'AI Knowledge Graph',
        graphDescription: 'Vector similarity network of AI concepts'
    });
    console.log('  ✅ GEXF export ready (for Gephi)');
    // Neo4j (for graph database)
    const neo4j = (0, index_js_1.exportToNeo4j)(graph, {
        includeMetadata: true
    });
    console.log('  ✅ Neo4j Cypher queries ready');
    // D3.js (for web visualization)
    const d3Data = (0, index_js_1.exportToD3)(graph);
    console.log('  ✅ D3.js JSON ready (for web viz)');
    // ========== 6. Interactive Web UI ==========
    console.log('\n🌐 Step 6: Launch Interactive Web UI');
    console.log('Starting web server...');
    const uiServer = await (0, index_js_1.startUIServer)(db, 3000);
    console.log('✅ Web UI started at http://localhost:3000');
    console.log('\n📱 Features:');
    console.log('   • Force-directed graph visualization');
    console.log('   • Interactive node dragging & zoom');
    console.log('   • Real-time similarity search');
    console.log('   • Metadata inspection');
    console.log('   • Export as PNG/SVG');
    console.log('   • WebSocket live updates');
    // ========== Summary ==========
    console.log('\n' + '='.repeat(60));
    console.log('🎉 Complete Integration Successful!\n');
    console.log('Summary:');
    console.log(`  📊 Database: ${await db.len()} vectors (1536-dim)`);
    console.log(`  💾 Persistence: 1 snapshot, auto-save enabled`);
    console.log(`  ⏰ Versions: 2 versions tracked`);
    console.log(`  📈 Graph: ${graph.nodes.length} nodes, ${graph.edges.length} edges`);
    console.log(`  📦 Exports: GraphML, GEXF, Neo4j, D3.js ready`);
    console.log(`  🌐 UI Server: Running on port 3000`);
    console.log('\n📖 Next Steps:');
    console.log('  1. Open http://localhost:3000 to explore the graph');
    console.log('  2. Import GraphML into Gephi for advanced visualization');
    console.log('  3. Run Neo4j queries to analyze relationships');
    console.log('  4. Use temporal tracking to monitor changes over time');
    console.log('  5. Set up auto-save for production deployments');
    console.log('\n💡 Pro Tips:');
    console.log('  • Use OpenAI embeddings for best semantic understanding');
    console.log('  • Create snapshots before major updates');
    console.log('  • Enable auto-save for production (already enabled in this demo)');
    console.log('  • Export to Neo4j for complex graph queries');
    console.log('  • Monitor versions to track ontology evolution');
    console.log('\n🛑 Press Ctrl+C to stop the UI server');
    console.log('='.repeat(60) + '\n');
    // Keep server running
    process.on('SIGINT', async () => {
        console.log('\n\n🛑 Shutting down...');
        await uiServer.stop();
        await persistence.shutdown();
        console.log('✅ Cleanup complete. Goodbye!');
        process.exit(0);
    });
}
// Run example
main().catch(console.error);
//# sourceMappingURL=complete-integration.js.map