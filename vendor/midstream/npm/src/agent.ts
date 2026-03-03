/**
 * MidStream Agent - High-level wrapper for Lean Agentic Learning System
 */

export interface AgentConfig {
  maxHistory?: number;
  embeddingDim?: number;
  schedulingPolicy?: string;
}

export interface AnalysisResult {
  messageCount: number;
  patterns: any[];
  metaLearning: any;
  temporalAnalysis?: any;
}

export interface BehaviorAnalysis {
  attractorType?: string;
  lyapunovExponent?: number;
  isStable?: boolean;
  isChaotic?: boolean;
}

export class MidStreamAgent {
  private wasmAgent: any;
  private config: AgentConfig;
  private conversationHistory: string[] = [];
  private rewardHistory: number[] = [];

  constructor(config: AgentConfig = {}) {
    this.config = {
      maxHistory: config.maxHistory || 1000,
      embeddingDim: config.embeddingDim || 3,
      schedulingPolicy: config.schedulingPolicy || 'EDF',
    };

    // Load WASM module
    try {
      const wasm = require('../wasm/midstream_wasm');
      this.wasmAgent = new wasm.MidStreamAgent(this.config);
    } catch (error) {
      console.warn('WASM module not available, using fallback implementation');
      this.wasmAgent = null;
    }
  }

  /**
   * Process a single message
   */
  processMessage(message: string): any {
    this.conversationHistory.push(message);

    if (this.conversationHistory.length > this.config.maxHistory!) {
      this.conversationHistory.shift();
    }

    if (this.wasmAgent) {
      return this.wasmAgent.process_message(message);
    }

    // Fallback implementation
    return {
      processed: true,
      message,
      timestamp: Date.now(),
    };
  }

  /**
   * Analyze a complete conversation
   */
  analyzeConversation(messages: string[]): AnalysisResult {
    if (this.wasmAgent) {
      return this.wasmAgent.analyze_conversation(messages);
    }

    // Fallback implementation
    return {
      messageCount: messages.length,
      patterns: [],
      metaLearning: {
        currentLevel: 'Object',
        knowledgeCounts: [messages.length, 0, 0, 0],
      },
    };
  }

  /**
   * Compare two sequences using temporal analysis
   */
  compareSequences(seq1: string[], seq2: string[], algorithm: string = 'dtw'): number {
    if (this.wasmAgent) {
      const comparator = this.wasmAgent.temporal;
      return comparator?.compare(seq1, seq2, algorithm) || 0;
    }

    // Simple fallback: Jaccard similarity
    const set1 = new Set(seq1);
    const set2 = new Set(seq2);
    const intersection = new Set([...set1].filter(x => set2.has(x)));
    const union = new Set([...set1, ...set2]);

    return intersection.size / union.size;
  }

  /**
   * Detect pattern in sequence
   */
  detectPattern(sequence: string[], pattern: string[]): number[] {
    const positions: number[] = [];

    if (pattern.length === 0 || sequence.length < pattern.length) {
      return positions;
    }

    for (let i = 0; i <= sequence.length - pattern.length; i++) {
      let match = true;
      for (let j = 0; j < pattern.length; j++) {
        if (sequence[i + j] !== pattern[j]) {
          match = false;
          break;
        }
      }
      if (match) {
        positions.push(i);
      }
    }

    return positions;
  }

  /**
   * Analyze behavior using attractor analysis
   */
  analyzeBehavior(rewards: number[]): BehaviorAnalysis {
    this.rewardHistory.push(...rewards);

    if (this.rewardHistory.length > this.config.maxHistory!) {
      this.rewardHistory = this.rewardHistory.slice(-this.config.maxHistory!);
    }

    // Simple stability check (fallback)
    const mean = rewards.reduce((a, b) => a + b, 0) / rewards.length;
    const variance = rewards.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / rewards.length;
    const stdDev = Math.sqrt(variance);

    return {
      isStable: stdDev < 0.1,
      isChaotic: stdDev > 0.5,
      lyapunovExponent: stdDev > 0.5 ? 0.5 : -0.5,
    };
  }

  /**
   * Perform meta-learning
   */
  learn(content: string, reward: number): void {
    this.rewardHistory.push(reward);

    if (this.wasmAgent) {
      this.wasmAgent.process_message(content);
    }
  }

  /**
   * Get meta-learning summary
   */
  getMetaLearningSummary(): any {
    if (this.wasmAgent) {
      return this.wasmAgent.get_status();
    }

    return {
      currentLevel: 'Object',
      knowledgeCounts: [this.conversationHistory.length, 0, 0, 0],
      numStrangeLoops: 0,
      numModificationRules: 0,
      safetyViolations: 0,
    };
  }

  /**
   * Get agent status
   */
  getStatus(): any {
    return {
      conversationHistorySize: this.conversationHistory.length,
      rewardHistorySize: this.rewardHistory.length,
      config: this.config,
      metaLearning: this.getMetaLearningSummary(),
      averageReward: this.rewardHistory.length > 0
        ? this.rewardHistory.reduce((a, b) => a + b, 0) / this.rewardHistory.length
        : 0,
    };
  }

  /**
   * Clear all history
   */
  reset(): void {
    this.conversationHistory = [];
    this.rewardHistory = [];
  }
}
