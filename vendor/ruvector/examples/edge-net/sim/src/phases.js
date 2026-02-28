/**
 * Phase Management for Network Lifecycle
 * Tracks and validates phase transitions
 */

export class PhaseManager {
  constructor() {
    this.currentPhase = 'genesis';
    this.phaseHistory = [];
    this.phaseMetrics = new Map();

    this.initializePhases();
  }

  /**
   * Initialize phase definitions
   */
  initializePhases() {
    this.phases = {
      genesis: {
        name: 'Genesis Phase',
        nodeRange: [0, 10000],
        description: 'Network bootstrap with genesis nodes',
        features: [
          'Genesis node initialization',
          'Early adopter multiplier (10x)',
          'Network bootstrap',
          'Initial task distribution',
          'Security learning initialization',
        ],
        validations: [
          { metric: 'genesisNodesActive', min: 1, description: 'At least 1 genesis node active' },
          { metric: 'earlyMultiplier', min: 5.0, description: 'High early adopter multiplier' },
        ],
      },
      transition: {
        name: 'Transition Phase',
        nodeRange: [10000, 50000],
        description: 'Genesis sunset preparation',
        features: [
          'Genesis node connection limiting',
          'Network resilience testing',
          'Task routing optimization',
          'Economic sustainability threshold',
          'Topology self-organization',
        ],
        validations: [
          { metric: 'genesisConnectionLimit', max: 500, description: 'Genesis connections limited' },
          { metric: 'networkResilience', min: 0.7, description: 'Network resilient without full genesis' },
          { metric: 'taskRoutingSuccess', min: 0.85, description: 'Efficient task routing' },
        ],
      },
      maturity: {
        name: 'Maturity Phase',
        nodeRange: [50000, 100000],
        description: 'Genesis read-only mode',
        features: [
          'Genesis nodes read-only',
          'Full network self-sustenance',
          'Economic health monitoring',
          'Security threat response',
          'Founder tribute distribution',
        ],
        validations: [
          { metric: 'genesisReadOnly', exact: true, description: 'Genesis nodes read-only' },
          { metric: 'economicHealth', min: 0.75, description: 'Healthy economic metrics' },
          { metric: 'selfSustaining', exact: true, description: 'Network self-sustaining' },
        ],
      },
      'post-genesis': {
        name: 'Post-Genesis Phase',
        nodeRange: [100000, Infinity],
        description: 'Full decentralization',
        features: [
          'Genesis retirement complete',
          'Independent network operation',
          'Long-term stability',
          'Economic equilibrium',
          'Community governance',
        ],
        validations: [
          { metric: 'genesisRetired', exact: true, description: 'All genesis nodes retired' },
          { metric: 'networkStability', min: 0.8, description: 'Stable network operation' },
          { metric: 'economicEquilibrium', min: 0.7, description: 'Economic equilibrium reached' },
        ],
      },
    };
  }

  /**
   * Transition to a new phase
   */
  transition(newPhase) {
    if (this.currentPhase === newPhase) return;

    const previousPhase = this.currentPhase;
    this.currentPhase = newPhase;

    this.phaseHistory.push({
      from: previousPhase,
      to: newPhase,
      timestamp: Date.now(),
    });

    console.log(`\n${'='.repeat(60)}`);
    console.log(`🔄 PHASE TRANSITION: ${previousPhase} → ${newPhase}`);
    console.log(`${'='.repeat(60)}`);
    console.log(`\n${this.phases[newPhase].description}\n`);
    console.log('Features:');
    this.phases[newPhase].features.forEach(f => console.log(`  ✓ ${f}`));
    console.log('');
  }

  /**
   * Get current phase definition
   */
  getCurrentPhaseInfo() {
    return this.phases[this.currentPhase];
  }

  /**
   * Validate phase metrics
   */
  validatePhase(metrics) {
    const phase = this.phases[this.currentPhase];
    if (!phase) return { valid: false, errors: ['Unknown phase'] };

    const errors = [];
    const validations = phase.validations || [];

    for (const validation of validations) {
      const value = metrics[validation.metric];

      if (validation.min !== undefined && value < validation.min) {
        errors.push(`${validation.description}: ${value} < ${validation.min}`);
      }

      if (validation.max !== undefined && value > validation.max) {
        errors.push(`${validation.description}: ${value} > ${validation.max}`);
      }

      if (validation.exact !== undefined && value !== validation.exact) {
        errors.push(`${validation.description}: ${value} !== ${validation.exact}`);
      }
    }

    return {
      valid: errors.length === 0,
      errors,
      phase: this.currentPhase,
      validations,
    };
  }

  /**
   * Record phase metrics
   */
  recordMetrics(phase, metrics) {
    if (!this.phaseMetrics.has(phase)) {
      this.phaseMetrics.set(phase, []);
    }

    this.phaseMetrics.get(phase).push({
      timestamp: Date.now(),
      ...metrics,
    });
  }

  /**
   * Get phase report
   */
  getReport() {
    return {
      currentPhase: this.currentPhase,
      phaseInfo: this.getCurrentPhaseInfo(),
      history: this.phaseHistory,
      metrics: Object.fromEntries(this.phaseMetrics),
    };
  }

  /**
   * Get expected phase for node count
   */
  getExpectedPhase(nodeCount) {
    for (const [phaseName, phase] of Object.entries(this.phases)) {
      const [min, max] = phase.nodeRange;
      if (nodeCount >= min && nodeCount < max) {
        return phaseName;
      }
    }
    return 'post-genesis';
  }
}
