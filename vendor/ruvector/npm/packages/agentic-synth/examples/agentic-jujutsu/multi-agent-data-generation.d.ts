/**
 * Multi-Agent Data Generation Example
 *
 * Demonstrates coordinating multiple agents generating different types
 * of synthetic data using jujutsu branches, merging contributions,
 * and resolving conflicts.
 */
interface Agent {
    id: string;
    name: string;
    dataType: string;
    branch: string;
    schema: any;
}
interface AgentContribution {
    agentId: string;
    dataType: string;
    recordCount: number;
    commitHash: string;
    quality: number;
    conflicts: string[];
}
declare class MultiAgentDataCoordinator {
    private synth;
    private repoPath;
    private agents;
    constructor(repoPath: string);
    /**
     * Initialize multi-agent data generation environment
     */
    initialize(): Promise<void>;
    /**
     * Register a new agent for data generation
     */
    registerAgent(id: string, name: string, dataType: string, schema: any): Promise<Agent>;
    /**
     * Agent generates data on its dedicated branch
     */
    agentGenerate(agentId: string, count: number, description: string): Promise<AgentContribution>;
    /**
     * Coordinate parallel data generation from multiple agents
     */
    coordinateParallelGeneration(tasks: Array<{
        agentId: string;
        count: number;
        description: string;
    }>): Promise<AgentContribution[]>;
    /**
     * Merge agent contributions into main branch
     */
    mergeContributions(agentIds: string[], strategy?: 'sequential' | 'octopus'): Promise<any>;
    /**
     * Resolve conflicts between agent contributions
     */
    resolveConflicts(conflictFiles: string[], strategy?: 'ours' | 'theirs' | 'manual'): Promise<void>;
    /**
     * Synchronize agent branches with main
     */
    synchronizeAgents(agentIds?: string[]): Promise<void>;
    /**
     * Get agent activity summary
     */
    getAgentActivity(agentId: string): Promise<any>;
    private getLatestCommitHash;
    private calculateQuality;
    private detectConflicts;
}
export { MultiAgentDataCoordinator, Agent, AgentContribution };
//# sourceMappingURL=multi-agent-data-generation.d.ts.map