/**
 * Basic SONA Usage Example
 * Demonstrates core functionality of the SONA engine
 */

const { SonaEngine } = require('../index.js');

function main() {
  console.log('🧠 SONA - Self-Optimizing Neural Architecture\n');

  // Create engine with hidden dimension
  console.log('Creating SONA engine with hidden_dim=256...');
  const engine = new SonaEngine(256);
  console.log('✓ Engine created\n');

  // Simulate some inference trajectories
  console.log('Recording inference trajectories...');
  for (let i = 0; i < 10; i++) {
    // Create query embedding
    const queryEmbedding = Array(256).fill(0).map(() => Math.random());

    // Start trajectory
    const builder = engine.beginTrajectory(queryEmbedding);

    // Simulate inference steps
    for (let step = 0; step < 3; step++) {
      const activations = Array(256).fill(0).map(() => Math.random());
      const attentionWeights = Array(64).fill(0).map(() => Math.random());
      const reward = 0.7 + Math.random() * 0.3; // Random reward between 0.7-1.0

      builder.addStep(activations, attentionWeights, reward);
    }

    // Set route and context
    builder.setRoute(`model_${i % 3}`);
    builder.addContext(`context_${i}`);

    // Complete trajectory
    const quality = 0.75 + Math.random() * 0.25; // Quality between 0.75-1.0
    engine.endTrajectory(builder, quality);
  }
  console.log('✓ Recorded 10 trajectories\n');

  // Apply micro-LoRA transformation
  console.log('Applying micro-LoRA transformation...');
  const input = Array(256).fill(1.0);
  const output = engine.applyMicroLora(input);
  console.log(`✓ Transformed ${input.length} -> ${output.length} dimensions\n`);

  // Find similar patterns
  console.log('Finding similar patterns...');
  const queryEmbedding = Array(256).fill(0).map(() => Math.random());
  const patterns = engine.findPatterns(queryEmbedding, 5);
  console.log(`✓ Found ${patterns.length} patterns\n`);

  // Get statistics
  console.log('Engine statistics:');
  const stats = engine.getStats();
  console.log(stats);
  console.log();

  // Force learning cycle
  console.log('Running background learning cycle...');
  const result = engine.forceLearn();
  console.log(`✓ ${result}\n`);

  console.log('✓ Example completed successfully!');
}

main();
