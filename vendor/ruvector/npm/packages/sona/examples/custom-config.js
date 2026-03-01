/**
 * Custom Configuration Example
 * Demonstrates advanced configuration options
 */

const { SonaEngine } = require('../index.js');

function main() {
  console.log('🔧 SONA - Custom Configuration Example\n');

  // Create engine with custom configuration
  const config = {
    hiddenDim: 512,
    embeddingDim: 512,
    microLoraRank: 2,
    baseLoraRank: 16,
    microLoraLr: 0.002,
    baseLoraLr: 0.0002,
    ewcLambda: 500.0,
    patternClusters: 100,
    trajectoryCapacity: 5000,
    backgroundIntervalMs: 1800000, // 30 minutes
    qualityThreshold: 0.7,
    enableSimd: true,
  };

  console.log('Configuration:', JSON.stringify(config, null, 2));
  const engine = SonaEngine.withConfig(config);
  console.log('✓ Engine created with custom config\n');

  // Record high-quality trajectories
  console.log('Recording high-quality trajectories...');
  for (let i = 0; i < 20; i++) {
    const queryEmbedding = Array(512).fill(0).map(() => Math.random());
    const builder = engine.beginTrajectory(queryEmbedding);

    // Multiple inference steps
    for (let step = 0; step < 5; step++) {
      const activations = Array(512).fill(0).map(() => Math.random());
      const attentionWeights = Array(128).fill(0).map(() => Math.random());
      const reward = 0.8 + Math.random() * 0.2;

      builder.addStep(activations, attentionWeights, reward);
    }

    builder.setRoute(`high_quality_model_${i % 4}`);
    const quality = 0.85 + Math.random() * 0.15;
    engine.endTrajectory(builder, quality);
  }
  console.log('✓ Recorded 20 high-quality trajectories\n');

  // Apply both micro and base LoRA
  console.log('Applying LoRA transformations...');
  const input = Array(512).fill(1.0);

  const microOutput = engine.applyMicroLora(input);
  console.log(`✓ Micro-LoRA: ${input.length} -> ${microOutput.length}`);

  const baseOutput = engine.applyBaseLora(0, input);
  console.log(`✓ Base-LoRA (layer 0): ${input.length} -> ${baseOutput.length}\n`);

  // Pattern analysis
  console.log('Pattern analysis...');
  const testQuery = Array(512).fill(0).map(() => Math.random());
  const topPatterns = engine.findPatterns(testQuery, 10);

  console.log(`Found ${topPatterns.length} patterns:`);
  topPatterns.slice(0, 3).forEach((pattern, i) => {
    console.log(`  ${i + 1}. ID: ${pattern.id}`);
    console.log(`     Quality: ${pattern.avgQuality.toFixed(3)}`);
    console.log(`     Cluster size: ${pattern.clusterSize}`);
    console.log(`     Type: ${pattern.patternType}`);
  });
  console.log();

  // Enable/disable engine
  console.log('Testing enable/disable...');
  console.log(`Engine enabled: ${engine.isEnabled()}`);
  engine.setEnabled(false);
  console.log(`Engine enabled: ${engine.isEnabled()}`);
  engine.setEnabled(true);
  console.log(`Engine enabled: ${engine.isEnabled()}\n`);

  console.log('✓ Custom configuration example completed!');
}

main();
