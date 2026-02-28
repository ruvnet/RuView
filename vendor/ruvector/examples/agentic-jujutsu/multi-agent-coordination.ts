/**
 * Agentic-Jujutsu Multi-Agent Coordination Example
 *
 * Demonstrates how multiple AI agents can work simultaneously:
 * - Concurrent commits without locks
 * - Shared learning across agents
 * - Collaborative workflows
 * - Conflict-free coordination
 */

interface JjWrapper {
  startTrajectory(task: string): string;
  addToTrajectory(): void;
  finalizeTrajectory(score: number, critique?: string): void;
  getSuggestion(task: string): string;
  newCommit(message: string): Promise<any>;
  branchCreate(name: string): Promise<any>;
  diff(from: string, to: string): Promise<any>;
}

async function multiAgentCoordinationExample() {
  console.log('=== Agentic-Jujutsu Multi-Agent Coordination ===\n');

  console.log('Scenario: Three AI agents working on different features simultaneously\n');

  console.log('=== Agent 1: Backend Developer ===');
  console.log('const backend = new JjWrapper();');
  console.log('backend.startTrajectory("Implement REST API");');
  console.log('await backend.branchCreate("feature/api");');
  console.log('await backend.newCommit("Add API endpoints");');
  console.log('backend.addToTrajectory();');
  console.log('backend.finalizeTrajectory(0.9, "API complete");\n');

  console.log('=== Agent 2: Frontend Developer (running concurrently) ===');
  console.log('const frontend = new JjWrapper();');
  console.log('frontend.startTrajectory("Build UI components");');
  console.log('await frontend.branchCreate("feature/ui");');
  console.log('await frontend.newCommit("Add React components");');
  console.log('frontend.addToTrajectory();');
  console.log('frontend.finalizeTrajectory(0.85, "UI components ready");\n');

  console.log('=== Agent 3: Tester (benefits from both agents) ===');
  console.log('const tester = new JjWrapper();');
  console.log('// Get AI suggestions based on previous agents\' work');
  console.log('const suggestion = JSON.parse(tester.getSuggestion("Test API and UI"));');
  console.log('console.log("AI Recommendation:", suggestion.reasoning);');
  console.log('console.log("Confidence:", suggestion.confidence);\n');

  console.log('tester.startTrajectory("Create test suite");');
  console.log('await tester.branchCreate("feature/tests");');
  console.log('await tester.newCommit("Add integration tests");');
  console.log('tester.addToTrajectory();');
  console.log('tester.finalizeTrajectory(0.95, "Comprehensive test coverage");\n');

  console.log('=== Key Benefits ===');
  console.log('✓ No locks or waiting - 23x faster than Git');
  console.log('✓ All agents learn from each other\'s experience');
  console.log('✓ Automatic conflict resolution (87% success rate)');
  console.log('✓ Shared pattern discovery across agents');
  console.log('✓ Context switching <100ms (10x faster than Git)\n');

  console.log('=== Coordinated Code Review ===');
  console.log('async function coordinatedReview(agents) {');
  console.log('  const reviews = await Promise.all(agents.map(async (agent) => {');
  console.log('    const jj = new JjWrapper();');
  console.log('    jj.startTrajectory(`Review by ${agent.name}`);');
  console.log('    ');
  console.log('    const diff = await jj.diff("@", "@-");');
  console.log('    const issues = await agent.analyze(diff);');
  console.log('    ');
  console.log('    jj.addToTrajectory();');
  console.log('    jj.finalizeTrajectory(');
  console.log('      issues.length === 0 ? 0.9 : 0.6,');
  console.log('      `Found ${issues.length} issues`');
  console.log('    );');
  console.log('    ');
  console.log('    return { agent: agent.name, issues };');
  console.log('  }));');
  console.log('  ');
  console.log('  return reviews;');
  console.log('}\n');
}

if (require.main === module) {
  multiAgentCoordinationExample().catch(console.error);
}

export { multiAgentCoordinationExample };
