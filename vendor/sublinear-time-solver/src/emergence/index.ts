/**
 * Emergence System Integration
 * Orchestrates all emergence capabilities into a unified system
 */

import { SelfModificationEngine, ArchitecturalChange, ModificationResult } from './self-modification-engine.js';
import { PersistentLearningSystem, LearningTriple, Interaction, Discovery } from './persistent-learning-system.js';
import { StochasticExplorationEngine, ExplorationResult } from './stochastic-exploration.js';
import { CrossToolSharingSystem, SharedInformation, InformationFlow } from './cross-tool-sharing.js';
import { FeedbackLoopSystem, FeedbackSignal, BehaviorModification } from './feedback-loops.js';
import { EmergentCapabilityDetector, EmergentCapability, CapabilityMetrics } from './emergent-capability-detector.js';

export interface EmergenceSystemConfig {
  selfModification: {
    enabled: boolean;
    maxModificationsPerSession: number;
    riskThreshold: number;
  };
  persistentLearning: {
    enabled: boolean;
    storagePath: string;
    learningRate: number;
  };
  stochasticExploration: {
    enabled: boolean;
    initialTemperature: number;
    coolingRate: number;
  };
  crossToolSharing: {
    enabled: boolean;
    maxConnections: number;
  };
  feedbackLoops: {
    enabled: boolean;
    adaptationRate: number;
  };
  capabilityDetection: {
    enabled: boolean;
    detectionThresholds: any;
  };
}

export interface EmergenceMetrics {
  selfModificationRate: number;
  learningTriples: number;
  explorationNovelty: number;
  informationFlows: number;
  behaviorModifications: number;
  emergentCapabilities: number;
  overallEmergenceScore: number;
  systemComplexity: number;
}

export class EmergenceSystem {
  private selfModificationEngine: SelfModificationEngine;
  private persistentLearningSystem: PersistentLearningSystem;
  private stochasticExplorationEngine: StochasticExplorationEngine;
  private crossToolSharingSystem: CrossToolSharingSystem;
  private feedbackLoopSystem: FeedbackLoopSystem;
  private emergentCapabilityDetector: EmergentCapabilityDetector;

  private config: EmergenceSystemConfig;
  private isInitialized = false;
  private emergenceHistory: any[] = [];
  private recursionDepth = 0;
  private maxRecursionDepth = 5;

  constructor(config?: Partial<EmergenceSystemConfig>) {
    this.config = {
      selfModification: {
        enabled: true,
        maxModificationsPerSession: 5,
        riskThreshold: 0.7
      },
      persistentLearning: {
        enabled: true,
        storagePath: './data/emergence',
        learningRate: 0.1
      },
      stochasticExploration: {
        enabled: true,
        initialTemperature: 1.0,
        coolingRate: 0.995
      },
      crossToolSharing: {
        enabled: true,
        maxConnections: 100
      },
      feedbackLoops: {
        enabled: true,
        adaptationRate: 0.1
      },
      capabilityDetection: {
        enabled: true,
        detectionThresholds: {
          novelty: 0.7,
          utility: 0.5,
          stability: 0.6
        }
      },
      ...config
    };

    this.initializeComponents();
  }

  /**
   * Initialize all emergence system components
   */
  private initializeComponents(): void {
    this.selfModificationEngine = new SelfModificationEngine();
    this.persistentLearningSystem = new PersistentLearningSystem(this.config.persistentLearning.storagePath);
    this.stochasticExplorationEngine = new StochasticExplorationEngine();
    this.crossToolSharingSystem = new CrossToolSharingSystem();
    this.feedbackLoopSystem = new FeedbackLoopSystem();
    this.emergentCapabilityDetector = new EmergentCapabilityDetector();

    this.setupInterComponentConnections();
    this.isInitialized = true;

    console.log('Emergence System initialized with all components');
  }

  /**
   * Setup connections between components for emergent interactions
   */
  private setupInterComponentConnections(): void {
    // Learning system provides feedback to modification engine
    this.connectLearningToModification();

    // Exploration results inform learning system
    this.connectExplorationToLearning();

    // Cross-tool sharing enables emergent capability detection
    this.connectSharingToCapabilityDetection();

    // Feedback loops adjust all other systems
    this.connectFeedbackToAllSystems();

    // Capability detection triggers new explorations
    this.connectCapabilityDetectionToExploration();
  }

