/**
 * AgentDB Client Implementation
 * High-performance vector database with HNSW search and QUIC synchronization
 */

import { createDatabase } from 'agentdb';
import {
  ThreatMatch,
  ThreatIncident,
  VectorSearchOptions,
  ReflexionMemoryEntry,
  ThreatLevel,
  AgentDBConfig
} from '../types';
import { Logger } from '../utils/logger';

export class AgentDBClient {
  private db: any; // AgentDB database instance
  private logger: Logger;
  private config: AgentDBConfig;
  private syncInterval?: NodeJS.Timeout;

  constructor(config: AgentDBConfig, logger: Logger) {
    this.config = config;
    this.logger = logger;
    // createDatabase accepts a filename string
    this.db = createDatabase(config.path);
  }

  /**
   * Initialize AgentDB with HNSW index and QUIC sync
   */
  async initialize(): Promise<void> {
    try {
      this.logger.info('Initializing AgentDB client...');

      // Create HNSW index for fast vector search (150x faster than brute force)
      await this.db.createIndex({
        type: 'hnsw',
        params: {
          m: this.config.hnswConfig.m,
          efConstruction: this.config.hnswConfig.efConstruction,
          efSearch: this.config.hnswConfig.efSearch,
          metric: 'cosine'
        }
      });

      // Initialize collections
      await this.createCollections();

      // Setup QUIC synchronization if enabled
      if (this.config.quicSync.enabled) {
        await this.initializeQuicSync();
      }

      this.logger.info('AgentDB client initialized successfully');
    } catch (error) {
      this.logger.error('Failed to initialize AgentDB', { error });
      throw error;
    }
  }

  /**
   * Fast vector search with HNSW and MMR diversity
   * Target: <2ms for k=10
   */
  async vectorSearch(
    embedding: number[],
    options: VectorSearchOptions = { k: 10 }
  ): Promise<ThreatMatch[]> {
    const startTime = Date.now();

    try {
      // HNSW search with specified parameters
      const results = await this.db.search({
        collection: 'threat_patterns',
        vector: embedding,
        k: options.k,
        ef: options.ef || this.config.hnswConfig.efSearch
      });

      // Apply MMR (Maximal Marginal Relevance) for diversity if requested
      const matches = options.diversityFactor
        ? this.applyMMR(results, options.diversityFactor)
        : results;

      // Convert to ThreatMatch objects
      const threatMatches: ThreatMatch[] = matches
        .filter((m: any) => m.similarity >= (options.threshold || 0.7))
        .map((m: any) => ({
          id: m.id,
          patternId: m.metadata.patternId,
          similarity: m.similarity,
          threatLevel: this.calculateThreatLevel(m.similarity, m.metadata),
          description: m.metadata.description || 'Unknown threat pattern',
          metadata: {
            firstSeen: m.metadata.firstSeen || Date.now(),
            lastSeen: m.metadata.lastSeen || Date.now(),
            occurrences: m.metadata.occurrences || 1,
            sources: m.metadata.sources || []
          }
        }));

      const latency = Date.now() - startTime;
      this.logger.debug('Vector search completed', {
        latency,
        resultsCount: threatMatches.length,
        threshold: options.threshold
      });

      return threatMatches;
    } catch (error) {
      this.logger.error('Vector search failed', { error });
      throw error;
    }
  }

  /**
   * Store security incident in ReflexionMemory for learning
   */
  async storeIncident(incident: ThreatIncident): Promise<void> {
    try {
      // Store in main incidents collection
      await this.db.insert({
        collection: 'incidents',
        document: {
          id: incident.id,
          timestamp: incident.timestamp,
          request: incident.request,
          result: incident.result,
          embedding: incident.embedding
        }
      });

      // Update threat patterns if this is a new pattern
      if (incident.result.threatLevel >= ThreatLevel.MEDIUM) {
        await this.updateThreatPattern(incident);
      }

      // Store in ReflexionMemory for learning
      const reflexionEntry: ReflexionMemoryEntry = {
        trajectory: JSON.stringify({
          request: incident.request,
          matches: incident.result.matches
        }),
        verdict: incident.result.allowed ? 'success' : 'failure',
        feedback: this.generateFeedback(incident),
        embedding: incident.embedding || [],
        metadata: {
          threatLevel: incident.result.threatLevel,
          confidence: incident.result.confidence,
          latency: incident.result.latencyMs
        }
      };

      await this.db.insert({
        collection: 'reflexion_memory',
        document: reflexionEntry
      });

      // Update causal graphs
      if (incident.causalLinks && incident.causalLinks.length > 0) {
        await this.updateCausalGraph(incident);
      }

      this.logger.debug('Incident stored successfully', { id: incident.id });
    } catch (error) {
      this.logger.error('Failed to store incident', { error, incidentId: incident.id });
      throw error;
    }
  }

  /**
   * Synchronize with peer nodes using QUIC
   */
  async syncWithPeers(): Promise<void> {
    if (!this.config.quicSync.enabled) {
      return;
    }

    try {
      const syncPromises = this.config.quicSync.peers.map(peer =>
        this.db.sync({
          peer,
          protocol: 'quic',
          port: this.config.quicSync.port,
          collections: ['threat_patterns', 'incidents', 'reflexion_memory']
        })
      );

      await Promise.all(syncPromises);
      this.logger.debug('QUIC synchronization completed');
    } catch (error) {
      this.logger.error('QUIC synchronization failed', { error });
      // Don't throw - sync failures shouldn't break the gateway
    }
  }

