/**
 * Continual Learning Dataset Generation
 *
 * This example demonstrates:
 * - Incremental training data generation
 * - Domain adaptation scenarios
 * - Catastrophic forgetting prevention data
 * - Transfer learning datasets
 */

import { AgenticSynth, createSynth } from '../../src/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Incremental Training Data
// ============================================================================

/**
 * Generate incremental training batches for continual learning
 */
export async function generateIncrementalData() {
  console.log('\n📈 Example 1: Incremental Training Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate data for multiple training phases
  const phases = [];

  for (let phase = 1; phase <= 5; phase++) {
    console.log(`\nGenerating Phase ${phase} data...`);

    const phaseData = await synth.generateStructured({
      count: 200,
      schema: {
        sample_id: 'UUID',
        phase: `number (${phase})`,

        // Features (gradually evolving)
        features: {
          core_feature_1: 'number (0-100)',
          core_feature_2: 'number (0-100)',
          // New features introduced in later phases
          phase_specific_feature: `number (0-100) or null (null if phase < ${Math.min(phase + 1, 3)})`,
          evolving_feature: `number (${phase * 10}-${(phase + 1) * 10})`,
        },

        // Label (distribution shifts over phases)
        label: 'number (0-4)',
        label_distribution_bias: `number (${phase - 1}-${phase}, bias toward class ${phase - 1})`,

        // Data characteristics
        noise_level: `number (${0.05 * phase}-${0.05 * (phase + 1)}, increasing noise)`,
        difficulty: 'easy | medium | hard',

        // Metadata
        timestamp: 'ISO timestamp',
        data_source: `source_${phase}`,
      },
      constraints: [
        `Label distribution should be biased toward class ${phase - 1}`,
        'Noise level should increase with phase number',
        'Difficulty should vary across phases',
        'phase_specific_feature should be null for early phases',
      ],
    });

    console.log(`  - Generated ${phaseData.data.length} samples`);
    console.log(`  - Avg noise level: ${calculateAverage(phaseData.data, 'noise_level').toFixed(3)}`);
    console.log(`  - Label distribution:`, getLabelDistribution(phaseData.data));

    phases.push(phaseData);
  }

  console.log('\n✅ Incremental data generation complete');
  console.log(`Total phases: ${phases.length}`);
  console.log(`Total samples: ${phases.reduce((sum, p) => sum + p.data.length, 0)}`);

  return phases;
}

// ============================================================================
// Example 2: Domain Adaptation Scenarios
// ============================================================================

/**
 * Generate source and target domain data for domain adaptation
 */
export async function generateDomainAdaptationData() {
  console.log('\n🌍 Example 2: Domain Adaptation Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Source domain: Product reviews from electronics
  console.log('Generating source domain data (Electronics Reviews)...');
  const sourceData = await synth.generateStructured({
    count: 300,
    schema: {
      review_id: 'UUID',
      domain: 'string (source_electronics)',

      // Text data
      review_text: 'product review for electronics (2-4 sentences)',
      sentiment: 'positive | negative | neutral',

      // Domain-specific features
      domain_features: {
        mentions_battery: 'boolean',
        mentions_screen: 'boolean',
        mentions_performance: 'boolean',
        technical_terms_count: 'number (0-10)',
      },

      // Labels
      rating: 'number (1-5)',
      helpful_votes: 'number (0-100)',

      // Feature representation
      feature_vector: ['array of 50 numbers (0-1, embedding)'],

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Sentiment should correlate with rating',
      'Technical terms should be common (avg 3-5)',
      'Electronics-specific vocabulary',
    ],
  });

  console.log(`  - Source samples: ${sourceData.data.length}`);
  console.log(`  - Avg rating: ${calculateAverage(sourceData.data, 'rating').toFixed(2)}`);

  // Target domain: Product reviews from home goods (different distribution)
  console.log('\nGenerating target domain data (Home Goods Reviews)...');
  const targetData = await synth.generateStructured({
    count: 300,
    schema: {
      review_id: 'UUID',
      domain: 'string (target_home_goods)',

      // Text data
      review_text: 'product review for home goods/furniture (2-4 sentences)',
      sentiment: 'positive | negative | neutral',

      // Domain-specific features (different from source)
      domain_features: {
        mentions_comfort: 'boolean',
        mentions_quality: 'boolean',
        mentions_design: 'boolean',
        technical_terms_count: 'number (0-3, fewer technical terms)',
      },

      // Labels (same task, different domain)
      rating: 'number (1-5)',
      helpful_votes: 'number (0-100)',

      // Feature representation (different distribution)
      feature_vector: ['array of 50 numbers (0-1, embedding with distribution shift)'],

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Sentiment should correlate with rating',
      'Fewer technical terms than source domain',
      'Home goods-specific vocabulary',
      'Feature vectors should have different distribution than source',
    ],
  });

  console.log(`  - Target samples: ${targetData.data.length}`);
  console.log(`  - Avg rating: ${calculateAverage(targetData.data, 'rating').toFixed(2)}`);

  // Generate small labeled target set for adaptation
  console.log('\nGenerating labeled target samples for adaptation...');
  const labeledTargetData = await synth.generateStructured({
    count: 50, // Small labeled set
    schema: {
      review_id: 'UUID',
      domain: 'string (target_home_goods_labeled)',
      review_text: 'product review for home goods/furniture (2-4 sentences)',
      sentiment: 'positive | negative | neutral',

      domain_features: {
        mentions_comfort: 'boolean',
        mentions_quality: 'boolean',
        mentions_design: 'boolean',
        technical_terms_count: 'number (0-3)',
      },

      rating: 'number (1-5)',
      helpful_votes: 'number (0-100)',
      feature_vector: ['array of 50 numbers (0-1)'],

      // Adaptation metadata
      used_for_adaptation: 'boolean (true)',
      similarity_to_source: 'number (0-1, measure of domain similarity)',

      timestamp: 'ISO timestamp',
    },
  });

  console.log(`  - Labeled target samples: ${labeledTargetData.data.length}`);

  console.log('\n✅ Domain adaptation data generated');

  return {
    source: sourceData,
    target: targetData,
    labeledTarget: labeledTargetData,
  };
}

// ============================================================================
// Example 3: Catastrophic Forgetting Prevention Data
// ============================================================================

/**
 * Generate replay buffer and interleaved training data
 */
export async function generateAntiCatastrophicData() {
  console.log('\n🧠 Example 3: Catastrophic Forgetting Prevention\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Task 1: Image classification (animals)
  console.log('Generating Task 1 data (Animal Classification)...');
  const task1Data = await synth.generateStructured({
    count: 200,
    schema: {
      sample_id: 'UUID',
      task_id: 'number (1)',
      task_name: 'string (animal_classification)',

      // Image features (simulated)
      image_features: ['array of 100 numbers (0-1, CNN features)'],

      // Labels
      category: 'cat | dog | bird | fish',
      subcategory: 'specific breed or species',

      // Importance for replay
      importance_score: 'number (0-1, for experience replay)',
      difficulty: 'number (0-1)',

      timestamp: 'ISO timestamp',
    },
  });

  console.log(`  - Task 1 samples: ${task1Data.data.length}`);

  // Task 2: Image classification (vehicles) - New task
  console.log('\nGenerating Task 2 data (Vehicle Classification)...');
  const task2Data = await synth.generateStructured({
    count: 200,
    schema: {
      sample_id: 'UUID',
      task_id: 'number (2)',
      task_name: 'string (vehicle_classification)',

      // Image features (different distribution)
      image_features: ['array of 100 numbers (0-1, CNN features)'],

      // Labels (different classes)
      category: 'car | truck | motorcycle | bicycle',
      subcategory: 'specific model or type',

      importance_score: 'number (0-1)',
      difficulty: 'number (0-1)',

      timestamp: 'ISO timestamp',
    },
  });

  console.log(`  - Task 2 samples: ${task2Data.data.length}`);

  // Generate replay buffer (selected samples from Task 1)
  console.log('\nGenerating replay buffer...');
  const replayBuffer = await synth.generateStructured({
    count: 50, // 25% of Task 1
    schema: {
      sample_id: 'UUID',
      task_id: 'number (1)',
      task_name: 'string (animal_classification)',

      image_features: ['array of 100 numbers (0-1)'],

      category: 'cat | dog | bird | fish',
      subcategory: 'specific breed or species',

      // Replay metadata
      importance_score: 'number (0.5-1.0, high importance)',
      replay_count: 'number (0-5)',
      last_replayed: 'ISO timestamp',

      is_replay_sample: 'boolean (true)',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'importance_score should be high (>0.5)',
      'Select diverse and difficult samples for replay',
    ],
  });

  console.log(`  - Replay buffer size: ${replayBuffer.data.length}`);

  // Generate interleaved training data
  console.log('\nGenerating interleaved training batches...');
  const interleavedBatch = await synth.generateStructured({
    count: 100,
    schema: {
      batch_id: 'UUID',
      batch_number: 'number (1-20)',

      // Mix of Task 2 (new) and Task 1 (replay)
      samples: [
        {
          sample_id: 'UUID',
          task_id: 'number (1 or 2)',
          is_replay: 'boolean (true for task_id=1)',
          features: ['array of 100 numbers'],
          label: 'string',
        },
      ],

      // Batch composition
      task1_ratio: 'number (0.2-0.3, 20-30% replay)',
      task2_ratio: 'number (0.7-0.8, 70-80% new task)',

      // Forgetting metrics
      task1_performance_estimate: 'number (0.7-0.95, should stay high)',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Each batch should contain 20-30% Task 1 samples (replay)',
      'Replay samples should maintain Task 1 performance',
    ],
  });

  console.log(`  - Interleaved batches: ${interleavedBatch.data.length}`);

  console.log('\n✅ Anti-catastrophic forgetting data generated');

  return {
    task1: task1Data,
    task2: task2Data,
    replay: replayBuffer,
    interleaved: interleavedBatch,
  };
}

// ============================================================================
// Example 4: Transfer Learning Datasets
// ============================================================================

/**
 * Generate pre-training and fine-tuning datasets
 */
export async function generateTransferLearningData() {
  console.log('\n🔄 Example 4: Transfer Learning Datasets\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Pre-training data: Large, general dataset
  console.log('Generating pre-training data (General Text)...');
  const pretrainingData = await synth.generateStructured({
    count: 1000,
    schema: {
      sample_id: 'UUID',
      stage: 'string (pretraining)',

      // General text data
      text: 'general text passage (3-5 sentences)',
      domain: 'news | wikipedia | books | web',

      // Self-supervised labels
      masked_tokens: ['array of masked token positions'],
      next_sentence_label: 'boolean (is next sentence)',

      // Features
      embedding: ['array of 768 numbers (transformer embedding)'],
      token_count: 'number (50-200)',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Diverse domains and topics',
      'General language patterns',
      'High-quality, grammatical text',
    ],
  });

  console.log(`  - Pre-training samples: ${pretrainingData.data.length}`);

  // Fine-tuning data: Smaller, task-specific dataset
  console.log('\nGenerating fine-tuning data (Sentiment Analysis)...');
  const finetuningData = await synth.generateStructured({
    count: 200,
    schema: {
      sample_id: 'UUID',
      stage: 'string (finetuning)',

      // Task-specific text
      text: 'product or movie review (2-4 sentences)',
      domain: 'string (reviews)',

      // Supervised labels
      sentiment: 'positive | negative | neutral',
      confidence: 'number (0-1)',

      // Features (initialized from pre-trained model)
      embedding: ['array of 768 numbers (fine-tuned embedding)'],
      token_count: 'number (30-150)',

      // Fine-tuning metadata
      learning_phase: 'early | middle | late',
      layer_to_finetune: 'all | last_2 | last_4 | classifier_only',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Domain-specific vocabulary',
      'Clear sentiment labels',
      'Smaller dataset than pre-training',
    ],
  });

  console.log(`  - Fine-tuning samples: ${finetuningData.data.length}`);

  // Generate few-shot learning data
  console.log('\nGenerating few-shot learning data...');
  const fewShotData = await synth.generateStructured({
    count: 50, // Very small
    schema: {
      sample_id: 'UUID',
      stage: 'string (few_shot)',

      // Task-specific examples
      text: 'specialized domain text (legal, medical, technical)',
      domain: 'legal | medical | scientific',

      // Labels
      category: 'specialized category',
      requires_expertise: 'boolean (true)',

      // Few-shot metadata
      support_set: 'boolean (used as few-shot example)',
      shot_number: 'number (1-5, which shot in few-shot set)',

      embedding: ['array of 768 numbers'],

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Highly specialized domain',
      'Very limited samples (few-shot)',
      'Clear, prototypical examples',
    ],
  });

  console.log(`  - Few-shot samples: ${fewShotData.data.length}`);

  console.log('\n✅ Transfer learning data generated');
  console.log('Data pipeline: Pre-training → Fine-tuning → Few-shot');

  return {
    pretraining: pretrainingData,
    finetuning: finetuningData,
    fewShot: fewShotData,
  };
}

// ============================================================================
// Example 5: Curriculum Learning Data
// ============================================================================

/**
 * Generate data organized by difficulty for curriculum learning
 */
export async function generateCurriculumData() {
  console.log('\n🎓 Example 5: Curriculum Learning Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  const difficulties = ['easy', 'medium', 'hard', 'expert'];
  const curriculum = [];

  for (const difficulty of difficulties) {
    console.log(`\nGenerating ${difficulty} difficulty data...`);

    const difficultyData = await synth.generateStructured({
      count: 150,
      schema: {
        sample_id: 'UUID',
        difficulty_level: `string (${difficulty})`,
        curriculum_stage: `number (${difficulties.indexOf(difficulty) + 1})`,

        // Math problem (example task)
        problem: {
          question: `math word problem (${difficulty} difficulty)`,
          steps_required: `number (${difficulties.indexOf(difficulty) + 1}-${difficulties.indexOf(difficulty) + 3})`,
          concepts: [`array of ${difficulties.indexOf(difficulty) + 1}-${difficulties.indexOf(difficulty) + 2} math concepts`],
        },

        // Solution
        solution: {
          answer: 'correct numerical answer',
          explanation: 'step-by-step solution',
          intermediate_steps: ['array of solution steps'],
        },

        // Difficulty metrics
        estimated_time_seconds: `number (${(difficulties.indexOf(difficulty) + 1) * 30}-${(difficulties.indexOf(difficulty) + 2) * 30})`,
        concept_complexity: `number (${difficulties.indexOf(difficulty) + 1}-${difficulties.indexOf(difficulty) + 2})`,
        prerequisite_skills: [`array of required skills (more for harder problems)`],

        // Learning metadata
        success_rate_expected: `number (${0.9 - difficulties.indexOf(difficulty) * 0.15}-${0.95 - difficulties.indexOf(difficulty) * 0.15})`,

        timestamp: 'ISO timestamp',
      },
      constraints: [
        `Problems should be ${difficulty} difficulty`,
        'Success rate should decrease with difficulty',
        'More concepts required for harder problems',
        'Prerequisite skills accumulate',
      ],
    });

    console.log(`  - ${difficulty} samples: ${difficultyData.data.length}`);
    console.log(`  - Avg steps: ${calculateAverage(difficultyData.data, (d: any) => d.problem.steps_required)}`);

    curriculum.push({
      stage: difficulties.indexOf(difficulty) + 1,
      difficulty,
      data: difficultyData,
    });
  }

  console.log('\n✅ Curriculum learning data generated');
  console.log(`Curriculum stages: ${curriculum.length}`);
  console.log('Learning progression: easy → medium → hard → expert');

  return curriculum;
}

// ============================================================================
// Example 6: Online Learning Stream
// ============================================================================

/**
 * Generate streaming data for online learning
 */
export async function generateOnlineLearningStream() {
  console.log('\n📡 Example 6: Online Learning Stream\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate time-series data stream
  const streamData = await synth.generateTimeSeries({
    count: 500, // 500 time points
    interval: '1m', // One sample per minute
    schema: {
      timestamp: 'ISO timestamp',
      sequence_number: 'number (sequential)',

      // Incoming data point
      features: ['array of 20 numbers (0-1)'],
      label: 'number (0-4)',

      // Distribution characteristics
      distribution_shift: 'number (0-1, gradual increase over time)',
      concept_drift_indicator: 'boolean',

      // Model state
      current_model_accuracy: 'number (0.7-0.95, may degrade over time)',
      should_update_model: 'boolean (true if drift detected)',

      // Online learning metadata
      learning_rate: 'number (0.0001-0.01)',
      update_applied: 'boolean',
      samples_since_update: 'number (0-100)',

      // Performance tracking
      prediction_error: 'number (0-1)',
      cumulative_regret: 'number (increasing)',
    },
    trend: 'stable',
    seasonality: false,
    constraints: [
      'Distribution shift should gradually increase',
      'Model accuracy should correlate inversely with drift',
      'should_update_model when accuracy drops or drift detected',
      'cumulative_regret increases when predictions are wrong',
    ],
  });

  const driftPoints = streamData.data.filter((d: any) => d.concept_drift_indicator);

  console.log('Online Learning Stream:');
  console.log(`- Stream length: ${streamData.data.length} samples`);
  console.log(`- Concept drift points: ${driftPoints.length}`);
  console.log(`- Avg accuracy: ${calculateAverage(streamData.data, 'current_model_accuracy').toFixed(3)}`);
  console.log(`- Model updates: ${streamData.data.filter((d: any) => d.update_applied).length}`);

  console.log('\n✅ Online learning stream generated');

  return streamData;
}

// ============================================================================
// Utility Functions
// ============================================================================

function calculateAverage(data: any[], field: string | ((d: any) => number)): number {
  const values =
    typeof field === 'string'
      ? data.map((d) => d[field]).filter((v) => typeof v === 'number')
      : data.map(field).filter((v) => typeof v === 'number');

  if (values.length === 0) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function getLabelDistribution(data: any[]): Record<string, number> {
  const dist: Record<string, number> = {};
  data.forEach((d) => {
    const label = d.label.toString();
    dist[label] = (dist[label] || 0) + 1;
  });
  return dist;
}

// ============================================================================
// Complete Continual Learning Pipeline
// ============================================================================

/**
 * Demonstrate complete continual learning pipeline
 */
export async function completeContinualLearningPipeline() {
  console.log('\n🚀 Complete Continual Learning Pipeline\n');
  console.log('='.repeat(60));

  console.log('\nStage 1: Initial Training with Curriculum');
  const curriculum = await generateCurriculumData();

  console.log('\nStage 2: Domain Adaptation');
  const domainData = await generateDomainAdaptationData();

  console.log('\nStage 3: Incremental Learning');
  const incrementalData = await generateIncrementalData();

  console.log('\nStage 4: Catastrophic Forgetting Prevention');
  const antiForgetData = await generateAntiCatastrophicData();

  console.log('\nStage 5: Online Learning');
  const onlineData = await generateOnlineLearningStream();

  console.log('\n' + '='.repeat(60));
  console.log('✅ Complete continual learning pipeline executed');
  console.log('\nPipeline Summary:');
  console.log('  1. Curriculum Learning (easy → hard)');
  console.log('  2. Domain Adaptation (source → target)');
  console.log('  3. Incremental Learning (phase 1 → phase N)');
  console.log('  4. Experience Replay (prevent forgetting)');
  console.log('  5. Online Learning (continuous stream)');
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllContinualLearningExamples() {
  console.log('🎯 Continual Learning Dataset Generation\n');
  console.log('='.repeat(60));

  try {
    await generateIncrementalData();
    console.log('='.repeat(60));

    await generateDomainAdaptationData();
    console.log('='.repeat(60));

    await generateAntiCatastrophicData();
    console.log('='.repeat(60));

    await generateTransferLearningData();
    console.log('='.repeat(60));

    await generateCurriculumData();
    console.log('='.repeat(60));

    await generateOnlineLearningStream();
    console.log('='.repeat(60));

    console.log('\n✅ All continual learning examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error:', error.message);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllContinualLearningExamples().catch(console.error);
}
