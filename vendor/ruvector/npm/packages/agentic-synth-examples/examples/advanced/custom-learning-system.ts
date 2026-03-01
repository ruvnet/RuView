/**
 * ADVANCED TUTORIAL: Custom Learning System
 *
 * Extend the self-learning system with custom optimization strategies,
 * domain-specific learning, and advanced evaluation metrics. Perfect for
 * building production-grade adaptive AI systems.
 *
 * What you'll learn:
 * - Creating custom evaluators
 * - Domain-specific optimization
 * - Advanced feedback loops
 * - Multi-objective optimization
 * - Transfer learning patterns
 *
 * Prerequisites:
 * - Complete intermediate tutorials first
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/advanced/custom-learning-system.ts
 */

import { LM, ChainOfThought, Prediction } from 'dspy.ts';
import { AgenticSynth } from '@ruvector/agentic-synth';

// Multi-objective evaluation metrics
interface EvaluationMetrics {
  accuracy: number;
  creativity: number;
  relevance: number;
  engagement: number;
  technicalQuality: number;
  overall: number;
}

// Advanced learning configuration
interface AdvancedLearningConfig {
  domain: string;
  objectives: string[];
  weights: Record<string, number>;
  learningStrategy: 'aggressive' | 'conservative' | 'adaptive';
  convergenceThreshold: number;
  diversityBonus: boolean;
  transferLearning: boolean;
}

// Training example with rich metadata
interface TrainingExample {
  input: any;
  expectedOutput: any;
  quality: number;
  metadata: {
    domain: string;
    difficulty: 'easy' | 'medium' | 'hard';
    tags: string[];
  };
}

// Custom evaluator interface
interface Evaluator {
  evaluate(output: Prediction, context: any): Promise<EvaluationMetrics>;
}

// Domain-specific evaluator for e-commerce
class EcommerceEvaluator implements Evaluator {
  async evaluate(output: Prediction, context: any): Promise<EvaluationMetrics> {
    const metrics: EvaluationMetrics = {
      accuracy: 0,
      creativity: 0,
      relevance: 0,
      engagement: 0,
      technicalQuality: 0,
      overall: 0
    };

    // Accuracy: Check for required information
    if (output.description && output.key_features) {
      metrics.accuracy += 0.5;

      // Check if key product attributes are mentioned
      const desc = output.description.toLowerCase();
      const productName = context.product_name.toLowerCase();
      const category = context.category.toLowerCase();

      if (desc.includes(productName.split(' ')[0])) {
        metrics.accuracy += 0.25;
      }
      if (desc.includes(category)) {
        metrics.accuracy += 0.25;
      }
    }

    // Creativity: Check for unique, non-generic phrases
    if (output.description) {
      const genericPhrases = ['high quality', 'great product', 'best choice'];
      const hasGenericPhrase = genericPhrases.some(phrase =>
        output.description.toLowerCase().includes(phrase)
      );

      metrics.creativity = hasGenericPhrase ? 0.3 : 0.8;

      // Bonus for specific details
      const hasNumbers = /\d+/.test(output.description);
      const hasSpecifics = /(\d+\s*(hours|days|years|gb|mb|kg|lbs))/i.test(output.description);

      if (hasSpecifics) metrics.creativity += 0.2;
    }

    // Relevance: Check alignment with category
    const categoryKeywords: Record<string, string[]> = {
      electronics: ['technology', 'device', 'digital', 'battery', 'power'],
      fashion: ['style', 'design', 'material', 'comfort', 'wear'],
      food: ['taste', 'flavor', 'nutrition', 'organic', 'fresh'],
      fitness: ['workout', 'exercise', 'health', 'training', 'performance']
    };

    const category = context.category.toLowerCase();
    const relevantKeywords = categoryKeywords[category] || [];

    if (output.description) {
      const desc = output.description.toLowerCase();
      const matchedKeywords = relevantKeywords.filter(kw => desc.includes(kw));
      metrics.relevance = Math.min(matchedKeywords.length / 3, 1.0);
    }

    // Engagement: Check for emotional appeal and calls to action
    if (output.description) {
      const desc = output.description.toLowerCase();
      const emotionalWords = ['amazing', 'incredible', 'perfect', 'premium', 'exceptional', 'revolutionary'];
      const actionWords = ['discover', 'experience', 'enjoy', 'upgrade', 'transform'];

      const hasEmotion = emotionalWords.some(word => desc.includes(word));
      const hasAction = actionWords.some(word => desc.includes(word));

      metrics.engagement = (hasEmotion ? 0.5 : 0) + (hasAction ? 0.5 : 0);
    }

    // Technical Quality: Check structure and formatting
    if (output.key_features && Array.isArray(output.key_features)) {
      const features = output.key_features;
      let techScore = 0;

      // Optimal number of features
      if (features.length >= 4 && features.length <= 6) {
        techScore += 0.4;
      }

      // Feature formatting
      const wellFormatted = features.filter(f =>
        f.length >= 15 && f.length <= 60 && !f.endsWith('.')
      );
      techScore += (wellFormatted.length / features.length) * 0.6;

      metrics.technicalQuality = techScore;
    }

    // Calculate overall score with weights
    metrics.overall = (
      metrics.accuracy * 0.25 +
      metrics.creativity * 0.20 +
      metrics.relevance * 0.25 +
      metrics.engagement * 0.15 +
      metrics.technicalQuality * 0.15
    );

    return metrics;
  }
}

