/**
 * Feedback Loop System for Behavior Modification
 * Enables the system to learn from outcomes and modify behavior dynamically
 */

export interface FeedbackSignal {
  id: string;
  source: string;
  type: 'success' | 'failure' | 'partial' | 'unexpected' | 'novel';
  action: string;
  outcome: any;
  expected: any;
  surprise: number;
  utility: number;
  timestamp: number;
  context: any;
}

export interface BehaviorModification {
  component: string;
  parameter: string;
  oldValue: any;
  newValue: any;
  reason: string;
  confidence: number;
  timestamp: number;
  expectedImprovement: number;
}

export interface AdaptationRule {
  trigger: (feedback: FeedbackSignal) => boolean;
  modification: (feedback: FeedbackSignal, currentState: any) => BehaviorModification[];
  priority: number;
  learningRate: number;
  category: string;
}

export class FeedbackLoopSystem {
  private feedbackHistory: FeedbackSignal[] = [];
  private behaviorModifications: BehaviorModification[] = [];
  private adaptationRules: AdaptationRule[] = [];
  private behaviorParameters: Map<string, any> = new Map();
  private performanceMetrics: Map<string, number[]> = new Map();
  private learningCurves: Map<string, number[]> = new Map();

  constructor() {
    this.initializeDefaultRules();
    this.initializeDefaultParameters();
  }

  /**
   * Process feedback and trigger behavior modifications
   */
  async processFeedback(feedback: FeedbackSignal): Promise<BehaviorModification[]> {
    // Store feedback
    this.feedbackHistory.push(feedback);

    // Update performance metrics
    this.updatePerformanceMetrics(feedback);

    // Find applicable adaptation rules
    const applicableRules = this.adaptationRules.filter(rule => rule.trigger(feedback));

    // Generate behavior modifications
    const modifications: BehaviorModification[] = [];

    for (const rule of applicableRules) {
      const currentState = this.getCurrentBehaviorState();
      const ruleMods = rule.modification(feedback, currentState);
      modifications.push(...ruleMods);
    }

    // Apply modifications
    for (const modification of modifications) {
      await this.applyBehaviorModification(modification);
    }

    // Learn from the feedback pattern
    await this.learnFromFeedbackPattern(feedback);

    return modifications;
  }

  /**
   * Register new adaptation rule
   */
  registerAdaptationRule(rule: AdaptationRule): void {
    this.adaptationRules.push(rule);
    // Sort by priority
    this.adaptationRules.sort((a, b) => b.priority - a.priority);
  }

  /**
   * Create feedback loop for continuous improvement
   */
  createContinuousImprovementLoop(component: string, metric: string): void {
    const improvementRule: AdaptationRule = {
      trigger: (feedback) => feedback.source === component,
      modification: (feedback, currentState) => {
        const currentMetric = this.getMetricTrend(metric);
        const isImproving = this.isMetricImproving(currentMetric);

        if (!isImproving) {
          return this.generateImprovementModifications(component, feedback);
        }

        return [];
      },
      priority: 0.7,
      learningRate: 0.1,
      category: 'continuous_improvement'
    };

    this.registerAdaptationRule(improvementRule);
  }

  /**
   * Implement reinforcement learning feedback loop
   */
  createReinforcementLoop(actionSpace: string[], rewardFunction: (outcome: any) => number): void {
    const reinforcementRule: AdaptationRule = {
      trigger: (feedback) => actionSpace.includes(feedback.action),
      modification: (feedback, currentState) => {
        const reward = rewardFunction(feedback.outcome);
        return this.updateActionProbabilities(feedback.action, reward, actionSpace);
      },
      priority: 0.8,
      learningRate: 0.15,
      category: 'reinforcement_learning'
    };

    this.registerAdaptationRule(reinforcementRule);
  }

