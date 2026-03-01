#!/usr/bin/env node

/**
 * Hyperbolic Attention & Poincaré Ball Model Exploration
 *
 * This demonstration explores hyperbolic geometry and why it's superior
 * for representing hierarchical structures like:
 * - Knowledge taxonomies
 * - Organizational charts
 * - Concept hierarchies
 * - Skill trees
 *
 * Key Concepts:
 * - Poincaré ball model
 * - Hyperbolic space vs Euclidean space
 * - Natural hierarchy representation
 * - Distance preservation in hyperbolic geometry
 */

const {
  HyperbolicAttention,
  MultiHeadAttention,
  expMap,
  logMap,
  mobiusAddition,
  poincareDistance,
  projectToPoincareBall
} = require('@ruvector/attention');

console.log('🌀 Hyperbolic Attention & Poincaré Ball Model\n');
console.log('=' .repeat(70));

// ============================================================================
// PART 1: Understanding Hyperbolic Space
// ============================================================================

function explainPoincareModel() {
  console.log('\n📐 PART 1: Understanding the Poincaré Ball Model\n');
  console.log('=' .repeat(70));

  console.log('\n🌍 What is Hyperbolic Space?\n');
  console.log('Hyperbolic space is a non-Euclidean geometry where:');
  console.log('  • Space curves with negative curvature (like a saddle)');
  console.log('  • Parallel lines diverge (unlike Euclidean geometry)');
  console.log('  • Space grows exponentially as you move from the center');
  console.log('  • Perfect for representing hierarchies naturally\n');

  console.log('🔵 The Poincaré Ball Model:\n');
  console.log('Represents hyperbolic space as a ball where:');
  console.log('  • Center (0,0,0) = root of hierarchy');
  console.log('  • Points near center = high-level concepts');
  console.log('  • Points near boundary = specific/leaf concepts');
  console.log('  • Distance to boundary = level in hierarchy');
  console.log('  • Exponentially more space near boundary\n');

  console.log('📊 Why This Matters:\n');
  console.log('  Problem: In Euclidean space (normal geometry):');
  console.log('    • Need exponentially more dimensions for deep trees');
  console.log('    • Distance doesn\'t reflect hierarchical relationships');
  console.log('    • Embedding large trees causes distortion\n');

  console.log('  Solution: In Hyperbolic space:');
  console.log('    • Trees embed naturally with low dimensions');
  console.log('    • Distance reflects hierarchy (parent-child, siblings)');
  console.log('    • No distortion even for huge trees\n');

  console.log('💡 Real-World Example:\n');
  console.log('  Imagine organizing "Animals":');
  console.log('    Center:    "Animals" (most general)');
  console.log('    Mid-level: "Mammals", "Birds", "Fish"');
  console.log('    Boundary:  "Golden Retriever", "Sparrow", "Goldfish"\n');

  console.log('  In Euclidean space: All species equidistant from center');
  console.log('  In Hyperbolic space: Hierarchy preserved in distances!\n');
}

// ============================================================================
// PART 2: Visualizing Hyperbolic vs Euclidean
// ============================================================================

