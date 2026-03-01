/**
 * Prime-Radiant Advanced WASM - JavaScript/TypeScript API Example
 *
 * This example demonstrates usage of all 6 mathematical engines:
 * - CohomologyEngine: Sheaf cohomology computations
 * - CategoryEngine: Functorial retrieval and topos operations
 * - HoTTEngine: Type checking and path operations
 * - SpectralEngine: Eigenvalue computation and Cheeger bounds
 * - CausalEngine: Causal inference and interventions
 * - QuantumEngine: Topological invariants and quantum simulation
 */

import init, {
  CohomologyEngine,
  SpectralEngine,
  CausalEngine,
  QuantumEngine,
  CategoryEngine,
  HoTTEngine,
  getVersion,
  initModule,
  type SheafGraph,
  type SheafNode,
  type SheafEdge,
  type Graph,
  type CausalModel,
  type QuantumState,
  type Complex,
  type Category,
  type CatObject,
  type Morphism,
  type HoTTType,
  type HoTTTerm,
  type HoTTPath,
} from './prime_radiant_advanced_wasm';

// ============================================================================
// Initialization
// ============================================================================

async function main() {
  // Initialize WASM module
  await init();
  initModule();

  console.log(`Prime-Radiant Advanced WASM v${getVersion()}`);
  console.log('='.repeat(50));

  // Run all examples
  await cohomologyExample();
  await spectralExample();
  await causalExample();
  await quantumExample();
  await categoryExample();
  await hottExample();

  console.log('\nAll examples completed successfully!');
}

// ============================================================================
// Cohomology Engine Example
// ============================================================================

async function cohomologyExample() {
  console.log('\n--- Cohomology Engine Example ---');

  const cohomology = new CohomologyEngine();

  // Create a belief graph with consistent sections
  const consistentGraph: SheafGraph = {
    nodes: [
      { id: 0, label: 'Belief A', section: [1.0, 0.5], weight: 1.0 },
      { id: 1, label: 'Belief B', section: [1.0, 0.5], weight: 1.0 },
      { id: 2, label: 'Belief C', section: [1.0, 0.5], weight: 1.0 },
    ],
    edges: [
      {
        source: 0,
        target: 1,
        restriction_map: [1.0, 0.0, 0.0, 1.0], // Identity map
        source_dim: 2,
        target_dim: 2,
      },
      {
        source: 1,
        target: 2,
        restriction_map: [1.0, 0.0, 0.0, 1.0],
        source_dim: 2,
        target_dim: 2,
      },
    ],
  };

  // Compute cohomology
  const result = cohomology.computeCohomology(consistentGraph);
  console.log('Cohomology of consistent graph:');
  console.log(`  H^0 dimension: ${result.h0_dim}`);
  console.log(`  H^1 dimension: ${result.h1_dim}`);
  console.log(`  Euler characteristic: ${result.euler_characteristic}`);
  console.log(`  Is consistent: ${result.is_consistent}`);

  // Create an inconsistent graph
  const inconsistentGraph: SheafGraph = {
    nodes: [
      { id: 0, label: 'Belief A', section: [1.0, 0.0], weight: 1.0 },
      { id: 1, label: 'Belief B', section: [0.0, 1.0], weight: 1.0 }, // Different!
    ],
    edges: [
      {
        source: 0,
        target: 1,
        restriction_map: [1.0, 0.0, 0.0, 1.0],
        source_dim: 2,
        target_dim: 2,
      },
    ],
  };

  // Detect obstructions
  const obstructions = cohomology.detectObstructions(inconsistentGraph);
  console.log(`\nDetected ${obstructions.length} obstruction(s):`);
  for (const obs of obstructions) {
    console.log(`  ${obs.description}`);
  }

  // Compute consistency energy
  const energy = cohomology.consistencyEnergy(inconsistentGraph);
  console.log(`  Consistency energy: ${energy.toFixed(6)}`);
}

// ============================================================================
// Spectral Engine Example
// ============================================================================

