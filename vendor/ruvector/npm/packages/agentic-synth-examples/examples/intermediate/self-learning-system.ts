/**
 * INTERMEDIATE TUTORIAL: Self-Learning System
 *
 * Build an adaptive AI system that improves its output quality over time
 * through feedback loops and pattern recognition. This demonstrates how
 * to create systems that learn from their mistakes and successes.
 *
 * What you'll learn:
 * - Building feedback loops
 * - Tracking quality improvements
 * - Adaptive prompt engineering
 * - Learning from examples
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/intermediate/self-learning-system.ts
 */

import { LM, ChainOfThought, Prediction } from 'dspy.ts';

// Learning session configuration
interface LearningConfig {
  targetQualityThreshold: number; // Stop when this quality is reached
  maxIterations: number; // Maximum learning iterations
  improvementRate: number; // How aggressively to adjust (0.1 = 10% per iteration)
  minImprovement: number; // Minimum improvement to continue
}

// Feedback from each iteration
interface Feedback {
  quality: number;
  strengths: string[];
  weaknesses: string[];
  suggestions: string[];
}

// Learning history entry
interface LearningEntry {
  iteration: number;
  quality: number;
  output: Prediction;
  feedback: Feedback;
  promptModifications: string[];
  timestamp: Date;
}

// Self-learning generator class
class SelfLearningGenerator {
  private lm: LM;
  private history: LearningEntry[] = [];
  private config: LearningConfig;
  private basePrompt: string;
  private currentPromptAdditions: string[] = [];

  constructor(config: Partial<LearningConfig> = {}) {
    this.config = {
      targetQualityThreshold: config.targetQualityThreshold || 0.9,
      maxIterations: config.maxIterations || 10,
      improvementRate: config.improvementRate || 0.15,
      minImprovement: config.minImprovement || 0.02
    };

    this.lm = new LM({
      provider: 'google-genai',
      model: 'gemini-2.0-flash-exp',
      apiKey: process.env.GEMINI_API_KEY || '',
      temperature: 0.8 // Higher temperature for creativity during learning
    });

    this.basePrompt = '';
  }

  // Evaluate the quality of generated output
  private evaluateOutput(prediction: Prediction, criteria: any): Feedback {
    let quality = 0;
    const strengths: string[] = [];
    const weaknesses: string[] = [];
    const suggestions: string[] = [];

    // Check description quality
    if (prediction.description) {
      const desc = prediction.description;
      const length = desc.length;

      if (length >= 100 && length <= 200) {
        quality += 0.3;
        strengths.push('Description length is optimal');
      } else if (length < 50) {
        weaknesses.push('Description too short');
        suggestions.push('Expand description with more details');
      } else if (length > 250) {
        weaknesses.push('Description too verbose');
        suggestions.push('Make description more concise');
      } else {
        quality += 0.15;
      }

      // Check for emotional/engaging language
      const emotionalWords = ['amazing', 'powerful', 'innovative', 'premium', 'exceptional'];
      const hasEmotionalLanguage = emotionalWords.some(word =>
        desc.toLowerCase().includes(word)
      );

      if (hasEmotionalLanguage) {
        quality += 0.2;
        strengths.push('Uses engaging language');
      } else {
        weaknesses.push('Could be more engaging');
        suggestions.push('Add more descriptive and emotional words');
      }
    } else {
      weaknesses.push('Missing description');
      suggestions.push('Generate a complete description');
    }

    // Check features
    if (prediction.key_features && Array.isArray(prediction.key_features)) {
      const features = prediction.key_features;

      if (features.length >= 4 && features.length <= 6) {
        quality += 0.3;
        strengths.push('Optimal number of features');
      } else if (features.length < 3) {
        weaknesses.push('Too few features');
        suggestions.push('Include at least 4 key features');
      } else {
        quality += 0.15;
      }

      // Check feature quality (should be concise)
      const wellFormedFeatures = features.filter(f =>
        f.length >= 10 && f.length <= 50
      );

      if (wellFormedFeatures.length === features.length) {
        quality += 0.2;
        strengths.push('All features are well-formed');
      } else {
        weaknesses.push('Some features need better formatting');
        suggestions.push('Keep features concise (10-50 chars)');
      }
    } else {
      weaknesses.push('Missing features');
      suggestions.push('Generate key features list');
    }

    return { quality, strengths, weaknesses, suggestions };
  }