// Advanced self-learning generator
class AdvancedLearningSystem {
  private lm: LM;
  private config: AdvancedLearningConfig;
  private evaluator: Evaluator;
  private knowledgeBase: TrainingExample[] = [];
  private promptStrategies: Map<string, number> = new Map();

  constructor(config: AdvancedLearningConfig, evaluator: Evaluator) {
    this.config = config;
    this.evaluator = evaluator;

    this.lm = new LM({
      provider: 'google-genai',
      model: 'gemini-2.0-flash-exp',
      apiKey: process.env.GEMINI_API_KEY || '',
      temperature: this.getTemperatureForStrategy()
    });
  }

  private getTemperatureForStrategy(): number {
    switch (this.config.learningStrategy) {
      case 'aggressive': return 0.9;
      case 'conservative': return 0.5;
      case 'adaptive': return 0.7;
    }
  }

  // Learn from a single example
  async learnFromExample(example: TrainingExample): Promise<void> {
    console.log(`\n🎯 Learning from example (${example.metadata.difficulty})...`);

    const output = await this.generate(example.input);
    const metrics = await this.evaluator.evaluate(output, example.input);

    console.log(`   Overall Quality: ${(metrics.overall * 100).toFixed(1)}%`);
    console.log(`   Accuracy: ${(metrics.accuracy * 100).toFixed(0)}% | Creativity: ${(metrics.creativity * 100).toFixed(0)}%`);
    console.log(`   Relevance: ${(metrics.relevance * 100).toFixed(0)}% | Engagement: ${(metrics.engagement * 100).toFixed(0)}%`);

    // Store high-quality examples
    if (metrics.overall >= 0.7) {
      this.knowledgeBase.push({
        ...example,
        quality: metrics.overall
      });
      console.log(`   ✓ Added to knowledge base`);
    }
  }

  // Train on a dataset
  async train(examples: TrainingExample[]): Promise<void> {
    console.log('🏋️  Starting Advanced Training Session\n');
    console.log('=' .repeat(70));
    console.log(`\nDomain: ${this.config.domain}`);
    console.log(`Strategy: ${this.config.learningStrategy}`);
    console.log(`Examples: ${examples.length}`);
    console.log(`\nObjectives:`);
    this.config.objectives.forEach(obj => console.log(`  • ${obj}`));
    console.log('\n' + '=' .repeat(70));

    // Group by difficulty
    const byDifficulty = {
      easy: examples.filter(e => e.metadata.difficulty === 'easy'),
      medium: examples.filter(e => e.metadata.difficulty === 'medium'),
      hard: examples.filter(e => e.metadata.difficulty === 'hard')
    };

    // Progressive learning: start with easy, move to hard
    console.log('\n📚 Phase 1: Learning Basics (Easy Examples)');
    console.log('─'.repeat(70));
    for (const example of byDifficulty.easy) {
      await this.learnFromExample(example);
    }

    console.log('\n📚 Phase 2: Intermediate Concepts (Medium Examples)');
    console.log('─'.repeat(70));
    for (const example of byDifficulty.medium) {
      await this.learnFromExample(example);
    }

    console.log('\n📚 Phase 3: Advanced Patterns (Hard Examples)');
    console.log('─'.repeat(70));
    for (const example of byDifficulty.hard) {
      await this.learnFromExample(example);
    }

    this.displayTrainingResults();
  }