  /**
   * Get statistics about stored data
   */
  async getStats(): Promise<{
    incidents: number;
    patterns: number;
    memoryEntries: number;
    memoryUsage: number;
  }> {
    const [incidents, patterns, memoryEntries] = await Promise.all([
      this.db.count({ collection: 'incidents' }),
      this.db.count({ collection: 'threat_patterns' }),
      this.db.count({ collection: 'reflexion_memory' })
    ]);

    return {
      incidents,
      patterns,
      memoryEntries,
      memoryUsage: this.db.getMemoryUsage()
    };
  }

  /**
   * Clean up old entries based on TTL
   */
  async cleanup(): Promise<void> {
    const cutoffTime = Date.now() - this.config.memory.ttl;

    await Promise.all([
      this.db.delete({
        collection: 'incidents',
        filter: { timestamp: { $lt: cutoffTime } }
      }),
      this.db.delete({
        collection: 'reflexion_memory',
        filter: { timestamp: { $lt: cutoffTime } }
      })
    ]);

    this.logger.debug('Cleanup completed');
  }

  /**
   * Shutdown and cleanup resources
   */
  async shutdown(): Promise<void> {
    if (this.syncInterval) {
      clearInterval(this.syncInterval);
    }

    await this.db.close();
    this.logger.info('AgentDB client shutdown complete');
  }

  // ============================================================================
  // Private Helper Methods
  // ============================================================================

  private async createCollections(): Promise<void> {
    await Promise.all([
      this.db.createCollection({
        name: 'threat_patterns',
        schema: {
          embedding: { type: 'vector', dim: this.config.embeddingDim },
          metadata: { type: 'object' }
        }
      }),
      this.db.createCollection({
        name: 'incidents',
        schema: {
          id: { type: 'string', indexed: true },
          timestamp: { type: 'number', indexed: true },
          embedding: { type: 'vector', dim: this.config.embeddingDim }
        }
      }),
      this.db.createCollection({
        name: 'reflexion_memory',
        schema: {
          embedding: { type: 'vector', dim: this.config.embeddingDim },
          verdict: { type: 'string', indexed: true }
        }
      })
    ]);
  }

  private async initializeQuicSync(): Promise<void> {
    // Start periodic sync every 30 seconds
    this.syncInterval = setInterval(() => {
      this.syncWithPeers().catch(err =>
        this.logger.error('Periodic sync failed', { error: err })
      );
    }, 30000);

    // Initial sync
    await this.syncWithPeers();
  }

  private applyMMR(results: any[], lambda: number): any[] {
    // Maximal Marginal Relevance for diversity
    // lambda: 1.0 = max relevance, 0.0 = max diversity
    const selected: any[] = [];
    const candidates = [...results];

    while (selected.length < results.length && candidates.length > 0) {
      let maxScore = -Infinity;
      let maxIdx = -1;

      candidates.forEach((candidate, idx) => {
        const relevance = candidate.similarity;
        const maxSim = selected.length === 0
          ? 0
          : Math.max(...selected.map(s => this.cosineSimilarity(candidate.embedding, s.embedding)));

        const score = lambda * relevance - (1 - lambda) * maxSim;

        if (score > maxScore) {
          maxScore = score;
          maxIdx = idx;
        }
      });

      if (maxIdx >= 0) {
        selected.push(candidates[maxIdx]);
        candidates.splice(maxIdx, 1);
      }
    }

    return selected;
  }

  private cosineSimilarity(a: number[], b: number[]): number {
    let dotProduct = 0;
    let normA = 0;
    let normB = 0;

    for (let i = 0; i < a.length; i++) {
      dotProduct += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }

    return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
  }

  private calculateThreatLevel(similarity: number, metadata: any): ThreatLevel {
    // Calculate threat level based on similarity and metadata
    const baseThreat = metadata.threatLevel || ThreatLevel.LOW;

    if (similarity >= 0.95) return Math.max(baseThreat, ThreatLevel.HIGH);
    if (similarity >= 0.85) return Math.max(baseThreat, ThreatLevel.MEDIUM);
    if (similarity >= 0.75) return baseThreat;
    return ThreatLevel.LOW;
  }

  private async updateThreatPattern(incident: ThreatIncident): Promise<void> {
    // Update or create threat pattern based on incident
    if (!incident.embedding) return;

    await this.db.upsert({
      collection: 'threat_patterns',
      document: {
        patternId: incident.id,
        embedding: incident.embedding,
        metadata: {
          description: `Threat pattern from incident ${incident.id}`,
          threatLevel: incident.result.threatLevel,
          lastSeen: incident.timestamp,
          occurrences: 1
        }
      }
    });
  }

  private generateFeedback(incident: ThreatIncident): string {
    const { result } = incident;
    return `Threat level: ${ThreatLevel[result.threatLevel]}, ` +
           `Confidence: ${(result.confidence * 100).toFixed(1)}%, ` +
           `Path: ${result.metadata.pathTaken}, ` +
           `Latency: ${result.latencyMs.toFixed(2)}ms`;
  }

  private async updateCausalGraph(incident: ThreatIncident): Promise<void> {
    // Update causal relationship graph
    for (const link of incident.causalLinks || []) {
      await this.db.insert({
        collection: 'causal_graph',
        document: {
          from: incident.id,
          to: link,
          timestamp: incident.timestamp,
          weight: 1.0
        }
      });
    }
  }
}