function visualizeSpaceComparison() {
  console.log('\n' + '=' .repeat(70));
  console.log('\n📊 PART 2: Hyperbolic vs Euclidean Space\n');
  console.log('=' .repeat(70));

  console.log('\n🔷 EUCLIDEAN SPACE (Normal geometry):\n');
  console.log('   Representing a 3-level hierarchy:');
  console.log('');
  console.log('                    Root');
  console.log('                     │');
  console.log('         ┌───────────┼───────────┐');
  console.log('         A           B           C');
  console.log('       ┌─┼─┐       ┌─┼─┐       ┌─┼─┐');
  console.log('       1 2 3       4 5 6       7 8 9');
  console.log('');
  console.log('   Problem: All leaf nodes (1-9) same distance from root');
  console.log('            Siblings (1,2,3) same distance as cousins (1,4,7)');
  console.log('            Hierarchy information LOST in distance!\n');

  console.log('🌀 HYPERBOLIC SPACE (Poincaré Ball):\n');
  console.log('   Same hierarchy in hyperbolic space:');
  console.log('');
  console.log('        ╔═══════════════════════════════════╗');
  console.log('        ║                                   ║');
  console.log('        ║           ●Root (0.0)             ║');
  console.log('        ║            │                      ║');
  console.log('        ║    ┌───────┼───────┐             ║');
  console.log('        ║    ●A      ●B      ●C  (0.4)     ║');
  console.log('        ║   ┌┼┐     ┌┼┐     ┌┼┐            ║');
  console.log('        ║   ●●●     ●●●     ●●●  (0.7)     ║');
  console.log('        ║   123     456     789             ║');
  console.log('        ║                                   ║');
  console.log('        ╚═══════════════════════════════════╝');
  console.log('         ^                                 ^');
  console.log('       Center                          Boundary');
  console.log('');
  console.log('   Benefits:');
  console.log('   • Siblings (1,2,3) closer than cousins (1,4,7) ✓');
  console.log('   • Parent-child distance consistent ✓');
  console.log('   • Root central, leaves at boundary ✓');
  console.log('   • Hierarchy preserved in geometry! ✓\n');

  console.log('📏 Distance Comparison:\n');
  console.log('   Euclidean:');
  console.log('     d(1, 2) ≈ d(1, 4) ≈ d(1, 7)  ← All similar!');
  console.log('     Hierarchy NOT captured\n');

  console.log('   Hyperbolic (Poincaré):');
  console.log('     d(1, 2) < d(1, 4) < d(1, 7)  ← Reflects hierarchy!');
  console.log('     Siblings closer than cousins ✓\n');
}

// ============================================================================
// PART 3: Poincaré Ball Operations
// ============================================================================