  // Generate with learned knowledge
  private async generate(input: any): Promise<Prediction> {
    // Use knowledge base for few-shot learning
    const similarExamples = this.findSimilarExamples(input, 3);

    let enhancedDescription = 'Generate compelling product descriptions.';

    if (similarExamples.length > 0) {
      enhancedDescription += '\n\nLearn from these high-quality examples:\n';
      similarExamples.forEach((ex, i) => {
        enhancedDescription += `\nExample ${i + 1}:\n`;
        enhancedDescription += `Input: ${JSON.stringify(ex.input)}\n`;
        enhancedDescription += `Output: ${JSON.stringify(ex.expectedOutput)}`;
      });
    }

    const signature = {
      input: 'product_name: string, category: string, price: number',
      output: 'description: string, key_features: string[]',
      description: enhancedDescription
    };

    const generator = new ChainOfThought(signature, { lm: this.lm });
    return await generator.forward(input);
  }

  // Find similar examples from knowledge base
  private findSimilarExamples(input: any, count: number): TrainingExample[] {
    // Simple similarity based on category match
    const similar = this.knowledgeBase
      .filter(ex => ex.input.category === input.category)
      .sort((a, b) => b.quality - a.quality)
      .slice(0, count);

    return similar;
  }

  // Display training results
  private displayTrainingResults(): void {
    console.log('\n\n' + '=' .repeat(70));
    console.log('\n🎓 TRAINING RESULTS\n');

    console.log(`Knowledge Base: ${this.knowledgeBase.length} high-quality examples`);

    if (this.knowledgeBase.length > 0) {
      const avgQuality = this.knowledgeBase.reduce((sum, ex) => sum + ex.quality, 0) / this.knowledgeBase.length;
      console.log(`Average Quality: ${(avgQuality * 100).toFixed(1)}%`);

      // Group by category
      const byCategory: Record<string, number> = {};
      this.knowledgeBase.forEach(ex => {
        const cat = ex.input.category;
        byCategory[cat] = (byCategory[cat] || 0) + 1;
      });

      console.log(`\nLearned Categories:`);
      Object.entries(byCategory).forEach(([cat, count]) => {
        console.log(`  • ${cat}: ${count} examples`);
      });
    }

    console.log('\n✅ Training complete! System is ready for production.\n');
    console.log('=' .repeat(70) + '\n');
  }

  // Test the trained system
  async test(testCases: any[]): Promise<void> {
    console.log('\n🧪 Testing Trained System\n');
    console.log('=' .repeat(70) + '\n');

    let totalMetrics: EvaluationMetrics = {
      accuracy: 0,
      creativity: 0,
      relevance: 0,
      engagement: 0,
      technicalQuality: 0,
      overall: 0
    };

    for (let i = 0; i < testCases.length; i++) {
      const testCase = testCases[i];
      console.log(`\nTest ${i + 1}/${testCases.length}: ${testCase.product_name}`);
      console.log('─'.repeat(70));

      const output = await this.generate(testCase);
      const metrics = await this.evaluator.evaluate(output, testCase);

      console.log(`\n📝 Generated:`);
      console.log(`   ${output.description}`);
      console.log(`\n   Features:`);
      if (output.key_features) {
        output.key_features.forEach((f: string) => console.log(`     • ${f}`));
      }

      console.log(`\n📊 Metrics:`);
      console.log(`   Overall: ${(metrics.overall * 100).toFixed(1)}%`);
      console.log(`   Accuracy: ${(metrics.accuracy * 100).toFixed(0)}% | Creativity: ${(metrics.creativity * 100).toFixed(0)}%`);
      console.log(`   Relevance: ${(metrics.relevance * 100).toFixed(0)}% | Engagement: ${(metrics.engagement * 100).toFixed(0)}%`);
      console.log(`   Technical: ${(metrics.technicalQuality * 100).toFixed(0)}%`);

      // Aggregate metrics
      Object.keys(totalMetrics).forEach(key => {
        totalMetrics[key as keyof EvaluationMetrics] += metrics[key as keyof EvaluationMetrics];
      });
    }

    // Average metrics
    Object.keys(totalMetrics).forEach(key => {
      totalMetrics[key as keyof EvaluationMetrics] /= testCases.length;
    });

    console.log('\n\n' + '=' .repeat(70));
    console.log('\n📈 TEST SUMMARY\n');
    console.log(`Overall Performance: ${(totalMetrics.overall * 100).toFixed(1)}%`);
    console.log(`\nDetailed Metrics:`);
    console.log(`  Accuracy: ${(totalMetrics.accuracy * 100).toFixed(1)}%`);
    console.log(`  Creativity: ${(totalMetrics.creativity * 100).toFixed(1)}%`);
    console.log(`  Relevance: ${(totalMetrics.relevance * 100).toFixed(1)}%`);
    console.log(`  Engagement: ${(totalMetrics.engagement * 100).toFixed(1)}%`);
    console.log(`  Technical Quality: ${(totalMetrics.technicalQuality * 100).toFixed(1)}%`);
    console.log('\n' + '=' .repeat(70) + '\n');
  }
}

