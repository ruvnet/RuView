/**
 * Network State Management
 * Manages the P2P network state and phase transitions
 */
import { Cell } from './cell.js';
export declare enum NetworkPhase {
    GENESIS = "genesis",// 0 - 10K nodes
    GROWTH = "growth",// 10K - 50K nodes
    MATURATION = "maturation",// 50K - 100K nodes
    INDEPENDENCE = "independence"
}
export interface NetworkConfig {
    genesisNodeCount: number;
    targetNodeCount: number;
    nodesPerTick: number;
    taskGenerationRate: number;
    baseTaskReward: number;
    connectionCost: number;
    maxConnectionsPerNode: number;
}
export declare class Network {
    cells: Map<string, Cell>;
    currentPhase: NetworkPhase;
    currentTick: number;
    config: NetworkConfig;
    genesisCells: Set<string>;
    private taskQueue;
    constructor(config?: Partial<NetworkConfig>);
    /**
     * Initialize network with genesis nodes
     */
    initialize(): void;
    /**
     * Connect all genesis nodes to each other
     */
    private connectGenesisNodes;
    /**
     * Add new regular nodes to the network
     */
    spawnNodes(count: number): void;
    /**
     * Connect a new node to the network
     */
    private connectNewNode;
    /**
     * Select targets using preferential attachment
     */
    private selectPreferentialTargets;
    /**
     * Generate tasks for the network
     */
    private generateTasks;
    /**
     * Distribute tasks to capable cells
     */
    private distributeTasks;
    /**
     * Update network phase based on node count
     */
    private updatePhase;
    /**
     * Handle phase transition events
     */
    private onPhaseTransition;
    /**
     * Simulate one tick of the network
     */
    tick(): void;
    /**
     * Get network statistics
     */
    getStats(): {
        tick: number;
        phase: NetworkPhase;
        nodeCount: number;
        genesisNodes: {
            count: number;
            active: number;
            readOnly: number;
            retired: number;
            avgMultiplier: number;
        };
        regularNodes: {
            count: number;
        };
        economy: {
            totalEnergy: number;
            totalEarned: number;
            totalSpent: number;
            netEnergy: number;
            avgEnergyPerNode: number;
        };
        tasks: {
            completed: number;
            queued: number;
            avgPerNode: number;
        };
        network: {
            avgConnections: number;
            avgSuccessRate: number;
        };
    };
}
//# sourceMappingURL=network.d.ts.map