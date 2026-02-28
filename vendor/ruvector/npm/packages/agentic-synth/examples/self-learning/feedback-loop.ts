/**
 * Self-Improving Data Generation with Feedback Loops
 *
 * This example demonstrates:
 * - Quality scoring and regeneration
 * - A/B testing data for model improvement
 * - Pattern learning from production data
 * - Adaptive schema evolution
 */

import { AgenticSynth, createSynth } from '../../src/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Quality Scoring and Regeneration
// ============================================================================

/**
 * Generate data with quality scores and regenerate low-quality samples
 */
export async function qualityScoringLoop() {
  console.log('\n⭐ Example 1: Quality Scoring and Regeneration\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Initial generation with quality metrics
  const initialData = await synth.generateStructured({
    count: 100,
    schema: {
      id: 'UUID',
      content: 'product description (2-3 sentences)',
      category: 'electronics | clothing | home | sports',
      price: 'number (10-1000)',

      // Quality metrics (would be computed by quality model)
      quality_score: 'number (0-1, overall quality)',
      metrics: {
        coherence: 'number (0-1)',
        relevance: 'number (0-1)',
        completeness: 'number (0-1)',
        grammar: 'number (0-1)',
      },

      // Metadata
      generation_attempt: 'number (1)',
      timestamp: 'ISO timestamp',
    },
    constraints: [
      'quality_score should be average of metrics',
      '20% of samples should have quality_score < 0.7 (for regeneration demo)',
      'grammar score should be high (0.8-1.0)',
    ],
  });

  console.log('Initial Generation:');
  console.log(`- Total samples: ${initialData.data.length}`);
  console.log(`- Average quality: ${calculateAverage(initialData.data, 'quality_score')}`);

  // Identify low-quality samples
  const lowQuality = initialData.data.filter((d: any) => d.quality_score < 0.7);
  console.log(`- Low quality samples: ${lowQuality.length}`);

  if (lowQuality.length > 0) {
    // Regenerate low-quality samples with feedback
    const regenerated = await synth.generateStructured({
      count: lowQuality.length,
      schema: {
        id: 'UUID',
        content: 'product description (2-3 sentences, improve coherence and completeness)',
        category: 'electronics | clothing | home | sports',
        price: 'number (10-1000)',

        // Quality metrics
        quality_score: 'number (0.7-1.0, improved quality)',
        metrics: {
          coherence: 'number (0.7-1.0)',
          relevance: 'number (0.7-1.0)',
          completeness: 'number (0.7-1.0)',
          grammar: 'number (0.9-1.0)',
        },

        // Track regeneration
        generation_attempt: 'number (2)',
        previous_issues: ['array of issues that were fixed'],
        timestamp: 'ISO timestamp',
      },
      constraints: [
        'All samples should have quality_score >= 0.7',
        'Focus on improving coherence and completeness',
        'Maintain high grammar scores',
      ],
    });

    console.log('\nRegenerated Samples:');
    console.log(`- Count: ${regenerated.data.length}`);
    console.log(`- Average quality: ${calculateAverage(regenerated.data, 'quality_score')}`);
    console.log(`- Quality improvement: ${
      calculateAverage(regenerated.data, 'quality_score') -
      calculateAverage(lowQuality, 'quality_score')
    }`);
  }

  console.log('\n✅ Quality scoring loop complete');
}

// ============================================================================
// Example 2: A/B Testing Data for Model Improvement
// ============================================================================

/**
 * Generate A/B test data to improve model performance
 */
export async function abTestingData() {
  console.log('\n🔬 Example 2: A/B Testing Data Generation\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate A/B test scenarios
  const abTests = await synth.generateStructured({
    count: 200,
    schema: {
      test_id: 'UUID',
      variant: 'A | B',

      // Input features
      user_profile: {
        age: 'number (18-80)',
        location: 'US state',
        interests: ['array of 2-5 interests'],
        past_purchases: 'number (0-100)',
      },

      // Model predictions
      model_variant: 'baseline_model | improved_model (based on variant)',
      prediction: 'number (0-1, predicted conversion probability)',
      confidence: 'number (0-1)',

      // Actual outcome
      actual_conversion: 'boolean',
      conversion_value: 'number (0-500) if converted',

      // Performance metrics
      prediction_error: 'number (absolute error)',
      calibration_error: 'number',

      // Metadata
      timestamp: 'ISO timestamp',
      feature_version: 'v1.0 | v1.1',
    },
    constraints: [
      'Variant A should use baseline_model',
      'Variant B should use improved_model',
      'Variant B should have higher accuracy (lower prediction_error)',
      'Variant B should have better calibration',
      'Distribution of user_profile should be similar across variants',
      'prediction should correlate with actual_conversion',
    ],
  });

  // Analyze A/B test results
  const variantA = abTests.data.filter((d: any) => d.variant === 'A');
  const variantB = abTests.data.filter((d: any) => d.variant === 'B');

  console.log('A/B Test Results:');
  console.log(`\nVariant A (Baseline):`);
  console.log(`  - Samples: ${variantA.length}`);
  console.log(`  - Avg prediction error: ${calculateAverage(variantA, 'prediction_error').toFixed(4)}`);
  console.log(`  - Conversion rate: ${calculateConversionRate(variantA)}%`);

  console.log(`\nVariant B (Improved):`);
  console.log(`  - Samples: ${variantB.length}`);
  console.log(`  - Avg prediction error: ${calculateAverage(variantB, 'prediction_error').toFixed(4)}`);
  console.log(`  - Conversion rate: ${calculateConversionRate(variantB)}%`);

  const improvement = (
    ((calculateAverage(variantA, 'prediction_error') -
      calculateAverage(variantB, 'prediction_error')) /
      calculateAverage(variantA, 'prediction_error')) *
    100
  );

  console.log(`\nImprovement: ${improvement.toFixed(2)}% reduction in error`);
  console.log('✅ A/B testing data generated');

  return abTests;
}

// ============================================================================
// Example 3: Pattern Learning from Production Data
// ============================================================================

/**
 * Learn patterns from production data and generate similar synthetic data
 */
export async function patternLearningLoop() {
  console.log('\n🧠 Example 3: Pattern Learning from Production\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Simulate production data patterns
  const productionPatterns = {
    common_sequences: [
      ['login', 'browse', 'add_to_cart', 'checkout'],
      ['login', 'browse', 'search', 'view_product'],
      ['browse', 'search', 'add_to_cart', 'abandon'],
    ],
    time_distributions: {
      peak_hours: [9, 12, 18, 20],
      avg_session_duration: 420, // seconds
      bounce_rate: 0.35,
    },
    user_segments: {
      frequent_buyers: 0.15,
      casual_browsers: 0.50,
      one_time_visitors: 0.35,
    },
  };

  // Generate synthetic data matching learned patterns
  const syntheticData = await synth.generateStructured({
    count: 500,
    schema: {
      session_id: 'UUID',
      user_segment: 'frequent_buyer | casual_browser | one_time_visitor',

      // Event sequence following learned patterns
      events: [
        {
          event_type: 'login | browse | search | add_to_cart | checkout | abandon | view_product',
          timestamp: 'ISO timestamp',
          duration: 'number (5-300, seconds)',
        },
      ],

      // Session metrics
      total_duration: 'number (60-900, seconds, should match avg from patterns)',
      hour_of_day: 'number (0-23, biased toward peak hours)',
      bounced: 'boolean (35% true)',
      converted: 'boolean',

      // Pattern conformance
      matches_common_sequence: 'boolean (80% should be true)',
      pattern_id: 'number (0-2) if matches sequence',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'User segment distribution should match: 15% frequent_buyer, 50% casual_browser, 35% one_time_visitor',
      'Hour of day should be biased toward 9, 12, 18, 20',
      'Event sequences should follow common patterns 80% of time',
      'total_duration should be around 420 seconds on average',
      'bounce_rate should be approximately 35%',
      'frequent_buyers should have higher conversion rate',
    ],
  });

  console.log('Pattern-Learned Synthetic Data:');
  console.log(`- Total sessions: ${syntheticData.data.length}`);
  console.log(`- User segment distribution:`, getUserSegmentDist(syntheticData.data));
  console.log(`- Avg session duration: ${calculateAverage(syntheticData.data, 'total_duration').toFixed(0)}s`);
  console.log(`- Bounce rate: ${calculateBounceRate(syntheticData.data)}%`);
  console.log(`- Pattern conformance: ${calculatePatternConformance(syntheticData.data)}%`);

  console.log('\n✅ Pattern learning complete');

  return syntheticData;
}

// ============================================================================
// Example 4: Adaptive Schema Evolution
// ============================================================================

/**
 * Evolve data schema based on feedback and changing requirements
 */
export async function adaptiveSchemaEvolution() {
  console.log('\n🔄 Example 4: Adaptive Schema Evolution\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Version 1: Initial schema
  console.log('Schema V1 (Initial):');
  const v1Data = await synth.generateStructured({
    count: 50,
    schema: {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',

      schema_version: 'string (v1.0)',
    },
  });
  console.log(`  - Generated ${v1Data.data.length} records`);
  console.log(`  - Fields: id, name, email, age`);

  // Simulate feedback: need more demographic info
  console.log('\nFeedback: Need location and preferences');

  // Version 2: Add fields based on feedback
  console.log('\nSchema V2 (Enhanced):');
  const v2Data = await synth.generateStructured({
    count: 50,
    schema: {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',

      // New fields
      location: {
        city: 'city name',
        state: 'US state',
        country: 'country name',
      },
      preferences: ['array of 3-5 preference categories'],

      schema_version: 'string (v2.0)',
    },
  });
  console.log(`  - Generated ${v2Data.data.length} records`);
  console.log(`  - Fields: id, name, email, age, location, preferences`);

  // Simulate more feedback: need behavioral data
  console.log('\nFeedback: Need behavioral and engagement metrics');

  // Version 3: Add behavioral tracking
  console.log('\nSchema V3 (Full Featured):');
  const v3Data = await synth.generateStructured({
    count: 50,
    schema: {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',

      location: {
        city: 'city name',
        state: 'US state',
        country: 'country name',
      },
      preferences: ['array of 3-5 preference categories'],

      // New behavioral fields
      behavioral_metrics: {
        total_sessions: 'number (0-500)',
        avg_session_duration: 'number (60-3600, seconds)',
        last_active: 'ISO timestamp (within last 30 days)',
        engagement_score: 'number (0-100)',
        ltv: 'number (0-10000, lifetime value)',
      },

      // New segmentation
      user_segment: 'high_value | medium_value | low_value | churned',
      predicted_churn: 'boolean',
      churn_risk_score: 'number (0-1)',

      schema_version: 'string (v3.0)',
    },
    constraints: [
      'engagement_score should correlate with total_sessions',
      'ltv should be higher for high_value segment',
      'churned users should have old last_active dates',
      'churn_risk_score should be high for predicted_churn=true',
    ],
  });
  console.log(`  - Generated ${v3Data.data.length} records`);
  console.log(`  - Fields: All previous + behavioral_metrics, user_segment, churn predictions`);

  // Show schema evolution
  console.log('\nSchema Evolution Summary:');
  console.log('  V1 → V2: Added location and preferences');
  console.log('  V2 → V3: Added behavioral metrics and churn prediction');
  console.log('  Field count: 5 → 7 → 12');

  console.log('\n✅ Adaptive schema evolution complete');

  return { v1: v1Data, v2: v2Data, v3: v3Data };
}

// ============================================================================
// Example 5: Active Learning Data Generation
// ============================================================================

/**
 * Generate data for active learning - focus on uncertain/informative samples
 */
export async function activeLearningData() {
  console.log('\n🎯 Example 5: Active Learning Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate samples with uncertainty scores
  const activeLearningData = await synth.generateStructured({
    count: 300,
    schema: {
      sample_id: 'UUID',

      // Features
      features: {
        feature_1: 'number (0-100)',
        feature_2: 'number (0-100)',
        feature_3: 'number (0-100)',
        feature_4: 'number (0-100)',
      },

      // Model predictions
      predicted_class: 'number (0-4)',
      prediction_confidence: 'number (0-1)',
      uncertainty_score: 'number (0-1, inverse of confidence)',

      // Active learning strategy
      query_strategy: 'uncertainty_sampling | query_by_committee | expected_model_change',
      should_label: 'boolean (true if high uncertainty)',

      // If labeled
      true_label: 'number (0-4) or null',
      was_useful: 'boolean or null (if labeled)',

      // Metadata
      iteration: 'number (1-10, active learning iteration)',
      timestamp: 'ISO timestamp',
    },
    constraints: [
      'uncertainty_score should equal 1 - prediction_confidence',
      'should_label should be true for samples with uncertainty > 0.6',
      '30% of samples should have high uncertainty (>0.6)',
      'true_label should be provided if should_label is true',
      'was_useful should correlate with uncertainty_score',
    ],
  });

  const highUncertainty = activeLearningData.data.filter((d: any) => d.uncertainty_score > 0.6);
  const shouldLabel = activeLearningData.data.filter((d: any) => d.should_label);

  console.log('Active Learning Data:');
  console.log(`- Total samples: ${activeLearningData.data.length}`);
  console.log(`- High uncertainty samples: ${highUncertainty.length}`);
  console.log(`- Samples to label: ${shouldLabel.length}`);
  console.log(`- Avg uncertainty: ${calculateAverage(activeLearningData.data, 'uncertainty_score').toFixed(3)}`);
  console.log(`- Strategy distribution:`, getStrategyDistribution(activeLearningData.data));

  console.log('\n✅ Active learning data generated');

  return activeLearningData;
}

// ============================================================================
// Example 6: Continuous Model Evaluation Data
// ============================================================================

/**
 * Generate evaluation data for continuous model monitoring
 */
export async function continuousEvaluationData() {
  console.log('\n📊 Example 6: Continuous Evaluation Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate time-series evaluation data
  const evaluationData = await synth.generateTimeSeries({
    count: 168, // One week, hourly
    interval: '1h',
    schema: {
      timestamp: 'ISO timestamp',
      hour: 'number (0-23)',

      // Model performance metrics
      accuracy: 'number (0.7-0.95)',
      precision: 'number (0.7-0.95)',
      recall: 'number (0.7-0.95)',
      f1_score: 'number (0.7-0.95)',

      // Data distribution metrics
      prediction_distribution: {
        class_0: 'number (0-1, proportion)',
        class_1: 'number (0-1, proportion)',
      },
      confidence_distribution: {
        high: 'number (0-1, >0.8)',
        medium: 'number (0-1, 0.5-0.8)',
        low: 'number (0-1, <0.5)',
      },

      // Drift detection
      feature_drift_score: 'number (0-1)',
      prediction_drift_score: 'number (0-1)',
      alert_triggered: 'boolean (true if drift > 0.3)',

      // System metrics
      inference_latency_ms: 'number (10-100)',
      throughput_qps: 'number (100-1000)',
      error_rate: 'number (0-0.05)',
    },
    trend: 'stable',
    seasonality: true,
    constraints: [
      'Performance should degrade slightly during peak hours (9-17)',
      'alert_triggered should be true when drift scores > 0.3',
      'Drift should gradually increase over time (concept drift)',
      'Latency should be higher during peak traffic',
    ],
  });

  const alerts = evaluationData.data.filter((d: any) => d.alert_triggered);

  console.log('Continuous Evaluation Data:');
  console.log(`- Time points: ${evaluationData.data.length}`);
  console.log(`- Average accuracy: ${calculateAverage(evaluationData.data, 'accuracy').toFixed(3)}`);
  console.log(`- Average drift score: ${calculateAverage(evaluationData.data, 'feature_drift_score').toFixed(3)}`);
  console.log(`- Drift alerts: ${alerts.length}`);
  console.log(`- Average latency: ${calculateAverage(evaluationData.data, 'inference_latency_ms').toFixed(1)}ms`);

  console.log('\n✅ Continuous evaluation data generated');

  return evaluationData;
}

// ============================================================================
// Utility Functions
// ============================================================================

function calculateAverage(data: any[], field: string): number {
  const values = data.map((d) => d[field]).filter((v) => typeof v === 'number');
  if (values.length === 0) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function calculateConversionRate(data: any[]): number {
  const converted = data.filter((d) => d.actual_conversion).length;
  return (converted / data.length) * 100;
}

function calculateBounceRate(data: any[]): number {
  const bounced = data.filter((d) => d.bounced).length;
  return (bounced / data.length) * 100;
}

function calculatePatternConformance(data: any[]): number {
  const matching = data.filter((d) => d.matches_common_sequence).length;
  return (matching / data.length) * 100;
}

function getUserSegmentDist(data: any[]): Record<string, number> {
  const dist: Record<string, number> = {};
  data.forEach((d) => {
    dist[d.user_segment] = (dist[d.user_segment] || 0) + 1;
  });
  return dist;
}

function getStrategyDistribution(data: any[]): Record<string, number> {
  const dist: Record<string, number> = {};
  data.forEach((d) => {
    dist[d.query_strategy] = (dist[d.query_strategy] || 0) + 1;
  });
  return dist;
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllFeedbackLoopExamples() {
  console.log('🔄 Self-Improving Feedback Loop Examples\n');
  console.log('='.repeat(60));

  try {
    await qualityScoringLoop();
    console.log('='.repeat(60));

    await abTestingData();
    console.log('='.repeat(60));

    await patternLearningLoop();
    console.log('='.repeat(60));

    await adaptiveSchemaEvolution();
    console.log('='.repeat(60));

    await activeLearningData();
    console.log('='.repeat(60));

    await continuousEvaluationData();
    console.log('='.repeat(60));

    console.log('\n✅ All feedback loop examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error:', error.message);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllFeedbackLoopExamples().catch(console.error);
}
