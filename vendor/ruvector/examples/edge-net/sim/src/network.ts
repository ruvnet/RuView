/**
 * Network State Management
 * Manages the P2P network state and phase transitions
 */

import { Cell, CellType, CellState } from './cell.js';

export enum NetworkPhase {
  GENESIS = 'genesis',        // 0 - 10K nodes
  GROWTH = 'growth',          // 10K - 50K nodes
  MATURATION = 'maturation',  // 50K - 100K nodes
  INDEPENDENCE = 'independence', // 100K+ nodes
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

export class Network {
  public cells: Map<string, Cell>;
  public currentPhase: NetworkPhase;
  public currentTick: number;
  public config: NetworkConfig;
  public genesisCells: Set<string>;
  private taskQueue: number[];

  constructor(config?: Partial<NetworkConfig>) {
    this.cells = new Map();
    this.currentPhase = NetworkPhase.GENESIS;
    this.currentTick = 0;
    this.genesisCells = new Set();
    this.taskQueue = [];

    this.config = {
      genesisNodeCount: config?.genesisNodeCount ?? 100,
      targetNodeCount: config?.targetNodeCount ?? 120000,
      nodesPerTick: config?.nodesPerTick ?? 10,
      taskGenerationRate: config?.taskGenerationRate ?? 5,
      baseTaskReward: config?.baseTaskReward ?? 1.0,
      connectionCost: config?.connectionCost ?? 0.5,
      maxConnectionsPerNode: config?.maxConnectionsPerNode ?? 50,
    };
  }

  /**
   * Initialize network with genesis nodes
   */
  public initialize(): void {
    console.log(`Initializing network with ${this.config.genesisNodeCount} genesis nodes...`);

    for (let i = 0; i < this.config.genesisNodeCount; i++) {
      const cell = new Cell(CellType.GENESIS, this.currentTick, {
        computePower: 0.8 + Math.random() * 0.2, // Genesis nodes are powerful
        bandwidth: 0.8 + Math.random() * 0.2,
        reliability: 0.9 + Math.random() * 0.1,
        storage: 0.8 + Math.random() * 0.2,
      });

      this.cells.set(cell.id, cell);
      this.genesisCells.add(cell.id);
    }

    // Connect genesis nodes to each other (mesh topology)
    this.connectGenesisNodes();
  }

  /**
   * Connect all genesis nodes to each other
   */
  private connectGenesisNodes(): void {
    const genesisArray = Array.from(this.genesisCells);
    for (let i = 0; i < genesisArray.length; i++) {
      for (let j = i + 1; j < genesisArray.length; j++) {
        const cell1 = this.cells.get(genesisArray[i])!;
        const cell2 = this.cells.get(genesisArray[j])!;

        cell1.connectTo(cell2.id);
        cell2.connectTo(cell1.id);
      }
    }
  }

  /**
   * Add new regular nodes to the network
   */
  public spawnNodes(count: number): void {
    for (let i = 0; i < count; i++) {
      const cell = new Cell(CellType.REGULAR, this.currentTick);
      this.cells.set(cell.id, cell);

      // Connect to random existing nodes (preferential attachment)
      this.connectNewNode(cell);
    }
  }

  /**
   * Connect a new node to the network
   */
  private connectNewNode(newCell: Cell): void {
    const connectionCount = Math.min(
      5 + Math.floor(Math.random() * 5),
      this.config.maxConnectionsPerNode
    );

    const potentialTargets = Array.from(this.cells.values())
      .filter(c => c.id !== newCell.id)
      .filter(c => {
        // In Phase 2+, genesis nodes don't accept new connections
        if (this.currentPhase !== NetworkPhase.GENESIS && c.type === CellType.GENESIS) {
          return false;
        }
        return c.state === CellState.ACTIVE && c.connectedCells.size < this.config.maxConnectionsPerNode;
      });

    // Preferential attachment: higher fitness = more likely to connect
    const selectedTargets = this.selectPreferentialTargets(potentialTargets, connectionCount);

    for (const target of selectedTargets) {
      newCell.connectTo(target.id);
      target.connectTo(newCell.id);

      // Connection costs energy
      newCell.spendEnergy(this.config.connectionCost);
      target.spendEnergy(this.config.connectionCost);
    }
  }

  /**
   * Select targets using preferential attachment
   */
  private selectPreferentialTargets(candidates: Cell[], count: number): Cell[] {
    if (candidates.length <= count) {
      return candidates;
    }

    const selected: Cell[] = [];
    const weights = candidates.map(c => c.getFitnessScore() * (1 + c.connectedCells.size));
    const totalWeight = weights.reduce((sum, w) => sum + w, 0);

    for (let i = 0; i < count && candidates.length > 0; i++) {
      let random = Math.random() * totalWeight;
      let selectedIndex = 0;

      for (let j = 0; j < weights.length; j++) {
        random -= weights[j];
        if (random <= 0) {
          selectedIndex = j;
          break;
        }
      }

      selected.push(candidates[selectedIndex]);
      candidates.splice(selectedIndex, 1);
      weights.splice(selectedIndex, 1);
    }

    return selected;
  }

