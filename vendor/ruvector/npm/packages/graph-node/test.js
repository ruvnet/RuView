const { GraphDatabase, version, hello } = require('./index.js');
const fs = require('fs');
const path = require('path');
const os = require('os');

console.log('RuVector Graph Node Test');
console.log('========================\n');

// Test 1: Version and hello
console.log('1. Testing version and hello functions:');
console.log('   Version:', version());
console.log('   Hello:', hello());
console.log('   ✓ Basic functions work\n');

// Test 2: Create database
console.log('2. Creating graph database:');
const db = new GraphDatabase({
  distanceMetric: 'Cosine',
  dimensions: 3
});
console.log('   ✓ Database created\n');

// Test 3: Create nodes
console.log('3. Creating nodes:');
(async () => {
  try {
    const nodeId1 = await db.createNode({
      id: 'alice',
      embedding: new Float32Array([1.0, 0.0, 0.0]),
      labels: ['Person', 'Employee'],
      properties: { name: 'Alice', age: '30' }
    });
    console.log('   Created node:', nodeId1);

    const nodeId2 = await db.createNode({
      id: 'bob',
      embedding: new Float32Array([0.0, 1.0, 0.0]),
      labels: ['Person'],
      properties: { name: 'Bob', age: '25' }
    });
    console.log('   Created node:', nodeId2);
    console.log('   ✓ Nodes created\n');

    // Test 4: Create edge
    console.log('4. Creating edge:');
    const edgeId = await db.createEdge({
      from: 'alice',
      to: 'bob',
      description: 'knows',
      embedding: new Float32Array([0.5, 0.5, 0.0]),
      confidence: 0.95
    });
    console.log('   Created edge:', edgeId);
    console.log('   ✓ Edge created\n');

    // Test 5: Create hyperedge
    console.log('5. Creating hyperedge:');
    const nodeId3 = await db.createNode({
      id: 'charlie',
      embedding: new Float32Array([0.0, 0.0, 1.0])
    });

    const hyperedgeId = await db.createHyperedge({
      nodes: ['alice', 'bob', 'charlie'],
      description: 'collaborated_on_project',
      embedding: new Float32Array([0.33, 0.33, 0.33]),
      confidence: 0.85
    });
    console.log('   Created hyperedge:', hyperedgeId);
    console.log('   ✓ Hyperedge created\n');

    // Test 6: Query
    console.log('6. Querying graph:');
    const results = await db.query('MATCH (n) RETURN n');
    console.log('   Query results:', JSON.stringify(results, null, 2));
    console.log('   ✓ Query executed\n');

    // Test 7: Search hyperedges
    console.log('7. Searching hyperedges:');
    const searchResults = await db.searchHyperedges({
      embedding: new Float32Array([0.3, 0.3, 0.3]),
      k: 5
    });
    console.log('   Search results:', searchResults);
    console.log('   ✓ Search completed\n');

    // Test 8: k-hop neighbors
    console.log('8. Finding k-hop neighbors:');
    const neighbors = await db.kHopNeighbors('alice', 2);
    console.log('   Neighbors:', neighbors);
    console.log('   ✓ Neighbors found\n');

    // Test 9: Statistics
    console.log('9. Getting statistics:');
    const stats = await db.stats();
    console.log('   Stats:', stats);
    console.log('   ✓ Statistics retrieved\n');

    // Test 10: Transactions
    console.log('10. Testing transactions:');
    const txId = await db.begin();
    console.log('    Transaction started:', txId);
    await db.commit(txId);
    console.log('    Transaction committed');
    console.log('    ✓ Transaction test passed\n');

    // Test 11: Batch insert
    console.log('11. Testing batch insert:');
    const batchResult = await db.batchInsert({
      nodes: [
        { id: 'n1', embedding: new Float32Array([1, 0, 0]) },
        { id: 'n2', embedding: new Float32Array([0, 1, 0]) }
      ],
      edges: [
        {
          from: 'n1',
          to: 'n2',
          description: 'connects',
          embedding: new Float32Array([0.5, 0.5, 0])
        }
      ]
    });
    console.log('    Batch result:', batchResult);
    console.log('    ✓ Batch insert completed\n');

    // Test 12: Persistence
    console.log('12. Testing persistence:');
    const tmpDir = os.tmpdir();
    const dbPath = path.join(tmpDir, `ruvector-test-${Date.now()}.db`);

    console.log('    Creating persistent database at:', dbPath);
    const persistentDb = new GraphDatabase({
      distanceMetric: 'Cosine',
      dimensions: 3,
      storagePath: dbPath
    });

    console.log('    isPersistent():', persistentDb.isPersistent());
    console.log('    getStoragePath():', persistentDb.getStoragePath());

    // Add data to persistent database
    await persistentDb.createNode({
      id: 'persistent_node_1',
      embedding: new Float32Array([1.0, 0.5, 0.2]),
      labels: ['PersistentTest'],
      properties: { testKey: 'testValue' }
    });
    console.log('    Created node in persistent database');

    const persistentStats = await persistentDb.stats();
    console.log('    Persistent DB stats:', persistentStats);

    // Test opening existing database
    console.log('    Opening existing database with GraphDatabase.open()...');
    const reopenedDb = GraphDatabase.open(dbPath);
    console.log('    Reopened isPersistent():', reopenedDb.isPersistent());

    // Cleanup
    try {
      fs.unlinkSync(dbPath);
      console.log('    Cleaned up test database');
    } catch (e) {
      // Ignore cleanup errors
    }
    console.log('    ✓ Persistence test passed\n');

    console.log('✅ All tests passed!');
  } catch (error) {
    console.error('❌ Test failed:', error);
    process.exit(1);
  }
})();
