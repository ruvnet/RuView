/**
 * RuVector Hooks Storage Layer
 *
 * Supports PostgreSQL (preferred) with JSON fallback
 * Uses ruvector extension for vector operations and pgvector-compatible storage
 */

import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

// Storage interface for hooks intelligence data
export interface QPattern {
  state: string;
  action: string;
  q_value: number;
  visits: number;
  last_update: number;
}

export interface MemoryEntry {
  id: string;
  memory_type: string;
  content: string;
  embedding: number[];
  metadata: Record<string, string>;
  timestamp: number;
}

export interface Trajectory {
  id: string;
  state: string;
  action: string;
  outcome: string;
  reward: number;
  timestamp: number;
}

export interface ErrorPattern {
  code: string;
  error_type: string;
  message: string;
  fixes: string[];
  occurrences: number;
}

export interface SwarmAgent {
  id: string;
  agent_type: string;
  capabilities: string[];
  success_rate: number;
  task_count: number;
  status: string;
}

export interface SwarmEdge {
  source: string;
  target: string;
  weight: number;
  coordination_count: number;
}

export interface IntelligenceStats {
  total_patterns: number;
  total_memories: number;
  total_trajectories: number;
  total_errors: number;
  session_count: number;
  last_session: number;
}

export interface StorageBackend {
  // Core operations
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  isConnected(): boolean;

  // Q-Learning
  updateQ(state: string, action: string, reward: number): Promise<void>;
  getQ(state: string, action: string): Promise<number>;
  getBestAction(state: string, actions: string[]): Promise<{ action: string; confidence: number }>;

  // Memory
  remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string>;
  recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]>;

  // Trajectories
  recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string>;

  // Errors
  recordError(code: string, errorType: string, message: string): Promise<void>;
  getErrorFixes(code: string): Promise<ErrorPattern | null>;

  // File sequences
  recordSequence(fromFile: string, toFile: string): Promise<void>;
  getNextFiles(file: string, limit: number): Promise<Array<{ file: string; count: number }>>;

  // Swarm
  registerAgent(id: string, type: string, capabilities: string[]): Promise<void>;
  coordinateAgents(source: string, target: string, weight: number): Promise<void>;
  getSwarmStats(): Promise<{ agents: number; edges: number; avgSuccess: number }>;

  // Stats
  sessionStart(): Promise<void>;
  getStats(): Promise<IntelligenceStats>;
}

// ============================================================================
// JSON Storage Backend (Fallback)
// ============================================================================

const JSON_PATH = path.join(os.homedir(), '.ruvector', 'intelligence.json');

interface JsonData {
  patterns: Record<string, QPattern>;
  memories: MemoryEntry[];
  trajectories: Trajectory[];
  errors: Record<string, ErrorPattern>;
  file_sequences: Array<{ from_file: string; to_file: string; count: number }>;
  agents: Record<string, SwarmAgent>;
  edges: SwarmEdge[];
  stats: IntelligenceStats;
}

export class JsonStorage implements StorageBackend {
  private data: JsonData;
  private alpha = 0.1;

  constructor() {
    this.data = this.load();
  }

  private load(): JsonData {
    try {
      if (fs.existsSync(JSON_PATH)) {
        return JSON.parse(fs.readFileSync(JSON_PATH, 'utf-8'));
      }
    } catch {}
    return {
      patterns: {},
      memories: [],
      trajectories: [],
      errors: {},
      file_sequences: [],
      agents: {},
      edges: [],
      stats: {
        total_patterns: 0,
        total_memories: 0,
        total_trajectories: 0,
        total_errors: 0,
        session_count: 0,
        last_session: 0
      }
    };
  }

  private save(): void {
    const dir = path.dirname(JSON_PATH);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(JSON_PATH, JSON.stringify(this.data, null, 2));
  }

  private now(): number {
    return Math.floor(Date.now() / 1000);
  }

  async connect(): Promise<void> {
    // JSON storage is always available
  }

  async disconnect(): Promise<void> {
    this.save();
  }

  isConnected(): boolean {
    return true;
  }

  async updateQ(state: string, action: string, reward: number): Promise<void> {
    const key = `${state}|${action}`;
    if (!this.data.patterns[key]) {
      this.data.patterns[key] = { state, action, q_value: 0, visits: 0, last_update: 0 };
    }
    const p = this.data.patterns[key];
    p.q_value = p.q_value + this.alpha * (reward - p.q_value);
    p.visits++;
    p.last_update = this.now();
    this.data.stats.total_patterns = Object.keys(this.data.patterns).length;
    this.save();
  }

