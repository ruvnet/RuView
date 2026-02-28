/**
 * BEGINNER TUTORIAL: Simple Data Generation
 *
 * Learn how to generate structured synthetic data with agentic-synth.
 * Perfect for creating test data, mock APIs, or prototyping.
 *
 * What you'll learn:
 * - Defining data schemas
 * - Generating structured data
 * - Saving output to files
 * - Working with different formats
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install @ruvector/agentic-synth
 *
 * Run: npx tsx examples/beginner/simple-data-generation.ts
 */
import { AgenticSynth } from '@ruvector/agentic-synth';
declare const synth: AgenticSynth;
declare function generateUserData(): Promise<void>;
declare function generateWithConstraints(): Promise<void>;
export { generateUserData, generateWithConstraints, synth };
//# sourceMappingURL=simple-data-generation.d.ts.map