async function demonstratePoincareOperations() {
  console.log('\n' + '=' .repeat(70));
  console.log('\n🧮 PART 3: Poincaré Ball Operations\n');
  console.log('=' .repeat(70));

  console.log('\n🔧 Key Operations in Hyperbolic Geometry:\n');

  // 1. Exponential Map
  console.log('1️⃣  EXPONENTIAL MAP (expMap)');
  console.log('   Maps from tangent space → Poincaré ball');
  console.log('   Moves a point in a direction with hyperbolic distance\n');

  try {
    const point = new Float32Array([0.1, 0.2, 0.3]);
    const direction = new Float32Array([0.05, 0.05, 0.05]);

    console.log('   Example: Move a point in hyperbolic space');
    console.log(`   Point:     [${Array.from(point).map(x => x.toFixed(2)).join(', ')}]`);
    console.log(`   Direction: [${Array.from(direction).map(x => x.toFixed(2)).join(', ')}]`);

    const result = expMap(point, direction);
    console.log(`   Result:    [${Array.from(result).map(x => x.toFixed(2)).join(', ')}]`);
    console.log('   ✓ Point moved along hyperbolic geodesic\n');
  } catch (e) {
    console.log(`   ⚠️  ${e.message}\n`);
  }

  // 2. Logarithmic Map
  console.log('2️⃣  LOGARITHMIC MAP (logMap)');
  console.log('   Maps from Poincaré ball → tangent space');
  console.log('   Finds the direction from one point to another\n');

  try {
    const origin = new Float32Array([0.1, 0.1, 0.1]);
    const target = new Float32Array([0.3, 0.2, 0.1]);

    console.log('   Example: Find direction between two points');
    console.log(`   From: [${Array.from(origin).map(x => x.toFixed(2)).join(', ')}]`);
    console.log(`   To:   [${Array.from(target).map(x => x.toFixed(2)).join(', ')}]`);

    const direction = logMap(origin, target);
    console.log(`   Direction: [${Array.from(direction).map(x => x.toFixed(2)).join(', ')}]`);
    console.log('   ✓ Direction in tangent space computed\n');
  } catch (e) {
    console.log(`   ⚠️  ${e.message}\n`);
  }

  // 3. Möbius Addition
  console.log('3️⃣  MÖBIUS ADDITION (mobiusAddition)');
  console.log('   "Addition" in hyperbolic space (not standard +)');
  console.log('   Combines points while respecting curvature\n');

  try {
    const a = new Float32Array([0.2, 0.1, 0.0]);
    const b = new Float32Array([0.1, 0.2, 0.0]);

    console.log('   Example: Add two points hyperbolically');
    console.log(`   A: [${Array.from(a).map(x => x.toFixed(2)).join(', ')}]`);
    console.log(`   B: [${Array.from(b).map(x => x.toFixed(2)).join(', ')}]`);

    const sum = mobiusAddition(a, b);
    console.log(`   A ⊕ B: [${Array.from(sum).map(x => x.toFixed(2)).join(', ')}]`);
    console.log('   ✓ Hyperbolic addition computed\n');
  } catch (e) {
    console.log(`   ⚠️  ${e.message}\n`);
  }

  // 4. Poincaré Distance
  console.log('4️⃣  POINCARÉ DISTANCE (poincareDistance)');
  console.log('   Distance metric in hyperbolic space');
  console.log('   Grows exponentially near the boundary\n');

  try {
    const p1 = new Float32Array([0.1, 0.1, 0.1]);
    const p2Near = new Float32Array([0.2, 0.1, 0.1]);
    const p2Far = new Float32Array([0.5, 0.5, 0.5]);

    console.log('   Example: Measure hyperbolic distances');
    console.log(`   From point: [${Array.from(p1).map(x => x.toFixed(2)).join(', ')}]`);

    const distNear = poincareDistance(p1, p2Near);
    const distFar = poincareDistance(p1, p2Far);

    console.log(`   To nearby:  [${Array.from(p2Near).map(x => x.toFixed(2)).join(', ')}] → distance: ${distNear.toFixed(3)}`);
    console.log(`   To far:     [${Array.from(p2Far).map(x => x.toFixed(2)).join(', ')}] → distance: ${distFar.toFixed(3)}`);
    console.log('   ✓ Hyperbolic distances computed\n');
  } catch (e) {
    console.log(`   ⚠️  ${e.message}\n`);
  }

  // 5. Project to Poincaré Ball
  console.log('5️⃣  PROJECT TO POINCARÉ BALL (projectToPoincareBall)');
  console.log('   Ensures points stay inside the unit ball');
  console.log('   Boundary represents infinite distance\n');

  try {
    const outside = new Float32Array([1.5, 1.5, 1.5]);

    console.log('   Example: Project point outside ball');
    console.log(`   Outside: [${Array.from(outside).map(x => x.toFixed(2)).join(', ')}]`);

    const projected = projectToPoincareBall(outside);
    console.log(`   Projected: [${Array.from(projected).map(x => x.toFixed(2)).join(', ')}]`);
    console.log('   ✓ Point now inside unit ball\n');
  } catch (e) {
    console.log(`   ⚠️  ${e.message}\n`);
  }
}

// ============================================================================
// PART 4: Hyperbolic Attention in Action
// ============================================================================