  async getQ(state: string, action: string): Promise<number> {
    const key = `${state}|${action}`;
    return this.data.patterns[key]?.q_value ?? 0;
  }

  async getBestAction(state: string, actions: string[]): Promise<{ action: string; confidence: number }> {
    let bestAction = actions[0] ?? '';
    let bestQ = -Infinity;
    for (const action of actions) {
      const q = await this.getQ(state, action);
      if (q > bestQ) {
        bestQ = q;
        bestAction = action;
      }
    }
    return { action: bestAction, confidence: bestQ > 0 ? Math.min(bestQ, 1) : 0 };
  }

  async remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string> {
    const id = `mem_${this.now()}`;
    this.data.memories.push({
      id,
      memory_type: type,
      content,
      embedding,
      metadata,
      timestamp: this.now()
    });
    if (this.data.memories.length > 5000) {
      this.data.memories.splice(0, 1000);
    }
    this.data.stats.total_memories = this.data.memories.length;
    this.save();
    return id;
  }

  async recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]> {
    const similarity = (a: number[], b: number[]): number => {
      if (a.length !== b.length) return 0;
      const dot = a.reduce((sum, v, i) => sum + v * b[i], 0);
      const normA = Math.sqrt(a.reduce((sum, v) => sum + v * v, 0));
      const normB = Math.sqrt(b.reduce((sum, v) => sum + v * v, 0));
      return normA > 0 && normB > 0 ? dot / (normA * normB) : 0;
    };

    return this.data.memories
      .filter(m => m.embedding && m.embedding.length > 0)
      .map(m => ({ score: similarity(queryEmbedding, m.embedding), memory: m }))
      .sort((a, b) => b.score - a.score)
      .slice(0, topK)
      .map(r => r.memory);
  }

  async recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string> {
    const id = `traj_${this.now()}`;
    await this.updateQ(state, action, reward);
    this.data.trajectories.push({ id, state, action, outcome, reward, timestamp: this.now() });
    if (this.data.trajectories.length > 1000) {
      this.data.trajectories.splice(0, 200);
    }
    this.data.stats.total_trajectories = this.data.trajectories.length;
    this.save();
    return id;
  }

  async recordError(code: string, errorType: string, message: string): Promise<void> {
    if (!this.data.errors[code]) {
      this.data.errors[code] = { code, error_type: errorType, message, fixes: [], occurrences: 0 };
    }
    this.data.errors[code].occurrences++;
    this.data.stats.total_errors = Object.keys(this.data.errors).length;
    this.save();
  }

  async getErrorFixes(code: string): Promise<ErrorPattern | null> {
    return this.data.errors[code] ?? null;
  }

  async recordSequence(fromFile: string, toFile: string): Promise<void> {
    const existing = this.data.file_sequences.find(
      s => s.from_file === fromFile && s.to_file === toFile
    );
    if (existing) {
      existing.count++;
    } else {
      this.data.file_sequences.push({ from_file: fromFile, to_file: toFile, count: 1 });
    }
    this.save();
  }

  async getNextFiles(file: string, limit: number): Promise<Array<{ file: string; count: number }>> {
    return this.data.file_sequences
      .filter(s => s.from_file === file)
      .sort((a, b) => b.count - a.count)
      .slice(0, limit)
      .map(s => ({ file: s.to_file, count: s.count }));
  }

  async registerAgent(id: string, type: string, capabilities: string[]): Promise<void> {
    this.data.agents[id] = {
      id,
      agent_type: type,
      capabilities,
      success_rate: 1.0,
      task_count: 0,
      status: 'active'
    };
    this.save();
  }

  async coordinateAgents(source: string, target: string, weight: number): Promise<void> {
    const existing = this.data.edges.find(e => e.source === source && e.target === target);
    if (existing) {
      existing.weight = (existing.weight + weight) / 2;
      existing.coordination_count++;
    } else {
      this.data.edges.push({ source, target, weight, coordination_count: 1 });
    }
    this.save();
  }

  async getSwarmStats(): Promise<{ agents: number; edges: number; avgSuccess: number }> {
    const agents = Object.keys(this.data.agents).length;
    const edges = this.data.edges.length;
    const avgSuccess = agents > 0
      ? Object.values(this.data.agents).reduce((sum, a) => sum + a.success_rate, 0) / agents
      : 0;
    return { agents, edges, avgSuccess };
  }

  async sessionStart(): Promise<void> {
    this.data.stats.session_count++;
    this.data.stats.last_session = this.now();
    this.save();
  }

  async getStats(): Promise<IntelligenceStats> {
    return this.data.stats;
  }
}