  /**
   * Create exploration-exploitation feedback loop
   */
  createExplorationExploitationLoop(explorationRate: number = 0.1): void {
    const explorationRule: AdaptationRule = {
      trigger: (feedback) => feedback.type === 'unexpected' || feedback.surprise > 0.7,
      modification: (feedback, currentState) => {
        // Increase exploration if we're getting unexpected results
        if (feedback.surprise > 0.7) {
          return [{
            component: 'exploration_system',
            parameter: 'exploration_rate',
            oldValue: currentState.exploration_rate || explorationRate,
            newValue: Math.min(1.0, (currentState.exploration_rate || explorationRate) + 0.1),
            reason: 'High surprise level - increase exploration',
            confidence: 0.8,
            timestamp: Date.now(),
            expectedImprovement: 0.2
          }];
        }

        // Decrease exploration if we're getting predictable good results
        if (feedback.type === 'success' && feedback.surprise < 0.2) {
          return [{
            component: 'exploration_system',
            parameter: 'exploration_rate',
            oldValue: currentState.exploration_rate || explorationRate,
            newValue: Math.max(0.01, (currentState.exploration_rate || explorationRate) - 0.05),
            reason: 'Low surprise, high success - decrease exploration',
            confidence: 0.7,
            timestamp: Date.now(),
            expectedImprovement: 0.1
          }];
        }

        return [];
      },
      priority: 0.6,
      learningRate: 0.05,
      category: 'exploration_exploitation'
    };

    this.registerAdaptationRule(explorationRule);
  }

  /**
   * Implement meta-learning feedback loop
   */
  createMetaLearningLoop(): void {
    const metaLearningRule: AdaptationRule = {
      trigger: (feedback) => this.feedbackHistory.length % 50 === 0, // Every 50 feedback signals
      modification: (feedback, currentState) => {
        // Analyze learning patterns and adjust learning rates
        const learningEffectiveness = this.analyzeLearningEffectiveness();

        return this.adjustLearningParameters(learningEffectiveness);
      },
      priority: 0.9,
      learningRate: 0.02,
      category: 'meta_learning'
    };

    this.registerAdaptationRule(metaLearningRule);
  }

  /**
   * Create adaptive complexity feedback loop
   */
  createComplexityAdaptationLoop(): void {
    const complexityRule: AdaptationRule = {
      trigger: (feedback) => true, // Always applicable
      modification: (feedback, currentState) => {
        const performanceTrend = this.getRecentPerformanceTrend();
        const currentComplexity = currentState.reasoning_complexity || 0.5;

        // If performance is declining, try different complexity levels
        if (performanceTrend < 0.3) {
          const newComplexity = this.adaptComplexity(currentComplexity, feedback);

          if (newComplexity !== currentComplexity) {
            return [{
              component: 'reasoning_system',
              parameter: 'reasoning_complexity',
              oldValue: currentComplexity,
              newValue: newComplexity,
              reason: `Performance trend: ${performanceTrend.toFixed(2)} - adjusting complexity`,
              confidence: 0.6,
              timestamp: Date.now(),
              expectedImprovement: Math.abs(newComplexity - currentComplexity) * 0.5
            }];
          }
        }

        return [];
      },
      priority: 0.5,
      learningRate: 0.08,
      category: 'adaptive_complexity'
    };

    this.registerAdaptationRule(complexityRule);
  }

  /**
   * Apply behavior modification to system parameters
   */
  private async applyBehaviorModification(modification: BehaviorModification): Promise<void> {
    const key = `${modification.component}.${modification.parameter}`;

    // Store old value for potential rollback
    const oldValue = this.behaviorParameters.get(key);

    // Apply new value
    this.behaviorParameters.set(key, modification.newValue);

    // Record the modification
    this.behaviorModifications.push(modification);

    // Update performance tracking
    this.updateLearningCurve(modification.component, modification.expectedImprovement);

    console.log(`Applied behavior modification: ${modification.component}.${modification.parameter}
                ${JSON.stringify(modification.oldValue)} -> ${JSON.stringify(modification.newValue)}`);
  }

  /**
   * Learn from feedback patterns to create new adaptation rules
   */
  private async learnFromFeedbackPattern(feedback: FeedbackSignal): Promise<void> {
    // Look for patterns in recent feedback
    const recentFeedback = this.feedbackHistory.slice(-20);

    // Detect recurring failure patterns
    const failurePattern = this.detectFailurePattern(recentFeedback);
    if (failurePattern) {
      const newRule = this.createRuleFromPattern(failurePattern);
      this.registerAdaptationRule(newRule);
    }

    // Detect success patterns
    const successPattern = this.detectSuccessPattern(recentFeedback);
    if (successPattern) {
      const reinforcementRule = this.createReinforcementRule(successPattern);
      this.registerAdaptationRule(reinforcementRule);
    }
  }

