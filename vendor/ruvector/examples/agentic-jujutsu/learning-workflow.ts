/**
 * Agentic-Jujutsu Learning Workflow Example
 *
 * Demonstrates ReasoningBank self-learning capabilities:
 * - Trajectory tracking
 * - Pattern discovery
 * - AI-powered suggestions
 * - Continuous improvement
 */

interface JjWrapper {
  startTrajectory(task: string): string;
  addToTrajectory(): void;
  finalizeTrajectory(score: number, critique?: string): void;
  getSuggestion(task: string): string;
  getLearningStats(): string;
  getPatterns(): string;
  newCommit(message: string): Promise<any>;
  branchCreate(name: string): Promise<any>;
}

async function learningWorkflowExample() {
  console.log('=== Agentic-Jujutsu Learning Workflow ===\n');

  // In actual usage:
  // const { JjWrapper } = require('agentic-jujutsu');
  // const jj = new JjWrapper();

  console.log('1. Start a learning trajectory');
  console.log('   const trajectoryId = jj.startTrajectory("Implement authentication");');
  console.log('   Output: Unique trajectory ID\n');

  console.log('2. Perform operations (automatically tracked)');
  console.log('   await jj.branchCreate("feature/auth");');
  console.log('   await jj.newCommit("Add auth endpoints");');
  console.log('   await jj.newCommit("Add tests");\n');

  console.log('3. Record operations to trajectory');
  console.log('   jj.addToTrajectory();\n');

  console.log('4. Finalize with success score and critique');
  console.log('   jj.finalizeTrajectory(0.9, "Clean implementation, good test coverage");\n');

  console.log('5. Later: Get AI-powered suggestions');
  console.log('   const suggestion = JSON.parse(jj.getSuggestion("Implement logout"));');
  console.log('   console.log("Confidence:", suggestion.confidence);');
  console.log('   console.log("Expected success:", suggestion.expectedSuccessRate);');
  console.log('   console.log("Recommended steps:", suggestion.recommendedOperations);\n');

  console.log('6. View learning statistics');
  console.log('   const stats = JSON.parse(jj.getLearningStats());');
  console.log('   console.log("Total trajectories:", stats.totalTrajectories);');
  console.log('   console.log("Patterns discovered:", stats.totalPatterns);');
  console.log('   console.log("Average success:", stats.avgSuccessRate);');
  console.log('   console.log("Improvement rate:", stats.improvementRate);\n');

  console.log('7. Discover patterns');
  console.log('   const patterns = JSON.parse(jj.getPatterns());');
  console.log('   patterns.forEach(p => {');
  console.log('     console.log("Pattern:", p.name);');
  console.log('     console.log("Success rate:", p.successRate);');
  console.log('     console.log("Operations:", p.operationSequence);');
  console.log('   });\n');
}

if (require.main === module) {
  learningWorkflowExample().catch(console.error);
}

export { learningWorkflowExample };