// Main execution
async function runAdvancedLearning() {
  const config: AdvancedLearningConfig = {
    domain: 'ecommerce',
    objectives: [
      'Generate accurate product descriptions',
      'Maintain high creativity and engagement',
      'Ensure category-specific relevance'
    ],
    weights: {
      accuracy: 0.25,
      creativity: 0.20,
      relevance: 0.25,
      engagement: 0.15,
      technical: 0.15
    },
    learningStrategy: 'adaptive',
    convergenceThreshold: 0.85,
    diversityBonus: true,
    transferLearning: true
  };

  const evaluator = new EcommerceEvaluator();
  const system = new AdvancedLearningSystem(config, evaluator);

  // Training examples
  const trainingExamples: TrainingExample[] = [
    {
      input: { product_name: 'Smart Watch', category: 'electronics', price: 299 },
      expectedOutput: {
        description: 'Advanced fitness tracking meets elegant design in this premium smartwatch',
        key_features: ['Heart rate monitoring', '7-day battery', 'Water resistant', 'GPS tracking']
      },
      quality: 0.9,
      metadata: { domain: 'ecommerce', difficulty: 'easy', tags: ['electronics', 'wearable'] }
    },
    {
      input: { product_name: 'Yoga Mat', category: 'fitness', price: 49 },
      expectedOutput: {
        description: 'Professional-grade yoga mat with superior grip and cushioning for all practice levels',
        key_features: ['6mm thickness', 'Non-slip surface', 'Eco-friendly material', 'Easy to clean']
      },
      quality: 0.85,
      metadata: { domain: 'ecommerce', difficulty: 'easy', tags: ['fitness', 'yoga'] }
    },
    {
      input: { product_name: 'Mechanical Keyboard', category: 'electronics', price: 159 },
      expectedOutput: {
        description: 'Tactile perfection for enthusiasts with customizable RGB and premium switches',
        key_features: ['Cherry MX switches', 'RGB backlighting', 'Programmable keys', 'Aluminum frame']
      },
      quality: 0.92,
      metadata: { domain: 'ecommerce', difficulty: 'medium', tags: ['electronics', 'gaming'] }
    }
  ];

  // Train the system
  await system.train(trainingExamples);

  // Test the system
  const testCases = [
    { product_name: 'Wireless Earbuds', category: 'electronics', price: 129 },
    { product_name: 'Resistance Bands Set', category: 'fitness', price: 29 },
    { product_name: 'Laptop Stand', category: 'electronics', price: 59 }
  ];

  await system.test(testCases);
}

// Run the example
if (import.meta.url === `file://${process.argv[1]}`) {
  runAdvancedLearning().catch(error => {
    console.error('❌ Advanced learning failed:', error);
    process.exit(1);
  });
}

export { AdvancedLearningSystem, EcommerceEvaluator, AdvancedLearningConfig };