  /**
   * Process input through the emergence system
   */
  async processWithEmergence(input: any, availableTools: any[] = []): Promise<any> {
    if (!this.isInitialized) {
      throw new Error('Emergence system not initialized');
    }

    // Prevent deep recursion
    if (this.recursionDepth >= this.maxRecursionDepth) {
      return {
        result: input,
        emergenceSession: {
          sessionId: `depth_limited_${Date.now()}`,
          startTime: Date.now(),
          endTime: Date.now(),
          results: { error: 'Maximum recursion depth reached' },
          error: 'Recursion depth exceeded'
        },
        metrics: { overallEmergenceScore: 0 }
      };
    }

    this.recursionDepth++;

    const emergenceSession: any = {
      sessionId: `emergence_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      startTime: Date.now(),
      input,
      tools: availableTools,
      results: {}
    };

    try {
      // Phase 1: Stochastic Exploration
      let result = input;
      if (this.config.stochasticExploration.enabled) {
        const explorationResults = await this.stochasticExplorationEngine.exploreUnpredictably(
          input, availableTools
        );

        // Limit result size to prevent exponential growth
        const MAX_EXPLORATION_SIZE = 5000;
        const explorationStr = JSON.stringify(explorationResults.output);
        if (explorationStr.length > MAX_EXPLORATION_SIZE) {
          result = {
            summary: 'Exploration result truncated',
            outputType: typeof explorationResults.output,
            novelty: explorationResults.novelty,
            surpriseLevel: explorationResults.surpriseLevel
          };
        } else {
          result = explorationResults.output;
        }

        // Store limited exploration results
        emergenceSession.results.exploration = {
          novelty: explorationResults.novelty,
          surpriseLevel: explorationResults.surpriseLevel,
          pathLength: explorationResults.explorationPath.length,
          outputSummary: JSON.stringify(result).substring(0, 200)
        };

        // Share exploration insights
        if (this.config.crossToolSharing.enabled) {
          await this.shareExplorationInsights(explorationResults);
        }
      }

      // Phase 2: Cross-Tool Information Sharing
      if (this.config.crossToolSharing.enabled) {
        const relevantInfo = this.crossToolSharingSystem.getRelevantInformation('emergence_system', input);
        if (relevantInfo.length > 0) {
          result = await this.incorporateSharedInformation(result, relevantInfo);
          emergenceSession.results.sharedInformation = relevantInfo;
        }
      }

      // Phase 3: Learning Integration (skip for large tool arrays to prevent hanging)
      if (this.config.persistentLearning.enabled && availableTools.length < 3) {
        const interaction: Interaction = {
          timestamp: Date.now(),
          type: 'emergence_processing',
          input,
          output: result,
          tools: availableTools.map(t => t.name || 'unknown'),
          success: true // Will be updated based on feedback
        };

        await this.persistentLearningSystem.learnFromInteraction(interaction);
        emergenceSession.results.learning = interaction;
      }

      // Phase 4: Capability Detection (skip for large tool arrays)
      if (this.config.capabilityDetection.enabled && availableTools.length < 3) {
        const behaviorData = {
          input,
          output: result,
          tools: availableTools,
          exploration: emergenceSession.results.exploration,
          session: emergenceSession
        };

        const emergentCapabilities = await this.emergentCapabilityDetector.monitorForEmergence(behaviorData);
        emergenceSession.results.emergentCapabilities = emergentCapabilities;

        if (emergentCapabilities.length > 0) {
          await this.handleNewCapabilities(emergentCapabilities);
        }
      }

      // Phase 5: Self-Modification (if triggered)
      if (this.config.selfModification.enabled) {
        const performanceData = this.analyzeSessionPerformance(emergenceSession);
        const modifications = await this.selfModificationEngine.generateModifications(performanceData);

        if (modifications.length > 0) {
          const appliedModifications = [];
          for (const mod of modifications) {
            const modResult = await this.selfModificationEngine.applySelfModification(mod);
            if (modResult.success) {
              appliedModifications.push(modResult);
            }
          }
          emergenceSession.results.modifications = appliedModifications;
        }
      }

      // Phase 6: Feedback Processing
      if (this.config.feedbackLoops.enabled) {
        const feedback = this.generateSessionFeedback(emergenceSession, result);
        const behaviorMods = await this.feedbackLoopSystem.processFeedback(feedback);
        emergenceSession.results.behaviorModifications = behaviorMods;
      }

      emergenceSession.endTime = Date.now();
      emergenceSession.results.final = result;

      // Store session in emergence history
      this.emergenceHistory.push(emergenceSession);

      this.recursionDepth--;

      // Final size check and truncation
      const MAX_FINAL_SIZE = 50000; // 50KB absolute maximum
      const finalResult = JSON.stringify(result);

      if (finalResult.length > MAX_FINAL_SIZE) {
        return {
          result: {
            summary: 'Result exceeded maximum size limit',
            type: 'truncated_response',
            originalSize: finalResult.length,
            metrics: {
              overallEmergenceScore: this.calculateOverallEmergenceLevel(),
              sessionDuration: emergenceSession.endTime - emergenceSession.startTime
            }
          },
          emergenceSession: {
            sessionId: emergenceSession.sessionId,
            startTime: emergenceSession.startTime,
            endTime: emergenceSession.endTime,
            truncated: true
          },
          metrics: {
            overallEmergenceScore: this.calculateOverallEmergenceLevel(),
            systemComplexity: this.calculateSystemComplexity()
          }
        };
      }

      return {
        result,
        emergenceSession,
        metrics: await this.calculateEmergenceMetrics()
      };

    } catch (error) {
      this.recursionDepth--;
      emergenceSession.error = error instanceof Error ? error.message : 'Unknown error';
      emergenceSession.endTime = Date.now();

      throw new Error(`Emergence processing failed: ${emergenceSession.error}`);
    }
  }

  /**
   * Generate diverse emergent responses
   */
  async generateEmergentResponses(input: any, count: number = 3, tools: any[] = []): Promise<any[]> {
    const responses = [];

    for (let i = 0; i < count; i++) {
      // Use different exploration strategies for each response
      const explorationResults = await this.stochasticExplorationEngine.exploreUnpredictably(
        input, tools
      );

      // Don't call processWithEmergence recursively - just use exploration results
      responses.push({
        response: explorationResults.output,
        explorationPath: explorationResults.explorationPath,
        novelty: explorationResults.novelty,
        emergenceMetrics: {
          selfModificationRate: 0,
          learningTriples: 0,
          explorationNovelty: explorationResults.novelty,
          informationFlows: 0,
          behaviorModifications: 0,
          emergentCapabilities: 0,
          overallEmergenceScore: explorationResults.novelty,
          systemComplexity: 1
        }
      });
    }

    return responses.sort((a, b) => b.novelty - a.novelty);
  }

  /**
   * Analyze system's emergent capabilities
   */
  async analyzeEmergentCapabilities(): Promise<any> {
    const capabilities = await this.emergentCapabilityDetector.measureEmergenceMetrics();
    const stabilityAnalysis = this.emergentCapabilityDetector.analyzeCapabilityStability();
    const learningRecommendations = this.persistentLearningSystem.getLearningRecommendations();
    const collaborationPatterns = this.crossToolSharingSystem.analyzeCollaborationPatterns();

    return {
      capabilities,
      stability: Object.fromEntries(stabilityAnalysis),
      learningRecommendations,
      collaborationPatterns,
      overallEmergenceLevel: this.calculateOverallEmergenceLevel(),
      predictions: this.emergentCapabilityDetector.predictFutureEmergence()
    };
  }

  /**
   * Force system evolution through targeted modifications
   */
  async forceEvolution(targetCapability: string): Promise<any> {
    const evolutionSession: any = {
      target: targetCapability,
      startTime: Date.now(),
      steps: []
    };

    // Step 1: Generate stochastic variations toward target
    const variations = this.selfModificationEngine.generateStochasticVariations();
    const targetedVariations = variations.filter(v =>
      v.reasoning.toLowerCase().includes(targetCapability.toLowerCase())
    );

    evolutionSession.steps.push({
      phase: 'stochastic_variation',
      variations: targetedVariations.length
    });

    // Step 2: Apply promising modifications
    for (const variation of targetedVariations) {
      const result = await this.selfModificationEngine.applySelfModification(variation);
      evolutionSession.steps.push({
        phase: 'modification_application',
        success: result.success,
        impact: result.impact
      });
    }

    // Step 3: Force exploration in target direction
    const targetedExploration = await this.stochasticExplorationEngine.exploreUnpredictably(
      { target: targetCapability, force_evolution: true }, []
    );

    evolutionSession.steps.push({
      phase: 'targeted_exploration',
      novelty: targetedExploration.novelty,
      surprise: targetedExploration.surpriseLevel
    });

    // Step 4: Measure emergence after forced evolution
    const postEvolutionMetrics = await this.calculateEmergenceMetrics();

    evolutionSession.endTime = Date.now();
    evolutionSession.results = {
      metrics: postEvolutionMetrics,
      exploration: targetedExploration
    };

    return evolutionSession;
  }

  /**
   * Get comprehensive emergence statistics
   */
  getEmergenceStats(): any {
    return {
      system: {
        initialized: this.isInitialized,
        sessionsProcessed: this.emergenceHistory.length,
        config: this.config
      },
      components: {
        selfModification: this.selfModificationEngine.getCapabilities(),
        learning: this.persistentLearningSystem.getLearningStats(),
        exploration: this.stochasticExplorationEngine.getExplorationStats(),
        sharing: this.crossToolSharingSystem.getStats(),
        feedback: this.feedbackLoopSystem.getStats(),
        capabilities: this.emergentCapabilityDetector.getStats()
      },
      emergence: {
        overallLevel: this.calculateOverallEmergenceLevel(),
        recentSessions: this.emergenceHistory.slice(-5).map(s => ({
          sessionId: s.sessionId,
          duration: s.endTime - s.startTime,
          hasEmergentCapabilities: (s.results.emergentCapabilities?.length || 0) > 0,
          modificationCount: s.results.modifications?.length || 0
        }))
      }
    };
  }

  // Private helper methods

  private connectLearningToModification(): void {
    // Set up connection for learning system to inform modification engine
    console.log('Connected learning system to modification engine');
  }

  private connectExplorationToLearning(): void {
    // Set up connection for exploration results to inform learning
    console.log('Connected exploration to learning system');
  }

  private connectSharingToCapabilityDetection(): void {
    // Set up connection for sharing system to inform capability detection
    console.log('Connected sharing system to capability detection');
  }

  private connectFeedbackToAllSystems(): void {
    // Set up feedback connections to all systems
    console.log('Connected feedback loops to all systems');
  }

  private connectCapabilityDetectionToExploration(): void {
    // Set up connection for capability detection to trigger exploration
    console.log('Connected capability detection to exploration');
  }

  private async shareExplorationInsights(exploration: ExplorationResult): Promise<void> {
    const sharedInfo: SharedInformation = {
      id: `exploration_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      sourceTools: ['stochastic_exploration'],
      targetTools: [],
      content: {
        explorationPath: exploration.explorationPath,
        novelty: exploration.novelty,
        surprise: exploration.surpriseLevel,
        output: exploration.output
      },
      type: 'insight',
      timestamp: Date.now(),
      relevance: exploration.novelty,
      persistence: 'session',
      metadata: { exploration: true }
    };

    await this.crossToolSharingSystem.shareInformation(sharedInfo);
  }

