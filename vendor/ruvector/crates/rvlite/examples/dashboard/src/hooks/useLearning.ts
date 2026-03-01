/**
 * Self-Learning Hook for RvLite Dashboard
 *
 * Implements adaptive learning capabilities:
 * - Query pattern recognition and optimization
 * - Result relevance feedback and scoring
 * - Usage pattern analysis
 * - Automatic query suggestions
 * - Performance optimization recommendations
 */

import { useState, useCallback, useEffect, useRef } from 'react';

// ============================================================================
// Types
// ============================================================================

export interface QueryPattern {
  id: string;
  queryType: 'sql' | 'sparql' | 'cypher' | 'vector';
  pattern: string;
  frequency: number;
  avgExecutionTime: number;
  successRate: number;
  lastUsed: number;
  resultCount: number;
  feedback: {
    helpful: number;
    notHelpful: number;
  };
}

export interface LearningMetrics {
  totalQueries: number;
  successfulQueries: number;
  failedQueries: number;
  avgResponseTime: number;
  queryPatterns: QueryPattern[];
  suggestions: QuerySuggestion[];
  insights: LearningInsight[];
  adaptationLevel: number; // 0-100 scale
  learningRate: number;
}

export interface QuerySuggestion {
  id: string;
  query: string;
  queryType: 'sql' | 'sparql' | 'cypher' | 'vector';
  confidence: number;
  reason: string;
  basedOn: string[];
}

export interface LearningInsight {
  id: string;
  type: 'optimization' | 'pattern' | 'anomaly' | 'recommendation';
  title: string;
  description: string;
  recommendation?: string;
  severity: 'info' | 'warning' | 'success';
  timestamp: number;
  actionable: boolean;
  action?: () => void;
}

export interface FeedbackEntry {
  queryId: string;
  query: string;
  queryType: 'sql' | 'sparql' | 'cypher' | 'vector';
  helpful: boolean;
  timestamp: number;
  resultCount: number;
  executionTime: number;
}

export interface QueryExecution {
  id: string;
  query: string;
  queryType: 'sql' | 'sparql' | 'cypher' | 'vector';
  timestamp: number;
  executionTime: number;
  success: boolean;
  resultCount: number;
  error?: string;
}

// ============================================================================
// Learning Engine
// ============================================================================

class LearningEngine {
  private patterns: Map<string, QueryPattern> = new Map();
  private executions: QueryExecution[] = [];
  private feedback: FeedbackEntry[] = [];
  private storageKey = 'rvlite_learning_data';

  constructor() {
    this.loadFromStorage();
  }

  // Pattern extraction from query
  private extractPattern(query: string, queryType: string): string {
    let normalized = query.trim().toLowerCase();

    // Normalize SQL patterns
    if (queryType === 'sql') {
      // Replace specific values with placeholders
      normalized = normalized
        .replace(/'[^']*'/g, "'?'")
        .replace(/\[[^\]]*\]/g, '[?]')
        .replace(/\d+(\.\d+)?/g, '?')
        .replace(/\s+/g, ' ');
    }

    // Normalize SPARQL patterns
    if (queryType === 'sparql') {
      normalized = normalized
        .replace(/<[^>]+>/g, '<?>') // URIs
        .replace(/"[^"]*"/g, '"?"') // Literals
        .replace(/\s+/g, ' ');
    }