async function spectralExample() {
  console.log('\n--- Spectral Engine Example ---');

  const spectral = new SpectralEngine();

  // Create a path graph: 0 -- 1 -- 2 -- 3 -- 4
  const pathGraph: Graph = {
    n: 5,
    edges: [
      [0, 1, 1.0],
      [1, 2, 1.0],
      [2, 3, 1.0],
      [3, 4, 1.0],
    ],
  };

  // Compute Cheeger bounds
  const cheeger = spectral.computeCheegerBounds(pathGraph);
  console.log('Cheeger bounds for path graph:');
  console.log(`  Lower bound: ${cheeger.lower_bound.toFixed(6)}`);
  console.log(`  Upper bound: ${cheeger.upper_bound.toFixed(6)}`);
  console.log(`  Fiedler value (λ₂): ${cheeger.fiedler_value.toFixed(6)}`);

  // Compute spectral gap
  const gap = spectral.computeSpectralGap(pathGraph);
  console.log(`\nSpectral gap analysis:`);
  console.log(`  λ₁ = ${gap.lambda_1.toFixed(6)}`);
  console.log(`  λ₂ = ${gap.lambda_2.toFixed(6)}`);
  console.log(`  Gap = ${gap.gap.toFixed(6)}`);
  console.log(`  Ratio = ${gap.ratio.toFixed(6)}`);

  // Predict minimum cut
  const prediction = spectral.predictMinCut(pathGraph);
  console.log(`\nMin-cut prediction:`);
  console.log(`  Predicted cut: ${prediction.predicted_cut.toFixed(6)}`);
  console.log(`  Confidence: ${(prediction.confidence * 100).toFixed(1)}%`);
  console.log(`  Cut nodes: [${prediction.cut_nodes.join(', ')}]`);

  // Create a barbell graph (two cliques connected by single edge)
  const barbellGraph: Graph = {
    n: 6,
    edges: [
      // First clique
      [0, 1, 1.0], [0, 2, 1.0], [1, 2, 1.0],
      // Second clique
      [3, 4, 1.0], [3, 5, 1.0], [4, 5, 1.0],
      // Bridge
      [2, 3, 1.0],
    ],
  };

  const barbellGap = spectral.computeSpectralGap(barbellGraph);
  console.log(`\nBarbell graph spectral gap: ${barbellGap.gap.toFixed(6)}`);
  console.log('(Small gap indicates bottleneck structure)');
}

// ============================================================================
// Causal Engine Example
// ============================================================================

async function causalExample() {
  console.log('\n--- Causal Engine Example ---');

  const causal = new CausalEngine();

  // Build a causal model: Age -> Income, Education -> Income, Income -> Savings
  const model: CausalModel = {
    variables: [
      { name: 'Age', var_type: 'continuous' },
      { name: 'Education', var_type: 'discrete' },
      { name: 'Income', var_type: 'continuous' },
      { name: 'Savings', var_type: 'continuous' },
    ],
    edges: [
      { from: 'Age', to: 'Income' },
      { from: 'Education', to: 'Income' },
      { from: 'Income', to: 'Savings' },
    ],
  };

  // Check if valid DAG
  const isValid = causal.isValidDag(model);
  console.log(`Model is valid DAG: ${isValid}`);

  // Get topological order
  const order = causal.topologicalOrder(model);
  console.log(`Topological order: ${order.join(' -> ')}`);

  // Check d-separation
  const dSep = causal.checkDSeparation(model, 'Age', 'Savings', ['Income']);
  console.log(`\nD-separation test:`);
  console.log(`  Age ⊥ Savings | Income: ${dSep.d_separated}`);

  const dSep2 = causal.checkDSeparation(model, 'Age', 'Savings', []);
  console.log(`  Age ⊥ Savings | ∅: ${dSep2.d_separated}`);

  // Find confounders
  const confounders = causal.findConfounders(model, 'Education', 'Savings');
  console.log(`\nConfounders between Education and Savings: [${confounders.join(', ')}]`);

  // Compute causal effect
  const effect = causal.computeCausalEffect(model, 'Income', 'Savings', 10000);
  console.log(`\nCausal effect of do(Income = 10000) on Savings:`);
  console.log(`  Effect: ${effect.causal_effect}`);
  console.log(`  Affected variables: [${effect.affected_variables.join(', ')}]`);
}

