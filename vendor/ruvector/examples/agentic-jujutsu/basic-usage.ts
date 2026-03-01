/**
 * Agentic-Jujutsu Basic Usage Example
 *
 * Demonstrates fundamental operations:
 * - Repository initialization
 * - Creating commits
 * - Branch management
 * - Basic version control workflows
 */

// Note: This is a reference implementation for testing purposes
// Actual implementation would use: import { JjWrapper } from 'agentic-jujutsu';

interface JjWrapper {
  status(): Promise<JjResult>;
  newCommit(message: string): Promise<JjResult>;
  log(limit: number): Promise<JjCommit[]>;
  branchCreate(name: string, rev?: string): Promise<JjResult>;
  diff(from: string, to: string): Promise<JjDiff>;
}

interface JjResult {
  success: boolean;
  stdout: string;
  stderr: string;
}

interface JjCommit {
  id: string;
  message: string;
  author: string;
  timestamp: string;
}

interface JjDiff {
  changes: string;
  filesModified: number;
}

async function basicUsageExample() {
  console.log('=== Agentic-Jujutsu Basic Usage ===\n');

  // In actual usage:
  // const { JjWrapper } = require('agentic-jujutsu');
  // const jj = new JjWrapper();

  console.log('1. Check repository status');
  console.log('   const result = await jj.status();');
  console.log('   Output: Working directory status\n');

  console.log('2. Create a new commit');
  console.log('   const commit = await jj.newCommit("Add new feature");');
  console.log('   Output: Created commit with message\n');

  console.log('3. View commit history');
  console.log('   const log = await jj.log(10);');
  console.log('   Output: Last 10 commits\n');

  console.log('4. Create a branch');
  console.log('   await jj.branchCreate("feature/new-feature");');
  console.log('   Output: Created new branch\n');

  console.log('5. View differences');
  console.log('   const diff = await jj.diff("@", "@-");');
  console.log('   Output: Changes between current and previous commit\n');
}

if (require.main === module) {
  basicUsageExample().catch(console.error);
}

export { basicUsageExample };