  // Adapt prompt based on feedback
  private adaptPrompt(feedback: Feedback): string[] {
    const modifications: string[] = [];

    // Add specific instructions based on weaknesses
    feedback.suggestions.forEach(suggestion => {
      if (suggestion.includes('short')) {
        modifications.push('Write detailed descriptions (100-200 characters)');
      } else if (suggestion.includes('verbose')) {
        modifications.push('Keep descriptions concise and focused');
      } else if (suggestion.includes('engaging')) {
        modifications.push('Use descriptive, engaging language');
      } else if (suggestion.includes('features')) {
        modifications.push('Include 4-6 specific, measurable key features');
      } else if (suggestion.includes('concise')) {
        modifications.push('Format features as short, punchy statements');
      }
    });

    // Remove duplicates
    return [...new Set(modifications)];
  }

  // Generate with current prompt
  private async generate(input: any): Promise<Prediction> {
    // Build enhanced signature with learned improvements
    const enhancedInstructions = this.currentPromptAdditions.length > 0
      ? '\n\nImportant guidelines:\n' + this.currentPromptAdditions.map((s, i) => `${i + 1}. ${s}`).join('\n')
      : '';

    const signature = {
      input: 'product_name: string, category: string, price: number',
      output: 'description: string, key_features: string[]',
      description: 'Generate compelling product descriptions' + enhancedInstructions
    };

    const generator = new ChainOfThought(signature, { lm: this.lm });
    return await generator.forward(input);
  }

  // Main learning loop
  async learn(input: any, criteria: any = {}): Promise<void> {
    console.log('🧠 Starting Self-Learning Session\n');
    console.log('=' .repeat(70));
    console.log(`\nTarget Quality: ${(this.config.targetQualityThreshold * 100).toFixed(0)}%`);
    console.log(`Max Iterations: ${this.config.maxIterations}`);
    console.log(`Input: ${JSON.stringify(input, null, 2)}\n`);
    console.log('=' .repeat(70) + '\n');

    let iteration = 0;
    let previousQuality = 0;

    while (iteration < this.config.maxIterations) {
      iteration++;
      console.log(`\n📊 Iteration ${iteration}/${this.config.maxIterations}`);
      console.log('─'.repeat(70));

      // Generate output
      const startTime = Date.now();
      const output = await this.generate(input);
      const duration = Date.now() - startTime;

      // Evaluate
      const feedback = this.evaluateOutput(output, criteria);

      // Store in history
      this.history.push({
        iteration,
        quality: feedback.quality,
        output,
        feedback,
        promptModifications: [...this.currentPromptAdditions],
        timestamp: new Date()
      });

      // Display results
      console.log(`\n⏱️  Generation time: ${duration}ms`);
      console.log(`\n📝 Output:`);
      console.log(`   Description: ${output.description || 'N/A'}`);
      if (output.key_features) {
        console.log(`   Features:`);
        output.key_features.forEach((f: string) => console.log(`     • ${f}`));
      }

      console.log(`\n📈 Quality: ${(feedback.quality * 100).toFixed(1)}%`);

      if (feedback.strengths.length > 0) {
        console.log(`\n✅ Strengths:`);
        feedback.strengths.forEach(s => console.log(`   • ${s}`));
      }

      if (feedback.weaknesses.length > 0) {
        console.log(`\n⚠️  Weaknesses:`);
        feedback.weaknesses.forEach(w => console.log(`   • ${w}`));
      }

      // Check if target reached
      if (feedback.quality >= this.config.targetQualityThreshold) {
        console.log(`\n🎯 Target quality reached!`);
        break;
      }

      // Check for improvement
      const improvement = feedback.quality - previousQuality;
      if (iteration > 1 && improvement < this.config.minImprovement) {
        console.log(`\n⚠️  Improvement too small (${(improvement * 100).toFixed(1)}%), stopping...`);
        break;
      }

      // Adapt for next iteration
      const modifications = this.adaptPrompt(feedback);
      if (modifications.length > 0) {
        console.log(`\n🔧 Adapting strategy:`);
        modifications.forEach(m => console.log(`   • ${m}`));

        // Add new modifications
        modifications.forEach(m => {
          if (!this.currentPromptAdditions.includes(m)) {
            this.currentPromptAdditions.push(m);
          }
        });
      }

      previousQuality = feedback.quality;

      // Brief pause between iterations
      await new Promise(resolve => setTimeout(resolve, 1000));
    }

    // Final summary
    this.displaySummary();
  }

