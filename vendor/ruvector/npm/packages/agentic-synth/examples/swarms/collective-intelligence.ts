/**
 * Collective Intelligence Examples
 *
 * Demonstrates swarm intelligence patterns including collaborative
 * problem-solving, knowledge sharing, emergent behavior simulation,
 * voting and consensus mechanisms, and reputation systems.
 *
 * Integrates with:
 * - claude-flow: Neural pattern recognition and learning
 * - ruv-swarm: Collective intelligence coordination
 * - AgenticDB: Distributed knowledge storage
 */

import { AgenticSynth, createSynth } from '../../dist/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Collaborative Problem-Solving
// ============================================================================

/**
 * Generate collaborative problem-solving session data
 */
export async function collaborativeProblemSolving() {
  console.log('\n🧩 Example 1: Collaborative Problem-Solving\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate problem-solving sessions
  const sessions = await synth.generateStructured({
    count: 30,
    schema: {
      session_id: 'UUID',
      problem: {
        id: 'UUID',
        title: 'complex problem title',
        description: 'detailed problem description',
        domain: 'software_architecture | data_analysis | optimization | debugging | design',
        complexity: 'number (1-10)',
        estimated_time_hours: 'number (1-48)',
      },
      participating_agents: [
        {
          agent_id: 'agent-{1-20}',
          role: 'researcher | analyst | implementer | reviewer | facilitator',
          expertise: ['array of 2-4 expertise areas'],
          contribution_count: 'number (0-50)',
          quality_score: 'number (0-100)',
        },
      ],
      solution_proposals: [
        {
          proposal_id: 'UUID',
          proposer_agent_id: 'agent id from participants',
          approach: 'detailed solution approach',
          estimated_effort: 'number (1-40 hours)',
          pros: ['array of 2-4 advantages'],
          cons: ['array of 1-3 disadvantages'],
          votes_for: 'number (0-20)',
          votes_against: 'number (0-20)',
          feasibility_score: 'number (0-100)',
        },
      ],
      collaboration_events: [
        {
          event_id: 'UUID',
          event_type: 'proposal | critique | enhancement | agreement | disagreement',
          agent_id: 'agent id',
          content: 'event description',
          timestamp: 'ISO timestamp',
          references: ['array of related event_ids or empty'],
        },
      ],
      selected_solution_id: 'UUID (from proposals)',
      outcome: 'successful | partial | failed | ongoing',
      actual_time_hours: 'number',
      quality_metrics: {
        solution_quality: 'number (0-100)',
        collaboration_efficiency: 'number (0-100)',
        innovation_score: 'number (0-100)',
        consensus_level: 'number (0-100)',
      },
      started_at: 'ISO timestamp',
      completed_at: 'ISO timestamp or null',
    },
    constraints: [
      'Sessions should have 3-8 participating agents',
      'Should have 2-5 solution proposals per session',
      '70% of sessions should be successful',
      'Higher agent expertise should correlate with better outcomes',
    ],
  });

  // Analyze collaborative sessions
  const successfulSessions = sessions.data.filter((s: any) => s.outcome === 'successful');
  const avgParticipants = sessions.data.reduce(
    (sum: number, s: any) => sum + s.participating_agents.length,
    0
  ) / sessions.data.length;

  console.log('Collaborative Problem-Solving Analysis:');
  console.log(`- Total sessions: ${sessions.data.length}`);
  console.log(`- Successful: ${successfulSessions.length} (${((successfulSessions.length / sessions.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Average participants: ${avgParticipants.toFixed(1)}`);

  // Calculate quality metrics
  const avgQuality = successfulSessions.reduce(
    (sum: number, s: any) => sum + s.quality_metrics.solution_quality,
    0
  ) / successfulSessions.length;
  const avgInnovation = successfulSessions.reduce(
    (sum: number, s: any) => sum + s.quality_metrics.innovation_score,
    0
  ) / successfulSessions.length;

  console.log(`\nQuality Metrics:`);
  console.log(`- Average solution quality: ${avgQuality.toFixed(1)}/100`);
  console.log(`- Average innovation score: ${avgInnovation.toFixed(1)}/100`);

  // Domain distribution
  const domains = new Map<string, number>();
  sessions.data.forEach((s: any) => {
    domains.set(s.problem.domain, (domains.get(s.problem.domain) || 0) + 1);
  });

  console.log('\nProblem Domain Distribution:');
  domains.forEach((count, domain) => {
    console.log(`- ${domain}: ${count}`);
  });

  // Claude-Flow integration
  console.log('\nClaude-Flow Neural Integration:');
  console.log('npx claude-flow@alpha hooks neural-train --pattern "collaboration"');
  console.log('// Store successful patterns in AgenticDB for learning');

  return sessions;
}

// ============================================================================
// Example 2: Knowledge Sharing Patterns
// ============================================================================

/**
 * Generate knowledge sharing and transfer data
 */
export async function knowledgeSharingPatterns() {
  console.log('\n📚 Example 2: Knowledge Sharing Patterns\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate knowledge base entries
  const knowledgeBase = await synth.generateStructured({
    count: 200,
    schema: {
      entry_id: 'UUID',
      category: 'best_practice | lesson_learned | solution_pattern | troubleshooting | architecture',
      title: 'descriptive title',
      content: 'detailed knowledge content (2-4 paragraphs)',
      tags: ['array of 3-6 tags'],
      author_agent_id: 'agent-{1-50}',
      contributors: ['array of 0-5 agent ids'],
      related_entries: ['array of 0-3 entry_ids or empty'],
      quality_rating: 'number (0-5.0)',
      usefulness_count: 'number (0-100)',
      view_count: 'number (0-1000)',
      created_at: 'ISO timestamp',
      updated_at: 'ISO timestamp',
    },
  });

  // Generate knowledge transfer events
  const transferEvents = await synth.generateEvents({
    count: 500,
    eventTypes: [
      'knowledge_created',
      'knowledge_shared',
      'knowledge_applied',
      'knowledge_validated',
      'knowledge_updated',
    ],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      knowledge_entry_id: 'UUID (from knowledgeBase)',
      source_agent_id: 'agent-{1-50}',
      target_agent_id: 'agent-{1-50} or null',
      context: 'description of usage context',
      effectiveness: 'number (0-100)',
      timestamp: 'ISO timestamp',
    },
    distribution: 'uniform',
  });

  // Generate agent knowledge profiles
  const agentProfiles = await synth.generateStructured({
    count: 50,
    schema: {
      agent_id: 'agent-{1-50}',
      expertise_areas: ['array of 3-8 expertise domains'],
      knowledge_contributed: 'number (0-20)',
      knowledge_consumed: 'number (0-100)',
      sharing_frequency: 'high | medium | low',
      learning_rate: 'number (0-1.0)',
      collaboration_score: 'number (0-100)',
      influence_score: 'number (0-100)',
    },
  });

  console.log('Knowledge Sharing Analysis:');
  console.log(`- Knowledge entries: ${knowledgeBase.data.length}`);
  console.log(`- Transfer events: ${transferEvents.data.length}`);
  console.log(`- Agent profiles: ${agentProfiles.data.length}`);

  // Analyze knowledge distribution
  const categoryCount = new Map<string, number>();
  knowledgeBase.data.forEach((entry: any) => {
    categoryCount.set(entry.category, (categoryCount.get(entry.category) || 0) + 1);
  });

  console.log('\nKnowledge Categories:');
  categoryCount.forEach((count, category) => {
    console.log(`- ${category}: ${count}`);
  });

  // Calculate sharing metrics
  const avgRating = knowledgeBase.data.reduce(
    (sum: number, entry: any) => sum + entry.quality_rating,
    0
  ) / knowledgeBase.data.length;

  console.log(`\nSharing Metrics:`);
  console.log(`- Average quality rating: ${avgRating.toFixed(2)}/5.0`);
  console.log(`- High sharers: ${agentProfiles.data.filter((a: any) => a.sharing_frequency === 'high').length}`);

  // AgenticDB integration
  console.log('\nAgenticDB Integration:');
  console.log('// Store knowledge embeddings for semantic search');
  console.log('await agenticDB.storeVector({ text: knowledge.content, metadata: {...} });');
  console.log('// Query similar knowledge: agenticDB.search({ query, topK: 10 });');

  return { knowledgeBase, transferEvents, agentProfiles };
}

// ============================================================================
// Example 3: Emergent Behavior Simulation
// ============================================================================

/**
 * Generate emergent behavior patterns in swarm systems
 */
export async function emergentBehaviorSimulation() {
  console.log('\n🌀 Example 3: Emergent Behavior Simulation\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate swarm state evolution
  const swarmStates = await synth.generateTimeSeries({
    count: 100,
    interval: '1m',
    metrics: [
      'agent_count',
      'cluster_count',
      'avg_cluster_size',
      'coordination_level',
      'task_completion_rate',
      'communication_density',
      'adaptation_score',
    ],
    trend: 'up',
    seasonality: false,
  });

  // Generate agent interactions
  const interactions = await synth.generateStructured({
    count: 1000,
    schema: {
      interaction_id: 'UUID',
      agent_a_id: 'agent-{1-100}',
      agent_b_id: 'agent-{1-100}',
      interaction_type: 'cooperation | competition | information_exchange | resource_sharing | conflict_resolution',
      context: 'brief description of interaction',
      outcome: 'positive | negative | neutral',
      influence_on_behavior: 'number (-1.0 to 1.0)',
      timestamp: 'ISO timestamp',
    },
    constraints: [
      'agent_a_id should be different from agent_b_id',
      '70% of interactions should be positive',
      'Cooperation should be more common than competition',
    ],
  });

  // Generate emergent patterns
  const emergentPatterns = await synth.generateStructured({
    count: 20,
    schema: {
      pattern_id: 'UUID',
      pattern_type: 'clustering | leader_emergence | task_specialization | self_organization | collective_decision',
      description: 'detailed pattern description',
      participants: ['array of 5-30 agent ids'],
      emergence_time: 'ISO timestamp',
      stability_score: 'number (0-100)',
      efficiency_gain: 'number (0-50 percent)',
      conditions: ['array of 2-4 conditions that led to emergence'],
      observed_behaviors: ['array of 3-6 behaviors'],
    },
  });

  // Generate agent behavior evolution
  const behaviorEvolution = await synth.generateStructured({
    count: 300,
    schema: {
      agent_id: 'agent-{1-100}',
      timestamp: 'ISO timestamp',
      behavior_traits: {
        cooperation_tendency: 'number (0-1.0)',
        exploration_vs_exploitation: 'number (0-1.0)',
        risk_tolerance: 'number (0-1.0)',
        social_connectivity: 'number (0-1.0)',
        task_focus: 'generalist | specialist',
      },
      influenced_by: ['array of 0-3 agent ids or empty'],
      role_in_swarm: 'leader | follower | bridge | isolate | specialist',
      performance_score: 'number (0-100)',
    },
  });

  console.log('Emergent Behavior Analysis:');
  console.log(`- Swarm state snapshots: ${swarmStates.data.length}`);
  console.log(`- Agent interactions: ${interactions.data.length}`);
  console.log(`- Emergent patterns: ${emergentPatterns.data.length}`);
  console.log(`- Behavior evolution points: ${behaviorEvolution.data.length}`);

  // Analyze interaction types
  const interactionTypes = new Map<string, number>();
  interactions.data.forEach((i: any) => {
    interactionTypes.set(i.interaction_type, (interactionTypes.get(i.interaction_type) || 0) + 1);
  });

  console.log('\nInteraction Distribution:');
  interactionTypes.forEach((count, type) => {
    console.log(`- ${type}: ${count} (${((count / interactions.data.length) * 100).toFixed(1)}%)`);
  });

  // Pattern analysis
  console.log('\nEmergent Patterns:');
  emergentPatterns.data.forEach((pattern: any) => {
    console.log(`- ${pattern.pattern_type}: ${pattern.participants.length} participants, stability ${pattern.stability_score}/100`);
  });

  // Ruv-Swarm collective intelligence
  console.log('\nRuv-Swarm Collective Intelligence:');
  console.log('npx ruv-swarm mcp start');
  console.log('// MCP: neural_patterns to analyze emergent behaviors');

  return { swarmStates, interactions, emergentPatterns, behaviorEvolution };
}

// ============================================================================
// Example 4: Voting and Consensus Data
// ============================================================================

/**
 * Generate voting and consensus mechanism data
 */
export async function votingAndConsensusData() {
  console.log('\n🗳️  Example 4: Voting and Consensus Mechanisms\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate voting sessions
  const votingSessions = await synth.generateStructured({
    count: 50,
    schema: {
      session_id: 'UUID',
      voting_method: 'simple_majority | qualified_majority | unanimous | weighted | ranked_choice',
      topic: {
        id: 'UUID',
        title: 'decision topic',
        description: 'topic description',
        importance: 'critical | high | medium | low',
      },
      eligible_voters: ['array of 10-50 agent ids'],
      votes: [
        {
          voter_id: 'agent id from eligible_voters',
          vote_value: 'for | against | abstain',
          weight: 'number (1.0-5.0)',
          reasoning: 'brief explanation',
          confidence: 'number (0-100)',
          cast_at: 'ISO timestamp',
        },
      ],
      result: {
        decision: 'accepted | rejected | tie',
        votes_for: 'number',
        votes_against: 'number',
        votes_abstain: 'number',
        weighted_score: 'number',
        participation_rate: 'number (0-100)',
        consensus_level: 'number (0-100)',
      },
      duration_minutes: 'number (5-180)',
      started_at: 'ISO timestamp',
      completed_at: 'ISO timestamp',
    },
    constraints: [
      'Votes array should match eligible_voters count',
      'Critical topics should have higher participation',
      'Weighted votes should affect weighted_score',
    ],
  });

  // Generate consensus mechanisms
  const consensusMechanisms = await synth.generateStructured({
    count: 100,
    schema: {
      mechanism_id: 'UUID',
      mechanism_type: 'deliberative | aggregative | iterative | delegative',
      session_id: 'UUID (from votingSessions)',
      rounds: [
        {
          round_number: 'number (1-5)',
          proposals: ['array of 2-5 proposal descriptions'],
          discussions: 'number (10-100)',
          opinion_shifts: 'number (0-20)',
          convergence_score: 'number (0-100)',
          duration_minutes: 'number (5-60)',
        },
      ],
      final_consensus: 'strong | moderate | weak | none',
      compromises_made: 'number (0-5)',
      dissenting_opinions: ['array of 0-3 dissenting viewpoints or empty'],
    },
  });

  // Generate agent voting behavior
  const votingBehavior = await synth.generateStructured({
    count: 200,
    schema: {
      agent_id: 'agent-{1-50}',
      total_votes: 'number (0-50)',
      voting_pattern: 'consistent | moderate | swing',
      influence_level: 'high | medium | low',
      consensus_seeking: 'number (0-100)',
      independence_score: 'number (0-100)',
      expertise_alignment: 'number (0-100)',
    },
  });

  console.log('Voting and Consensus Analysis:');
  console.log(`- Voting sessions: ${votingSessions.data.length}`);
  console.log(`- Consensus mechanisms: ${consensusMechanisms.data.length}`);
  console.log(`- Agent behaviors: ${votingBehavior.data.length}`);

  // Analyze voting outcomes
  const acceptedCount = votingSessions.data.filter(
    (s: any) => s.result.decision === 'accepted'
  ).length;
  const avgParticipation = votingSessions.data.reduce(
    (sum: number, s: any) => sum + s.result.participation_rate,
    0
  ) / votingSessions.data.length;

  console.log(`\nVoting Outcomes:`);
  console.log(`- Accepted: ${acceptedCount} (${((acceptedCount / votingSessions.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Average participation: ${avgParticipation.toFixed(1)}%`);

  // Consensus strength
  const strongConsensus = consensusMechanisms.data.filter(
    (m: any) => m.final_consensus === 'strong'
  ).length;

  console.log(`\nConsensus Quality:`);
  console.log(`- Strong consensus: ${strongConsensus} (${((strongConsensus / consensusMechanisms.data.length) * 100).toFixed(1)}%)`);

  return { votingSessions, consensusMechanisms, votingBehavior };
}

// ============================================================================
// Example 5: Reputation Systems
// ============================================================================

/**
 * Generate reputation and trust system data
 */
export async function reputationSystems() {
  console.log('\n⭐ Example 5: Reputation and Trust Systems\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate agent reputation profiles
  const reputationProfiles = await synth.generateStructured({
    count: 100,
    schema: {
      agent_id: 'agent-{1-100}',
      overall_reputation: 'number (0-100)',
      reputation_components: {
        reliability: 'number (0-100)',
        expertise: 'number (0-100)',
        collaboration: 'number (0-100)',
        responsiveness: 'number (0-100)',
        quality: 'number (0-100)',
      },
      trust_score: 'number (0-100)',
      endorsements: 'number (0-50)',
      negative_feedback: 'number (0-20)',
      tasks_completed: 'number (0-1000)',
      success_rate: 'number (0-100)',
      tenure_days: 'number (1-730)',
      reputation_trend: 'rising | stable | declining',
      badges: ['array of 0-5 achievement badges or empty'],
    },
    constraints: [
      'Overall reputation should correlate with component scores',
      'Higher success rate should correlate with higher reputation',
      'Trust score should factor in tenure and feedback',
    ],
  });

  // Generate reputation events
  const reputationEvents = await synth.generateEvents({
    count: 500,
    eventTypes: [
      'endorsement_received',
      'feedback_positive',
      'feedback_negative',
      'task_success',
      'task_failure',
      'badge_earned',
      'collaboration_rated',
    ],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      subject_agent_id: 'agent-{1-100}',
      evaluator_agent_id: 'agent-{1-100} or null',
      impact: 'number (-10 to +10)',
      context: 'brief description',
      evidence: 'supporting evidence description or null',
      timestamp: 'ISO timestamp',
    },
    distribution: 'poisson',
  });

  // Generate trust relationships
  const trustRelationships = await synth.generateStructured({
    count: 300,
    schema: {
      relationship_id: 'UUID',
      trustor_agent_id: 'agent-{1-100}',
      trustee_agent_id: 'agent-{1-100}',
      trust_level: 'number (0-100)',
      relationship_type: 'direct | transitive | institutional',
      interaction_count: 'number (1-100)',
      successful_interactions: 'number (proportional to interaction_count)',
      last_interaction: 'ISO timestamp',
      trust_evolution: 'building | established | declining',
    },
    constraints: [
      'trustor_agent_id should be different from trustee_agent_id',
      'Trust level should correlate with success rate',
      'More interactions should increase trust stability',
    ],
  });

  // Generate reputation decay and recovery
  const reputationChanges = await synth.generateTimeSeries({
    count: 200,
    interval: '1d',
    metrics: [
      'avg_reputation',
      'reputation_variance',
      'positive_events',
      'negative_events',
      'trust_network_density',
      'endorsement_rate',
    ],
    trend: 'stable',
  });

  console.log('Reputation System Analysis:');
  console.log(`- Agent profiles: ${reputationProfiles.data.length}`);
  console.log(`- Reputation events: ${reputationEvents.data.length}`);
  console.log(`- Trust relationships: ${trustRelationships.data.length}`);
  console.log(`- Time series points: ${reputationChanges.data.length}`);

  // Analyze reputation distribution
  const highReputation = reputationProfiles.data.filter(
    (p: any) => p.overall_reputation >= 80
  ).length;
  const lowReputation = reputationProfiles.data.filter(
    (p: any) => p.overall_reputation < 40
  ).length;

  console.log(`\nReputation Distribution:`);
  console.log(`- High reputation (≥80): ${highReputation} (${((highReputation / reputationProfiles.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Low reputation (<40): ${lowReputation} (${((lowReputation / reputationProfiles.data.length) * 100).toFixed(1)}%)`);

  // Trust network analysis
  const avgTrustLevel = trustRelationships.data.reduce(
    (sum: number, r: any) => sum + r.trust_level,
    0
  ) / trustRelationships.data.length;

  console.log(`\nTrust Network:`);
  console.log(`- Average trust level: ${avgTrustLevel.toFixed(1)}/100`);
  console.log(`- Established relationships: ${trustRelationships.data.filter((r: any) => r.trust_evolution === 'established').length}`);

  // Integration with reputation tracking
  console.log('\nReputation Tracking Integration:');
  console.log('// Store reputation events in AgenticDB');
  console.log('// Use claude-flow hooks to update reputation after tasks');
  console.log('npx claude-flow@alpha hooks post-task --update-reputation true');

  return { reputationProfiles, reputationEvents, trustRelationships, reputationChanges };
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllCollectiveIntelligenceExamples() {
  console.log('🚀 Running All Collective Intelligence Examples\n');
  console.log('='.repeat(70));

  try {
    await collaborativeProblemSolving();
    console.log('='.repeat(70));

    await knowledgeSharingPatterns();
    console.log('='.repeat(70));

    await emergentBehaviorSimulation();
    console.log('='.repeat(70));

    await votingAndConsensusData();
    console.log('='.repeat(70));

    await reputationSystems();
    console.log('='.repeat(70));

    console.log('\n✅ All collective intelligence examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error running examples:', error.message);
    throw error;
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllCollectiveIntelligenceExamples().catch(console.error);
}
