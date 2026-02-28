/**
 * Cell (Node) Simulation
 * Represents a single node in the edge-net network
 */
export declare enum CellType {
    GENESIS = "genesis",
    REGULAR = "regular"
}
export declare enum CellState {
    ACTIVE = "active",
    READ_ONLY = "read_only",
    RETIRED = "retired"
}
export interface CellCapabilities {
    computePower: number;
    bandwidth: number;
    reliability: number;
    storage: number;
}
export interface CellMetrics {
    tasksCompleted: number;
    energyEarned: number;
    energySpent: number;
    connections: number;
    uptime: number;
    successRate: number;
}
export declare class Cell {
    readonly id: string;
    readonly type: CellType;
    readonly joinedAtTick: number;
    state: CellState;
    capabilities: CellCapabilities;
    energy: number;
    metrics: CellMetrics;
    connectedCells: Set<string>;
    genesisMultiplier: number;
    constructor(type: CellType, joinedAtTick: number, capabilities?: Partial<CellCapabilities>);
    private randomCapability;
    /**
     * Process a task and earn energy
     */
    processTask(taskComplexity: number, baseReward: number): boolean;
    /**
     * Spend energy (for network operations, connections, etc.)
     */
    spendEnergy(amount: number): boolean;
    /**
     * Connect to another cell
     */
    connectTo(cellId: string): void;
    /**
     * Disconnect from a cell
     */
    disconnectFrom(cellId: string): void;
    /**
     * Update cell state based on network phase
     */
    updateState(networkSize: number): void;
    /**
     * Simulate one tick of operation
     */
    tick(): void;
    /**
     * Update success rate with exponential moving average
     */
    private updateSuccessRate;
    /**
     * Get cell's overall fitness score
     */
    getFitnessScore(): number;
    /**
     * Serialize cell state for reporting
     */
    toJSON(): {
        id: string;
        type: CellType;
        state: CellState;
        joinedAtTick: number;
        energy: number;
        genesisMultiplier: number;
        capabilities: CellCapabilities;
        metrics: {
            netEnergy: number;
            tasksCompleted: number;
            energyEarned: number;
            energySpent: number;
            connections: number;
            uptime: number;
            successRate: number;
        };
        connections: number;
        fitnessScore: number;
    };
}
//# sourceMappingURL=cell.d.ts.map