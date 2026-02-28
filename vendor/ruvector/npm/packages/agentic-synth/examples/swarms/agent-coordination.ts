/**
 * Multi-Agent System Coordination Examples
 *
 * Demonstrates agent communication patterns, task distribution,
 * consensus building, load balancing, and fault tolerance scenarios
 * for distributed agent systems.
 *
 * Integrates with:
 * - claude-flow: Swarm initialization and orchestration
 * - ruv-swarm: Enhanced coordination patterns
 * - flow-nexus: Cloud-based agent management
 */

import { AgenticSynth, createSynth } from '../../dist/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Agent Communication Patterns
// ============================================================================

/**
 * Generate communication patterns for multi-agent systems
 */
export async function agentCommunicationPatterns() {
  console.log('\n🤖 Example 1: Agent Communication Patterns\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate message passing data
  const messages = await synth.generateEvents({
    count: 500,
    eventTypes: [
      'direct_message',
      'broadcast',
      'multicast',
      'request_reply',
      'publish_subscribe',
    ],
    schema: {
      message_id: 'UUID',
      sender_agent_id: 'agent-{1-20}',
      receiver_agent_id: 'agent-{1-20} or "broadcast"',
      message_type: 'one of eventTypes',
      payload: {
        action: 'task_request | status_update | data_sync | error_report | ack',
        data: 'JSON object with task details',
        priority: 'high | medium | low',
        requires_response: 'boolean',
      },
      timestamp: 'ISO timestamp',
      latency_ms: 'number (1-500)',
      success: 'boolean (95% true)',
    },
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 3600000), // Last hour
      end: new Date(),
    },
  });

  console.log('Communication Patterns Generated:');
  console.log(`- Total messages: ${messages.data.length}`);
  console.log(`- Direct messages: ${messages.data.filter((m: any) => m.message_type === 'direct_message').length}`);
  console.log(`- Broadcasts: ${messages.data.filter((m: any) => m.message_type === 'broadcast').length}`);
  console.log(`- Average latency: ${(messages.data.reduce((sum: number, m: any) => sum + m.latency_ms, 0) / messages.data.length).toFixed(2)}ms`);

  // Integration with claude-flow hooks
  console.log('\nClaude-Flow Integration:');
  console.log('npx claude-flow@alpha hooks notify --message "Communication patterns generated"');
  console.log('npx claude-flow@alpha hooks post-edit --file "messages.json" --memory-key "swarm/coordinator/messages"');

  return messages;
}

// ============================================================================
// Example 2: Task Distribution Scenarios
// ============================================================================

/**
 * Generate task distribution data for load balancing
 */
