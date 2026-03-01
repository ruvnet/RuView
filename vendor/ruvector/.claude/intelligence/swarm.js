/**
 * Swarm Optimization Module
 *
 * Implements hive-mind coordination patterns inspired by:
 * - ruvector-mincut: Graph partitioning for optimal agent allocation
 * - Collective intelligence: Emergent behavior from local interactions
 * - Self-healing networks: Dynamic reconfiguration on failure
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, 'data');
const SWARM_STATE_FILE = join(DATA_DIR, 'swarm-state.json');
const COORDINATION_FILE = join(DATA_DIR, 'coordination-graph.json');

if (!existsSync(DATA_DIR)) mkdirSync(DATA_DIR, { recursive: true });

/**
 * Agent coordination graph - models relationships between agents
 * Edges represent coordination strength (higher = more interaction needed)
 */
class CoordinationGraph {
  constructor() {
    this.nodes = new Map();  // agentId -> { type, capabilities, load }
    this.edges = new Map();  // "src:dst" -> { weight, interactions }
    this.load();
  }

  load() {
    if (existsSync(COORDINATION_FILE)) {
      try {
        const data = JSON.parse(readFileSync(COORDINATION_FILE, 'utf-8'));
        this.nodes = new Map(Object.entries(data.nodes || {}));
        this.edges = new Map(Object.entries(data.edges || {}));
      } catch { /* fresh start */ }
    }
  }

  save() {
    const data = {
      nodes: Object.fromEntries(this.nodes),
      edges: Object.fromEntries(this.edges),
      lastUpdated: new Date().toISOString()
    };
    writeFileSync(COORDINATION_FILE, JSON.stringify(data, null, 2));
  }

  addAgent(id, type, capabilities = []) {
    this.nodes.set(id, { type, capabilities, load: 0, active: true });
    this.save();
  }

  removeAgent(id) {
    this.nodes.delete(id);
    // Remove all edges involving this agent
    for (const key of this.edges.keys()) {
      if (key.startsWith(id + ':') || key.endsWith(':' + id)) {
        this.edges.delete(key);
      }
    }
    this.save();
  }

  recordInteraction(srcId, dstId, weight = 1) {
    const key = `${srcId}:${dstId}`;
    const edge = this.edges.get(key) || { weight: 0, interactions: 0 };
    edge.weight += weight;
    edge.interactions++;
    this.edges.set(key, edge);
    this.save();
  }

  /**
   * Find the minimum cut between agent groups
   * Uses a simplified Karger-like approach for demonstration
   * In production, this would use ruvector-mincut's subpolynomial algorithm
   */
  findMinCut() {
    if (this.nodes.size < 2) return { cut: 0, groups: [[...this.nodes.keys()], []] };

    const nodes = [...this.nodes.keys()];
    const edges = [...this.edges.entries()].map(([key, val]) => {
      const [src, dst] = key.split(':');
      return { src, dst, weight: val.weight };
    });

    // Simple greedy cut: separate high-load agents
    const loads = nodes.map(id => ({
      id,
      load: this.nodes.get(id)?.load || 0
    })).sort((a, b) => b.load - a.load);

    const midpoint = Math.ceil(nodes.length / 2);
    const groupA = loads.slice(0, midpoint).map(n => n.id);
    const groupB = loads.slice(midpoint).map(n => n.id);

    // Calculate cut weight
    let cutWeight = 0;
    for (const edge of edges) {
      const srcInA = groupA.includes(edge.src);
      const dstInA = groupA.includes(edge.dst);
      if (srcInA !== dstInA) {
        cutWeight += edge.weight;
      }
    }

    return { cut: cutWeight, groups: [groupA, groupB] };
  }

  /**
   * Find critical agents (high betweenness centrality approximation)
   */
  findCriticalAgents() {
    const centrality = new Map();

    for (const nodeId of this.nodes.keys()) {
      let score = 0;
      for (const [key, edge] of this.edges.entries()) {
        if (key.includes(nodeId)) {
          score += edge.weight * edge.interactions;
        }
      }
      centrality.set(nodeId, score);
    }

    return [...centrality.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([id, score]) => ({ id, score, ...this.nodes.get(id) }));
  }

  /**
   * Recommend optimal agent for a task based on graph structure
   */
  recommendAgent(taskType, requiredCapabilities = []) {
    const candidates = [];

    for (const [id, node] of this.nodes.entries()) {
      if (!node.active) continue;

      // Score based on capabilities match
      let score = 0;
      for (const cap of requiredCapabilities) {
        if (node.capabilities?.includes(cap)) score += 10;
      }

      // Prefer agents with lower load
      score -= node.load * 0.5;

      // Prefer agents with more connections (more coordination experience)
      for (const key of this.edges.keys()) {
        if (key.includes(id)) score += 0.1;
      }

      candidates.push({ id, score, ...node });
    }

    return candidates.sort((a, b) => b.score - a.score);
  }

  getStats() {
    const activeAgents = [...this.nodes.values()].filter(n => n.active).length;
    const totalEdges = this.edges.size;
    const avgWeight = totalEdges > 0
      ? [...this.edges.values()].reduce((sum, e) => sum + e.weight, 0) / totalEdges
      : 0;

    return {
      agents: this.nodes.size,
      activeAgents,
      edges: totalEdges,
      avgEdgeWeight: avgWeight.toFixed(2),
      criticalAgents: this.findCriticalAgents().slice(0, 3)
    };
  }
}

