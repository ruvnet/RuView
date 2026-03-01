/**
 * Agent Lifecycle Management Examples
 *
 * Demonstrates agent lifecycle patterns including spawning/termination,
 * state synchronization, health checks, recovery patterns, and
 * version migration for dynamic agent systems.
 *
 * Integrates with:
 * - claude-flow: Agent lifecycle hooks and state management
 * - ruv-swarm: Dynamic agent spawning and coordination
 * - Kubernetes: Container orchestration patterns
 */

import { AgenticSynth, createSynth } from '../../dist/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Agent Spawning and Termination
// ============================================================================

/**
 * Generate agent spawning and termination lifecycle events
 */
export async function agentSpawningTermination() {
  console.log('\n🚀 Example 1: Agent Spawning and Termination\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate agent lifecycle events
  const lifecycleEvents = await synth.generateEvents({
    count: 500,
    eventTypes: [
      'agent_spawn_requested',
      'agent_initializing',
      'agent_ready',
      'agent_active',
      'agent_idle',
      'agent_terminating',
      'agent_terminated',
      'agent_failed',
    ],
    schema: {
      event_id: 'UUID',
      agent_id: 'agent-{1-100}',
      event_type: 'one of eventTypes',
      reason: 'spawn reason or termination reason',
      requested_by: 'coordinator | auto_scaler | user | system',
      resource_allocation: {
        cpu_cores: 'number (0.5-8.0)',
        memory_mb: 'number (512-8192)',
        disk_mb: 'number (1024-10240)',
      },
      initialization_config: {
        agent_type: 'worker | coordinator | specialist | observer',
        capabilities: ['array of 1-5 capabilities'],
        priority: 'high | medium | low',
        max_lifetime_minutes: 'number (60-14400) or null',
      },
      state_data_size_mb: 'number (0-1000)',
      startup_time_ms: 'number (100-10000)',
      shutdown_time_ms: 'number (50-5000)',
      exit_code: 'number (0-255) or null',
      timestamp: 'ISO timestamp',
    },
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 86400000), // Last 24 hours
      end: new Date(),
    },
  });

  // Generate agent spawn strategies
  const spawnStrategies = await synth.generateStructured({
    count: 20,
    schema: {
      strategy_id: 'UUID',
      strategy_name: 'descriptive name',
      trigger_type: 'manual | scheduled | load_based | event_driven | predictive',
      conditions: ['array of 2-4 conditions'],
      agent_template: {
        agent_type: 'worker | coordinator | specialist | observer',
        base_config: 'JSON configuration object',
        scaling_limits: {
          min_instances: 'number (1-5)',
          max_instances: 'number (10-100)',
          scale_up_threshold: 'number (0-100)',
          scale_down_threshold: 'number (0-100)',
        },
      },
      spawn_pattern: 'immediate | gradual | burst',
      cooldown_seconds: 'number (30-600)',
      success_rate: 'number (0-100)',
      avg_spawn_time_ms: 'number (500-15000)',
    },
  });

  // Generate resource pool state
  const resourcePool = await synth.generateTimeSeries({
    count: 100,
    interval: '5m',
    metrics: [
      'active_agents',
      'spawning_agents',
      'terminating_agents',
      'failed_spawns',
      'total_cpu_usage',
      'total_memory_mb',
      'available_slots',
    ],
    trend: 'mixed',
  });

  console.log('Agent Lifecycle Analysis:');
  console.log(`- Lifecycle events: ${lifecycleEvents.data.length}`);
  console.log(`- Spawn strategies: ${spawnStrategies.data.length}`);
  console.log(`- Resource pool snapshots: ${resourcePool.data.length}`);

  // Analyze lifecycle events
  const spawnedCount = lifecycleEvents.data.filter(
    (e: any) => e.event_type === 'agent_ready'
  ).length;
  const terminatedCount = lifecycleEvents.data.filter(
    (e: any) => e.event_type === 'agent_terminated'
  ).length;
  const failedCount = lifecycleEvents.data.filter(
    (e: any) => e.event_type === 'agent_failed'
  ).length;

  console.log(`\nLifecycle Statistics:`);
  console.log(`- Successfully spawned: ${spawnedCount}`);
  console.log(`- Terminated: ${terminatedCount}`);
  console.log(`- Failed: ${failedCount} (${((failedCount / lifecycleEvents.data.length) * 100).toFixed(1)}%)`);

  // Calculate average spawn time
  const avgSpawnTime = lifecycleEvents.data
    .filter((e: any) => e.startup_time_ms)
    .reduce((sum: number, e: any) => sum + e.startup_time_ms, 0) / spawnedCount;

  console.log(`- Average spawn time: ${avgSpawnTime.toFixed(0)}ms`);

  // Claude-Flow integration
  console.log('\nClaude-Flow Integration:');
  console.log('npx claude-flow@alpha hooks pre-task --spawn-agents 5');
  console.log('// MCP: agent_spawn with configuration');
  console.log('npx claude-flow@alpha hooks post-task --cleanup-agents true');

  return { lifecycleEvents, spawnStrategies, resourcePool };
}

