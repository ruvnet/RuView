"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const ruvector_1 = require("ruvector");
const ui_server_js_1 = require("../ui-server.js");
/**
 * Example: Interactive Graph Explorer UI
 *
 * This example demonstrates how to launch the interactive web UI
 * for exploring vector embeddings as a force-directed graph.
 */
async function main() {
    console.log('🚀 Starting RuVector Graph Explorer Example\n');
    // Initialize database
    const db = new ruvector_1.VectorDB({
        dimension: 384,
        distanceMetric: 'cosine'
    });
    console.log('📊 Populating database with sample data...\n');
    // Create sample embeddings with different categories
    const categories = ['research', 'code', 'documentation', 'test'];
    const sampleData = [];
    for (let i = 0; i < 50; i++) {
        const category = categories[i % categories.length];
        // Generate random embedding with some structure
        const baseVector = Array.from({ length: 384 }, () => Math.random() - 0.5);
        // Add category-specific bias to make similar items cluster
        const categoryBias = i % categories.length;
        for (let j = 0; j < 96; j++) {
            baseVector[j + categoryBias * 96] += 0.5;
        }
        // Normalize vector
        const magnitude = Math.sqrt(baseVector.reduce((sum, val) => sum + val * val, 0));
        const embedding = baseVector.map(val => val / magnitude);
        const id = `node-${i.toString().padStart(3, '0')}`;
        const metadata = {
            label: `${category} ${i}`,
            category,
            timestamp: Date.now() - Math.random() * 86400000 * 30,
            importance: Math.random(),
            tags: [category, `tag-${Math.floor(Math.random() * 5)}`]
        };
        sampleData.push({ id, embedding, metadata });
    }
    // Add all vectors to database
    for (const { id, embedding, metadata } of sampleData) {
        await db.add(id, embedding, metadata);
    }
    console.log(`✅ Added ${sampleData.length} sample nodes\n`);
    // Get database statistics
    const stats = await db.getStats();
    console.log('📈 Database Statistics:');
    console.log(`   Total vectors: ${stats.totalVectors}`);
    console.log(`   Dimension: ${stats.dimension}`);
    console.log(`   Distance metric: ${stats.distanceMetric}\n`);
    // Start UI server
    console.log('🌐 Starting UI server...\n');
    const port = parseInt(process.env.PORT || '3000');
    const server = await (0, ui_server_js_1.startUIServer)(db, port);
    console.log('✨ UI Features:');
    console.log('   • Interactive force-directed graph visualization');
    console.log('   • Drag nodes to reposition');
    console.log('   • Zoom and pan with mouse/touch');
    console.log('   • Search nodes by ID or metadata');
    console.log('   • Click nodes to view metadata');
    console.log('   • Double-click or use "Find Similar" to highlight similar nodes');
    console.log('   • Export graph as PNG or SVG');
    console.log('   • Real-time updates via WebSocket');
    console.log('   • Responsive design for mobile devices\n');
    console.log('💡 Try these actions:');
    console.log('   1. Search for "research" to filter nodes');
    console.log('   2. Click any node to see its metadata');
    console.log('   3. Click "Find Similar Nodes" to discover connections');
    console.log('   4. Adjust the similarity threshold slider');
    console.log('   5. Export the visualization as PNG or SVG\n');
    // Demonstrate adding nodes in real-time
    console.log('🔄 Adding nodes in real-time (every 10 seconds)...\n');
    let counter = 50;
    const interval = setInterval(async () => {
        const category = categories[counter % categories.length];
        const baseVector = Array.from({ length: 384 }, () => Math.random() - 0.5);
        const categoryBias = counter % categories.length;
        for (let j = 0; j < 96; j++) {
            baseVector[j + categoryBias * 96] += 0.5;
        }
        const magnitude = Math.sqrt(baseVector.reduce((sum, val) => sum + val * val, 0));
        const embedding = baseVector.map(val => val / magnitude);
        const id = `node-${counter.toString().padStart(3, '0')}`;
        const metadata = {
            label: `${category} ${counter}`,
            category,
            timestamp: Date.now(),
            importance: Math.random(),
            tags: [category, `tag-${Math.floor(Math.random() * 5)}`]
        };
        await db.add(id, embedding, metadata);
        // Notify UI of update
        server.notifyGraphUpdate();
        console.log(`✅ Added new node: ${id} (${category})`);
        counter++;
        // Stop after adding 10 more nodes
        if (counter >= 60) {
            clearInterval(interval);
            console.log('\n✨ Real-time updates complete!\n');
        }
    }, 10000);
    // Handle graceful shutdown
    process.on('SIGINT', async () => {
        console.log('\n\n🛑 Shutting down gracefully...');
        clearInterval(interval);
        await server.stop();
        await db.close();
        console.log('👋 Goodbye!\n');
        process.exit(0);
    });
}
// Run example
main().catch(error => {
    console.error('❌ Error:', error);
    process.exit(1);
});
//# sourceMappingURL=ui-example.js.map