async function demonstrateHyperbolicAttention() {
  console.log('\n' + '=' .repeat(70));
  console.log('\n🧠 PART 4: Hyperbolic Attention Mechanism\n');
  console.log('=' .repeat(70));

  console.log('\n🎯 How Hyperbolic Attention Works:\n');
  console.log('Standard Attention (Euclidean):');
  console.log('  Attention(Q, K, V) = softmax(QK^T / √d) V');
  console.log('  • Operates in flat Euclidean space');
  console.log('  • All points treated equally');
  console.log('  • No hierarchical bias\n');

  console.log('Hyperbolic Attention (Poincaré):');
  console.log('  1. Map Q, K, V to Poincaré ball');
  console.log('  2. Compute Poincaré distances (not dot products)');
  console.log('  3. Apply attention using hyperbolic geometry');
  console.log('  4. Combine values respecting curvature');
  console.log('  • Naturally preserves hierarchies');
  console.log('  • Similar ancestors attend to each other');
  console.log('  • Hierarchical relationships maintained\n');

  console.log('🔧 Creating Hierarchical Data...\n');

  // Create a knowledge hierarchy
  const hierarchy = {
    'Science': {
      level: 0,
      radius: 0.0,
      children: ['Physics', 'Chemistry', 'Biology']
    },
    'Physics': {
      level: 1,
      radius: 0.35,
      children: ['Quantum', 'Classical', 'Relativity']
    },
    'Chemistry': {
      level: 1,
      radius: 0.35,
      children: ['Organic', 'Inorganic', 'Physical']
    },
    'Biology': {
      level: 1,
      radius: 0.35,
      children: ['Molecular', 'Ecology', 'Evolution']
    }
  };

  console.log('📚 Knowledge Hierarchy:');
  console.log('   Science (root, r=0.0)');
  console.log('     ├─ Physics (r=0.35)');
  console.log('     │    ├─ Quantum');
  console.log('     │    ├─ Classical');
  console.log('     │    └─ Relativity');
  console.log('     ├─ Chemistry (r=0.35)');
  console.log('     │    ├─ Organic');
  console.log('     │    ├─ Inorganic');
  console.log('     │    └─ Physical');
  console.log('     └─ Biology (r=0.35)');
  console.log('          ├─ Molecular');
  console.log('          ├─ Ecology');
  console.log('          └─ Evolution\n');

  // Create embeddings in hyperbolic space
  function createHierarchicalEmbedding(level, index, totalAtLevel, dim = 64) {
    const vec = new Float32Array(dim);

    // Radius based on level (0 = center, deeper = closer to boundary)
    const radius = level * 0.3;

    // Angle based on position among siblings
    const angle = (index / totalAtLevel) * 2 * Math.PI;

    // First few dimensions encode position
    vec[0] = radius * Math.cos(angle);
    vec[1] = radius * Math.sin(angle);
    vec[2] = level * 0.1; // Depth encoding

    // Remaining dimensions for semantic content
    for (let i = 3; i < dim; i++) {
      vec[i] = Math.sin(i * angle) * (1 - radius);
    }

    return vec;
  }

  console.log('🌀 Testing Hyperbolic Attention...\n');

  // Create test data
  const dim = 64;
  const curvature = -1.0; // Negative for hyperbolic space

  // Query: "Physics" (level 1, position 0)
  const query = createHierarchicalEmbedding(1, 0, 3, dim);

  // Keys: All topics
  const keys = [
    createHierarchicalEmbedding(0, 0, 1, dim), // Science (parent)
    createHierarchicalEmbedding(1, 0, 3, dim), // Physics (self)
    createHierarchicalEmbedding(1, 1, 3, dim), // Chemistry (sibling)
    createHierarchicalEmbedding(1, 2, 3, dim), // Biology (sibling)
    createHierarchicalEmbedding(2, 0, 3, dim), // Quantum (child)
  ];

  const values = keys.map(k => Float32Array.from(k));

  console.log('Query: "Physics"');
  console.log('Comparing attention to:');
  console.log('  - Science (parent)');
  console.log('  - Physics (self)');
  console.log('  - Chemistry (sibling)');
  console.log('  - Biology (sibling)');
  console.log('  - Quantum (child)\n');

  // Hyperbolic Attention
  const hyperbolicAttn = new HyperbolicAttention(dim, curvature);
  const start = performance.now();
  const output = hyperbolicAttn.compute(query, keys, values);
  const duration = performance.now() - start;

  console.log(`✅ Hyperbolic Attention computed in ${duration.toFixed(3)}ms`);
  console.log(`   Output dimension: ${output.length}`);
  console.log(`   Curvature: ${curvature}`);
  console.log(`   Geometry: Poincaré ball model\n`);

  // Compare with standard Multi-Head Attention
  const standardAttn = new MultiHeadAttention(dim, 1);
  const standardStart = performance.now();
  const standardOutput = standardAttn.compute(query, keys, values);
  const standardDuration = performance.now() - standardStart;

  console.log(`✅ Standard Attention computed in ${standardDuration.toFixed(3)}ms`);
  console.log(`   Output dimension: ${standardOutput.length}\n`);

  console.log('🔍 Expected Behavior:\n');
  console.log('Hyperbolic Attention should attend more to:');
  console.log('  ✓ Self (Physics) - highest weight');
  console.log('  ✓ Parent (Science) - structural relationship');
  console.log('  ✓ Children (Quantum, Classical, Relativity) - hierarchical');
  console.log('  ✓ Siblings (Chemistry, Biology) - same level\n');

  console.log('Standard Attention treats all equally:');
  console.log('  • No hierarchical bias');
  console.log('  • Pure semantic similarity');
  console.log('  • Ignores tree structure\n');
}

