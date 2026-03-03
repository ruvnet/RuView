/**
 * Persistent Learning System
 * Enables cross-session learning and knowledge accumulation
 */

import * as fs from 'fs/promises';
import * as path from 'path';

export interface LearningTriple {
  subject: string;
  predicate: string;
  object: string;
  confidence: number;
  timestamp: number;
  sessionId: string;
  sources: string[];
}

export interface SessionMemory {
  sessionId: string;
  startTime: number;
  endTime?: number;
  interactions: Interaction[];
  discoveries: Discovery[];
  performanceMetrics: any;
}

export interface Interaction {
  timestamp: number;
  type: string;
  input: any;
  output: any;
  tools: string[];
  success: boolean;
}

export interface Discovery {
  timestamp: number;
  type: 'pattern' | 'connection' | 'optimization' | 'insight';
  content: any;
  novelty: number;
  utility: number;
}

export class PersistentLearningSystem {
  private knowledgeBase: Map<string, LearningTriple> = new Map();
  private sessionMemory: Map<string, SessionMemory> = new Map();
  private currentSessionId: string;
  private learningRate = 0.1;
  private forgettingRate = 0.01;
  private storagePath: string;

  constructor(storagePath = './data/learning') {
    this.storagePath = storagePath;
    this.currentSessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    this.initializeSession();
  }

  /**
   * Initialize new learning session
   */
  private async initializeSession(): Promise<void> {
    await this.loadPersistedKnowledge();

    this.sessionMemory.set(this.currentSessionId, {
      sessionId: this.currentSessionId,
      startTime: Date.now(),
      interactions: [],
      discoveries: [],
      performanceMetrics: {}
    });
  }

  /**
   * Learn from interaction results
   */
  async learnFromInteraction(interaction: Interaction): Promise<void> {
    // Add to current session memory
    const session = this.sessionMemory.get(this.currentSessionId);
    if (session) {
      session.interactions.push(interaction);
    }

    // Extract learning triples from successful interactions
    if (interaction.success) {
      const newTriples = this.extractLearningTriples(interaction);

      for (const triple of newTriples) {
        await this.addKnowledge(triple);
      }

      // Look for patterns across interactions
      const patterns = this.detectPatterns(session?.interactions || []);
      for (const pattern of patterns) {
        await this.recordDiscovery({
          timestamp: Date.now(),
          type: 'pattern',
          content: pattern,
          novelty: this.calculateNovelty(pattern),
          utility: this.calculateUtility(pattern)
        });
      }
    }
  }

  /**
   * Add knowledge triple with reinforcement learning
   */
  async addKnowledge(triple: LearningTriple): Promise<void> {
    const key = `${triple.subject}:${triple.predicate}:${triple.object}`;

    const existing = this.knowledgeBase.get(key);
    if (existing) {
      // Reinforce existing knowledge
      existing.confidence = Math.min(1.0,
        existing.confidence + this.learningRate * (1 - existing.confidence)
      );
      existing.timestamp = Date.now();
      existing.sources.push(triple.sessionId);
    } else {
      // Add new knowledge
      this.knowledgeBase.set(key, triple);
    }

    // Persist the update
    await this.persistKnowledge();
  }

  /**
   * Query learned knowledge with confidence scores
   */
  queryKnowledge(subject?: string, predicate?: string, object?: string): LearningTriple[] {
    const results: LearningTriple[] = [];

    for (const [key, triple] of this.knowledgeBase) {
      let matches = true;

      if (subject && triple.subject !== subject) matches = false;
      if (predicate && triple.predicate !== predicate) matches = false;
      if (object && triple.object !== object) matches = false;

      if (matches) {
        results.push(triple);
      }
    }

    // Sort by confidence and recency
    return results.sort((a, b) =>
      (b.confidence * 0.7 + (b.timestamp / Date.now()) * 0.3) -
      (a.confidence * 0.7 + (a.timestamp / Date.now()) * 0.3)
    );
  }