// ============================================================================
// Example 2: State Synchronization
// ============================================================================

/**
 * Generate state synchronization data for distributed agents
 */
export async function stateSynchronization() {
  console.log('\n🔄 Example 2: State Synchronization\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate agent state snapshots
  const stateSnapshots = await synth.generateStructured({
    count: 500,
    schema: {
      snapshot_id: 'UUID',
      agent_id: 'agent-{1-50}',
      version: 'number (1-1000)',
      state_type: 'full | incremental | checkpoint',
      state_data: {
        memory: {
          short_term: 'JSON object',
          long_term: 'JSON object',
          working_set_size_mb: 'number (1-500)',
        },
        task_queue: ['array of 0-20 task ids'],
        active_connections: ['array of 0-10 agent ids'],
        performance_metrics: {
          tasks_completed: 'number (0-1000)',
          cpu_usage: 'number (0-100)',
          memory_usage: 'number (0-100)',
        },
        custom_state: 'JSON object',
      },
      state_size_bytes: 'number (1024-10485760)',
      compression_ratio: 'number (0.3-1.0)',
      checksum: 'SHA-256 hash',
      created_at: 'ISO timestamp',
    },
  });

  // Generate synchronization events
  const syncEvents = await synth.generateEvents({
    count: 1000,
    eventTypes: [
      'state_saved',
      'state_loaded',
      'state_replicated',
      'state_conflict',
      'state_merged',
      'state_rollback',
    ],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      agent_id: 'agent-{1-50}',
      snapshot_id: 'UUID (from stateSnapshots)',
      target_location: 'local | remote | backup | replica-{1-5}',
      sync_strategy: 'optimistic | pessimistic | eventual | strong',
      duration_ms: 'number (10-5000)',
      data_transferred_mb: 'number (0.1-100)',
      success: 'boolean (95% true)',
      conflict_resolved: 'boolean or null',
      timestamp: 'ISO timestamp',
    },
    distribution: 'poisson',
  });

  // Generate state consistency checks
  const consistencyChecks = await synth.generateStructured({
    count: 100,
    schema: {
      check_id: 'UUID',
      check_type: 'integrity | consistency | replication | divergence',
      agents_checked: ['array of 3-10 agent ids'],
      snapshot_versions: ['array of version numbers'],
      consistency_score: 'number (0-100)',
      divergent_agents: ['array of 0-3 agent ids or empty'],
      anomalies_detected: ['array of 0-2 anomaly descriptions or empty'],
      corrective_action: 'none | resync | repair | rollback | null',
      duration_ms: 'number (100-10000)',
      timestamp: 'ISO timestamp',
    },
  });

  // Generate state synchronization topology
  const syncTopology = await synth.generateStructured({
    count: 1,
    schema: {
      topology_id: 'UUID',
      sync_pattern: 'star | mesh | ring | hierarchical | hybrid',
      nodes: [
        {
          node_id: 'node-{1-10}',
          role: 'primary | replica | backup | cache',
          agents_hosted: ['array of 5-10 agent ids'],
          sync_frequency_seconds: 'number (1-300)',
          replication_lag_ms: 'number (0-1000)',
          storage_capacity_gb: 'number (10-1000)',
          storage_used_gb: 'number (proportional to capacity)',
        },
      ],
      sync_protocol: 'raft | paxos | gossip | two_phase_commit',
      conflict_resolution: 'last_write_wins | merge | vector_clock | manual',
    },
  });

  console.log('State Synchronization Analysis:');
  console.log(`- State snapshots: ${stateSnapshots.data.length}`);
  console.log(`- Sync events: ${syncEvents.data.length}`);
  console.log(`- Consistency checks: ${consistencyChecks.data.length}`);

  // Analyze synchronization success
  const successfulSyncs = syncEvents.data.filter((e: any) => e.success).length;
  const avgSyncTime = syncEvents.data.reduce(
    (sum: number, e: any) => sum + e.duration_ms,
    0
  ) / syncEvents.data.length;

  console.log(`\nSynchronization Metrics:`);
  console.log(`- Success rate: ${((successfulSyncs / syncEvents.data.length) * 100).toFixed(1)}%`);
  console.log(`- Average sync time: ${avgSyncTime.toFixed(0)}ms`);

  // Consistency analysis
  const avgConsistency = consistencyChecks.data.reduce(
    (sum: number, c: any) => sum + c.consistency_score,
    0
  ) / consistencyChecks.data.length;

  console.log(`- Average consistency score: ${avgConsistency.toFixed(1)}/100`);
  console.log(`- Anomalies detected: ${consistencyChecks.data.filter((c: any) => c.anomalies_detected.length > 0).length}`);

  // Integration pattern
  console.log('\nState Sync Integration:');
  console.log('// Store state in distributed storage (Redis, etcd)');
  console.log('await redis.set(`agent:${agentId}:state`, JSON.stringify(state));');
  console.log('// Use claude-flow memory for coordination state');
  console.log('npx claude-flow@alpha hooks session-end --export-state true');

  return { stateSnapshots, syncEvents, consistencyChecks, syncTopology };
}