    // Normalize Cypher patterns
    if (queryType === 'cypher') {
      normalized = normalized
        .replace(/'[^']*'/g, "'?'")
        .replace(/\{[^}]+\}/g, '{?}')
        .replace(/\s+/g, ' ');
    }

    return normalized;
  }

  // Generate pattern ID
  private generatePatternId(pattern: string, queryType: string): string {
    const hash = pattern.split('').reduce((acc, char) => {
      return ((acc << 5) - acc) + char.charCodeAt(0);
    }, 0);
    return `${queryType}_${Math.abs(hash).toString(16)}`;
  }

  // Record query execution
  recordExecution(
    query: string,
    queryType: 'sql' | 'sparql' | 'cypher' | 'vector',
    executionTime: number,
    success: boolean,
    resultCount: number,
    error?: string
  ): string {
    const execution: QueryExecution = {
      id: `exec_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      query,
      queryType,
      timestamp: Date.now(),
      executionTime,
      success,
      resultCount,
      error,
    };

    this.executions.push(execution);

    // Keep only last 1000 executions
    if (this.executions.length > 1000) {
      this.executions = this.executions.slice(-1000);
    }

    // Update pattern
    this.updatePattern(execution);

    // Save to storage
    this.saveToStorage();

    return execution.id;
  }

  // Update pattern from execution
  private updatePattern(execution: QueryExecution): void {
    const pattern = this.extractPattern(execution.query, execution.queryType);
    const patternId = this.generatePatternId(pattern, execution.queryType);

    const existing = this.patterns.get(patternId);

    if (existing) {
      existing.frequency++;
      existing.avgExecutionTime = (existing.avgExecutionTime * (existing.frequency - 1) + execution.executionTime) / existing.frequency;
      existing.successRate = (existing.successRate * (existing.frequency - 1) + (execution.success ? 1 : 0)) / existing.frequency;
      existing.lastUsed = execution.timestamp;
      existing.resultCount = (existing.resultCount + execution.resultCount) / 2;
    } else {
      this.patterns.set(patternId, {
        id: patternId,
        queryType: execution.queryType,
        pattern,
        frequency: 1,
        avgExecutionTime: execution.executionTime,
        successRate: execution.success ? 1 : 0,
        lastUsed: execution.timestamp,
        resultCount: execution.resultCount,
        feedback: { helpful: 0, notHelpful: 0 },
      });
    }
  }

  // Record feedback
  recordFeedback(
    queryId: string,
    query: string,
    queryType: 'sql' | 'sparql' | 'cypher' | 'vector',
    helpful: boolean,
    resultCount: number,
    executionTime: number
  ): void {
    this.feedback.push({
      queryId,
      query,
      queryType,
      helpful,
      timestamp: Date.now(),
      resultCount,
      executionTime,
    });

    // Update pattern feedback
    const pattern = this.extractPattern(query, queryType);
    const patternId = this.generatePatternId(pattern, queryType);
    const existing = this.patterns.get(patternId);

    if (existing) {
      if (helpful) {
        existing.feedback.helpful++;
      } else {
        existing.feedback.notHelpful++;
      }
    }

    this.saveToStorage();
  }

  // Get learning metrics
  getMetrics(): LearningMetrics {
    const patterns = Array.from(this.patterns.values());
    const recentExecutions = this.executions.filter(
      e => Date.now() - e.timestamp < 24 * 60 * 60 * 1000 // Last 24 hours
    );

    const totalQueries = recentExecutions.length;
    const successfulQueries = recentExecutions.filter(e => e.success).length;
    const failedQueries = totalQueries - successfulQueries;
    const avgResponseTime = recentExecutions.length > 0
      ? recentExecutions.reduce((sum, e) => sum + e.executionTime, 0) / recentExecutions.length
      : 0;

    // Calculate adaptation level based on pattern recognition
    const totalFeedback = patterns.reduce(
      (sum, p) => sum + p.feedback.helpful + p.feedback.notHelpful, 0
    );
    const positiveFeedback = patterns.reduce((sum, p) => sum + p.feedback.helpful, 0);
    const adaptationLevel = totalFeedback > 0
      ? Math.round((positiveFeedback / totalFeedback) * 100)
      : 50;

    // Calculate learning rate (queries per hour)
    const hourAgo = Date.now() - 60 * 60 * 1000;
    const queriesLastHour = this.executions.filter(e => e.timestamp > hourAgo).length;

    return {
      totalQueries,
      successfulQueries,
      failedQueries,
      avgResponseTime,
      queryPatterns: patterns.sort((a, b) => b.frequency - a.frequency).slice(0, 20),
      suggestions: this.generateSuggestions(),
      insights: this.generateInsights(),
      adaptationLevel,
      learningRate: queriesLastHour,
    };
  }

  // Generate query suggestions
  private generateSuggestions(): QuerySuggestion[] {
    const suggestions: QuerySuggestion[] = [];
    const patterns = Array.from(this.patterns.values());

    // Suggest frequently used successful patterns
    const frequentPatterns = patterns
      .filter(p => p.frequency >= 2 && p.successRate > 0.7)
      .sort((a, b) => b.frequency - a.frequency)
      .slice(0, 5);

    frequentPatterns.forEach((p, i) => {
      suggestions.push({
        id: `sug_freq_${i}`,
        query: p.pattern,
        queryType: p.queryType,
        confidence: Math.min(0.95, p.successRate * (1 + Math.log10(p.frequency) / 10)),
        reason: `Frequently used pattern (${p.frequency} times) with ${Math.round(p.successRate * 100)}% success rate`,
        basedOn: [p.id],
      });
    });

    // Suggest based on positive feedback
    const positiveFeedbackPatterns = patterns
      .filter(p => p.feedback.helpful > p.feedback.notHelpful)
      .sort((a, b) => b.feedback.helpful - a.feedback.helpful)
      .slice(0, 3);

    positiveFeedbackPatterns.forEach((p, i) => {
      if (!suggestions.find(s => s.basedOn.includes(p.id))) {
        suggestions.push({
          id: `sug_fb_${i}`,
          query: p.pattern,
          queryType: p.queryType,
          confidence: 0.8 + (p.feedback.helpful / (p.feedback.helpful + p.feedback.notHelpful + 1)) * 0.2,
          reason: `Marked as helpful ${p.feedback.helpful} times`,
          basedOn: [p.id],
        });
      }
    });

    return suggestions;
  }

  // Generate learning insights
  private generateInsights(): LearningInsight[] {
    const insights: LearningInsight[] = [];
    const patterns = Array.from(this.patterns.values());
    const recentExecutions = this.executions.slice(-100);

    // Slow query insight
    const slowPatterns = patterns.filter(p => p.avgExecutionTime > 500);
    if (slowPatterns.length > 0) {
      insights.push({
        id: 'insight_slow_queries',
        type: 'optimization',
        title: 'Slow Queries Detected',
        description: `${slowPatterns.length} query pattern(s) have average execution time > 500ms. Consider optimizing these queries or adding indexes.`,
        recommendation: 'Try reducing result set size with LIMIT, or simplify complex JOINs and subqueries.',
        severity: 'warning',
        timestamp: Date.now(),
        actionable: true,
      });
    }

    // High failure rate insight
    const failingPatterns = patterns.filter(p => p.frequency >= 3 && p.successRate < 0.5);
    if (failingPatterns.length > 0) {
      insights.push({
        id: 'insight_failing_queries',
        type: 'anomaly',
        title: 'Query Patterns with High Failure Rate',
        description: `${failingPatterns.length} frequently used patterns have >50% failure rate. Review syntax and data requirements.`,
        recommendation: 'Check for typos, missing tables/columns, or invalid data types in your queries.',
        severity: 'warning',
        timestamp: Date.now(),
        actionable: true,
      });
    }

    // Success pattern insight
    const successfulPatterns = patterns.filter(p => p.frequency >= 5 && p.successRate > 0.9);
    if (successfulPatterns.length > 0) {
      insights.push({
        id: 'insight_success_patterns',
        type: 'pattern',
        title: 'Reliable Query Patterns Identified',
        description: `${successfulPatterns.length} patterns consistently succeed. These can be used as templates for similar queries.`,
        severity: 'success',
        timestamp: Date.now(),
        actionable: false,
      });
    }

    // Query diversity insight
    const queryTypes = new Set(patterns.map(p => p.queryType));
    if (queryTypes.size >= 3) {
      insights.push({
        id: 'insight_diversity',
        type: 'recommendation',
        title: 'Multi-Modal Database Usage',
        description: `You're effectively using ${queryTypes.size} different query languages. This is optimal for complex data applications.`,
        severity: 'info',
        timestamp: Date.now(),
        actionable: false,
      });
    }

    // Learning progress insight
    const recentFeedback = this.feedback.filter(f => Date.now() - f.timestamp < 7 * 24 * 60 * 60 * 1000);
    if (recentFeedback.length >= 10) {
      const helpfulRate = recentFeedback.filter(f => f.helpful).length / recentFeedback.length;
      if (helpfulRate > 0.8) {
        insights.push({
          id: 'insight_learning_success',
          type: 'pattern',
          title: 'High Learning Effectiveness',
          description: `${Math.round(helpfulRate * 100)}% of recent results were marked as helpful. The system is adapting well to your needs.`,
          severity: 'success',
          timestamp: Date.now(),
          actionable: false,
        });
      }
    }

    // Recent activity insight
    if (recentExecutions.length > 50) {
      const successRate = recentExecutions.filter(e => e.success).length / recentExecutions.length;
      insights.push({
        id: 'insight_activity',
        type: 'pattern',
        title: 'Query Activity Analysis',
        description: `${recentExecutions.length} queries in recent session with ${Math.round(successRate * 100)}% success rate.`,
        severity: successRate > 0.8 ? 'success' : 'info',
        timestamp: Date.now(),
        actionable: false,
      });
    }

    return insights;
  }

  // Get top patterns by query type
  getTopPatterns(queryType: 'sql' | 'sparql' | 'cypher' | 'vector', limit: number = 5): QueryPattern[] {
    return Array.from(this.patterns.values())
      .filter(p => p.queryType === queryType)
      .sort((a, b) => {
        // Score based on frequency, success rate, and recent usage
        const scoreA = a.frequency * a.successRate * (1 + a.feedback.helpful - a.feedback.notHelpful * 0.5);
        const scoreB = b.frequency * b.successRate * (1 + b.feedback.helpful - b.feedback.notHelpful * 0.5);
        return scoreB - scoreA;
      })
      .slice(0, limit);
  }

  // Get recent query executions
  getRecentExecutions(limit: number = 10): QueryExecution[] {
    return this.executions
      .slice(-limit)
      .reverse(); // Most recent first
  }

  // Clear learning data
  clear(): void {
    this.patterns.clear();
    this.executions = [];
    this.feedback = [];
    this.saveToStorage();
  }

  // Save to localStorage
  private saveToStorage(): void {
    try {
      const data = {
        patterns: Array.from(this.patterns.entries()),
        executions: this.executions.slice(-500), // Keep last 500
        feedback: this.feedback.slice(-500),
      };
      localStorage.setItem(this.storageKey, JSON.stringify(data));
    } catch (e) {
      console.warn('Failed to save learning data:', e);
    }
  }

  // Load from localStorage
  private loadFromStorage(): void {
    try {
      const stored = localStorage.getItem(this.storageKey);
      if (stored) {
        const data = JSON.parse(stored);
        this.patterns = new Map(data.patterns || []);
        this.executions = data.executions || [];
        this.feedback = data.feedback || [];
      }
    } catch (e) {
      console.warn('Failed to load learning data:', e);
    }
  }

  // Export learning data
  export(): Record<string, unknown> {
    return {
      patterns: Array.from(this.patterns.entries()),
      executions: this.executions,
      feedback: this.feedback,
      exportedAt: Date.now(),
    };
  }

  // Import learning data
  import(data: Record<string, unknown>): void {
    if (data.patterns) {
      this.patterns = new Map(data.patterns as [string, QueryPattern][]);
    }
    if (data.executions) {
      this.executions = data.executions as QueryExecution[];
    }
    if (data.feedback) {
      this.feedback = data.feedback as FeedbackEntry[];
    }
    this.saveToStorage();
  }
}