  /**
   * Generate tasks for the network
   */
  private generateTasks(): void {
    const tasksToGenerate = Math.floor(
      this.cells.size * this.config.taskGenerationRate * Math.random()
    );

    for (let i = 0; i < tasksToGenerate; i++) {
      // Task complexity between 0.1 and 1.0
      this.taskQueue.push(0.1 + Math.random() * 0.9);
    }
  }

  /**
   * Distribute tasks to capable cells
   */
  private distributeTasks(): void {
    const activeCells = Array.from(this.cells.values())
      .filter(c => c.state === CellState.ACTIVE);

    while (this.taskQueue.length > 0 && activeCells.length > 0) {
      const task = this.taskQueue.shift()!;

      // Select cell based on fitness and availability
      const selectedCell = activeCells[Math.floor(Math.random() * activeCells.length)];
      selectedCell.processTask(task, this.config.baseTaskReward);
    }
  }

  /**
   * Update network phase based on node count
   */
  private updatePhase(): void {
    const nodeCount = this.cells.size;
    const oldPhase = this.currentPhase;

    if (nodeCount >= 100000) {
      this.currentPhase = NetworkPhase.INDEPENDENCE;
    } else if (nodeCount >= 50000) {
      this.currentPhase = NetworkPhase.MATURATION;
    } else if (nodeCount >= 10000) {
      this.currentPhase = NetworkPhase.GROWTH;
    } else {
      this.currentPhase = NetworkPhase.GENESIS;
    }

    if (oldPhase !== this.currentPhase) {
      console.log(`\n🔄 PHASE TRANSITION: ${oldPhase} → ${this.currentPhase} (${nodeCount} nodes)`);
      this.onPhaseTransition();
    }
  }

  /**
   * Handle phase transition events
   */
  private onPhaseTransition(): void {
    // Update all cells based on new phase
    this.cells.forEach(cell => cell.updateState(this.cells.size));

    // Phase-specific actions
    switch (this.currentPhase) {
      case NetworkPhase.GROWTH:
        console.log('  → Genesis nodes reducing 10x multiplier...');
        break;
      case NetworkPhase.MATURATION:
        console.log('  → Genesis nodes entering READ-ONLY mode...');
        break;
      case NetworkPhase.INDEPENDENCE:
        console.log('  → Genesis nodes RETIRED. Network is independent!');
        break;
    }
  }

  /**
   * Simulate one tick of the network
   */
  public tick(): void {
    this.currentTick++;

    // Spawn new nodes (if not at target)
    if (this.cells.size < this.config.targetNodeCount) {
      const nodesToSpawn = Math.min(
        this.config.nodesPerTick,
        this.config.targetNodeCount - this.cells.size
      );
      this.spawnNodes(nodesToSpawn);
    }

    // Generate and distribute tasks
    this.generateTasks();
    this.distributeTasks();

    // Update all cells
    this.cells.forEach(cell => {
      cell.tick();
      cell.updateState(this.cells.size);
    });

    // Check for phase transitions
    this.updatePhase();
  }

  /**
   * Get network statistics
   */
  public getStats() {
    const cells = Array.from(this.cells.values());
    const genesisCells = cells.filter(c => c.type === CellType.GENESIS);
    const regularCells = cells.filter(c => c.type === CellType.REGULAR);

    const totalEnergy = cells.reduce((sum, c) => sum + c.energy, 0);
    const totalEarned = cells.reduce((sum, c) => sum + c.metrics.energyEarned, 0);
    const totalSpent = cells.reduce((sum, c) => sum + c.metrics.energySpent, 0);
    const totalTasks = cells.reduce((sum, c) => sum + c.metrics.tasksCompleted, 0);

    return {
      tick: this.currentTick,
      phase: this.currentPhase,
      nodeCount: this.cells.size,
      genesisNodes: {
        count: genesisCells.length,
        active: genesisCells.filter(c => c.state === CellState.ACTIVE).length,
        readOnly: genesisCells.filter(c => c.state === CellState.READ_ONLY).length,
        retired: genesisCells.filter(c => c.state === CellState.RETIRED).length,
        avgMultiplier: genesisCells.reduce((sum, c) => sum + c.genesisMultiplier, 0) / genesisCells.length,
      },
      regularNodes: {
        count: regularCells.length,
      },
      economy: {
        totalEnergy,
        totalEarned,
        totalSpent,
        netEnergy: totalEarned - totalSpent,
        avgEnergyPerNode: totalEnergy / this.cells.size,
      },
      tasks: {
        completed: totalTasks,
        queued: this.taskQueue.length,
        avgPerNode: totalTasks / this.cells.size,
      },
      network: {
        avgConnections: cells.reduce((sum, c) => sum + c.connectedCells.size, 0) / this.cells.size,
        avgSuccessRate: cells.reduce((sum, c) => sum + c.metrics.successRate, 0) / this.cells.size,
      },
    };
  }
}