// ============================================================================
// PostgreSQL Storage Backend
// ============================================================================

export class PostgresStorage implements StorageBackend {
  private pool: any = null;
  private connectionString: string;
  private connected = false;

  constructor(connectionString?: string) {
    this.connectionString = connectionString ||
      process.env.RUVECTOR_POSTGRES_URL ||
      process.env.DATABASE_URL ||
      'postgresql://localhost:5432/ruvector';
  }

  async connect(): Promise<void> {
    try {
      // Dynamic import of pg to avoid bundling issues
      const pg = await import('pg');
      this.pool = new pg.Pool({ connectionString: this.connectionString });

      // Test connection
      const client = await this.pool.connect();
      await client.query('SELECT 1');
      client.release();

      // Initialize schema
      await this.initSchema();
      this.connected = true;
    } catch (err) {
      this.connected = false;
      throw err;
    }
  }

  async disconnect(): Promise<void> {
    if (this.pool) {
      await this.pool.end();
      this.pool = null;
      this.connected = false;
    }
  }

  isConnected(): boolean {
    return this.connected && this.pool !== null;
  }

  private async query<T = any>(sql: string, params?: any[]): Promise<T[]> {
    if (!this.pool) throw new Error('Not connected');
    const result = await this.pool.query(sql, params);
    return result.rows;
  }

