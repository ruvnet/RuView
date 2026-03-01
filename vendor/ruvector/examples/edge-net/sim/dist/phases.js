/**
 * Phase Transition Logic
 * Manages lifecycle phases and transition conditions
 */
import { NetworkPhase } from './network.js';
import { CellType, CellState } from './cell.js';
export class PhaseManager {
    network;
    metrics;
    conditions;
    lastPhase;
    constructor(network, metrics) {
        this.network = network;
        this.metrics = metrics;
        this.lastPhase = NetworkPhase.GENESIS;
        this.conditions = new Map([
            [NetworkPhase.GENESIS, {
                    minNodes: 0,
                    maxNodes: 10000,
                }],
            [NetworkPhase.GROWTH, {
                    minNodes: 10000,
                    maxNodes: 50000,
                    customCheck: (net) => {
                        // Verify genesis nodes are still active but reducing multiplier
                        const genesisCells = Array.from(net.cells.values())
                            .filter((c) => c.type === CellType.GENESIS);
                        const avgMultiplier = genesisCells.reduce((sum, c) => sum + c.genesisMultiplier, 0) / genesisCells.length;
                        return avgMultiplier < 10 && avgMultiplier > 1;
                    },
                }],
            [NetworkPhase.MATURATION, {
                    minNodes: 50000,
                    maxNodes: 100000,
                    customCheck: (net) => {
                        // Verify genesis nodes are entering read-only mode
                        const genesisCells = Array.from(net.cells.values())
                            .filter((c) => c.type === CellType.GENESIS);
                        const readOnlyCount = genesisCells.filter(c => c.state === CellState.READ_ONLY).length;
                        return readOnlyCount >= genesisCells.length * 0.5; // At least 50% read-only
                    },
                }],
            [NetworkPhase.INDEPENDENCE, {
                    minNodes: 100000,
                    maxNodes: Infinity,
                    customCheck: (net) => {
                        // Verify genesis nodes are retired
                        const genesisCells = Array.from(net.cells.values())
                            .filter((c) => c.type === CellType.GENESIS);
                        const retiredCount = genesisCells.filter(c => c.state === CellState.RETIRED).length;
                        return retiredCount >= genesisCells.length * 0.8; // At least 80% retired
                    },
                }],
        ]);
    }
    /**
     * Check if network should transition to next phase
     */
    checkTransition() {
        const currentPhase = this.network.currentPhase;
        const nodeCount = this.network.cells.size;
        // Determine target phase based on node count
        let targetPhase = NetworkPhase.GENESIS;
        if (nodeCount >= 100000) {
            targetPhase = NetworkPhase.INDEPENDENCE;
        }
        else if (nodeCount >= 50000) {
            targetPhase = NetworkPhase.MATURATION;
        }
        else if (nodeCount >= 10000) {
            targetPhase = NetworkPhase.GROWTH;
        }
        // If phase changed, validate transition
        if (targetPhase !== currentPhase) {
            const condition = this.conditions.get(targetPhase);
            if (condition) {
                // Check node count bounds
                if (nodeCount < condition.minNodes || nodeCount >= condition.maxNodes) {
                    return false;
                }
                // Check custom conditions
                if (condition.customCheck && !condition.customCheck(this.network)) {
                    return false;
                }
                // Valid transition
                this.onTransition(currentPhase, targetPhase);
                return true;
            }
        }
        return false;
    }
    /**
     * Handle phase transition
     */
    onTransition(fromPhase, toPhase) {
        console.log(`\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
        console.log(`🔄 PHASE TRANSITION: ${fromPhase.toUpperCase()} → ${toPhase.toUpperCase()}`);
        console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
        // Notify metrics collector
        this.metrics.onPhaseTransition(fromPhase, toPhase);
        // Log phase-specific information
        this.logPhaseInfo(toPhase);
        this.lastPhase = toPhase;
    }
    /**
     * Log phase-specific information
     */
    logPhaseInfo(phase) {
        const stats = this.network.getStats();
        console.log(`📊 Network Status:`);
        console.log(`   Nodes: ${stats.nodeCount.toLocaleString()}`);
        console.log(`   Genesis Nodes: ${stats.genesisNodes.count}`);
        console.log(`   Avg Connections: ${stats.network.avgConnections.toFixed(2)}`);
        console.log(`   Total Energy: ${stats.economy.totalEnergy.toFixed(2)} rUv`);
        switch (phase) {
            case NetworkPhase.GENESIS:
                console.log(`\n🌱 Genesis Phase:`);
                console.log(`   - Genesis nodes establishing network`);
                console.log(`   - 10x energy multiplier active`);
                console.log(`   - Target: 10,000 nodes`);
                break;
            case NetworkPhase.GROWTH:
                console.log(`\n🌿 Growth Phase:`);
                console.log(`   - Genesis multiplier: ${stats.genesisNodes.avgMultiplier.toFixed(2)}x`);
                console.log(`   - Genesis nodes reducing connections`);
                console.log(`   - Network self-organizing`);
                console.log(`   - Target: 50,000 nodes`);
                break;
            case NetworkPhase.MATURATION:
                console.log(`\n🌳 Maturation Phase:`);
                console.log(`   - Genesis nodes: ${stats.genesisNodes.readOnly} read-only`);
                console.log(`   - Network operating independently`);
                console.log(`   - Economic sustainability: ${(stats.economy.totalEarned / Math.max(stats.economy.totalSpent, 1)).toFixed(2)}x`);
                console.log(`   - Target: 100,000 nodes`);
                break;
            case NetworkPhase.INDEPENDENCE:
                console.log(`\n🚀 Independence Phase:`);
                console.log(`   - Genesis nodes: ${stats.genesisNodes.retired} retired`);
                console.log(`   - Pure P2P operation`);
                console.log(`   - Network fully autonomous`);
                console.log(`   - Target: Long-term stability`);
                break;
        }
        console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);
    }
    /**
     * Get phase progress (0-1)
     */
    getPhaseProgress() {
        const condition = this.conditions.get(this.network.currentPhase);
        if (!condition)
            return 0;
        const nodeCount = this.network.cells.size;
        const range = condition.maxNodes - condition.minNodes;
        const progress = (nodeCount - condition.minNodes) / range;
        return Math.max(0, Math.min(1, progress));
    }
    /**
     * Get estimated ticks to next phase
     */
    getTicksToNextPhase() {
        const condition = this.conditions.get(this.network.currentPhase);
        if (!condition || condition.maxNodes === Infinity)
            return -1;
        const nodeCount = this.network.cells.size;
        const nodesNeeded = condition.maxNodes - nodeCount;
        const ticksNeeded = Math.ceil(nodesNeeded / this.network.config.nodesPerTick);
        return Math.max(0, ticksNeeded);
    }
}
//# sourceMappingURL=phases.js.map