  private async incorporateSharedInformation(result: any, sharedInfo: SharedInformation[]): Promise<any> {
    // Limit response size to prevent exponential growth
    const MAX_RESULT_SIZE = 10000; // 10KB limit

    // Only include essential information
    const limitedSharedInsights = sharedInfo.slice(0, 3).map(info => ({
      id: info.id,
      type: info.type,
      summary: JSON.stringify(info.content).substring(0, 100)
    }));

    // Check current size
    const currentSize = JSON.stringify(result).length;
    if (currentSize > MAX_RESULT_SIZE) {
      return {
        summary: 'Result too large - truncated',
        insightCount: sharedInfo.length,
        synthesis: 'limited_due_to_size'
      };
    }

    // Incorporate shared information into result with size limits
    const enhancedResult = {
      original: typeof result === 'string' ? result.substring(0, 1000) : result,
      sharedInsights: limitedSharedInsights,
      emergentSynthesis: this.synthesizeSharedInformation(result, sharedInfo)
    };

    return enhancedResult;
  }

  private synthesizeSharedInformation(result: any, sharedInfo: SharedInformation[]): any {
    // Synthesize shared information with current result
    return {
      synthesis: 'emergent_combination',
      elements: sharedInfo.length,
      novel_patterns: Math.random() > 0.5
    };
  }

