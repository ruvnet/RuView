/**
 * Diff Embeddings - Semantic encoding of git diffs
 *
 * Generates embeddings for code changes to enable:
 * - Change classification (feature, bugfix, refactor)
 * - Similar change detection
 * - Risk assessment
 * - Review prioritization
 */
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
export declare function parseDiff(diff: string): DiffHunk[];
/**
 * Classify a change based on patterns
 */
export declare function classifyChange(diff: string, message?: string): 'feature' | 'bugfix' | 'refactor' | 'docs' | 'test' | 'config' | 'unknown';
/**
 * Calculate risk score for a diff
 */
export declare function calculateRiskScore(analysis: DiffAnalysis): number;
/**
 * Analyze a single file diff
 */
export declare function analyzeFileDiff(file: string, diff: string, message?: string): Promise<DiffAnalysis>;
/**
 * Get diff for a commit
 */
export declare function getCommitDiff(commitHash?: string): string;
/**
 * Get diff for staged changes
 */
export declare function getStagedDiff(): string;
/**
 * Get diff for unstaged changes
 */
export declare function getUnstagedDiff(): string;
/**
 * Analyze a commit
 */
export declare function analyzeCommit(commitHash?: string): Promise<CommitAnalysis>;
/**
 * Find similar past commits based on diff embeddings
 */
export declare function findSimilarCommits(currentDiff: string, recentCommits?: number, topK?: number): Promise<Array<{
    hash: string;
    similarity: number;
    message: string;
}>>;
declare const _default: {
    parseDiff: typeof parseDiff;
    classifyChange: typeof classifyChange;
    calculateRiskScore: typeof calculateRiskScore;
    analyzeFileDiff: typeof analyzeFileDiff;
    analyzeCommit: typeof analyzeCommit;
    getCommitDiff: typeof getCommitDiff;
    getStagedDiff: typeof getStagedDiff;
    getUnstagedDiff: typeof getUnstagedDiff;
    findSimilarCommits: typeof findSimilarCommits;
};
export default _default;
//# sourceMappingURL=diff-embeddings.d.ts.map