  // Display learning summary
  private displaySummary(): void {
    console.log('\n\n' + '=' .repeat(70));
    console.log('\n🎓 LEARNING SUMMARY\n');

    if (this.history.length === 0) {
      console.log('No learning history available.\n');
      return;
    }

    const firstQuality = this.history[0].quality;
    const lastQuality = this.history[this.history.length - 1].quality;
    const improvement = lastQuality - firstQuality;
    const improvementPercent = (improvement / firstQuality) * 100;

    console.log(`Total Iterations: ${this.history.length}`);
    console.log(`Starting Quality: ${(firstQuality * 100).toFixed(1)}%`);
    console.log(`Final Quality: ${(lastQuality * 100).toFixed(1)}%`);
    console.log(`Improvement: ${improvement >= 0 ? '+' : ''}${(improvement * 100).toFixed(1)}% (${improvementPercent >= 0 ? '+' : ''}${improvementPercent.toFixed(1)}%)`);

    console.log(`\n📊 Quality Progression:`);
    this.history.forEach(entry => {
      const bar = '█'.repeat(Math.floor(entry.quality * 50));
      const percent = (entry.quality * 100).toFixed(1);
      console.log(`   Iteration ${entry.iteration}: ${bar} ${percent}%`);
    });

    console.log(`\n🔧 Learned Improvements (${this.currentPromptAdditions.length}):`);
    this.currentPromptAdditions.forEach((mod, i) => {
      console.log(`   ${i + 1}. ${mod}`);
    });

    console.log('\n💡 Key Insights:');
    if (improvement > 0) {
      console.log(`   ✓ System successfully learned and improved`);
      console.log(`   ✓ Quality increased by ${(improvement * 100).toFixed(1)}%`);
    }
    console.log(`   ✓ Discovered ${this.currentPromptAdditions.length} optimization strategies`);
    console.log(`   ✓ These improvements can be applied to future generations\n`);

    console.log('=' .repeat(70) + '\n');
  }

  // Get the learned prompt modifications
  getLearnedImprovements(): string[] {
    return [...this.currentPromptAdditions];
  }

  // Get learning history
  getHistory(): LearningEntry[] {
    return [...this.history];
  }
}

// Main execution
async function runSelfLearning() {
  const generator = new SelfLearningGenerator({
    targetQualityThreshold: 0.85,
    maxIterations: 8,
    improvementRate: 0.15,
    minImprovement: 0.03
  });

  const testProduct = {
    product_name: 'Professional DSLR Camera',
    category: 'Photography',
    price: 1299
  };

  await generator.learn(testProduct);

  // Save learned improvements
  const improvements = generator.getLearnedImprovements();
  console.log('📝 Learned improvements can be reused:\n');
  console.log(JSON.stringify(improvements, null, 2) + '\n');
}

// Run the example
if (import.meta.url === `file://${process.argv[1]}`) {
  runSelfLearning().catch(error => {
    console.error('❌ Learning failed:', error);
    process.exit(1);
  });
}

export { SelfLearningGenerator, LearningConfig, LearningEntry };