/**
 * Hive Mind Coordinator - emergent collective intelligence
 */
class HiveMind {
  constructor(graph) {
    this.graph = graph;
    this.consensus = new Map();  // decision -> votes
    this.history = [];
  }

  /**
   * Propose a decision to the hive mind
   */
  propose(decision, agentId, confidence = 0.5) {
    const key = this.decisionKey(decision);
    const votes = this.consensus.get(key) || { yes: 0, no: 0, voters: [] };

    if (!votes.voters.includes(agentId)) {
      votes.voters.push(agentId);
      if (confidence > 0.5) {
        votes.yes += confidence;
      } else {
        votes.no += (1 - confidence);
      }
      this.consensus.set(key, votes);
    }

    return this.getConsensus(decision);
  }

  /**
   * Get current consensus on a decision
   */
  getConsensus(decision) {
    const key = this.decisionKey(decision);
    const votes = this.consensus.get(key) || { yes: 0, no: 0, voters: [] };
    const total = votes.yes + votes.no;

    if (total === 0) return { approved: null, confidence: 0, voters: 0 };

    const approval = votes.yes / total;
    return {
      approved: approval > 0.5,
      confidence: Math.abs(approval - 0.5) * 2,  // 0-1 scale
      approval: approval.toFixed(3),
      voters: votes.voters.length
    };
  }

  /**
   * Self-healing: redistribute load when agent fails
   */
  healPartition(failedAgentId) {
    const node = this.graph.nodes.get(failedAgentId);
    if (!node) return { healed: false, reason: 'Agent not found' };

    // Mark as inactive
    node.active = false;
    this.graph.nodes.set(failedAgentId, node);

    // Find replacement candidates
    const candidates = this.graph.recommendAgent(node.type, node.capabilities);
    const activeCandidate = candidates.find(c => c.id !== failedAgentId);

    if (activeCandidate) {
      // Redistribute load
      const redistributed = Math.floor(node.load / Math.max(1, candidates.length - 1));
      for (const candidate of candidates.filter(c => c.id !== failedAgentId)) {
        const candNode = this.graph.nodes.get(candidate.id);
        if (candNode) {
          candNode.load += redistributed;
          this.graph.nodes.set(candidate.id, candNode);
        }
      }

      this.graph.save();
      return {
        healed: true,
        failedAgent: failedAgentId,
        replacedBy: activeCandidate.id,
        loadRedistributed: redistributed * (candidates.length - 1)
      };
    }

    return { healed: false, reason: 'No suitable replacement found' };
  }

  decisionKey(decision) {
    if (typeof decision === 'string') return decision;
    return JSON.stringify(decision);
  }
}

/**
 * Swarm Optimizer - coordinates multiple agents efficiently
 */
class SwarmOptimizer {
  constructor() {
    this.graph = new CoordinationGraph();
    this.hiveMind = new HiveMind(this.graph);
    this.loadState();
  }

  loadState() {
    if (existsSync(SWARM_STATE_FILE)) {
      try {
        this.state = JSON.parse(readFileSync(SWARM_STATE_FILE, 'utf-8'));
      } catch {
        this.state = { tasks: [], optimizations: 0 };
      }
    } else {
      this.state = { tasks: [], optimizations: 0 };
    }
  }

  saveState() {
    this.state.lastUpdated = new Date().toISOString();
    writeFileSync(SWARM_STATE_FILE, JSON.stringify(this.state, null, 2));
  }

  /**
   * Register an agent in the swarm
   */
  registerAgent(id, type, capabilities = []) {
    this.graph.addAgent(id, type, capabilities);
    return { registered: true, id, type };
  }

  /**
   * Record coordination between agents
   */
  recordCoordination(srcAgent, dstAgent, weight = 1) {
    this.graph.recordInteraction(srcAgent, dstAgent, weight);
    return { recorded: true, edge: `${srcAgent} -> ${dstAgent}` };
  }

  /**
   * Get optimal task distribution across agents
   */
  optimizeTaskDistribution(tasks) {
    const { cut, groups } = this.graph.findMinCut();
    const distribution = { groups: [], cut, optimizations: ++this.state.optimizations };

    for (let i = 0; i < groups.length; i++) {
      const groupTasks = tasks.filter((_, idx) => idx % groups.length === i);
      distribution.groups.push({
        agents: groups[i],
        tasks: groupTasks,
        load: groupTasks.length
      });
    }

    this.saveState();
    return distribution;
  }

  /**
   * Get best agent recommendation for a task
   */
  recommendForTask(taskType, capabilities = []) {
    const candidates = this.graph.recommendAgent(taskType, capabilities);
    return {
      recommended: candidates[0] || null,
      alternatives: candidates.slice(1, 4),
      reason: candidates[0]
        ? `Best match for ${taskType} with ${candidates[0].score.toFixed(1)} score`
        : 'No suitable agents found'
    };
  }

  /**
   * Handle agent failure with self-healing
   */
  handleFailure(agentId) {
    return this.hiveMind.healPartition(agentId);
  }

  /**
   * Get swarm statistics
   */
  getStats() {
    return {
      graph: this.graph.getStats(),
      optimizations: this.state.optimizations,
      lastUpdated: this.state.lastUpdated
    };
  }
}

export { SwarmOptimizer, CoordinationGraph, HiveMind };
export default SwarmOptimizer;
