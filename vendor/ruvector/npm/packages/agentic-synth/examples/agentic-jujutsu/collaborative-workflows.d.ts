/**
 * Collaborative Workflows Example
 *
 * Demonstrates collaborative synthetic data generation workflows
 * using agentic-jujutsu for multiple teams, review processes,
 * quality gates, and shared repositories.
 */
interface Team {
    id: string;
    name: string;
    members: string[];
    branch: string;
    permissions: string[];
}
interface ReviewRequest {
    id: string;
    title: string;
    description: string;
    author: string;
    sourceBranch: string;
    targetBranch: string;
    status: 'pending' | 'approved' | 'rejected' | 'changes_requested';
    reviewers: string[];
    comments: Comment[];
    qualityGates: QualityGate[];
    createdAt: Date;
}
interface Comment {
    id: string;
    author: string;
    text: string;
    timestamp: Date;
    resolved: boolean;
}
interface QualityGate {
    name: string;
    status: 'passed' | 'failed' | 'pending';
    message: string;
    required: boolean;
}
interface Contribution {
    commitHash: string;
    author: string;
    team: string;
    filesChanged: string[];
    reviewStatus: string;
    timestamp: Date;
}
declare class CollaborativeDataWorkflow {
    private synth;
    private repoPath;
    private teams;
    private reviewRequests;
    constructor(repoPath: string);
    /**
     * Initialize collaborative workspace
     */
    initialize(): Promise<void>;
    /**
     * Create a team with dedicated workspace
     */
    createTeam(id: string, name: string, members: string[], permissions?: string[]): Promise<Team>;
    /**
     * Team generates data on their workspace
     */
    teamGenerate(teamId: string, author: string, schema: any, count: number, description: string): Promise<Contribution>;
    /**
     * Create a review request to merge team work
     */
    createReviewRequest(teamId: string, author: string, title: string, description: string, reviewers: string[]): Promise<ReviewRequest>;
    /**
     * Run quality gates on a review request
     */
    private runQualityGates;
    /**
     * Add comment to review request
     */
    addComment(requestId: string, author: string, text: string): Promise<void>;
    /**
     * Approve review request
     */
    approveReview(requestId: string, reviewer: string): Promise<void>;
    /**
     * Merge approved review
     */
    mergeReview(requestId: string): Promise<void>;
    /**
     * Design collaborative schema
     */
    designCollaborativeSchema(schemaName: string, contributors: string[], baseSchema: any): Promise<any>;
    /**
     * Get team statistics
     */
    getTeamStatistics(teamId: string): Promise<any>;
    private setupBranchProtection;
    private checkDataCompleteness;
    private validateSchema;
    private checkQualityThreshold;
    private getLatestCommitHash;
}
export { CollaborativeDataWorkflow, Team, ReviewRequest, Contribution };
//# sourceMappingURL=collaborative-workflows.d.ts.map