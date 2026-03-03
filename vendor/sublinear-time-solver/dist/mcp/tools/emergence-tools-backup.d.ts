/**
 * MCP Tools for Emergence System
 * Provides MCP interface to the emergence capabilities
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { EmergenceSystemConfig } from '../../emergence/index.js';
export declare class EmergenceTools {
    private emergenceSystem;
    constructor(config?: Partial<EmergenceSystemConfig>);
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    /**
     * Run test scenarios to verify emergence capabilities
     */
    private runTestScenarios;
    /**
     * Run a single test scenario
     */
    private runSingleTestScenario;
    /**
     * Test self-modification capabilities
     */
    private testSelfModification;
    /**
     * Test persistent learning capabilities
     */
    private testPersistentLearning;
    /**
     * Test stochastic exploration capabilities
     */
    private testStochasticExploration;
    /**
     * Test cross-tool sharing capabilities
     */
    private testCrossToolSharing;
    /**
     * Test feedback loop capabilities
     */
    private testFeedbackLoops;
    /**
     * Test emergent capability detection
     */
    private testEmergentCapabilities;
    /**
     * Generate test input for scenarios
     */
    private generateTestInput;
    /**
     * Calculate diversity in responses
     */
    private calculateResponseDiversity;
    /**
     * Calculate similarity between two responses
     */
    private calculateResponseSimilarity;
}