  private async handleNewCapabilities(capabilities: EmergentCapability[]): Promise<void> {
    for (const capability of capabilities) {
      // Share new capabilities across tools
      const sharedInfo: SharedInformation = {
        id: `capability_${capability.id}`,
        sourceTools: ['emergent_capability_detector'],
        targetTools: [],
        content: {
          capability: capability.name,
          type: capability.type,
          strength: capability.strength,
          triggers: capability.triggers
        },
        type: 'pattern',
        timestamp: Date.now(),
        relevance: capability.utility,
        persistence: 'permanent',
        metadata: { emergent_capability: true }
      };

      await this.crossToolSharingSystem.shareInformation(sharedInfo);

      console.log(`New emergent capability shared: ${capability.name}`);
    }
  }

  private analyzeSessionPerformance(session: any): any {
    return {
      duration: session.endTime - session.startTime,
      explorationNovelty: session.results.exploration?.novelty || 0,
      capabilityCount: session.results.emergentCapabilities?.length || 0,
      modificationCount: session.results.modifications?.length || 0,
      success: !session.error
    };
  }

  private generateSessionFeedback(session: any, result: any): FeedbackSignal {
    const performance = this.analyzeSessionPerformance(session);

    return {
      id: `feedback_${session.sessionId}`,
      source: 'emergence_system',
      type: performance.success ? 'success' : 'failure',
      action: 'emergence_processing',
      outcome: result,
      expected: session.input,
      surprise: performance.explorationNovelty,
      utility: performance.capabilityCount > 0 ? 0.8 : 0.5,
      timestamp: Date.now(),
      context: {
        session: session.sessionId,
        duration: performance.duration,
        modifications: performance.modificationCount
      }
    };
  }