// ============================================================================
// Quantum Engine Example
// ============================================================================

async function quantumExample() {
  console.log('\n--- Quantum Engine Example ---');

  const quantum = new QuantumEngine();

  // Create GHZ state (maximally entangled)
  const ghz = quantum.createGHZState(3);
  console.log(`GHZ state (3 qubits):`);
  console.log(`  Dimension: ${ghz.dimension}`);
  console.log(`  |000⟩ amplitude: ${ghz.amplitudes[0].re.toFixed(4)}`);
  console.log(`  |111⟩ amplitude: ${ghz.amplitudes[7].re.toFixed(4)}`);

  // Create W state
  const w = quantum.createWState(3);
  console.log(`\nW state (3 qubits):`);
  console.log(`  |001⟩ amplitude: ${w.amplitudes[1].re.toFixed(4)}`);
  console.log(`  |010⟩ amplitude: ${w.amplitudes[2].re.toFixed(4)}`);
  console.log(`  |100⟩ amplitude: ${w.amplitudes[4].re.toFixed(4)}`);

  // Compute fidelity between states
  const fidelity = quantum.computeFidelity(ghz, w);
  console.log(`\nFidelity between GHZ and W states:`);
  console.log(`  Fidelity: ${fidelity.fidelity.toFixed(6)}`);
  console.log(`  Trace distance: ${fidelity.trace_distance.toFixed(6)}`);

  // Compute entanglement entropy
  const entropy = quantum.computeEntanglementEntropy(ghz, 1);
  console.log(`\nEntanglement entropy of GHZ (split at qubit 1): ${entropy.toFixed(6)}`);

  // Compute topological invariants of a simplicial complex
  // Triangle: vertices {0,1,2}, edges {01,12,02}, face {012}
  const simplices = [
    [0], [1], [2],           // 0-simplices (vertices)
    [0, 1], [1, 2], [0, 2],  // 1-simplices (edges)
    [0, 1, 2],               // 2-simplex (face)
  ];

  const invariants = quantum.computeTopologicalInvariants(simplices);
  console.log(`\nTopological invariants of filled triangle:`);
  console.log(`  Euler characteristic: ${invariants.euler_characteristic}`);
  console.log(`  Is connected: ${invariants.is_connected}`);

  // Apply Hadamard gate
  const hadamard: Complex[][] = [
    [{ re: 1 / Math.sqrt(2), im: 0 }, { re: 1 / Math.sqrt(2), im: 0 }],
    [{ re: 1 / Math.sqrt(2), im: 0 }, { re: -1 / Math.sqrt(2), im: 0 }],
  ];

  const ground: QuantumState = {
    amplitudes: [{ re: 1, im: 0 }, { re: 0, im: 0 }],
    dimension: 2,
  };

  const result = quantum.applyGate(ground, hadamard, 0);
  console.log(`\nHadamard on |0⟩:`);
  console.log(`  |0⟩ amplitude: ${result.amplitudes[0].re.toFixed(4)}`);
  console.log(`  |1⟩ amplitude: ${result.amplitudes[1].re.toFixed(4)}`);
}

// ============================================================================
// Category Engine Example
// ============================================================================

