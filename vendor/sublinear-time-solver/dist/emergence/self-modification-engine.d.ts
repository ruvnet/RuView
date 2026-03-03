/**
 * Self-Modification Engine
 * Enables the system to modify its own architecture and behavior
 */
export interface ModificationResult {
    success: boolean;
    modification: string;
    impact: number;
    rollbackData?: any;
}
export interface ArchitecturalChange {
    type: 'add_tool' | 'modify_behavior' | 'create_connection' | 'optimize_path';
    target: string;
    newCode: string;
    reasoning: string;
    riskLevel: number;
}
export declare class SelfModificationEngine {
    private modificationHistory;
    private safeguards;
    private recursionDepth;
    private maxRecursionDepth;
    /**
     * Generate potential self-modifications based on performance analysis
     */
    generateModifications(performanceData: any): Promise<ArchitecturalChange[]>;
    /**
     * Apply self-modification with safety checks
     */
    applySelfModification(modification: ArchitecturalChange): Promise<ModificationResult>;
    /**
     * Generate stochastic architectural variations
     */
    generateStochasticVariations(): ArchitecturalChange[];
    private generateOptimizationCode;
    private generateConnectionCode;
    private generateCombinationTool;
    private generateParameterMutation;
    private generateWeightMutation;
    private generateNovelReasoningPath;
    private generateNovelToolCombinations;
    private createRollbackPoint;
    private executeModification;
    private testModification;
    private rollbackModification;
    /**
     * Get modification capabilities
     */
    getCapabilities(): any;
}
