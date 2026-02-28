const { VectorDB } = require('./index.js');

async function test() {
  console.log('Testing native module...');

  try {
    // Create database
    const db = VectorDB.withDimensions(128);
    console.log('✓ Created database');

    // Insert vector
    const id = await db.insert({
      vector: new Float32Array(128).fill(0.5)
    });
    console.log('✓ Inserted vector:', id);

    // Search
    const results = await db.search({
      vector: new Float32Array(128).fill(0.5),
      k: 1
    });
    console.log('✓ Search results:', results);

    // Check length
    const len = await db.len();
    console.log('✓ Database length:', len);

    console.log('\n✅ All tests passed!');
  } catch (error) {
    console.error('❌ Test failed:', error);
    process.exit(1);
  }
}

test();
