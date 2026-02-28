/**
 * Version Control Integration Example
 *
 * Demonstrates how to use agentic-jujutsu for version controlling
 * synthetic data generation, tracking changes, branching strategies,
 * and rolling back to previous versions.
 */
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
declare class VersionControlledDataGenerator {
    private synth;
    private repoPath;
    private dataPath;
    constructor(repoPath: string);
    /**
     * Initialize jujutsu repository for data versioning
     */
    initializeRepository(): Promise<void>;
    /**
     * Generate synthetic data and commit with metadata
     */
    generateAndCommit(schema: any, count: number, message: string): Promise<JujutsuCommit>;
    /**
     * Create a branch for experimenting with different generation strategies
     */
    createGenerationBranch(branchName: string, description: string): Promise<void>;
    /**
     * Compare datasets between two commits or branches
     */
    compareDatasets(ref1: string, ref2: string): Promise<any>;
    /**
     * Merge data generation branches
     */
    mergeBranches(sourceBranch: string, targetBranch: string): Promise<void>;
    /**
     * Rollback to a previous data version
     */
    rollbackToVersion(commitHash: string): Promise<void>;
    /**
     * Get data generation history
     */
    getHistory(limit?: number): Promise<any[]>;
    /**
     * Tag a specific data generation
     */
    tagVersion(tag: string, message: string): Promise<void>;
    private hashSchema;
    private calculateQuality;
    private getLatestCommitHash;
    private getDataFilesAtRef;
    private parseLogOutput;
}
export { VersionControlledDataGenerator, DataGenerationMetadata, JujutsuCommit };
//# sourceMappingURL=version-control-integration.d.ts.map