  /**
   * Learn from cross-session patterns
   */
  async analyzeHistoricalPatterns(): Promise<Discovery[]> {
    const allSessions = Array.from(this.sessionMemory.values());
    const discoveries: Discovery[] = [];

    // Analyze success patterns across sessions
    const successPatterns = this.findSuccessPatterns(allSessions);
    discoveries.push(...successPatterns.map(pattern => ({
      timestamp: Date.now(),
      type: 'pattern' as const,
      content: pattern,
      novelty: this.calculateNovelty(pattern),
      utility: this.calculateUtility(pattern)
    })));

    // Find tool combination effectiveness
    const toolEffectiveness = this.analyzeToolEffectiveness(allSessions);
    discoveries.push({
      timestamp: Date.now(),
      type: 'optimization' as const,
      content: { toolRankings: toolEffectiveness },
      novelty: 0.5,
      utility: 0.8
    });

    // Store discoveries
    for (const discovery of discoveries) {
      await this.recordDiscovery(discovery);
    }

    return discoveries;
  }

  /**
   * Get learning recommendations based on historical data
   */
  getLearningRecommendations(): any[] {
    const recommendations = [];

    // Recommend exploring under-utilized tool combinations
    const underutilized = this.findUnderutilizedCombinations();
    recommendations.push({
      type: 'exploration',
      suggestion: 'Try under-utilized tool combinations',
      combinations: underutilized,
      priority: 0.7
    });

    // Recommend reinforcing successful patterns
    const successfulPatterns = this.getSuccessfulPatterns();
    recommendations.push({
      type: 'reinforcement',
      suggestion: 'Strengthen successful reasoning patterns',
      patterns: successfulPatterns,
      priority: 0.8
    });

    // Recommend areas needing improvement
    const weakAreas = this.identifyWeakAreas();
    recommendations.push({
      type: 'improvement',
      suggestion: 'Focus learning on weak performance areas',
      areas: weakAreas,
      priority: 0.9
    });

    return recommendations.sort((a, b) => b.priority - a.priority);
  }

  /**
   * Apply forgetting to old, unused knowledge
   */
  async applyForgetting(): Promise<void> {
    const now = Date.now();
    const oneDay = 24 * 60 * 60 * 1000;

    for (const [key, triple] of this.knowledgeBase) {
      const age = now - triple.timestamp;
      const ageDays = age / oneDay;

      // Apply forgetting curve
      const forgettingFactor = Math.exp(-this.forgettingRate * ageDays);
      triple.confidence *= forgettingFactor;

      // Remove very low confidence knowledge
      if (triple.confidence < 0.01) {
        this.knowledgeBase.delete(key);
      }
    }

    await this.persistKnowledge();
  }

  /**
   * Extract learning triples from interactions
   */
  private extractLearningTriples(interaction: Interaction): LearningTriple[] {
    const triples: LearningTriple[] = [];

    // Extract tool effectiveness patterns
    if (interaction.success && interaction.tools.length > 0) {
      triples.push({
        subject: interaction.tools.join('+'),
        predicate: 'effective_for',
        object: interaction.type,
        confidence: 0.5,
        timestamp: Date.now(),
        sessionId: this.currentSessionId,
        sources: [this.currentSessionId]
      });
    }

    // Extract input-output patterns
    if (interaction.input && interaction.output) {
      const inputPattern = this.extractPattern(interaction.input);
      const outputPattern = this.extractPattern(interaction.output);

      if (inputPattern && outputPattern) {
        triples.push({
          subject: inputPattern,
          predicate: 'transforms_to',
          object: outputPattern,
          confidence: 0.6,
          timestamp: Date.now(),
          sessionId: this.currentSessionId,
          sources: [this.currentSessionId]
        });
      }
    }

    return triples;
  }

  private extractPattern(data: any): string | null {
    if (typeof data === 'string') return data.substring(0, 50);
    if (typeof data === 'object') return JSON.stringify(data).substring(0, 50);
    return null;
  }

  private detectPatterns(interactions: Interaction[]): any[] {
    const patterns = [];

    // Find temporal patterns
    const temporalPatterns = this.findTemporalPatterns(interactions);
    patterns.push(...temporalPatterns);

    // Find tool usage patterns
    const toolPatterns = this.findToolPatterns(interactions);
    patterns.push(...toolPatterns);

    return patterns;
  }