// ============================================================================
// PART 5: Use Cases for Hyperbolic Attention
// ============================================================================

function explainUseCases() {
  console.log('\n' + '=' .repeat(70));
  console.log('\n💼 PART 5: When to Use Hyperbolic Attention\n');
  console.log('=' .repeat(70));

  console.log('\n✅ PERFECT For:\n');

  console.log('1️⃣  Knowledge Graphs & Taxonomies');
  console.log('   • WordNet (concepts → synonyms → words)');
  console.log('   • Wikipedia categories');
  console.log('   • Product catalogs (Electronics → Computers → Laptops)');
  console.log('   • Medical ontologies (Disease → Symptom → Treatment)\n');

  console.log('2️⃣  Organizational Hierarchies');
  console.log('   • Company org charts');
  console.log('   • Military command structures');
  console.log('   • Government organizations');
  console.log('   • Academic departments\n');

  console.log('3️⃣  Skill & Technology Trees');
  console.log('   • Game skill trees');
  console.log('   • Programming dependencies');
  console.log('   • Course prerequisites');
  console.log('   • Research paper citations\n');

  console.log('4️⃣  Natural Language Hierarchies');
  console.log('   • Parse trees (sentence → phrase → word)');
  console.log('   • Document structure (book → chapter → section)');
  console.log('   • Code ASTs (program → function → statement)');
  console.log('   • File systems (root → dir → file)\n');

  console.log('❌ NOT Ideal For:\n');

  console.log('   • Flat data (no hierarchy)');
  console.log('   • Grid/mesh structures');
  console.log('   • Fully connected networks');
  console.log('   • Time series (use temporal attention)\n');

  console.log('🎯 Key Advantages:\n');

  console.log('   ✓ Preserves hierarchical relationships');
  console.log('   ✓ Efficient embedding of trees');
  console.log('   ✓ Natural distance metric for hierarchies');
  console.log('   ✓ Better generalization on tree-structured data');
  console.log('   ✓ Lower dimensional embeddings possible');
  console.log('   ✓ Mathematically elegant and proven\n');
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  try {
    // Part 1: Theory
    explainPoincareModel();

    // Part 2: Visualization
    visualizeSpaceComparison();

    // Part 3: Operations
    await demonstratePoincareOperations();

    // Part 4: Attention in Action
    await demonstrateHyperbolicAttention();

    // Part 5: Use Cases
    explainUseCases();

    // Summary
    console.log('\n' + '=' .repeat(70));
    console.log('\n🎓 SUMMARY: Hyperbolic Attention & Poincaré Ball\n');
    console.log('=' .repeat(70));

    console.log('\n📚 What We Learned:\n');
    console.log('  1. Hyperbolic space has negative curvature (saddle-shaped)');
    console.log('  2. Poincaré ball maps infinite space to unit ball');
    console.log('  3. Hierarchies embed naturally without distortion');
    console.log('  4. Distance preserves parent-child relationships');
    console.log('  5. Exponentially more space near boundary (for leaves)\n');

    console.log('🔧 Key Operations:\n');
    console.log('  • expMap: Move in hyperbolic space');
    console.log('  • logMap: Find direction between points');
    console.log('  • mobiusAddition: Combine points hyperbolically');
    console.log('  • poincareDistance: Measure hyperbolic distance');
    console.log('  • projectToPoincareBall: Keep points in valid range\n');

    console.log('🧠 Why It Matters:\n');
    console.log('  Hyperbolic Attention understands STRUCTURE, not just content.');
    console.log('  Perfect for knowledge graphs, org charts, taxonomies, and trees.\n');

    console.log('💡 Remember:\n');
    console.log('  "In hyperbolic space, hierarchies are geometry."');
    console.log('  "Distance tells you not just similarity, but relationship."\n');

    console.log('=' .repeat(70));
    console.log('\n✅ Hyperbolic Attention Exploration Complete!\n');

  } catch (error) {
    console.error('\n❌ Error:', error);
    console.error('\nStack:', error.stack);
    process.exit(1);
  }
}

main();
