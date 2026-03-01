// Example: Basic usage of Ruvector GNN Node.js bindings

const {
  RuvectorLayer,
  TensorCompress,
  differentiableSearch,
  hierarchicalForward,
  getCompressionLevel,
  init
} = require('../index.js');

console.log(init());
console.log('');

// ==================== Example 1: GNN Layer ====================
console.log('=== Example 1: GNN Layer ===');

const layer = new RuvectorLayer(4, 8, 2, 0.1);
console.log('Created GNN layer (input_dim: 4, hidden_dim: 8, heads: 2, dropout: 0.1)');

const nodeEmbedding = [1.0, 2.0, 3.0, 4.0];
const neighborEmbeddings = [
  [0.5, 1.0, 1.5, 2.0],
  [2.0, 3.0, 4.0, 5.0],
];
const edgeWeights = [0.3, 0.7];

const output = layer.forward(nodeEmbedding, neighborEmbeddings, edgeWeights);
console.log('Input embedding:', nodeEmbedding);
console.log('Output embedding (length):', output.length);
console.log('Output embedding (first 4 values):', output.slice(0, 4).map(x => x.toFixed(4)));
console.log('');

// ==================== Example 2: Tensor Compression ====================
console.log('=== Example 2: Tensor Compression ===');

const compressor = new TensorCompress();
const embedding = Array.from({ length: 64 }, (_, i) => Math.sin(i * 0.1));

// Test different access frequencies
const frequencies = [0.9, 0.5, 0.2, 0.05, 0.001];

frequencies.forEach(freq => {
  const level = getCompressionLevel(freq);
  const compressed = compressor.compress(embedding, freq);
  const decompressed = compressor.decompress(compressed);

  const originalSize = JSON.stringify(embedding).length;
  const compressedSize = compressed.length;
  const ratio = (compressedSize / originalSize * 100).toFixed(1);

  console.log(`Frequency: ${freq.toFixed(3)} | Level: ${level.padEnd(6)} | Size: ${ratio}% | Error: ${calculateMSE(embedding, decompressed).toFixed(6)}`);
});
console.log('');

// ==================== Example 3: Differentiable Search ====================
console.log('=== Example 3: Differentiable Search ===');

const query = [1.0, 0.0, 0.0];
const candidates = [
  [1.0, 0.0, 0.0],  // Perfect match
  [0.9, 0.1, 0.0],  // Close match
  [0.7, 0.3, 0.0],  // Medium match
  [0.0, 1.0, 0.0],  // Orthogonal
  [0.0, 0.0, 1.0],  // Orthogonal
];

console.log('Query:', query);
console.log('Number of candidates:', candidates.length);

const result = differentiableSearch(query, candidates, 3, 1.0);
console.log('Top-3 indices:', result.indices);
console.log('Soft weights:', result.weights.map(w => w.toFixed(4)));
console.log('Weights sum:', result.weights.reduce((a, b) => a + b, 0).toFixed(4));
console.log('');

// ==================== Example 4: Hierarchical Forward ====================
console.log('=== Example 4: Hierarchical Forward ===');

const query2 = [1.0, 0.0];
const layerEmbeddings = [
  [
    [1.0, 0.0],
    [0.0, 1.0],
    [0.7, 0.7],
  ],
];

const layer1 = new RuvectorLayer(2, 2, 1, 0.0);
const layers = [layer1.toJson()];

const finalEmbedding = hierarchicalForward(query2, layerEmbeddings, layers);
console.log('Query:', query2);
console.log('Final embedding:', finalEmbedding.map(x => x.toFixed(4)));
console.log('');

// ==================== Example 5: Layer Serialization ====================
console.log('=== Example 5: Layer Serialization ===');

const originalLayer = new RuvectorLayer(8, 16, 4, 0.2);
const serialized = originalLayer.toJson();
const deserialized = RuvectorLayer.fromJson(serialized);

console.log('Original layer created (8 -> 16, heads: 4, dropout: 0.2)');
console.log('Serialized size:', serialized.length, 'bytes');
console.log('Successfully deserialized');

// Test that deserialized layer works
const testInput = Array.from({ length: 8 }, () => Math.random());
const testNeighbors = [Array.from({ length: 8 }, () => Math.random())];
const testWeights = [1.0];

const output1 = originalLayer.forward(testInput, testNeighbors, testWeights);
const output2 = deserialized.forward(testInput, testNeighbors, testWeights);

console.log('Original output matches deserialized:', arraysEqual(output1, output2, 1e-6));
console.log('');

// ==================== Helper Functions ====================

function calculateMSE(a, b) {
  if (a.length !== b.length) return Infinity;
  const sum = a.reduce((acc, val, i) => acc + Math.pow(val - b[i], 2), 0);
  return sum / a.length;
}

function arraysEqual(a, b, epsilon = 1e-10) {
  if (a.length !== b.length) return false;
  return a.every((val, i) => Math.abs(val - b[i]) < epsilon);
}

console.log('All examples completed successfully!');