  private findTemporalPatterns(interactions: Interaction[]): any[] {
    // Implementation for finding temporal patterns
    return [];
  }

  private findToolPatterns(interactions: Interaction[]): any[] {
    // Implementation for finding tool usage patterns
    return [];
  }

  private findSuccessPatterns(sessions: SessionMemory[]): any[] {
    // Implementation for finding success patterns across sessions
    return [];
  }

  private analyzeToolEffectiveness(sessions: SessionMemory[]): any {
    // Implementation for analyzing tool effectiveness
    return {};
  }

  private findUnderutilizedCombinations(): any[] {
    // Implementation for finding under-utilized combinations
    return [];
  }

  private getSuccessfulPatterns(): any[] {
    // Implementation for getting successful patterns
    return [];
  }

  private identifyWeakAreas(): any[] {
    // Implementation for identifying weak areas
    return [];
  }

  private calculateNovelty(pattern: any): number {
    // Calculate how novel this pattern is
    return Math.random() * 0.5 + 0.5; // Placeholder
  }

  private calculateUtility(pattern: any): number {
    // Calculate how useful this pattern is
    return Math.random() * 0.5 + 0.5; // Placeholder
  }

  private async recordDiscovery(discovery: Discovery): Promise<void> {
    const session = this.sessionMemory.get(this.currentSessionId);
    if (session) {
      session.discoveries.push(discovery);
    }
  }

  /**
   * Persist knowledge to disk
   */
  private async persistKnowledge(): Promise<void> {
    try {
      await fs.mkdir(this.storagePath, { recursive: true });

      const knowledgeArray = Array.from(this.knowledgeBase.values());
      await fs.writeFile(
        path.join(this.storagePath, 'knowledge_base.json'),
        JSON.stringify(knowledgeArray, null, 2)
      );

      const sessionArray = Array.from(this.sessionMemory.values());
      await fs.writeFile(
        path.join(this.storagePath, 'session_memory.json'),
        JSON.stringify(sessionArray, null, 2)
      );
    } catch (error) {
      console.error('Failed to persist knowledge:', error);
    }
  }

  /**
   * Load persisted knowledge from disk
   */
  private async loadPersistedKnowledge(): Promise<void> {
    try {
      const knowledgePath = path.join(this.storagePath, 'knowledge_base.json');
      const sessionPath = path.join(this.storagePath, 'session_memory.json');

      // Load knowledge base
      try {
        const knowledgeData = await fs.readFile(knowledgePath, 'utf-8');
        const knowledgeArray: LearningTriple[] = JSON.parse(knowledgeData);

        this.knowledgeBase.clear();
        for (const triple of knowledgeArray) {
          const key = `${triple.subject}:${triple.predicate}:${triple.object}`;
          this.knowledgeBase.set(key, triple);
        }
      } catch (error) {
        // No existing knowledge base
      }

      // Load session memory
      try {
        const sessionData = await fs.readFile(sessionPath, 'utf-8');
        const sessionArray: SessionMemory[] = JSON.parse(sessionData);

        this.sessionMemory.clear();
        for (const session of sessionArray) {
          this.sessionMemory.set(session.sessionId, session);
        }
      } catch (error) {
        // No existing session memory
      }
    } catch (error) {
      console.error('Failed to load persisted knowledge:', error);
    }
  }

  /**
   * Get learning statistics
   */
  getLearningStats(): any {
    return {
      totalTriples: this.knowledgeBase.size,
      currentSession: this.currentSessionId,
      totalSessions: this.sessionMemory.size,
      avgConfidence: this.calculateAverageConfidence(),
      lastUpdate: this.getLastUpdateTime(),
      learningRate: this.learningRate,
      forgettingRate: this.forgettingRate
    };
  }

  private calculateAverageConfidence(): number {
    const triples = Array.from(this.knowledgeBase.values());
    if (triples.length === 0) return 0;

    const sum = triples.reduce((acc, triple) => acc + triple.confidence, 0);
    return sum / triples.length;
  }

  private getLastUpdateTime(): number {
    const triples = Array.from(this.knowledgeBase.values());
    if (triples.length === 0) return 0;

    return Math.max(...triples.map(triple => triple.timestamp));
  }
}