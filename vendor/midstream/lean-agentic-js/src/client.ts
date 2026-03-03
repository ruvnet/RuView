/**
 * Client for interacting with the Lean Agentic Learning System
 */

import axios, { AxiosInstance } from 'axios';
import {
  LeanAgenticConfig,
  Context,
  ProcessingResult,
  SystemStats,
  Entity,
  Theorem,
  LearningStats
} from './types';

export class LeanAgenticClient {
  private client: AxiosInstance;
  private config: LeanAgenticConfig;

  constructor(baseURL: string, config: Partial<LeanAgenticConfig> = {}) {
    this.client = axios.create({
      baseURL,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json',
      },
    });

    this.config = {
      enableFormalVerification: true,
      learningRate: 0.01,
      maxPlanningDepth: 5,
      actionThreshold: 0.7,
      enableMultiAgent: true,
      kgUpdateFreq: 100,
      ...config,
    };
  }

  /**
   * Process a stream chunk through the lean agentic system
   */
  async processChunk(
    chunk: string,
    context: Context
  ): Promise<ProcessingResult> {
    const response = await this.client.post<ProcessingResult>('/process', {
      chunk,
      context,
      config: this.config,
    });

    return response.data;
  }

  /**
   * Get system statistics
   */
  async getStats(): Promise<SystemStats> {
    const response = await this.client.get<SystemStats>('/stats');
    return response.data;
  }

  /**
   * Query entities from knowledge graph
   */
  async queryEntities(query: {
    entityType?: string;
    searchText?: string;
    limit?: number;
  }): Promise<Entity[]> {
    const response = await this.client.get<Entity[]>('/knowledge/entities', {
      params: query,
    });

    return response.data;
  }

  /**
   * Get theorems from the formal reasoning system
   */
  async getTheorems(tags?: string[]): Promise<Theorem[]> {
    const response = await this.client.get<Theorem[]>('/reasoning/theorems', {
      params: { tags: tags?.join(',') },
    });

    return response.data;
  }

  /**
   * Get learning statistics
   */
  async getLearningStats(): Promise<LearningStats> {
    const response = await this.client.get<LearningStats>('/learning/stats');
    return response.data;
  }

  /**
   * Update system configuration
   */
  async updateConfig(config: Partial<LeanAgenticConfig>): Promise<void> {
    this.config = { ...this.config, ...config };
    await this.client.post('/config', this.config);
  }

  /**
   * Create a new context
   */
  createContext(sessionId: string): Context {
    return {
      history: [],
      preferences: {},
      sessionId,
      environment: {},
      timestamp: Date.now(),
    };
  }
}

export default LeanAgenticClient;