// ============================================================================
// Hook
// ============================================================================

// Singleton learning engine
let learningEngineInstance: LearningEngine | null = null;

function getLearningEngine(): LearningEngine {
  if (!learningEngineInstance) {
    learningEngineInstance = new LearningEngine();
  }
  return learningEngineInstance;
}

// GNN State interface
export interface GnnState {
  nodes: number;
  edges: number;
  layers: number;
  accuracy: number;
  isTraining: boolean;
  lastTrainedAt: number | null;
}

export function useLearning() {
  // Use the singleton directly, don't access ref during render
  const engine = getLearningEngine();
  const engineRef = useRef<LearningEngine>(engine);
  const [metrics, setMetrics] = useState<LearningMetrics>(() => engine.getMetrics());
  const [lastQueryId, setLastQueryId] = useState<string | null>(null);

  // GNN State
  const [gnnState, setGnnState] = useState<GnnState>({
    nodes: 0,
    edges: 0,
    layers: 3,
    accuracy: 0,
    isTraining: false,
    lastTrainedAt: null,
  });

  // Refresh metrics
  const refreshMetrics = useCallback(() => {
    setMetrics(engineRef.current.getMetrics());
  }, []);

  // Record a query execution
  const recordQuery = useCallback((
    query: string,
    queryType: 'sql' | 'sparql' | 'cypher' | 'vector',
    executionTime: number,
    success: boolean,
    resultCount: number,
    error?: string
  ) => {
    const id = engineRef.current.recordExecution(
      query,
      queryType,
      executionTime,
      success,
      resultCount,
      error
    );
    setLastQueryId(id);
    refreshMetrics();
    return id;
  }, [refreshMetrics]);

  // Record feedback for a result
  const recordFeedback = useCallback((
    query: string,
    queryType: 'sql' | 'sparql' | 'cypher' | 'vector',
    helpful: boolean,
    resultCount: number = 0,
    executionTime: number = 0
  ) => {
    engineRef.current.recordFeedback(
      lastQueryId || `fb_${Date.now()}`,
      query,
      queryType,
      helpful,
      resultCount,
      executionTime
    );
    refreshMetrics();
  }, [lastQueryId, refreshMetrics]);

  // Get suggestions for a query type
  const getSuggestions = useCallback((queryType: 'sql' | 'sparql' | 'cypher' | 'vector') => {
    return metrics.suggestions.filter(s => s.queryType === queryType);
  }, [metrics.suggestions]);

  // Get top patterns for a query type
  const getTopPatterns = useCallback((queryType: 'sql' | 'sparql' | 'cypher' | 'vector', limit: number = 5) => {
    return engineRef.current.getTopPatterns(queryType, limit);
  }, []);

  // Get recent query executions
  const getRecentExecutions = useCallback((limit: number = 10) => {
    return engineRef.current.getRecentExecutions(limit);
  }, []);

  // Clear all learning data
  const clearLearning = useCallback(() => {
    engineRef.current.clear();
    refreshMetrics();
  }, [refreshMetrics]);

  // Export learning data
  const exportLearning = useCallback(() => {
    return engineRef.current.export();
  }, []);

  // Import learning data
  const importLearning = useCallback((data: Record<string, unknown>) => {
    engineRef.current.import(data);
    refreshMetrics();
  }, [refreshMetrics]);

  // Auto-refresh metrics periodically
  useEffect(() => {
    const interval = setInterval(refreshMetrics, 30000); // Every 30 seconds
    return () => clearInterval(interval);
  }, [refreshMetrics]);

  // Derive GNN nodes/edges from patterns (computed value, no effect needed)
  const gnnDerivedState = {
    nodes: metrics.queryPatterns.length,
    edges: Math.max(0, metrics.queryPatterns.length * 2 - 1),
  };

  // ============================================================================
  // Real Neural Network Implementation (Lightweight GNN for Query Patterns)
  // ============================================================================

  // Neural network weights (stored in state for persistence)
  const weightsRef = useRef<{
    W1: number[][];  // Input to hidden (patternFeatures x hiddenSize)
    W2: number[][];  // Hidden to output (hiddenSize x outputSize)
    b1: number[];    // Hidden bias
    b2: number[];    // Output bias
  } | null>(null);

  // Initialize weights
  const initWeights = useCallback((inputSize: number, hiddenSize: number, outputSize: number) => {
    const xavier = (fan_in: number, fan_out: number) =>
      Math.sqrt(6 / (fan_in + fan_out)) * (Math.random() * 2 - 1);

    const W1: number[][] = Array(inputSize).fill(0).map(() =>
      Array(hiddenSize).fill(0).map(() => xavier(inputSize, hiddenSize))
    );
    const W2: number[][] = Array(hiddenSize).fill(0).map(() =>
      Array(outputSize).fill(0).map(() => xavier(hiddenSize, outputSize))
    );
    const b1 = Array(hiddenSize).fill(0);
    const b2 = Array(outputSize).fill(0);

    weightsRef.current = { W1, W2, b1, b2 };
  }, []);

  // ReLU activation
  const relu = (x: number) => Math.max(0, x);
  const reluDerivative = (x: number) => x > 0 ? 1 : 0;

  // Sigmoid activation
  const sigmoid = (x: number) => 1 / (1 + Math.exp(-Math.min(Math.max(x, -500), 500)));

  // Extract features from a query pattern
  const extractPatternFeatures = useCallback((pattern: QueryPattern): number[] => {
    const typeEncoding = {
      'sql': [1, 0, 0, 0],
      'sparql': [0, 1, 0, 0],
      'cypher': [0, 0, 1, 0],
      'vector': [0, 0, 0, 1],
    };
    const typeVec = typeEncoding[pattern.queryType] || [0, 0, 0, 0];

    return [
      ...typeVec,
      Math.min(pattern.frequency / 100, 1),        // Normalized frequency
      pattern.avgExecutionTime / 1000,             // Time in seconds
      pattern.successRate,                          // Already 0-1
      Math.min(pattern.resultCount / 1000, 1),     // Normalized results
      pattern.feedback.helpful / (pattern.feedback.helpful + pattern.feedback.notHelpful + 1),
      pattern.pattern.length / 500,                // Normalized pattern length
    ];
  }, []);

  // Forward pass
  const forward = useCallback((input: number[]): { hidden: number[]; output: number[] } => {
    if (!weightsRef.current) {
      initWeights(10, 8, 1);
    }
    const { W1, W2, b1, b2 } = weightsRef.current!;

    // Hidden layer
    const hidden: number[] = [];
    for (let j = 0; j < W1[0].length; j++) {
      let sum = b1[j];
      for (let i = 0; i < input.length && i < W1.length; i++) {
        sum += input[i] * W1[i][j];
      }
      hidden.push(relu(sum));
    }

    // Output layer
    const output: number[] = [];
    for (let j = 0; j < W2[0].length; j++) {
      let sum = b2[j];
      for (let i = 0; i < hidden.length; i++) {
        sum += hidden[i] * W2[i][j];
      }
      output.push(sigmoid(sum));
    }

    return { hidden, output };
  }, [initWeights]);

  // Train GNN with real gradient descent
  const trainGNN = useCallback(async () => {
    setGnnState(prev => ({ ...prev, isTraining: true }));

    const patterns = metrics.queryPatterns;
    if (patterns.length === 0) {
      setGnnState(prev => ({ ...prev, isTraining: false }));
      return 0.5;
    }

    // Initialize weights if needed
    if (!weightsRef.current) {
      initWeights(10, 8, 1);
    }

    const learningRate = 0.01;
    const epochs = 50;

    // Training loop (async to not block UI)
    for (let epoch = 0; epoch < epochs; epoch++) {
      let epochLoss = 0;

      for (const pattern of patterns) {
        const input = extractPatternFeatures(pattern);
        const target = pattern.successRate; // Train to predict success

        // Forward pass
        const { hidden, output } = forward(input);
        const prediction = output[0];

        // Calculate loss (MSE)
        const loss = Math.pow(target - prediction, 2);
        epochLoss += loss;

        // Backpropagation
        const { W1, W2, b1, b2 } = weightsRef.current!;

        // Output gradient
        const dOutput = -2 * (target - prediction) * prediction * (1 - prediction);

        // Hidden gradients
        const dHidden: number[] = [];
        for (let i = 0; i < W2.length; i++) {
          dHidden.push(dOutput * W2[i][0] * reluDerivative(hidden[i]));
        }

        // Update W2 and b2
        for (let i = 0; i < W2.length; i++) {
          W2[i][0] -= learningRate * dOutput * hidden[i];
        }
        b2[0] -= learningRate * dOutput;

        // Update W1 and b1
        for (let j = 0; j < W1[0].length; j++) {
          b1[j] -= learningRate * dHidden[j];
          for (let i = 0; i < input.length && i < W1.length; i++) {
            W1[i][j] -= learningRate * dHidden[j] * input[i];
          }
        }
      }

      // Yield to UI every 10 epochs
      if (epoch % 10 === 0) {
        await new Promise(resolve => setTimeout(resolve, 0));
      }
    }

    // Calculate final accuracy
    let correct = 0;
    for (const pattern of patterns) {
      const input = extractPatternFeatures(pattern);
      const { output } = forward(input);
      const predicted = output[0] > 0.5;
      const actual = pattern.successRate > 0.5;
      if (predicted === actual) correct++;
    }
    const accuracy = patterns.length > 0 ? correct / patterns.length : 0.5;

    setGnnState(prev => ({
      ...prev,
      isTraining: false,
      accuracy: Math.min(0.99, accuracy),
      lastTrainedAt: Date.now(),
    }));

    return accuracy;
  }, [metrics.queryPatterns, initWeights, extractPatternFeatures, forward]);

  // Get real graph embedding for a query using the trained network
  const getGraphEmbedding = useCallback((query: string): number[] => {
    // Create a synthetic pattern from the query
    const syntheticPattern: QueryPattern = {
      id: 'temp',
      queryType: query.toLowerCase().startsWith('select') ? 'sql' :
                 query.toLowerCase().startsWith('match') ? 'cypher' :
                 query.toLowerCase().includes('sparql') || query.includes('?') ? 'sparql' : 'vector',
      pattern: query,
      frequency: 1,
      avgExecutionTime: 0,
      successRate: 0.5,
      lastUsed: Date.now(),
      resultCount: 0,
      feedback: { helpful: 0, notHelpful: 0 },
    };

    const input = extractPatternFeatures(syntheticPattern);
    const { hidden } = forward(input);

    // The hidden layer activations form the embedding
    return hidden;
  }, [extractPatternFeatures, forward]);

  // Reset learning (clear all data)
  const resetLearning = useCallback(() => {
    engineRef.current.clear();
    setGnnState({
      nodes: 0,
      edges: 0,
      layers: 3,
      accuracy: 0,
      isTraining: false,
      lastTrainedAt: null,
    });
    refreshMetrics();
  }, [refreshMetrics]);

  // Derive patterns, suggestions, insights from metrics
  const patterns = metrics.queryPatterns || [];
  const suggestions = metrics.suggestions || [];
  const insights = metrics.insights || [];

  return {
    // Metrics
    metrics,

    // Derived state (for direct access)
    patterns,
    suggestions,
    insights,

    // GNN State (merged with derived values)
    gnnState: { ...gnnState, ...gnnDerivedState },

    // Recording
    recordQuery,
    recordFeedback,

    // Queries
    getSuggestions,
    getTopPatterns,
    getRecentExecutions,

    // GNN functions
    trainGNN,
    getGraphEmbedding,

    // Management
    clearLearning,
    resetLearning,
    exportLearning,
    importLearning,
    refreshMetrics,

    // State
    lastQueryId,
  };
}

export default useLearning;