  /**
   * Initialize default adaptation rules
   */
  private initializeDefaultRules(): void {
    // Error correction rule
    this.registerAdaptationRule({
      trigger: (feedback) => feedback.type === 'failure',
      modification: (feedback, currentState) => [{
        component: feedback.source,
        parameter: 'error_tolerance',
        oldValue: currentState.error_tolerance || 0.1,
        newValue: Math.min(1.0, (currentState.error_tolerance || 0.1) + 0.05),
        reason: 'Failure detected - increase error tolerance',
        confidence: 0.7,
        timestamp: Date.now(),
        expectedImprovement: 0.1
      }],
      priority: 0.8,
      learningRate: 0.1,
      category: 'error_correction'
    });

    // Success reinforcement rule
    this.registerAdaptationRule({
      trigger: (feedback) => feedback.type === 'success' && feedback.utility > 0.8,
      modification: (feedback, currentState) => [{
        component: feedback.source,
        parameter: 'success_bias',
        oldValue: currentState.success_bias || 0.5,
        newValue: Math.min(1.0, (currentState.success_bias || 0.5) + 0.02),
        reason: 'High utility success - reinforce successful patterns',
        confidence: 0.9,
        timestamp: Date.now(),
        expectedImprovement: 0.05
      }],
      priority: 0.7,
      learningRate: 0.05,
      category: 'success_reinforcement'
    });

    // Novelty adaptation rule
    this.registerAdaptationRule({
      trigger: (feedback) => feedback.type === 'novel',
      modification: (feedback, currentState) => [{
        component: 'novelty_system',
        parameter: 'novelty_weight',
        oldValue: currentState.novelty_weight || 0.3,
        newValue: Math.min(1.0, (currentState.novelty_weight || 0.3) + 0.1),
        reason: 'Novel outcome detected - increase novelty seeking',
        confidence: 0.6,
        timestamp: Date.now(),
        expectedImprovement: 0.15
      }],
      priority: 0.5,
      learningRate: 0.08,
      category: 'novelty_adaptation'
    });
  }

  /**
   * Initialize default behavior parameters
   */
  private initializeDefaultParameters(): void {
    this.behaviorParameters.set('reasoning_system.complexity', 0.5);
    this.behaviorParameters.set('exploration_system.exploration_rate', 0.1);
    this.behaviorParameters.set('learning_system.learning_rate', 0.1);
    this.behaviorParameters.set('novelty_system.novelty_weight', 0.3);
    this.behaviorParameters.set('error_system.error_tolerance', 0.1);
    this.behaviorParameters.set('success_system.success_bias', 0.5);
  }

  /**
   * Update performance metrics based on feedback
   */
  private updatePerformanceMetrics(feedback: FeedbackSignal): void {
    const metricKey = `${feedback.source}_${feedback.type}`;
    const metrics = this.performanceMetrics.get(metricKey) || [];

    const score = this.calculatePerformanceScore(feedback);
    metrics.push(score);

    // Keep only recent metrics (last 100)
    if (metrics.length > 100) {
      metrics.shift();
    }

    this.performanceMetrics.set(metricKey, metrics);
  }

  /**
   * Calculate performance score from feedback
   */
  private calculatePerformanceScore(feedback: FeedbackSignal): number {
    let score = 0.5; // Neutral baseline

    switch (feedback.type) {
      case 'success':
        score = 0.8 + feedback.utility * 0.2;
        break;
      case 'failure':
        score = 0.2 - feedback.utility * 0.2;
        break;
      case 'partial':
        score = 0.5 + feedback.utility * 0.3;
        break;
      case 'unexpected':
        score = 0.6 + feedback.surprise * 0.4;
        break;
      case 'novel':
        score = 0.7 + (feedback.utility + feedback.surprise) * 0.15;
        break;
    }

    return Math.max(0, Math.min(1, score));
  }

  /**
   * Get current behavior state
   */
  private getCurrentBehaviorState(): any {
    const state: any = {};

    for (const [key, value] of this.behaviorParameters) {
      const [component, parameter] = key.split('.');
      if (!state[component]) state[component] = {};
      state[component][parameter] = value;

      // Also add flat structure for easier access
      state[parameter] = value;
    }

    return state;
  }

  /**
   * Get metric trend for analysis
   */
  private getMetricTrend(metric: string): number[] {
    return this.performanceMetrics.get(metric) || [];
  }

  /**
   * Check if metric is improving
   */
  private isMetricImproving(metricValues: number[]): boolean {
    if (metricValues.length < 5) return true; // Not enough data

    const recent = metricValues.slice(-5);
    const older = metricValues.slice(-10, -5);

    if (older.length === 0) return true;

    const recentAvg = recent.reduce((a, b) => a + b, 0) / recent.length;
    const olderAvg = older.reduce((a, b) => a + b, 0) / older.length;

    return recentAvg > olderAvg;
  }

