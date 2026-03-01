/**
 * Cell (Node) Simulation
 * Represents a single node in the edge-net network
 */
import { v4 as uuidv4 } from 'uuid';
export var CellType;
(function (CellType) {
    CellType["GENESIS"] = "genesis";
    CellType["REGULAR"] = "regular";
})(CellType || (CellType = {}));
export var CellState;
(function (CellState) {
    CellState["ACTIVE"] = "active";
    CellState["READ_ONLY"] = "read_only";
    CellState["RETIRED"] = "retired";
})(CellState || (CellState = {}));
export class Cell {
    id;
    type;
    joinedAtTick;
    state;
    capabilities;
    energy; // rUv balance
    metrics;
    connectedCells;
    genesisMultiplier; // 10x for genesis nodes initially
    constructor(type, joinedAtTick, capabilities) {
        this.id = uuidv4();
        this.type = type;
        this.joinedAtTick = joinedAtTick;
        this.state = CellState.ACTIVE;
        this.energy = type === CellType.GENESIS ? 1000 : 10; // Genesis starts with more
        this.connectedCells = new Set();
        this.genesisMultiplier = type === CellType.GENESIS ? 10 : 1;
        // Random capabilities or provided ones
        this.capabilities = {
            computePower: capabilities?.computePower ?? this.randomCapability(0.1, 1.0),
            bandwidth: capabilities?.bandwidth ?? this.randomCapability(0.1, 1.0),
            reliability: capabilities?.reliability ?? this.randomCapability(0.5, 1.0),
            storage: capabilities?.storage ?? this.randomCapability(0.1, 1.0),
        };
        this.metrics = {
            tasksCompleted: 0,
            energyEarned: 0,
            energySpent: 0,
            connections: 0,
            uptime: 0,
            successRate: 1.0,
        };
    }
    randomCapability(min, max) {
        return Math.random() * (max - min) + min;
    }
    /**
     * Process a task and earn energy
     */
    processTask(taskComplexity, baseReward) {
        // Check if cell is alive (reliability check)
        if (Math.random() > this.capabilities.reliability) {
            return false; // Cell failed this tick
        }
        // Check if cell has enough compute power
        if (this.capabilities.computePower < taskComplexity * 0.5) {
            return false; // Task too complex
        }
        // Success - earn energy with genesis multiplier
        const reward = baseReward * this.genesisMultiplier;
        this.energy += reward;
        this.metrics.energyEarned += reward;
        this.metrics.tasksCompleted++;
        // Update success rate
        this.updateSuccessRate(true);
        return true;
    }
    /**
     * Spend energy (for network operations, connections, etc.)
     */
    spendEnergy(amount) {
        if (this.energy >= amount) {
            this.energy -= amount;
            this.metrics.energySpent += amount;
            return true;
        }
        return false;
    }
    /**
     * Connect to another cell
     */
    connectTo(cellId) {
        if (!this.connectedCells.has(cellId)) {
            this.connectedCells.add(cellId);
            this.metrics.connections = this.connectedCells.size;
        }
    }
    /**
     * Disconnect from a cell
     */
    disconnectFrom(cellId) {
        this.connectedCells.delete(cellId);
        this.metrics.connections = this.connectedCells.size;
    }
    /**
     * Update cell state based on network phase
     */
    updateState(networkSize) {
        if (this.type === CellType.GENESIS) {
            if (networkSize >= 50000) {
                // Phase 3: Maturation - Genesis goes read-only
                this.state = CellState.READ_ONLY;
                this.genesisMultiplier = 1; // No more bonus
            }
            else if (networkSize >= 10000) {
                // Phase 2: Growth - Genesis reduces multiplier
                this.genesisMultiplier = Math.max(1, 10 * (1 - (networkSize - 10000) / 40000));
            }
            if (networkSize >= 100000) {
                // Phase 4: Independence - Genesis retires
                this.state = CellState.RETIRED;
            }
        }
    }
    /**
     * Simulate one tick of operation
     */
    tick() {
        this.metrics.uptime++;
        // Passive energy decay (network costs)
        const decayCost = 0.1 * this.connectedCells.size;
        this.spendEnergy(decayCost);
    }
    /**
     * Update success rate with exponential moving average
     */
    updateSuccessRate(success) {
        const alpha = 0.1; // Smoothing factor
        this.metrics.successRate = alpha * (success ? 1 : 0) + (1 - alpha) * this.metrics.successRate;
    }
    /**
     * Get cell's overall fitness score
     */
    getFitnessScore() {
        const { computePower, bandwidth, reliability, storage } = this.capabilities;
        return (computePower * 0.3 + bandwidth * 0.2 + reliability * 0.3 + storage * 0.2);
    }
    /**
     * Serialize cell state for reporting
     */
    toJSON() {
        return {
            id: this.id,
            type: this.type,
            state: this.state,
            joinedAtTick: this.joinedAtTick,
            energy: this.energy,
            genesisMultiplier: this.genesisMultiplier,
            capabilities: this.capabilities,
            metrics: {
                ...this.metrics,
                netEnergy: this.metrics.energyEarned - this.metrics.energySpent,
            },
            connections: this.connectedCells.size,
            fitnessScore: this.getFitnessScore(),
        };
    }
}
//# sourceMappingURL=cell.js.map