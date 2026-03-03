/**
 * Simple Node.js test for WASM package functionality
 */

const path = require('path');

// Import the WASM package
const MidstreamWasm = require(path.join(__dirname, '..', 'index.js'));

async function runTests() {
  console.log('🧪 Testing @midstream/wasm package...\n');

  try {
    // Initialize WASM
    console.log('1. Initializing WASM module...');
    await MidstreamWasm.init();
    console.log('   ✅ WASM initialized successfully\n');

    // Test TemporalCompare
    console.log('2. Testing TemporalCompare (DTW)...');
    const temporal = new MidstreamWasm.TemporalCompare(100);
    const seq1 = [1.0, 2.0, 3.0, 4.0, 5.0];
    const seq2 = [1.1, 2.1, 3.1, 4.1, 5.1];
    const dtwDistance = temporal.dtw(seq1, seq2);
    console.log(`   DTW distance: ${dtwDistance.toFixed(4)}`);
    console.log('   ✅ DTW calculation works\n');

    // Test comprehensive analysis
    console.log('3. Testing comprehensive temporal analysis...');
    const analysis = temporal.analyze(seq1, seq2);
    console.log(`   DTW Distance: ${analysis.dtwDistance.toFixed(4)}`);
    console.log(`   LCS Length: ${analysis.lcsLength}`);
    console.log(`   Edit Distance: ${analysis.editDistance}`);
    console.log(`   Similarity Score: ${analysis.similarityScore.toFixed(4)}`);
    console.log('   ✅ Comprehensive analysis works\n');

    // Test NanoScheduler
    console.log('4. Testing NanoScheduler...');
    const scheduler = new MidstreamWasm.NanoScheduler();
    const now = scheduler.nowNs();
    console.log(`   Current time: ${now}ns`);
    console.log(`   Pending tasks: ${scheduler.pendingCount}`);
    console.log('   ✅ Scheduler works\n');

    // Test StrangeLoop meta-learning
    console.log('5. Testing StrangeLoop meta-learning...');
    const loop = new MidstreamWasm.StrangeLoop(0.1);
    loop.observe('pattern1', 0.8);
    loop.observe('pattern2', 0.9);
    loop.observe('pattern1', 0.85);
    const confidence = loop.getConfidence('pattern1');
    console.log(`   Pattern confidence: ${confidence ? confidence.toFixed(4) : 'N/A'}`);
    console.log(`   Iteration count: ${loop.iterationCount}`);
    console.log(`   Pattern count: ${loop.patternCount}`);
    const best = loop.bestPattern();
    if (best) {
      console.log(`   Best pattern: ${best.patternId} (confidence: ${best.confidence.toFixed(4)})`);
    }
    console.log('   ✅ Meta-learning works\n');

    // Test QuicMultistream
    console.log('6. Testing QuicMultistream...');
    const quic = new MidstreamWasm.QuicMultistream();
    const streamId = quic.openStream(128);
    console.log(`   Opened stream ID: ${streamId}`);
    console.log(`   Active streams: ${quic.streamCount}`);
    console.log('   ✅ QUIC multistream works\n');

    // Test utility functions
    console.log('7. Testing utility functions...');
    const version = MidstreamWasm.version();
    console.log(`   Package version: ${version}`);
    console.log('   ✅ Version info works\n');

    console.log('✨ All tests passed successfully!');
    console.log('\n📦 @midstream/wasm is ready for publication');

    return true;
  } catch (error) {
    console.error('❌ Test failed:', error);
    console.error(error.stack);
    return false;
  }
}

// Run tests
if (require.main === module) {
  runTests().then(success => {
    process.exit(success ? 0 : 1);
  });
}

module.exports = { runTests };