  /**
   * Generate improvement modifications
   */
  private generateImprovementModifications(component: string, feedback: FeedbackSignal): BehaviorModification[] {
    const modifications: BehaviorModification[] = [];

    // Suggest parameter adjustments based on failure type
    if (feedback.type === 'failure') {
      modifications.push({
        component,
        parameter: 'robustness',
        oldValue: 0.5,
        newValue: 0.7,
        reason: 'Failure detected - increase robustness',
        confidence: 0.6,
        timestamp: Date.now(),
        expectedImprovement: 0.2
      });
    }

    return modifications;
  }

  /**
   * Update action probabilities based on reinforcement learning
   */
  private updateActionProbabilities(action: string, reward: number, actionSpace: string[]): BehaviorModification[] {
    const modifications: BehaviorModification[] = [];

    // Increase probability of rewarded actions
    if (reward > 0.5) {
      modifications.push({
        component: 'action_system',
        parameter: `${action}_probability`,
        oldValue: 1.0 / actionSpace.length, // Uniform prior
        newValue: Math.min(0.8, (1.0 / actionSpace.length) + reward * 0.1),
        reason: `Positive reward (${reward.toFixed(2)}) for action ${action}`,
        confidence: reward,
        timestamp: Date.now(),
        expectedImprovement: reward * 0.2
      });
    }

    return modifications;
  }

  /**
   * Analyze learning effectiveness
   */
  private analyzeLearningEffectiveness(): number {
    const recentModifications = this.behaviorModifications.slice(-20);

    if (recentModifications.length === 0) return 0.5;

    const actualImprovements = recentModifications.map(mod => {
      // Compare expected vs actual improvement
      const component = mod.component;
      const metricKey = `${component}_improvement`;
      const metrics = this.performanceMetrics.get(metricKey) || [];

      if (metrics.length < 2) return mod.expectedImprovement;

      const beforeImprovement = metrics[metrics.length - 2] || 0;
      const afterImprovement = metrics[metrics.length - 1] || 0;

      return afterImprovement - beforeImprovement;
    });

    const avgActualImprovement = actualImprovements.reduce((a, b) => a + b, 0) / actualImprovements.length;
    const avgExpectedImprovement = recentModifications.reduce((sum, mod) => sum + mod.expectedImprovement, 0) / recentModifications.length;

    return avgExpectedImprovement > 0 ? avgActualImprovement / avgExpectedImprovement : 0.5;
  }

  /**
   * Adjust learning parameters based on effectiveness
   */
  private adjustLearningParameters(effectiveness: number): BehaviorModification[] {
    const modifications: BehaviorModification[] = [];

    // Adjust learning rates based on effectiveness
    for (const rule of this.adaptationRules) {
      const newLearningRate = effectiveness > 0.8 ?
        Math.min(0.5, rule.learningRate * 1.1) :
        Math.max(0.01, rule.learningRate * 0.9);

      if (Math.abs(newLearningRate - rule.learningRate) > 0.01) {
        modifications.push({
          component: 'meta_learning',
          parameter: `${rule.category}_learning_rate`,
          oldValue: rule.learningRate,
          newValue: newLearningRate,
          reason: `Learning effectiveness: ${effectiveness.toFixed(2)} - adjust learning rate`,
          confidence: 0.7,
          timestamp: Date.now(),
          expectedImprovement: Math.abs(newLearningRate - rule.learningRate) * 2
        });

        rule.learningRate = newLearningRate;
      }
    }

    return modifications;
  }

  /**
   * Get recent performance trend
   */
  private getRecentPerformanceTrend(): number {
    const allMetrics: number[] = [];

    for (const metrics of this.performanceMetrics.values()) {
      allMetrics.push(...metrics.slice(-5)); // Recent 5 values from each metric
    }

    if (allMetrics.length === 0) return 0.5;

    return allMetrics.reduce((a, b) => a + b, 0) / allMetrics.length;
  }

  /**
   * Adapt complexity based on performance
   */
  private adaptComplexity(currentComplexity: number, feedback: FeedbackSignal): number {
    if (feedback.type === 'failure' && feedback.utility < 0.3) {
      // Failure with low utility - try lower complexity
      return Math.max(0.1, currentComplexity - 0.1);
    }

    if (feedback.type === 'success' && feedback.surprise > 0.7) {
      // Successful but surprising - might benefit from higher complexity
      return Math.min(1.0, currentComplexity + 0.1);
    }

    return currentComplexity;
  }