// ============================================================================
// Example 3: Health Check Scenarios
// ============================================================================

/**
 * Generate health check and monitoring data
 */
export async function healthCheckScenarios() {
  console.log('\n💊 Example 3: Health Check Scenarios\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate health check results
  const healthChecks = await synth.generateEvents({
    count: 1000,
    eventTypes: [
      'health_check_passed',
      'health_check_degraded',
      'health_check_failed',
      'health_check_timeout',
    ],
    schema: {
      check_id: 'UUID',
      agent_id: 'agent-{1-80}',
      event_type: 'one of eventTypes',
      check_type: 'liveness | readiness | startup | deep',
      health_score: 'number (0-100)',
      metrics: {
        response_time_ms: 'number (1-5000)',
        cpu_usage: 'number (0-100)',
        memory_usage: 'number (0-100)',
        active_connections: 'number (0-100)',
        error_rate: 'number (0-100)',
        queue_depth: 'number (0-1000)',
      },
      issues_detected: ['array of 0-3 issue descriptions or empty'],
      recovery_actions: ['array of 0-2 actions or empty'],
      timestamp: 'ISO timestamp',
    },
    distribution: 'poisson',
  });

  // Generate health monitoring configuration
  const healthConfigs = await synth.generateStructured({
    count: 50,
    schema: {
      agent_id: 'agent-{1-50}',
      liveness_probe: {
        enabled: 'boolean',
        interval_seconds: 'number (5-60)',
        timeout_seconds: 'number (1-10)',
        failure_threshold: 'number (3-10)',
        success_threshold: 'number (1-3)',
      },
      readiness_probe: {
        enabled: 'boolean',
        interval_seconds: 'number (5-30)',
        timeout_seconds: 'number (1-10)',
        failure_threshold: 'number (3-10)',
      },
      health_thresholds: {
        cpu_warning: 'number (70-85)',
        cpu_critical: 'number (85-95)',
        memory_warning: 'number (70-85)',
        memory_critical: 'number (85-95)',
        error_rate_threshold: 'number (5-20)',
      },
      auto_recovery_enabled: 'boolean',
      health_status: 'healthy | degraded | unhealthy | unknown',
    },
  });

  // Generate health time series
  const healthTimeSeries = await synth.generateTimeSeries({
    count: 200,
    interval: '1m',
    metrics: [
      'healthy_agents',
      'degraded_agents',
      'unhealthy_agents',
      'avg_health_score',
      'failed_checks',
      'recovery_actions',
    ],
    trend: 'stable',
  });

  // Generate auto-healing actions
  const healingActions = await synth.generateStructured({
    count: 50,
    schema: {
      action_id: 'UUID',
      agent_id: 'agent-{1-80}',
      trigger_check_id: 'UUID (from healthChecks)',
      action_type: 'restart | reset_state | scale_resources | failover | isolate',
      severity: 'low | medium | high | critical',
      executed_at: 'ISO timestamp',
      duration_ms: 'number (100-30000)',
      success: 'boolean (85% true)',
      health_before: 'number (0-50)',
      health_after: 'number (51-100) or same as health_before',
      side_effects: ['array of 0-2 side effects or empty'],
    },
  });

  console.log('Health Check Analysis:');
  console.log(`- Health checks: ${healthChecks.data.length}`);
  console.log(`- Agent configs: ${healthConfigs.data.length}`);
  console.log(`- Time series points: ${healthTimeSeries.data.length}`);
  console.log(`- Healing actions: ${healingActions.data.length}`);

  // Analyze health check results
  const passedChecks = healthChecks.data.filter(
    (c: any) => c.event_type === 'health_check_passed'
  ).length;
  const failedChecks = healthChecks.data.filter(
    (c: any) => c.event_type === 'health_check_failed'
  ).length;

  console.log(`\nHealth Statistics:`);
  console.log(`- Passed: ${passedChecks} (${((passedChecks / healthChecks.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Failed: ${failedChecks} (${((failedChecks / healthChecks.data.length) * 100).toFixed(1)}%)`);

  // Auto-healing effectiveness
  const successfulHealing = healingActions.data.filter((a: any) => a.success).length;
  const avgHealthImprovement = healingActions.data
    .filter((a: any) => a.success)
    .reduce((sum: number, a: any) => sum + (a.health_after - a.health_before), 0) / successfulHealing;

  console.log(`\nAuto-Healing:`);
  console.log(`- Actions taken: ${healingActions.data.length}`);
  console.log(`- Success rate: ${((successfulHealing / healingActions.data.length) * 100).toFixed(1)}%`);
  console.log(`- Average health improvement: +${avgHealthImprovement.toFixed(1)}`);

  // Kubernetes health checks
  console.log('\nKubernetes Integration:');
  console.log('// Define health probes in pod spec');
  console.log('livenessProbe: { httpGet: { path: "/health", port: 8080 } }');
  console.log('readinessProbe: { httpGet: { path: "/ready", port: 8080 } }');

  return { healthChecks, healthConfigs, healthTimeSeries, healingActions };
}

// ============================================================================
// Example 4: Recovery Patterns
// ============================================================================

/**
 * Generate failure recovery pattern data
 */
export async function recoveryPatterns() {
  console.log('\n🔧 Example 4: Recovery Patterns\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate failure scenarios
  const failures = await synth.generateStructured({
    count: 100,
    schema: {
      failure_id: 'UUID',
      agent_id: 'agent-{1-60}',
      failure_type: 'crash | hang | memory_leak | resource_exhaustion | network_partition | data_corruption',
      severity: 'critical | high | medium | low',
      impact_scope: 'single_agent | cluster | region | global',
      affected_tasks: ['array of 0-20 task ids'],
      data_at_risk: 'boolean',
      detected_at: 'ISO timestamp',
      detection_method: 'health_check | monitoring | user_report | self_reported',
      mttr_seconds: 'number (10-3600)',
      mttd_seconds: 'number (1-600)',
    },
  });

  // Generate recovery strategies
  const recoveryStrategies = await synth.generateStructured({
    count: failures.data.length,
    schema: {
      strategy_id: 'UUID',
      failure_id: 'UUID (from failures)',
      strategy_type: 'restart | failover | rollback | rebuild | manual_intervention',
      phases: [
        {
          phase_name: 'detection | isolation | recovery | verification | cleanup',
          actions: ['array of 2-5 actions'],
          duration_ms: 'number (100-60000)',
          success: 'boolean',
        },
      ],
      recovery_time_objective_seconds: 'number (60-3600)',
      recovery_point_objective_seconds: 'number (0-1800)',
      actual_recovery_time_seconds: 'number',
      data_loss_amount: 'number (0-1000 MB) or 0',
      automatic: 'boolean (80% true)',
      success: 'boolean (90% true)',
      lessons_learned: ['array of 2-4 lessons'],
    },
  });

  // Generate circuit breaker states
  const circuitBreakers = await synth.generateTimeSeries({
    count: 150,
    interval: '2m',
    metrics: [
      'closed_circuits',
      'open_circuits',
      'half_open_circuits',
      'total_requests',
      'failed_requests',
      'trips_per_interval',
    ],
    trend: 'mixed',
  });

  // Generate backup and restore operations
  const backupOperations = await synth.generateStructured({
    count: 200,
    schema: {
      operation_id: 'UUID',
      operation_type: 'backup | restore | verify | cleanup',
      agent_id: 'agent-{1-60}',
      backup_id: 'backup-UUID',
      data_size_mb: 'number (10-5000)',
      duration_ms: 'number (1000-300000)',
      storage_location: 'local | s3 | gcs | azure_blob',
      compression_enabled: 'boolean',
      encryption_enabled: 'boolean',
      success: 'boolean (95% true)',
      timestamp: 'ISO timestamp',
    },
  });

  console.log('Recovery Pattern Analysis:');
  console.log(`- Failures recorded: ${failures.data.length}`);
  console.log(`- Recovery strategies: ${recoveryStrategies.data.length}`);
  console.log(`- Circuit breaker metrics: ${circuitBreakers.data.length}`);
  console.log(`- Backup operations: ${backupOperations.data.length}`);

  // Analyze recovery effectiveness
  const successfulRecoveries = recoveryStrategies.data.filter((s: any) => s.success).length;
  const automaticRecoveries = recoveryStrategies.data.filter((s: any) => s.automatic).length;

  console.log(`\nRecovery Effectiveness:`);
  console.log(`- Success rate: ${((successfulRecoveries / recoveryStrategies.data.length) * 100).toFixed(1)}%`);
  console.log(`- Automatic recoveries: ${((automaticRecoveries / recoveryStrategies.data.length) * 100).toFixed(1)}%`);

  // Calculate average recovery times
  const avgMTTR = failures.data.reduce((sum: number, f: any) => sum + f.mttr_seconds, 0) / failures.data.length;
  const avgMTTD = failures.data.reduce((sum: number, f: any) => sum + f.mttd_seconds, 0) / failures.data.length;

  console.log(`- Average MTTR: ${avgMTTR.toFixed(0)} seconds`);
  console.log(`- Average MTTD: ${avgMTTD.toFixed(0)} seconds`);

  // Data loss analysis
  const dataLossIncidents = recoveryStrategies.data.filter(
    (s: any) => s.data_loss_amount > 0
  ).length;

  console.log(`\nData Protection:`);
  console.log(`- Incidents with data loss: ${dataLossIncidents} (${((dataLossIncidents / recoveryStrategies.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Successful backups: ${backupOperations.data.filter((b: any) => b.operation_type === 'backup' && b.success).length}`);

  console.log('\nRecovery Pattern Implementation:');
  console.log('// Implement circuit breaker pattern');
  console.log('// Use claude-flow hooks for automatic recovery');
  console.log('npx claude-flow@alpha hooks enable --type "error-recovery"');

  return { failures, recoveryStrategies, circuitBreakers, backupOperations };
}

// ============================================================================
// Example 5: Version Migration
// ============================================================================

/**
 * Generate agent version migration data
 */
export async function versionMigration() {
  console.log('\n🔄 Example 5: Version Migration\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate version information
  const versions = await synth.generateStructured({
    count: 10,
    schema: {
      version_id: 'UUID',
      version_number: 'semantic version (e.g., 2.5.1)',
      release_date: 'ISO timestamp',
      changes: {
        features: ['array of 2-5 new features'],
        improvements: ['array of 1-4 improvements'],
        bug_fixes: ['array of 2-6 bug fixes'],
        breaking_changes: ['array of 0-2 breaking changes or empty'],
      },
      compatibility: {
        backward_compatible: 'boolean',
        migration_required: 'boolean',
        rollback_supported: 'boolean',
      },
      deployment_strategy: 'blue_green | rolling | canary | recreate',
    },
  });

  // Generate migration operations
  const migrations = await synth.generateStructured({
    count: 50,
    schema: {
      migration_id: 'UUID',
      from_version: 'semantic version',
      to_version: 'semantic version',
      agent_id: 'agent-{1-100}',
      migration_type: 'in_place | side_by_side | recreate',
      migration_steps: [
        {
          step_id: 'UUID',
          step_name: 'descriptive step name',
          step_type: 'backup | stop | migrate_data | update_code | restart | verify',
          duration_ms: 'number (100-60000)',
          success: 'boolean',
          rollback_supported: 'boolean',
        },
      ],
      downtime_ms: 'number (0-120000)',
      data_migrated_mb: 'number (0-1000)',
      overall_status: 'in_progress | completed | failed | rolled_back',
      started_at: 'ISO timestamp',
      completed_at: 'ISO timestamp or null',
    },
    constraints: [
      'Migration should have 4-8 steps',
      '85% of migrations should complete successfully',
      'Failed migrations should support rollback',
    ],
  });

  // Generate canary deployment data
  const canaryDeployments = await synth.generateStructured({
    count: 15,
    schema: {
      deployment_id: 'UUID',
      version: 'semantic version',
      canary_percentage: 'number (5-50)',
      canary_agents: ['array of agent ids'],
      stable_agents: ['array of agent ids'],
      metrics_comparison: {
        canary_error_rate: 'number (0-10)',
        stable_error_rate: 'number (0-10)',
        canary_latency_p99: 'number (10-1000)',
        stable_latency_p99: 'number (10-1000)',
        canary_throughput: 'number (100-10000)',
        stable_throughput: 'number (100-10000)',
      },
      decision: 'promote | hold | rollback',
      decision_reason: 'detailed reason',
      monitoring_duration_minutes: 'number (30-1440)',
    },
  });

  // Generate rollback events
  const rollbacks = await synth.generateStructured({
    count: 10,
    schema: {
      rollback_id: 'UUID',
      migration_id: 'UUID (from migrations)',
      trigger: 'high_error_rate | performance_degradation | manual | data_inconsistency',
      rollback_strategy: 'automated | manual',
      affected_agents: ['array of 5-30 agent ids'],
      rollback_duration_ms: 'number (5000-300000)',
      data_reverted_mb: 'number (0-1000)',
      success: 'boolean (95% true)',
      timestamp: 'ISO timestamp',
    },
  });

  console.log('Version Migration Analysis:');
  console.log(`- Versions tracked: ${versions.data.length}`);
  console.log(`- Migrations executed: ${migrations.data.length}`);
  console.log(`- Canary deployments: ${canaryDeployments.data.length}`);
  console.log(`- Rollbacks: ${rollbacks.data.length}`);

  // Analyze migration success
  const successfulMigrations = migrations.data.filter(
    (m: any) => m.overall_status === 'completed'
  ).length;
  const failedMigrations = migrations.data.filter(
    (m: any) => m.overall_status === 'failed'
  ).length;

  console.log(`\nMigration Success Rate:`);
  console.log(`- Completed: ${successfulMigrations} (${((successfulMigrations / migrations.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Failed: ${failedMigrations}`);
  console.log(`- Rolled back: ${migrations.data.filter((m: any) => m.overall_status === 'rolled_back').length}`);

  // Analyze downtime
  const avgDowntime = migrations.data
    .filter((m: any) => m.overall_status === 'completed')
    .reduce((sum: number, m: any) => sum + m.downtime_ms, 0) / successfulMigrations;
  const zeroDowntime = migrations.data.filter((m: any) => m.downtime_ms === 0).length;

  console.log(`\nDowntime Analysis:`);
  console.log(`- Average downtime: ${(avgDowntime / 1000).toFixed(1)} seconds`);
  console.log(`- Zero-downtime migrations: ${zeroDowntime} (${((zeroDowntime / migrations.data.length) * 100).toFixed(1)}%)`);

  // Canary deployment decisions
  const promoted = canaryDeployments.data.filter((c: any) => c.decision === 'promote').length;
  const rolledBack = canaryDeployments.data.filter((c: any) => c.decision === 'rollback').length;

  console.log(`\nCanary Deployment Decisions:`);
  console.log(`- Promoted: ${promoted} (${((promoted / canaryDeployments.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Rolled back: ${rolledBack} (${((rolledBack / canaryDeployments.data.length) * 100).toFixed(1)}%)`);

  console.log('\nDeployment Integration:');
  console.log('// Blue-green deployment with Kubernetes');
  console.log('kubectl apply -f deployment-v2.yaml');
  console.log('kubectl set image deployment/agents agent=image:v2');
  console.log('// Monitor and rollback if needed');

  return { versions, migrations, canaryDeployments, rollbacks };
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllLifecycleExamples() {
  console.log('🚀 Running All Agent Lifecycle Examples\n');
  console.log('='.repeat(70));

  try {
    await agentSpawningTermination();
    console.log('='.repeat(70));

    await stateSynchronization();
    console.log('='.repeat(70));

    await healthCheckScenarios();
    console.log('='.repeat(70));

    await recoveryPatterns();
    console.log('='.repeat(70));

    await versionMigration();
    console.log('='.repeat(70));

    console.log('\n✅ All agent lifecycle examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error running examples:', error.message);
    throw error;
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllLifecycleExamples().catch(console.error);
}
