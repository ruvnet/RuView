/**
 * BEGINNER TUTORIAL: First DSPy Training
 *
 * This tutorial demonstrates the basics of training a single model using DSPy.ts
 * with agentic-synth for synthetic data generation.
 *
 * What you'll learn:
 * - How to set up a DSPy module
 * - Basic configuration options
 * - Training a model with examples
 * - Evaluating output quality
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/beginner/first-dspy-training.ts
 */
import { ChainOfThought } from 'dspy.ts';
declare class ProductDescriptionGenerator extends ChainOfThought {
    constructor();
}
declare function runTraining(): Promise<void>;
export { runTraining, ProductDescriptionGenerator };
//# sourceMappingURL=first-dspy-training.d.ts.map