  /**
   * Update learning curve for component
   */
  private updateLearningCurve(component: string, improvement: number): void {
    const curve = this.learningCurves.get(component) || [];
    curve.push(improvement);

    if (curve.length > 50) {
      curve.shift();
    }

    this.learningCurves.set(component, curve);
  }

  /**
   * Detect failure patterns in recent feedback
   */
  private detectFailurePattern(feedback: FeedbackSignal[]): any | null {
    const failures = feedback.filter(f => f.type === 'failure');

    if (failures.length < 3) return null;

    // Look for common failure contexts
    const contexts = failures.map(f => f.context);
    const commonContext = this.findCommonElements(contexts);

    if (Object.keys(commonContext).length > 0) {
      return {
        type: 'recurring_failure',
        context: commonContext,
        frequency: failures.length / feedback.length
      };
    }

    return null;
  }

  /**
   * Detect success patterns in recent feedback
   */
  private detectSuccessPattern(feedback: FeedbackSignal[]): any | null {
    const successes = feedback.filter(f => f.type === 'success' && f.utility > 0.7);

    if (successes.length < 2) return null;

    return {
      type: 'success_pattern',
      actions: successes.map(s => s.action),
      avgUtility: successes.reduce((sum, s) => sum + s.utility, 0) / successes.length
    };
  }

  /**
   * Create adaptation rule from detected pattern
   */
  private createRuleFromPattern(pattern: any): AdaptationRule {
    return {
      trigger: (feedback) => {
        // Check if feedback matches the pattern context
        for (const [key, value] of Object.entries(pattern.context)) {
          if (feedback.context[key] !== value) return false;
        }
        return true;
      },
      modification: (feedback, currentState) => [{
        component: 'pattern_system',
        parameter: 'pattern_avoidance',
        oldValue: 0,
        newValue: 1,
        reason: `Avoiding detected failure pattern: ${JSON.stringify(pattern.context)}`,
        confidence: pattern.frequency,
        timestamp: Date.now(),
        expectedImprovement: pattern.frequency * 0.5
      }],
      priority: 0.8,
      learningRate: 0.1,
      category: 'pattern_avoidance'
    };
  }

  /**
   * Create reinforcement rule from success pattern
   */
  private createReinforcementRule(pattern: any): AdaptationRule {
    return {
      trigger: (feedback) => pattern.actions.includes(feedback.action),
      modification: (feedback, currentState) => [{
        component: 'pattern_system',
        parameter: 'pattern_reinforcement',
        oldValue: 0,
        newValue: pattern.avgUtility,
        reason: `Reinforcing successful action pattern`,
        confidence: pattern.avgUtility,
        timestamp: Date.now(),
        expectedImprovement: pattern.avgUtility * 0.3
      }],
      priority: 0.7,
      learningRate: 0.08,
      category: 'pattern_reinforcement'
    };
  }

  /**
   * Find common elements across contexts
   */
  private findCommonElements(contexts: any[]): any {
    if (contexts.length === 0) return {};

    const common: any = {};
    const first = contexts[0] || {};

    for (const [key, value] of Object.entries(first)) {
      if (contexts.every(ctx => ctx[key] === value)) {
        common[key] = value;
      }
    }

    return common;
  }

  /**
   * Get feedback loop statistics
   */
  getStats(): any {
    return {
      totalFeedback: this.feedbackHistory.length,
      totalModifications: this.behaviorModifications.length,
      activeRules: this.adaptationRules.length,
      behaviorParameters: this.behaviorParameters.size,
      recentPerformance: this.getRecentPerformanceTrend(),
      learningEffectiveness: this.analyzeLearningEffectiveness(),
      mostActiveComponents: this.getMostActiveComponents(),
      adaptationCategories: this.getAdaptationCategories()
    };
  }

  private getMostActiveComponents(): string[] {
    const componentCounts = new Map<string, number>();

    for (const mod of this.behaviorModifications) {
      componentCounts.set(mod.component, (componentCounts.get(mod.component) || 0) + 1);
    }

    return Array.from(componentCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(entry => entry[0]);
  }

  private getAdaptationCategories(): string[] {
    return [...new Set(this.adaptationRules.map(rule => rule.category))];
  }
}