export async function taskDistributionScenarios() {
  console.log('\n📋 Example 2: Task Distribution Scenarios\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate task distribution events
  const tasks = await synth.generateStructured({
    count: 300,
    schema: {
      task_id: 'UUID',
      task_type: 'compute | data_processing | io_operation | ml_inference | api_call',
      assigned_agent: 'agent-{1-15}',
      estimated_duration_ms: 'number (100-10000)',
      actual_duration_ms: 'number (estimated_duration_ms * 0.8-1.2)',
      cpu_usage: 'number (0-100)',
      memory_mb: 'number (10-1000)',
      priority: 'number (1-10)',
      dependencies: ['array of 0-3 task_ids or empty'],
      status: 'pending | running | completed | failed',
      start_time: 'ISO timestamp',
      end_time: 'ISO timestamp or null',
      retry_count: 'number (0-3)',
    },
    constraints: [
      'Tasks should be distributed evenly across agents',
      'High priority tasks (8-10) should complete faster',
      '5% of tasks should have failed status',
      'Dependencies should form valid DAG (no cycles)',
    ],
  });

  // Analyze distribution
  const agentLoad = new Map<string, number>();
  tasks.data.forEach((task: any) => {
    agentLoad.set(task.assigned_agent, (agentLoad.get(task.assigned_agent) || 0) + 1);
  });

  console.log('Task Distribution Analysis:');
  console.log(`- Total tasks: ${tasks.data.length}`);
  console.log(`- Agents utilized: ${agentLoad.size}`);
  console.log(`- Tasks per agent (avg): ${(tasks.data.length / agentLoad.size).toFixed(1)}`);
  console.log(`- Failed tasks: ${tasks.data.filter((t: any) => t.status === 'failed').length}`);
  console.log(`- Completed tasks: ${tasks.data.filter((t: any) => t.status === 'completed').length}`);

  // Ruv-Swarm coordination pattern
  console.log('\nRuv-Swarm Coordination:');
  console.log('npx ruv-swarm mcp start');
  console.log('// Use MCP tools: swarm_init, task_orchestrate, agent_metrics');

  return tasks;
}

// ============================================================================
// Example 3: Consensus Building Data
// ============================================================================

/**
 * Generate consensus protocol data for distributed decision making
 */
export async function consensusBuildingData() {
  console.log('\n🤝 Example 3: Consensus Building Scenarios\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate consensus rounds
  const consensusRounds = await synth.generateStructured({
    count: 50,
    schema: {
      round_id: 'UUID',
      proposal_id: 'UUID',
      protocol: 'raft | paxos | byzantine | quorum',
      participants: ['array of 5-15 agent ids'],
      proposer: 'agent id from participants',
      proposal: {
        type: 'leader_election | config_change | state_update | task_assignment',
        value: 'JSON object with proposal details',
      },
      votes: [
        {
          agent_id: 'agent id from participants',
          vote: 'accept | reject | abstain',
          timestamp: 'ISO timestamp',
          reasoning: 'short explanation',
        },
      ],
      status: 'proposing | voting | committed | rejected | timeout',
      quorum_required: 'number (majority of participants)',
      quorum_reached: 'boolean',
      decision: 'accepted | rejected | timeout',
      round_duration_ms: 'number (100-5000)',
      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Votes array should have one entry per participant',
      '70% of rounds should reach quorum',
      '80% of committed rounds should be accepted',
      'Byzantine protocol should have 3f+1 participants (f failures)',
    ],
  });

  // Analyze consensus
  const successRate = consensusRounds.data.filter(
    (r: any) => r.decision === 'accepted'
  ).length / consensusRounds.data.length;

  console.log('Consensus Analysis:');
  console.log(`- Total rounds: ${consensusRounds.data.length}`);
  console.log(`- Success rate: ${(successRate * 100).toFixed(1)}%`);
  console.log(`- Average duration: ${(consensusRounds.data.reduce((sum: number, r: any) => sum + r.round_duration_ms, 0) / consensusRounds.data.length).toFixed(0)}ms`);
  console.log(`- Quorum reached: ${consensusRounds.data.filter((r: any) => r.quorum_reached).length} rounds`);

  // Protocol distribution
  const protocolCount = new Map<string, number>();
  consensusRounds.data.forEach((r: any) => {
    protocolCount.set(r.protocol, (protocolCount.get(r.protocol) || 0) + 1);
  });
  console.log('\nProtocol Usage:');
  protocolCount.forEach((count, protocol) => {
    console.log(`- ${protocol}: ${count} rounds`);
  });

  return consensusRounds;
}

// ============================================================================
// Example 4: Load Balancing Patterns
// ============================================================================

/**
 * Generate load balancing metrics and patterns
 */
export async function loadBalancingPatterns() {
  console.log('\n⚖️  Example 4: Load Balancing Patterns\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate time-series load balancing metrics
  const metrics = await synth.generateTimeSeries({
    count: 200,
    interval: '30s',
    metrics: [
      'agent_count',
      'total_requests',
      'avg_response_time_ms',
      'cpu_utilization',
      'memory_utilization',
      'queue_depth',
    ],
    trend: 'mixed',
    seasonality: true,
  });

  // Generate agent-specific metrics
  const agentMetrics = await synth.generateStructured({
    count: 100,
    schema: {
      timestamp: 'ISO timestamp',
      agent_id: 'agent-{1-10}',
      algorithm: 'round_robin | least_connections | weighted | ip_hash | consistent_hash',
      requests_handled: 'number (0-100)',
      active_connections: 'number (0-50)',
      cpu_percent: 'number (0-100)',
      memory_mb: 'number (100-2000)',
      response_time_p50: 'number (10-500)',
      response_time_p99: 'number (50-2000)',
      error_rate: 'number (0-0.05)',
      health_score: 'number (0-100)',
    },
    constraints: [
      'Load should be relatively balanced across agents',
      'High CPU should correlate with high request count',
      'Error rate should increase with overload',
      'Health score should decrease with high utilization',
    ],
  });

  console.log('Load Balancing Analysis:');
  console.log(`- Time series points: ${metrics.data.length}`);
  console.log(`- Agent metrics: ${agentMetrics.data.length}`);

  // Calculate balance score
  const requestsByAgent = new Map<string, number>();
  agentMetrics.data.forEach((m: any) => {
    requestsByAgent.set(
      m.agent_id,
      (requestsByAgent.get(m.agent_id) || 0) + m.requests_handled
    );
  });

  const avgRequests = Array.from(requestsByAgent.values()).reduce((a, b) => a + b, 0) / requestsByAgent.size;
  const variance = Array.from(requestsByAgent.values()).reduce(
    (sum, val) => sum + Math.pow(val - avgRequests, 2),
    0
  ) / requestsByAgent.size;

  console.log(`- Average requests per agent: ${avgRequests.toFixed(1)}`);
  console.log(`- Load variance: ${variance.toFixed(2)} (lower is better)`);

  // Flow-Nexus integration for cloud load balancing
  console.log('\nFlow-Nexus Cloud Integration:');
  console.log('npx flow-nexus@latest login');
  console.log('// Use MCP tools: swarm_scale, agent_spawn, sandbox_create');

  return { metrics, agentMetrics };
}

// ============================================================================
// Example 5: Fault Tolerance Scenarios
// ============================================================================

/**
 * Generate fault tolerance and failure recovery scenarios
 */
export async function faultToleranceScenarios() {
  console.log('\n🛡️  Example 5: Fault Tolerance Scenarios\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate failure events
  const failures = await synth.generateEvents({
    count: 100,
    eventTypes: [
      'agent_crash',
      'network_partition',
      'timeout',
      'out_of_memory',
      'resource_exhaustion',
      'byzantine_fault',
    ],
    schema: {
      incident_id: 'UUID',
      event_type: 'one of eventTypes',
      affected_agents: ['array of 1-5 agent ids'],
      severity: 'critical | high | medium | low',
      detection_time: 'ISO timestamp',
      recovery_initiated: 'ISO timestamp',
      recovery_completed: 'ISO timestamp or null',
      recovery_strategy: 'restart | failover | rollback | isolation | replication',
      data_lost: 'boolean',
      service_degraded: 'boolean',
      mttr_seconds: 'number (10-600)',
      root_cause: 'short description of root cause',
    },
    distribution: 'uniform',
    timeRange: {
      start: new Date(Date.now() - 86400000), // Last 24 hours
      end: new Date(),
    },
  });

  // Generate recovery actions
  const recoveryActions = await synth.generateStructured({
    count: failures.data.length,
    schema: {
      incident_id: 'UUID (from failures)',
      action_id: 'UUID',
      action_type: 'health_check | restart_agent | promote_backup | restore_state | load_balance',
      executor: 'coordinator | agent-{id} | auto_healer',
      success: 'boolean (90% true)',
      duration_ms: 'number (100-10000)',
      retries: 'number (0-3)',
      compensating_actions: ['array of 0-2 action types or empty'],
      timestamp: 'ISO timestamp',
    },
  });

  // Analyze fault tolerance
  const mttrAvg = failures.data.reduce((sum: number, f: any) => sum + f.mttr_seconds, 0) / failures.data.length;
  const recoveryRate = recoveryActions.data.filter((a: any) => a.success).length / recoveryActions.data.length;

  console.log('Fault Tolerance Analysis:');
  console.log(`- Total incidents: ${failures.data.length}`);
  console.log(`- Average MTTR: ${mttrAvg.toFixed(1)} seconds`);
  console.log(`- Recovery success rate: ${(recoveryRate * 100).toFixed(1)}%`);
  console.log(`- Data loss incidents: ${failures.data.filter((f: any) => f.data_lost).length}`);
  console.log(`- Service degraded: ${failures.data.filter((f: any) => f.service_degraded).length}`);

  // Failure type distribution
  const failureTypes = new Map<string, number>();
  failures.data.forEach((f: any) => {
    failureTypes.set(f.event_type, (failureTypes.get(f.event_type) || 0) + 1);
  });

  console.log('\nFailure Type Distribution:');
  failureTypes.forEach((count, type) => {
    console.log(`- ${type}: ${count} (${((count / failures.data.length) * 100).toFixed(1)}%)`);
  });

  // Self-healing integration
  console.log('\nSelf-Healing Integration:');
  console.log('// Enable auto-recovery hooks');
  console.log('npx claude-flow@alpha hooks enable --type "error-recovery"');

  return { failures, recoveryActions };
}

// ============================================================================
// Example 6: Hierarchical Coordination
// ============================================================================

/**
 * Generate hierarchical swarm coordination data
 */
export async function hierarchicalCoordination() {
  console.log('\n🏗️  Example 6: Hierarchical Swarm Coordination\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate hierarchical structure
  const swarmTopology = await synth.generateStructured({
    count: 1,
    schema: {
      topology_id: 'UUID',
      type: 'hierarchical',
      coordinator: {
        agent_id: 'coordinator-main',
        role: 'master_coordinator',
        responsibilities: ['task_distribution', 'health_monitoring', 'consensus_leader'],
      },
      sub_coordinators: [
        {
          agent_id: 'sub-coordinator-{1-5}',
          role: 'regional_coordinator',
          manages_agents: ['array of 5-10 worker agent ids'],
          region: 'zone-{A-E}',
        },
      ],
      workers: [
        {
          agent_id: 'worker-{1-50}',
          coordinator_id: 'sub-coordinator id',
          capabilities: ['array of 2-4 capabilities'],
          status: 'active | idle | busy | offline',
        },
      ],
      communication_patterns: {
        coordinator_to_sub: 'direct',
        sub_to_workers: 'multicast',
        worker_to_coordinator: 'via_sub_coordinator',
        peer_to_peer: 'disabled',
      },
    },
  });

  // Generate coordination events
  const coordinationEvents = await synth.generateEvents({
    count: 200,
    eventTypes: [
      'task_delegation',
      'status_report',
      'resource_request',
      'coordination_sync',
      'topology_update',
    ],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      from_agent: 'agent id from topology',
      to_agent: 'agent id from topology',
      hierarchy_level: 'coordinator | sub_coordinator | worker',
      payload: 'JSON object',
      timestamp: 'ISO timestamp',
    },
  });

  console.log('Hierarchical Coordination:');
  console.log(`- Total agents: ${swarmTopology.data[0].workers.length + swarmTopology.data[0].sub_coordinators.length + 1}`);
  console.log(`- Sub-coordinators: ${swarmTopology.data[0].sub_coordinators.length}`);
  console.log(`- Workers: ${swarmTopology.data[0].workers.length}`);
  console.log(`- Coordination events: ${coordinationEvents.data.length}`);

  // Claude-Flow hierarchical setup
  console.log('\nClaude-Flow Hierarchical Setup:');
  console.log('npx claude-flow@alpha mcp start');
  console.log('// MCP: swarm_init with topology: "hierarchical"');
  console.log('// MCP: agent_spawn with role assignments');

  return { topology: swarmTopology, events: coordinationEvents };
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllCoordinationExamples() {
  console.log('🚀 Running All Agent Coordination Examples\n');
  console.log('='.repeat(70));

  try {
    await agentCommunicationPatterns();
    console.log('='.repeat(70));

    await taskDistributionScenarios();
    console.log('='.repeat(70));

    await consensusBuildingData();
    console.log('='.repeat(70));

    await loadBalancingPatterns();
    console.log('='.repeat(70));

    await faultToleranceScenarios();
    console.log('='.repeat(70));

    await hierarchicalCoordination();
    console.log('='.repeat(70));

    console.log('\n✅ All agent coordination examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error running examples:', error.message);
    throw error;
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllCoordinationExamples().catch(console.error);
}