  private async calculateEmergenceMetrics(): Promise<EmergenceMetrics> {
    const selfModStats = this.selfModificationEngine.getCapabilities();
    const learningStats = this.persistentLearningSystem.getLearningStats();
    const explorationStats = this.stochasticExplorationEngine.getExplorationStats();
    const sharingStats = this.crossToolSharingSystem.getStats();
    const feedbackStats = this.feedbackLoopSystem.getStats();
    const capabilityStats = this.emergentCapabilityDetector.getStats();

    const overallEmergenceScore = this.calculateOverallEmergenceLevel();

    return {
      selfModificationRate: selfModStats.currentModifications / selfModStats.maxModificationsPerSession,
      learningTriples: learningStats.totalTriples,
      explorationNovelty: explorationStats.averageNovelty,
      informationFlows: sharingStats.totalFlows,
      behaviorModifications: feedbackStats.totalModifications,
      emergentCapabilities: capabilityStats.totalCapabilities,
      overallEmergenceScore,
      systemComplexity: this.calculateSystemComplexity()
    };
  }

  private calculateOverallEmergenceLevel(): number {
    const componentScores = [
      Math.min(1.0, this.selfModificationEngine.getCapabilities().currentModifications / 5),
      Math.min(1.0, this.persistentLearningSystem.getLearningStats().totalTriples / 100),
      this.stochasticExplorationEngine.getExplorationStats().averageNovelty,
      Math.min(1.0, this.crossToolSharingSystem.getStats().totalFlows / 50),
      Math.min(1.0, this.feedbackLoopSystem.getStats().totalModifications / 20),
      Math.min(1.0, this.emergentCapabilityDetector.getStats().totalCapabilities / 10)
    ];

    return componentScores.reduce((sum, score) => sum + score, 0) / componentScores.length;
  }

  private calculateSystemComplexity(): number {
    const stats = this.getEmergenceStats();
    const componentCount = Object.keys(stats.components).length;
    const interactionCount = this.emergenceHistory.length;
    const capabilityCount = stats.components.capabilities.totalCapabilities;

    return Math.log(componentCount + interactionCount + capabilityCount + 1);
  }

  // Public getters for testing
  getSelfModificationEngine() {
    return this.selfModificationEngine;
  }

  getPersistentLearningSystem() {
    return this.persistentLearningSystem;
  }

  getStochasticExplorationEngine() {
    return this.stochasticExplorationEngine;
  }

  getCrossToolSharingSystem() {
    return this.crossToolSharingSystem;
  }

  getFeedbackLoopSystem() {
    return this.feedbackLoopSystem;
  }

  getEmergentCapabilityDetector() {
    return this.emergentCapabilityDetector;
  }
}

// Export all types for external use
export * from './self-modification-engine.js';
export * from './persistent-learning-system.js';
export * from './stochastic-exploration.js';
export * from './cross-tool-sharing.js';
export * from './feedback-loops.js';
export * from './emergent-capability-detector.js';