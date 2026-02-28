/**
 * Diff Embeddings - Semantic encoding of git diffs
 *
 * Generates embeddings for code changes to enable:
 * - Change classification (feature, bugfix, refactor)
 * - Similar change detection
 * - Risk assessment
 * - Review prioritization
 */

import { execSync } from 'child_process';
import { embed, embedBatch, isReady, initOnnxEmbedder } from './onnx-embedder';

export interface DiffHunk {
  file: string;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  content: string;
  additions: string[];
  deletions: string[];
}

export interface DiffAnalysis {
  file: string;
  hunks: DiffHunk[];
  totalAdditions: number;
  totalDeletions: number;
  complexity: number;
  riskScore: number;
  category: 'feature' | 'bugfix' | 'refactor' | 'docs' | 'test' | 'config' | 'unknown';
  embedding?: number[];
}

export interface CommitAnalysis {
  hash: string;
  message: string;
  author: string;
  date: string;
  files: DiffAnalysis[];
  totalAdditions: number;
  totalDeletions: number;
  riskScore: number;
  embedding?: number[];
}

/**
 * Parse a unified diff into hunks
 */
export function parseDiff(diff: string): DiffHunk[] {
  const hunks: DiffHunk[] = [];
  const lines = diff.split('\n');

  let currentFile = '';
  let currentHunk: DiffHunk | null = null;

  for (const line of lines) {
    // File header
    if (line.startsWith('diff --git')) {
      const match = line.match(/diff --git a\/(.+) b\/(.+)/);
      if (match) {
        currentFile = match[2];
      }
    }

    // Hunk header
    if (line.startsWith('@@')) {
      if (currentHunk) {
        hunks.push(currentHunk);
      }

      const match = line.match(/@@ -(\d+),?(\d*) \+(\d+),?(\d*) @@/);
      if (match) {
        currentHunk = {
          file: currentFile,
          oldStart: parseInt(match[1]),
          oldLines: parseInt(match[2] || '1'),
          newStart: parseInt(match[3]),
          newLines: parseInt(match[4] || '1'),
          content: '',
          additions: [],
          deletions: [],
        };
      }
    } else if (currentHunk) {
      // Content lines
      if (line.startsWith('+') && !line.startsWith('+++')) {
        currentHunk.additions.push(line.substring(1));
        currentHunk.content += line + '\n';
      } else if (line.startsWith('-') && !line.startsWith('---')) {
        currentHunk.deletions.push(line.substring(1));
        currentHunk.content += line + '\n';
      } else if (line.startsWith(' ')) {
        currentHunk.content += line + '\n';
      }
    }
  }

  if (currentHunk) {
    hunks.push(currentHunk);
  }

  return hunks;
}

/**
 * Classify a change based on patterns
 */
export function classifyChange(diff: string, message: string = ''): 'feature' | 'bugfix' | 'refactor' | 'docs' | 'test' | 'config' | 'unknown' {
  const lowerMessage = message.toLowerCase();
  const lowerDiff = diff.toLowerCase();

  // Check message patterns
  if (/\b(fix|bug|issue|error|crash|patch)\b/.test(lowerMessage)) return 'bugfix';
  if (/\b(feat|feature|add|new|implement)\b/.test(lowerMessage)) return 'feature';
  if (/\b(refactor|clean|improve|optimize)\b/.test(lowerMessage)) return 'refactor';
  if (/\b(doc|readme|comment|jsdoc)\b/.test(lowerMessage)) return 'docs';
  if (/\b(test|spec|coverage)\b/.test(lowerMessage)) return 'test';
  if (/\b(config|ci|cd|build|deps)\b/.test(lowerMessage)) return 'config';

  // Check diff patterns
  if (/\.(md|txt|rst)$/.test(diff)) return 'docs';
  if (/\.(test|spec)\.[jt]sx?/.test(diff)) return 'test';
  if (/\.(json|ya?ml|toml|ini)$/.test(diff)) return 'config';

  // Check content patterns
  if (/\bcatch\b|\btry\b|\berror\b/.test(lowerDiff) && /\bfix\b/.test(lowerDiff)) return 'bugfix';
  if (/\bfunction\b|\bclass\b|\bexport\b/.test(lowerDiff)) return 'feature';

  return 'unknown';
}

/**
 * Calculate risk score for a diff
 */
export function calculateRiskScore(analysis: DiffAnalysis): number {
  let risk = 0;

  // Size risk
  const totalChanges = analysis.totalAdditions + analysis.totalDeletions;
  if (totalChanges > 500) risk += 0.3;
  else if (totalChanges > 200) risk += 0.2;
  else if (totalChanges > 50) risk += 0.1;

  // Complexity risk
  if (analysis.complexity > 20) risk += 0.2;
  else if (analysis.complexity > 10) risk += 0.1;

  // File type risk
  if (analysis.file.includes('auth') || analysis.file.includes('security')) risk += 0.2;
  if (analysis.file.includes('database') || analysis.file.includes('migration')) risk += 0.15;
  if (analysis.file.includes('api') || analysis.file.includes('endpoint')) risk += 0.1;

  // Pattern risk (deletions of error handling, etc.)
  for (const hunk of analysis.hunks) {
    for (const del of hunk.deletions) {
      if (/\bcatch\b|\berror\b|\bvalidat/.test(del)) risk += 0.1;
      if (/\bif\b.*\bnull\b|\bundefined\b/.test(del)) risk += 0.05;
    }
  }

  return Math.min(1, risk);
}

