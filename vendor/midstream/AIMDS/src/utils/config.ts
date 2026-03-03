/**
 * Configuration Management
 * Load and validate configuration from environment
 */

import { config as loadEnv } from 'dotenv';
import { z } from 'zod';
import { GatewayConfig, AgentDBConfig, LeanAgenticConfig } from '../types';

// Load environment variables
loadEnv();

// Configuration schema
const ConfigSchema = z.object({
  // Gateway config
  GATEWAY_PORT: z.string().default('3000'),
  GATEWAY_HOST: z.string().default('0.0.0.0'),
  ENABLE_COMPRESSION: z.string().default('true'),
  ENABLE_CORS: z.string().default('true'),
  RATE_LIMIT_WINDOW_MS: z.string().default('60000'),
  RATE_LIMIT_MAX: z.string().default('1000'),
  REQUEST_TIMEOUT: z.string().default('30000'),
  SHUTDOWN_TIMEOUT: z.string().default('10000'),

  // AgentDB config
  AGENTDB_PATH: z.string().default('./data/agentdb'),
  AGENTDB_EMBEDDING_DIM: z.string().default('384'),
  AGENTDB_HNSW_M: z.string().default('16'),
  AGENTDB_HNSW_EF_CONSTRUCTION: z.string().default('200'),
  AGENTDB_HNSW_EF_SEARCH: z.string().default('100'),
  AGENTDB_QUIC_ENABLED: z.string().default('false'),
  AGENTDB_QUIC_PEERS: z.string().default(''),
  AGENTDB_QUIC_PORT: z.string().default('4433'),
  AGENTDB_MEMORY_MAX_ENTRIES: z.string().default('100000'),
  AGENTDB_MEMORY_TTL: z.string().default('86400000'),

  // lean-agentic config
  LEAN_ENABLE_HASH_CONS: z.string().default('true'),
  LEAN_ENABLE_DEPENDENT_TYPES: z.string().default('true'),
  LEAN_ENABLE_THEOREM_PROVING: z.string().default('true'),
  LEAN_CACHE_SIZE: z.string().default('10000'),
  LEAN_PROOF_TIMEOUT: z.string().default('5000'),

  // Logging
  LOG_LEVEL: z.string().default('info'),
  NODE_ENV: z.string().default('development')
});

export class Config {
  private static instance: Config;
  private env: z.infer<typeof ConfigSchema>;

  private constructor() {
    this.env = ConfigSchema.parse(process.env);
  }

  static getInstance(): Config {
    if (!Config.instance) {
      Config.instance = new Config();
    }
    return Config.instance;
  }

  getGatewayConfig(): GatewayConfig {
    return {
      port: parseInt(this.env.GATEWAY_PORT),
      host: this.env.GATEWAY_HOST,
      enableCompression: this.env.ENABLE_COMPRESSION === 'true',
      enableCors: this.env.ENABLE_CORS === 'true',
      rateLimit: {
        windowMs: parseInt(this.env.RATE_LIMIT_WINDOW_MS),
        max: parseInt(this.env.RATE_LIMIT_MAX)
      },
      timeouts: {
        request: parseInt(this.env.REQUEST_TIMEOUT),
        shutdown: parseInt(this.env.SHUTDOWN_TIMEOUT)
      }
    };
  }

  getAgentDBConfig(): AgentDBConfig {
    return {
      path: this.env.AGENTDB_PATH,
      embeddingDim: parseInt(this.env.AGENTDB_EMBEDDING_DIM),
      hnswConfig: {
        m: parseInt(this.env.AGENTDB_HNSW_M),
        efConstruction: parseInt(this.env.AGENTDB_HNSW_EF_CONSTRUCTION),
        efSearch: parseInt(this.env.AGENTDB_HNSW_EF_SEARCH)
      },
      quicSync: {
        enabled: this.env.AGENTDB_QUIC_ENABLED === 'true',
        peers: this.env.AGENTDB_QUIC_PEERS.split(',').filter(p => p.length > 0),
        port: parseInt(this.env.AGENTDB_QUIC_PORT)
      },
      memory: {
        maxEntries: parseInt(this.env.AGENTDB_MEMORY_MAX_ENTRIES),
        ttl: parseInt(this.env.AGENTDB_MEMORY_TTL)
      }
    };
  }

  getLeanAgenticConfig(): LeanAgenticConfig {
    return {
      enableHashCons: this.env.LEAN_ENABLE_HASH_CONS === 'true',
      enableDependentTypes: this.env.LEAN_ENABLE_DEPENDENT_TYPES === 'true',
      enableTheoremProving: this.env.LEAN_ENABLE_THEOREM_PROVING === 'true',
      cacheSize: parseInt(this.env.LEAN_CACHE_SIZE),
      proofTimeout: parseInt(this.env.LEAN_PROOF_TIMEOUT)
    };
  }

  get nodeEnv(): string {
    return this.env.NODE_ENV;
  }

  get logLevel(): string {
    return this.env.LOG_LEVEL;
  }
}