async function categoryExample() {
  console.log('\n--- Category Engine Example ---');

  const category = new CategoryEngine();

  // Create a simple category with vector spaces
  const vecCategory: Category = {
    name: 'Vect',
    objects: [
      { id: 'R2', dimension: 2, data: [1.0, 0.0] },
      { id: 'R3', dimension: 3, data: [1.0, 0.0, 0.0] },
    ],
    morphisms: [],
  };

  // Create morphisms (linear maps)
  const projection: Morphism = {
    source: 'R3',
    target: 'R2',
    matrix: [1, 0, 0, 0, 1, 0], // Project to first two coordinates
    source_dim: 3,
    target_dim: 2,
  };

  const embedding: Morphism = {
    source: 'R2',
    target: 'R3',
    matrix: [1, 0, 0, 1, 0, 0], // Embed in first two coordinates
    source_dim: 2,
    target_dim: 3,
  };

  // Apply morphism
  const data = [1.0, 2.0, 3.0];
  const projected = category.applyMorphism(projection, data);
  console.log(`Projection of [${data.join(', ')}]: [${projected.map(x => x.toFixed(2)).join(', ')}]`);

  // Compose morphisms (embedding then projection = identity)
  const composed = category.composeMorphisms(embedding, projection);
  console.log(`\nComposed morphism (P ∘ E):`);
  console.log(`  Source: ${composed.source}`);
  console.log(`  Target: ${composed.target}`);
  console.log(`  Matrix: [${composed.matrix.map(x => x.toFixed(2)).join(', ')}]`);

  // Verify category laws
  vecCategory.morphisms.push(projection);
  const lawsValid = category.verifyCategoryLaws(vecCategory);
  console.log(`\nCategory laws verified: ${lawsValid}`);

  // Functorial retrieval
  const docsCategory: Category = {
    name: 'Docs',
    objects: [
      { id: 'doc1', dimension: 3, data: [1.0, 0.0, 0.0] },
      { id: 'doc2', dimension: 3, data: [0.9, 0.1, 0.0] },
      { id: 'doc3', dimension: 3, data: [0.0, 1.0, 0.0] },
      { id: 'doc4', dimension: 3, data: [0.0, 0.0, 1.0] },
    ],
    morphisms: [],
  };

  const query = [1.0, 0.0, 0.0];
  const results = category.functorialRetrieve(docsCategory, query, 2);
  console.log(`\nFunctorial retrieval for query [${query.join(', ')}]:`);
  for (const r of results) {
    console.log(`  ${r.object_id}: similarity = ${r.similarity.toFixed(4)}`);
  }
}

// ============================================================================
// HoTT Engine Example
// ============================================================================

async function hottExample() {
  console.log('\n--- HoTT Engine Example ---');

  const hott = new HoTTEngine();

  // Create terms
  const star: HoTTTerm = { kind: 'star', children: [] };
  const zero: HoTTTerm = { kind: 'zero', children: [] };
  const one: HoTTTerm = { kind: 'succ', children: [zero] };
  const pair: HoTTTerm = { kind: 'pair', children: [zero, star] };

  // Infer types
  console.log('Type inference:');
  const starType = hott.inferType(star);
  console.log(`  ★ : ${starType.inferred_type?.kind}`);

  const zeroType = hott.inferType(zero);
  console.log(`  0 : ${zeroType.inferred_type?.kind}`);

  const oneType = hott.inferType(one);
  console.log(`  S(0) : ${oneType.inferred_type?.kind}`);

  const pairType = hott.inferType(pair);
  console.log(`  (0, ★) : ${pairType.inferred_type?.name}`);

  // Type checking
  const natType: HoTTType = { name: 'Nat', level: 0, kind: 'nat', params: [] };
  const checkResult = hott.typeCheck(zero, natType);
  console.log(`\nType checking 0 : Nat: ${checkResult.is_valid}`);

  const boolType: HoTTType = { name: 'Bool', level: 0, kind: 'bool', params: [] };
  const checkResult2 = hott.typeCheck(zero, boolType);
  console.log(`Type checking 0 : Bool: ${checkResult2.is_valid}`);
  if (checkResult2.error) {
    console.log(`  Error: ${checkResult2.error}`);
  }

  // Path operations
  const refl = hott.createReflPath(natType, zero);
  console.log(`\nReflexivity path: refl(0) : 0 = 0`);

  // Compose paths
  const composed = hott.composePaths(refl, refl);
  if (composed.is_valid) {
    console.log('Path composition: refl ∙ refl is valid');
  }

  // Invert path
  const inverted = hott.invertPath(refl);
  if (inverted.is_valid) {
    console.log('Path inversion: refl⁻¹ is valid');
  }

  // Check type equivalence
  const nat1: HoTTType = { name: 'Nat', level: 0, kind: 'nat', params: [] };
  const nat2: HoTTType = { name: 'Nat', level: 0, kind: 'nat', params: [] };
  const equiv = hott.checkTypeEquivalence(nat1, nat2);
  console.log(`\nType equivalence Nat ≃ Nat: ${equiv}`);
}

// ============================================================================
// Run Examples
// ============================================================================

main().catch(console.error);
