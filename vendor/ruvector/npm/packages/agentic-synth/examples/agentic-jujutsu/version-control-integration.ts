/**
 * Version Control Integration Example
 *
 * Demonstrates how to use agentic-jujutsu for version controlling
 * synthetic data generation, tracking changes, branching strategies,
 * and rolling back to previous versions.
 */

import { AgenticSynth } from '../../src/core/synth';
import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

interface DataGenerationMetadata {
  version: string;
  timestamp: string;
  schemaHash: string;
  recordCount: number;
  generator: string;
  quality: number;
}

interface JujutsuCommit {
  hash: string;
  message: string;
  metadata: DataGenerationMetadata;
  timestamp: Date;
}

class VersionControlledDataGenerator {
  private synth: AgenticSynth;
  private repoPath: string;
  private dataPath: string;

  constructor(repoPath: string) {
    this.synth = new AgenticSynth();
    this.repoPath = repoPath;
    this.dataPath = path.join(repoPath, 'data');
  }

  /**
   * Initialize jujutsu repository for data versioning
   */
  async initializeRepository(): Promise<void> {
    try {
      // Initialize jujutsu repo
      console.log('🔧 Initializing jujutsu repository...');
      execSync('npx agentic-jujutsu@latest init', {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      // Create data directory
      if (!fs.existsSync(this.dataPath)) {
        fs.mkdirSync(this.dataPath, { recursive: true });
      }

      // Create .gitignore to ignore node_modules but track data
      const gitignore = `node_modules/
*.log
.env
!data/
`;
      fs.writeFileSync(path.join(this.repoPath, '.gitignore'), gitignore);

      console.log('✅ Repository initialized successfully');
    } catch (error) {
      throw new Error(`Failed to initialize repository: ${(error as Error).message}`);
    }
  }

  /**
   * Generate synthetic data and commit with metadata
   */
  async generateAndCommit(
    schema: any,
    count: number,
    message: string
  ): Promise<JujutsuCommit> {
    try {
      console.log(`🎲 Generating ${count} records...`);

      // Generate synthetic data
      const data = await this.synth.generate(schema, { count });

      // Calculate metadata
      const metadata: DataGenerationMetadata = {
        version: '1.0.0',
        timestamp: new Date().toISOString(),
        schemaHash: this.hashSchema(schema),
        recordCount: count,
        generator: 'agentic-synth',
        quality: this.calculateQuality(data)
      };

      // Save data and metadata
      const timestamp = Date.now();
      const dataFile = path.join(this.dataPath, `dataset_${timestamp}.json`);
      const metaFile = path.join(this.dataPath, `dataset_${timestamp}.meta.json`);

      fs.writeFileSync(dataFile, JSON.stringify(data, null, 2));
      fs.writeFileSync(metaFile, JSON.stringify(metadata, null, 2));

      console.log(`💾 Saved to ${dataFile}`);

      // Add files to jujutsu
      execSync(`npx agentic-jujutsu@latest add "${dataFile}"`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });
      execSync(`npx agentic-jujutsu@latest add "${metaFile}"`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      // Commit with metadata
      const commitMessage = `${message}\n\nMetadata:\n${JSON.stringify(metadata, null, 2)}`;
      const result = execSync(
        `npx agentic-jujutsu@latest commit -m "${commitMessage}"`,
        { cwd: this.repoPath, encoding: 'utf-8' }
      );

      // Get commit hash
      const hash = this.getLatestCommitHash();

      console.log(`✅ Committed: ${hash.substring(0, 8)}`);

      return {
        hash,
        message,
        metadata,
        timestamp: new Date()
      };
    } catch (error) {
      throw new Error(`Failed to generate and commit: ${(error as Error).message}`);
    }
  }

  /**
   * Create a branch for experimenting with different generation strategies
   */
  async createGenerationBranch(branchName: string, description: string): Promise<void> {
    try {
      console.log(`🌿 Creating branch: ${branchName}`);

      execSync(`npx agentic-jujutsu@latest branch create ${branchName}`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      // Save branch description
      const branchesDir = path.join(this.repoPath, '.jj', 'branches');
      if (!fs.existsSync(branchesDir)) {
        fs.mkdirSync(branchesDir, { recursive: true });
      }

      const descFile = path.join(branchesDir, `${branchName}.desc`);
      fs.writeFileSync(descFile, description);

      console.log(`✅ Branch ${branchName} created`);
    } catch (error) {
      throw new Error(`Failed to create branch: ${(error as Error).message}`);
    }
  }

  /**
   * Compare datasets between two commits or branches
   */
  async compareDatasets(ref1: string, ref2: string): Promise<any> {
    try {
      console.log(`📊 Comparing ${ref1} vs ${ref2}...`);

      // Get file lists at each ref
      const files1 = this.getDataFilesAtRef(ref1);
      const files2 = this.getDataFilesAtRef(ref2);

      const comparison = {
        ref1,
        ref2,
        filesAdded: files2.filter(f => !files1.includes(f)),
        filesRemoved: files1.filter(f => !files2.includes(f)),
        filesModified: [] as string[],
        statistics: {} as any
      };

      // Compare common files
      const commonFiles = files1.filter(f => files2.includes(f));
      for (const file of commonFiles) {
        const diff = execSync(
          `npx agentic-jujutsu@latest diff ${ref1} ${ref2} -- "${file}"`,
          { cwd: this.repoPath, encoding: 'utf-8' }
        );

        if (diff.trim()) {
          comparison.filesModified.push(file);
        }
      }

      console.log(`✅ Comparison complete:`);
      console.log(`   Added: ${comparison.filesAdded.length}`);
      console.log(`   Removed: ${comparison.filesRemoved.length}`);
      console.log(`   Modified: ${comparison.filesModified.length}`);

      return comparison;
    } catch (error) {
      throw new Error(`Failed to compare datasets: ${(error as Error).message}`);
    }
  }

  /**
   * Merge data generation branches
   */
  async mergeBranches(sourceBranch: string, targetBranch: string): Promise<void> {
    try {
      console.log(`🔀 Merging ${sourceBranch} into ${targetBranch}...`);

      // Switch to target branch
      execSync(`npx agentic-jujutsu@latest checkout ${targetBranch}`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      // Merge source branch
      execSync(`npx agentic-jujutsu@latest merge ${sourceBranch}`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      console.log(`✅ Merge complete`);
    } catch (error) {
      throw new Error(`Failed to merge branches: ${(error as Error).message}`);
    }
  }

  /**
   * Rollback to a previous data version
   */
  async rollbackToVersion(commitHash: string): Promise<void> {
    try {
      console.log(`⏮️  Rolling back to ${commitHash.substring(0, 8)}...`);

      // Create a new branch from the target commit
      const rollbackBranch = `rollback_${Date.now()}`;
      execSync(
        `npx agentic-jujutsu@latest branch create ${rollbackBranch} -r ${commitHash}`,
        { cwd: this.repoPath, stdio: 'inherit' }
      );

      // Checkout the rollback branch
      execSync(`npx agentic-jujutsu@latest checkout ${rollbackBranch}`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      console.log(`✅ Rolled back to ${commitHash.substring(0, 8)}`);
      console.log(`   New branch: ${rollbackBranch}`);
    } catch (error) {
      throw new Error(`Failed to rollback: ${(error as Error).message}`);
    }
  }

  /**
   * Get data generation history
   */
  async getHistory(limit: number = 10): Promise<any[]> {
    try {
      const log = execSync(
        `npx agentic-jujutsu@latest log --limit ${limit} --no-graph`,
        { cwd: this.repoPath, encoding: 'utf-8' }
      );

      // Parse log output
      const commits = this.parseLogOutput(log);

      console.log(`📜 Retrieved ${commits.length} commits`);
      return commits;
    } catch (error) {
      throw new Error(`Failed to get history: ${(error as Error).message}`);
    }
  }

  /**
   * Tag a specific data generation
   */
  async tagVersion(tag: string, message: string): Promise<void> {
    try {
      console.log(`🏷️  Creating tag: ${tag}`);

      execSync(`npx agentic-jujutsu@latest tag ${tag} -m "${message}"`, {
        cwd: this.repoPath,
        stdio: 'inherit'
      });

      console.log(`✅ Tag created: ${tag}`);
    } catch (error) {
      throw new Error(`Failed to create tag: ${(error as Error).message}`);
    }
  }

  // Helper methods

  private hashSchema(schema: any): string {
    const crypto = require('crypto');
    return crypto
      .createHash('sha256')
      .update(JSON.stringify(schema))
      .digest('hex')
      .substring(0, 16);
  }

  private calculateQuality(data: any[]): number {
    // Simple quality metric: completeness of data
    if (!data.length) return 0;

    let totalFields = 0;
    let completeFields = 0;

    data.forEach(record => {
      const fields = Object.keys(record);
      totalFields += fields.length;
      fields.forEach(field => {
        if (record[field] !== null && record[field] !== undefined && record[field] !== '') {
          completeFields++;
        }
      });
    });

    return totalFields > 0 ? completeFields / totalFields : 0;
  }

  private getLatestCommitHash(): string {
    const result = execSync(
      'npx agentic-jujutsu@latest log --limit 1 --no-graph --template "{commit_id}"',
      { cwd: this.repoPath, encoding: 'utf-8' }
    );
    return result.trim();
  }

  private getDataFilesAtRef(ref: string): string[] {
    try {
      const result = execSync(
        `npx agentic-jujutsu@latest files --revision ${ref}`,
        { cwd: this.repoPath, encoding: 'utf-8' }
      );
      return result
        .split('\n')
        .filter(line => line.includes('data/dataset_'))
        .map(line => line.trim());
    } catch (error) {
      return [];
    }
  }

  private parseLogOutput(log: string): any[] {
    // Simple log parser - in production, use structured output
    const commits: any[] = [];
    const lines = log.split('\n');

    let currentCommit: any = null;
    for (const line of lines) {
      if (line.startsWith('commit ')) {
        if (currentCommit) commits.push(currentCommit);
        currentCommit = {
          hash: line.split(' ')[1],
          message: '',
          timestamp: new Date()
        };
      } else if (currentCommit && line.trim()) {
        currentCommit.message += line.trim() + ' ';
      }
    }
    if (currentCommit) commits.push(currentCommit);

    return commits;
  }
}

// Example usage
async function main() {
  console.log('🚀 Agentic-Jujutsu Version Control Integration Example\n');

  const repoPath = path.join(process.cwd(), 'synthetic-data-repo');
  const generator = new VersionControlledDataGenerator(repoPath);

  try {
    // Initialize repository
    await generator.initializeRepository();

    // Define schema for user data
    const userSchema = {
      name: 'string',
      email: 'email',
      age: 'number',
      city: 'string',
      active: 'boolean'
    };

    // Generate initial dataset
    const commit1 = await generator.generateAndCommit(
      userSchema,
      1000,
      'Initial user dataset generation'
    );
    console.log(`📝 First commit: ${commit1.hash.substring(0, 8)}\n`);

    // Tag the baseline
    await generator.tagVersion('v1.0-baseline', 'Production baseline dataset');

    // Create experimental branch
    await generator.createGenerationBranch(
      'experiment-large-dataset',
      'Testing larger dataset generation'
    );

    // Generate more data on experimental branch
    const commit2 = await generator.generateAndCommit(
      userSchema,
      5000,
      'Large dataset experiment'
    );
    console.log(`📝 Second commit: ${commit2.hash.substring(0, 8)}\n`);

    // Compare datasets
    const comparison = await generator.compareDatasets(
      commit1.hash,
      commit2.hash
    );
    console.log('\n📊 Comparison result:', JSON.stringify(comparison, null, 2));

    // Merge if experiment was successful
    await generator.mergeBranches('experiment-large-dataset', 'main');

    // Get history
    const history = await generator.getHistory(5);
    console.log('\n📜 Recent history:', history);

    // Demonstrate rollback
    console.log('\n⏮️  Demonstrating rollback...');
    await generator.rollbackToVersion(commit1.hash);

    console.log('\n✅ Example completed successfully!');
  } catch (error) {
    console.error('❌ Error:', (error as Error).message);
    process.exit(1);
  }
}

// Run example if executed directly
if (require.main === module) {
  main().catch(console.error);
}

export { VersionControlledDataGenerator, DataGenerationMetadata, JujutsuCommit };