  private async initSchema(): Promise<void> {
    // Create tables if they don't exist
    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_patterns (
        id SERIAL PRIMARY KEY,
        state TEXT NOT NULL,
        action TEXT NOT NULL,
        q_value REAL DEFAULT 0.0,
        visits INTEGER DEFAULT 0,
        last_update TIMESTAMPTZ DEFAULT NOW(),
        UNIQUE(state, action)
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_memories (
        id SERIAL PRIMARY KEY,
        memory_type TEXT NOT NULL,
        content TEXT NOT NULL,
        embedding REAL[],
        metadata JSONB DEFAULT '{}',
        created_at TIMESTAMPTZ DEFAULT NOW()
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_trajectories (
        id SERIAL PRIMARY KEY,
        state TEXT NOT NULL,
        action TEXT NOT NULL,
        outcome TEXT,
        reward REAL DEFAULT 0.0,
        created_at TIMESTAMPTZ DEFAULT NOW()
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_errors (
        id SERIAL PRIMARY KEY,
        code TEXT NOT NULL UNIQUE,
        error_type TEXT NOT NULL,
        message TEXT,
        fixes TEXT[] DEFAULT '{}',
        occurrences INTEGER DEFAULT 1
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_file_sequences (
        id SERIAL PRIMARY KEY,
        from_file TEXT NOT NULL,
        to_file TEXT NOT NULL,
        count INTEGER DEFAULT 1,
        UNIQUE(from_file, to_file)
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_swarm_agents (
        id TEXT PRIMARY KEY,
        agent_type TEXT NOT NULL,
        capabilities TEXT[] DEFAULT '{}',
        success_rate REAL DEFAULT 1.0,
        task_count INTEGER DEFAULT 0,
        status TEXT DEFAULT 'active'
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_swarm_edges (
        id SERIAL PRIMARY KEY,
        source_agent TEXT NOT NULL,
        target_agent TEXT NOT NULL,
        weight REAL DEFAULT 1.0,
        coordination_count INTEGER DEFAULT 1,
        UNIQUE(source_agent, target_agent)
      )
    `);

    await this.query(`
      CREATE TABLE IF NOT EXISTS ruvector_hooks_stats (
        id INTEGER PRIMARY KEY DEFAULT 1,
        session_count INTEGER DEFAULT 0,
        last_session TIMESTAMPTZ DEFAULT NOW(),
        CHECK (id = 1)
      )
    `);

    await this.query(`
      INSERT INTO ruvector_hooks_stats (id) VALUES (1) ON CONFLICT (id) DO NOTHING
    `);
  }

  async updateQ(state: string, action: string, reward: number): Promise<void> {
    await this.query(`
      INSERT INTO ruvector_hooks_patterns (state, action, q_value, visits, last_update)
      VALUES ($1, $2, $3 * 0.1, 1, NOW())
      ON CONFLICT (state, action) DO UPDATE SET
        q_value = ruvector_hooks_patterns.q_value + 0.1 * ($3 - ruvector_hooks_patterns.q_value),
        visits = ruvector_hooks_patterns.visits + 1,
        last_update = NOW()
    `, [state, action, reward]);
  }

  async getQ(state: string, action: string): Promise<number> {
    const rows = await this.query<{ q_value: number }>(
      'SELECT q_value FROM ruvector_hooks_patterns WHERE state = $1 AND action = $2',
      [state, action]
    );
    return rows[0]?.q_value ?? 0;
  }

  async getBestAction(state: string, actions: string[]): Promise<{ action: string; confidence: number }> {
    const rows = await this.query<{ action: string; q_value: number }>(
      `SELECT action, q_value FROM ruvector_hooks_patterns
       WHERE state = $1 AND action = ANY($2)
       ORDER BY q_value DESC LIMIT 1`,
      [state, actions]
    );

    if (rows.length > 0) {
      const q = rows[0].q_value;
      return { action: rows[0].action, confidence: q > 0 ? Math.min(q, 1) : 0 };
    }
    return { action: actions[0] ?? '', confidence: 0 };
  }

  async remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string> {
    const rows = await this.query<{ id: number }>(
      `INSERT INTO ruvector_hooks_memories (memory_type, content, embedding, metadata)
       VALUES ($1, $2, $3, $4) RETURNING id`,
      [type, content, embedding, JSON.stringify(metadata)]
    );

    // Cleanup old memories
    await this.query(`
      DELETE FROM ruvector_hooks_memories WHERE id IN (
        SELECT id FROM ruvector_hooks_memories ORDER BY created_at ASC OFFSET 5000
      )
    `);

    return `mem_${rows[0].id}`;
  }

  async recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]> {
    // Use cosine similarity via array operations
    // Note: For optimal performance, use pgvector extension with <=> operator
    const rows = await this.query<{
      id: number;
      memory_type: string;
      content: string;
      embedding: number[];
      metadata: any;
      created_at: Date;
    }>(`
      SELECT id, memory_type, content, embedding, metadata,
             EXTRACT(EPOCH FROM created_at)::BIGINT as timestamp
      FROM ruvector_hooks_memories
      WHERE embedding IS NOT NULL
      ORDER BY created_at DESC
      LIMIT $1
    `, [topK * 10]);

    // Client-side similarity ranking (for optimal: use pgvector)
    const similarity = (a: number[], b: number[]): number => {
      if (!a || !b || a.length !== b.length) return 0;
      const dot = a.reduce((sum, v, i) => sum + v * b[i], 0);
      const normA = Math.sqrt(a.reduce((sum, v) => sum + v * v, 0));
      const normB = Math.sqrt(b.reduce((sum, v) => sum + v * v, 0));
      return normA > 0 && normB > 0 ? dot / (normA * normB) : 0;
    };

    return rows
      .map(r => ({
        score: similarity(queryEmbedding, r.embedding),
        entry: {
          id: `mem_${r.id}`,
          memory_type: r.memory_type,
          content: r.content,
          embedding: r.embedding,
          metadata: r.metadata || {},
          timestamp: Math.floor(new Date(r.created_at).getTime() / 1000)
        }
      }))
      .sort((a, b) => b.score - a.score)
      .slice(0, topK)
      .map(r => r.entry);
  }

  async recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string> {
    await this.updateQ(state, action, reward);

    const rows = await this.query<{ id: number }>(
      `INSERT INTO ruvector_hooks_trajectories (state, action, outcome, reward)
       VALUES ($1, $2, $3, $4) RETURNING id`,
      [state, action, outcome, reward]
    );

    // Cleanup old trajectories
    await this.query(`
      DELETE FROM ruvector_hooks_trajectories WHERE id IN (
        SELECT id FROM ruvector_hooks_trajectories ORDER BY created_at ASC OFFSET 1000
      )
    `);

    return `traj_${rows[0].id}`;
  }

  async recordError(code: string, errorType: string, message: string): Promise<void> {
    await this.query(`
      INSERT INTO ruvector_hooks_errors (code, error_type, message, occurrences)
      VALUES ($1, $2, $3, 1)
      ON CONFLICT (code) DO UPDATE SET
        occurrences = ruvector_hooks_errors.occurrences + 1,
        message = COALESCE($3, ruvector_hooks_errors.message)
    `, [code, errorType, message]);
  }

  async getErrorFixes(code: string): Promise<ErrorPattern | null> {
    const rows = await this.query<ErrorPattern>(
      'SELECT code, error_type, message, fixes, occurrences FROM ruvector_hooks_errors WHERE code = $1',
      [code]
    );
    return rows[0] ?? null;
  }

  async recordSequence(fromFile: string, toFile: string): Promise<void> {
    await this.query(`
      INSERT INTO ruvector_hooks_file_sequences (from_file, to_file, count)
      VALUES ($1, $2, 1)
      ON CONFLICT (from_file, to_file) DO UPDATE SET
        count = ruvector_hooks_file_sequences.count + 1
    `, [fromFile, toFile]);
  }

  async getNextFiles(file: string, limit: number): Promise<Array<{ file: string; count: number }>> {
    const rows = await this.query<{ to_file: string; count: number }>(
      `SELECT to_file, count FROM ruvector_hooks_file_sequences
       WHERE from_file = $1 ORDER BY count DESC LIMIT $2`,
      [file, limit]
    );
    return rows.map(r => ({ file: r.to_file, count: r.count }));
  }

  async registerAgent(id: string, type: string, capabilities: string[]): Promise<void> {
    await this.query(`
      INSERT INTO ruvector_hooks_swarm_agents (id, agent_type, capabilities)
      VALUES ($1, $2, $3)
      ON CONFLICT (id) DO UPDATE SET
        agent_type = $2,
        capabilities = $3
    `, [id, type, capabilities]);
  }

  async coordinateAgents(source: string, target: string, weight: number): Promise<void> {
    await this.query(`
      INSERT INTO ruvector_hooks_swarm_edges (source_agent, target_agent, weight, coordination_count)
      VALUES ($1, $2, $3, 1)
      ON CONFLICT (source_agent, target_agent) DO UPDATE SET
        weight = (ruvector_hooks_swarm_edges.weight + $3) / 2,
        coordination_count = ruvector_hooks_swarm_edges.coordination_count + 1
    `, [source, target, weight]);
  }

  async getSwarmStats(): Promise<{ agents: number; edges: number; avgSuccess: number }> {
    const rows = await this.query<{ agents: number; edges: number; avg_success: number }>(`
      SELECT
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_agents WHERE status = 'active') as agents,
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_edges) as edges,
        (SELECT COALESCE(AVG(success_rate), 0)::REAL FROM ruvector_hooks_swarm_agents WHERE status = 'active') as avg_success
    `);
    const r = rows[0];
    return { agents: r?.agents ?? 0, edges: r?.edges ?? 0, avgSuccess: r?.avg_success ?? 0 };
  }

  async sessionStart(): Promise<void> {
    await this.query(`
      UPDATE ruvector_hooks_stats SET session_count = session_count + 1, last_session = NOW() WHERE id = 1
    `);
  }

  async getStats(): Promise<IntelligenceStats> {
    const rows = await this.query<{
      patterns: number;
      memories: number;
      trajectories: number;
      errors: number;
      session_count: number;
      last_session: Date;
    }>(`
      SELECT
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_patterns) as patterns,
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_memories) as memories,
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_trajectories) as trajectories,
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_errors) as errors,
        s.session_count,
        s.last_session
      FROM ruvector_hooks_stats s WHERE s.id = 1
    `);

    const r = rows[0];
    return {
      total_patterns: r?.patterns ?? 0,
      total_memories: r?.memories ?? 0,
      total_trajectories: r?.trajectories ?? 0,
      total_errors: r?.errors ?? 0,
      session_count: r?.session_count ?? 0,
      last_session: r?.last_session ? Math.floor(new Date(r.last_session).getTime() / 1000) : 0
    };
  }
}

// ============================================================================
// Storage Factory
// ============================================================================

export async function createStorage(): Promise<StorageBackend> {
  // Try PostgreSQL first if configured
  const pgUrl = process.env.RUVECTOR_POSTGRES_URL || process.env.DATABASE_URL;

  if (pgUrl) {
    try {
      const pg = new PostgresStorage(pgUrl);
      await pg.connect();
      console.error('🐘 Connected to PostgreSQL');
      return pg;
    } catch (err) {
      console.error('⚠️  PostgreSQL unavailable, falling back to JSON storage');
    }
  }

  // Fallback to JSON
  const json = new JsonStorage();
  await json.connect();
  return json;
}

export function createStorageSync(): StorageBackend {
  // Synchronous version - always returns JSON storage
  // Use createStorage() for async with PostgreSQL support
  return new JsonStorage();
}
