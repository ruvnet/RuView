/**
 * Basic usage example for Ruvector (Node.js)
 *
 * Demonstrates:
 * - Creating a database
 * - Inserting vectors
 * - Searching for similar vectors
 */

const { VectorDB } = require('ruvector');

async function main() {
    console.log('🚀 Ruvector Basic Usage Example (Node.js)\n');

    // 1. Create a database
    console.log('1. Creating database...');
    const db = new VectorDB({
        dimensions: 128,
        storagePath: './examples_basic_node.db',
        distanceMetric: 'cosine'
    });
    console.log('   ✓ Database created with 128 dimensions\n');

    // 2. Insert a single vector
    console.log('2. Inserting single vector...');
    const vector = new Float32Array(128).fill(0.1);
    const id = await db.insert({
        id: 'doc_001',
        vector: vector,
        metadata: { text: 'Example document' }
    });
    console.log(`   ✓ Inserted vector: ${id}\n`);

    // 3. Insert multiple vectors
    console.log('3. Inserting multiple vectors...');
    const entries = Array.from({ length: 100 }, (_, i) => ({
        id: `doc_${String(i + 2).padStart(3, '0')}`,
        vector: new Float32Array(128).fill(0.1 + i * 0.001),
        metadata: { index: i + 2 }
    }));

    const ids = await db.insertBatch(entries);
    console.log(`   ✓ Inserted ${ids.length} vectors\n`);

    // 4. Search for similar vectors
    console.log('4. Searching for similar vectors...');
    const queryVector = new Float32Array(128).fill(0.15);
    const results = await db.search({
        vector: queryVector,
        k: 5,
        includeMetadata: true
    });

    console.log(`   ✓ Found ${results.length} results:`);
    results.forEach((result, i) => {
        console.log(`     ${i + 1}. ID: ${result.id}, Distance: ${result.distance.toFixed(6)}`);
    });
    console.log();

    // 5. Get database stats
    console.log('5. Database statistics:');
    const total = db.count();
    console.log(`   ✓ Total vectors: ${total}\n`);

    console.log('✅ Example completed successfully!');
}

main().catch(console.error);