/**
 * Analyze a single file diff
 */
export async function analyzeFileDiff(file: string, diff: string, message: string = ''): Promise<DiffAnalysis> {
  const hunks = parseDiff(diff).filter(h => h.file === file || h.file === '');

  const totalAdditions = hunks.reduce((sum, h) => sum + h.additions.length, 0);
  const totalDeletions = hunks.reduce((sum, h) => sum + h.deletions.length, 0);

  // Calculate complexity (branch keywords in additions)
  let complexity = 0;
  for (const hunk of hunks) {
    for (const add of hunk.additions) {
      if (/\bif\b|\belse\b|\bfor\b|\bwhile\b|\bswitch\b|\bcatch\b|\?/.test(add)) {
        complexity++;
      }
    }
  }

  const category = classifyChange(diff, message);

  const analysis: DiffAnalysis = {
    file,
    hunks,
    totalAdditions,
    totalDeletions,
    complexity,
    riskScore: 0,
    category,
  };

  analysis.riskScore = calculateRiskScore(analysis);

  // Generate embedding for the diff
  if (isReady()) {
    const diffText = hunks.map(h => h.content).join('\n');
    const result = await embed(`${category} change in ${file}: ${diffText.substring(0, 500)}`);
    analysis.embedding = result.embedding;
  }

  return analysis;
}

/**
 * Get diff for a commit
 */
export function getCommitDiff(commitHash: string = 'HEAD'): string {
  try {
    return execSync(`git show ${commitHash} --format="" 2>/dev/null`, {
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024,
    });
  } catch {
    return '';
  }
}

/**
 * Get diff for staged changes
 */
export function getStagedDiff(): string {
  try {
    return execSync('git diff --cached 2>/dev/null', {
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024,
    });
  } catch {
    return '';
  }
}

/**
 * Get diff for unstaged changes
 */
export function getUnstagedDiff(): string {
  try {
    return execSync('git diff 2>/dev/null', {
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024,
    });
  } catch {
    return '';
  }
}

/**
 * Analyze a commit
 */
export async function analyzeCommit(commitHash: string = 'HEAD'): Promise<CommitAnalysis> {
  const diff = getCommitDiff(commitHash);

  // Get commit metadata
  let message = '', author = '', date = '';
  try {
    const info = execSync(`git log -1 --format="%s|%an|%aI" ${commitHash} 2>/dev/null`, {
      encoding: 'utf8',
    }).trim();
    [message, author, date] = info.split('|');
  } catch {}

  // Parse hunks and group by file
  const hunks = parseDiff(diff);
  const fileHunks = new Map<string, DiffHunk[]>();

  for (const hunk of hunks) {
    if (!fileHunks.has(hunk.file)) {
      fileHunks.set(hunk.file, []);
    }
    fileHunks.get(hunk.file)!.push(hunk);
  }

  // Analyze each file
  const files: DiffAnalysis[] = [];
  for (const [file, fileHunkList] of fileHunks) {
    const fileDiff = fileHunkList.map(h => h.content).join('\n');
    const analysis = await analyzeFileDiff(file, diff, message);
    files.push(analysis);
  }

  const totalAdditions = files.reduce((sum, f) => sum + f.totalAdditions, 0);
  const totalDeletions = files.reduce((sum, f) => sum + f.totalDeletions, 0);
  const riskScore = files.length > 0
    ? files.reduce((sum, f) => sum + f.riskScore, 0) / files.length
    : 0;

  // Generate commit embedding
  let embedding: number[] | undefined;
  if (isReady()) {
    const commitText = `${message}\n\nFiles changed: ${files.map(f => f.file).join(', ')}\n+${totalAdditions} -${totalDeletions}`;
    const result = await embed(commitText);
    embedding = result.embedding;
  }

  return {
    hash: commitHash,
    message,
    author,
    date,
    files,
    totalAdditions,
    totalDeletions,
    riskScore,
    embedding,
  };
}

/**
 * Find similar past commits based on diff embeddings
 */
export async function findSimilarCommits(
  currentDiff: string,
  recentCommits: number = 50,
  topK: number = 5
): Promise<Array<{ hash: string; similarity: number; message: string }>> {
  if (!isReady()) {
    await initOnnxEmbedder();
  }

  // Get current diff embedding
  const currentEmbedding = (await embed(currentDiff.substring(0, 1000))).embedding;

  // Get recent commits
  let commits: string[] = [];
  try {
    commits = execSync(`git log -${recentCommits} --format="%H" 2>/dev/null`, {
      encoding: 'utf8',
    }).trim().split('\n');
  } catch {
    return [];
  }

  // Analyze and compare
  const results: Array<{ hash: string; similarity: number; message: string }> = [];

  for (const hash of commits.slice(0, Math.min(commits.length, recentCommits))) {
    const analysis = await analyzeCommit(hash);
    if (analysis.embedding) {
      const similarity = cosineSimilarity(currentEmbedding, analysis.embedding);
      results.push({ hash, similarity, message: analysis.message });
    }
  }

  return results
    .sort((a, b) => b.similarity - a.similarity)
    .slice(0, topK);
}

function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) return 0;

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  const magnitude = Math.sqrt(normA) * Math.sqrt(normB);
  return magnitude === 0 ? 0 : dotProduct / magnitude;
}

export default {
  parseDiff,
  classifyChange,
  calculateRiskScore,
  analyzeFileDiff,
  analyzeCommit,
  getCommitDiff,
  getStagedDiff,
  getUnstagedDiff,
  findSimilarCommits,
};
