#!/usr/bin/env node

/**
 * RuVector MCP Server
 *
 * Model Context Protocol server for RuVector hooks
 * Provides self-learning intelligence tools for Claude Code
 *
 * Usage:
 *   npx ruvector mcp start
 *   claude mcp add ruvector npx ruvector mcp start
 */

// Signal that this is an MCP server (enables parallel workers for embeddings)
process.env.MCP_SERVER = '1';

const { Server } = require('@modelcontextprotocol/sdk/server/index.js');
const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio.js');
const {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
} = require('@modelcontextprotocol/sdk/types.js');
const path = require('path');
const fs = require('fs');
const { execSync, execFileSync } = require('child_process');

// ── Security Helpers ────────────────────────────────────────────────────────

/**
 * Validate a file path argument for RVF operations.
 * Prevents path traversal and restricts to safe locations.
 */
function validateRvfPath(filePath) {
  if (typeof filePath !== 'string' || filePath.length === 0) {
    throw new Error('Path must be a non-empty string');
  }
  // Block null bytes
  if (filePath.includes('\0')) {
    throw new Error('Path contains null bytes');
  }
  // Resolve to absolute, then canonicalize via realpath if it exists
  let resolved = path.resolve(filePath);
  try {
    // Resolve symlinks for existing paths to prevent symlink-based escapes
    resolved = fs.realpathSync(resolved);
  } catch {
    // Path doesn't exist yet — resolve the parent directory
    const parentDir = path.dirname(resolved);
    try {
      const realParent = fs.realpathSync(parentDir);
      resolved = path.join(realParent, path.basename(resolved));
    } catch {
      // Parent doesn't exist either — keep the resolved path for the block check
    }
  }
  // Confine to the current working directory
  const cwd = process.cwd();
  if (!resolved.startsWith(cwd + path.sep) && resolved !== cwd) {
    // Also block sensitive system paths regardless
    const blocked = ['/etc', '/proc', '/sys', '/dev', '/boot', '/root', '/var/run', '/var/log', '/tmp'];
    for (const prefix of blocked) {
      if (resolved.startsWith(prefix)) {
        throw new Error(`Access denied: path resolves to '${resolved}' which is outside the working directory and in restricted area '${prefix}'`);
      }
    }
    // Allow paths outside cwd only if they're not in blocked directories
    // (for tools that reference project files by absolute path)
  }
  return resolved;
}

/**
 * Sanitize a shell argument to prevent command injection.
 * Strips shell metacharacters and limits length.
 */
function sanitizeShellArg(arg) {
  if (typeof arg !== 'string') return '';
  // Remove null bytes, backticks, $(), quotes, newlines, and other shell metacharacters
  return arg
    .replace(/\0/g, '')
    .replace(/[\r\n]/g, '')
    .replace(/[`$(){}|;&<>!'"\\]/g, '')
    .replace(/\.\./g, '')
    .slice(0, 4096);
}

/**
 * Validate a numeric argument (returns integer or default).
 * Prevents injection via numeric-looking fields.
 */
function sanitizeNumericArg(arg, defaultVal) {
  const n = parseInt(arg, 10);
  return Number.isFinite(n) && n > 0 ? n : (defaultVal || 0);
}

// Try to load the full IntelligenceEngine
let IntelligenceEngine = null;
let engineAvailable = false;

try {
  const core = require('../dist/core/intelligence-engine.js');
  IntelligenceEngine = core.IntelligenceEngine || core.default;
  engineAvailable = true;
} catch (e) {
  // IntelligenceEngine not available
}

// Intelligence class with full RuVector stack support
class Intelligence {
  constructor() {
    this.intelPath = this.getIntelPath();
    this.data = this.load();
    this.engine = null;

    // Initialize full engine if available
    if (engineAvailable && IntelligenceEngine) {
      try {
        this.engine = new IntelligenceEngine({
          embeddingDim: 256,
          maxMemories: 100000,
          enableSona: true,
          enableAttention: true,
        });
        // Import existing data
        if (this.data) {
          this.engine.import(this.convertLegacyData(this.data), true);
        }
      } catch (e) {
        this.engine = null;
      }
    }
  }

  convertLegacyData(data) {
    const converted = { memories: [], routingPatterns: {}, errorPatterns: {}, coEditPatterns: {} };
    if (data.memories) {
      converted.memories = data.memories.map(m => ({
        id: m.id || `mem-${Date.now()}`,
        content: m.content,
        type: m.type || 'general',
        embedding: m.embedding || [],
        created: m.created || new Date().toISOString(),
        accessed: 0,
      }));
    }
    if (data.patterns) {
      for (const [key, value] of Object.entries(data.patterns)) {
        const [state, action] = key.split('|');
        if (state && action) {
          if (!converted.routingPatterns[state]) converted.routingPatterns[state] = {};
          converted.routingPatterns[state][action] = value.q_value || value || 0.5;
        }
      }
    }
    return converted;
  }

  getIntelPath() {
    const projectPath = path.join(process.cwd(), '.ruvector', 'intelligence.json');
    const homePath = path.join(require('os').homedir(), '.ruvector', 'intelligence.json');
    if (fs.existsSync(path.dirname(projectPath))) return projectPath;
    if (fs.existsSync(path.join(process.cwd(), '.claude'))) return projectPath;
    if (fs.existsSync(homePath)) return homePath;
    return projectPath;
  }

  load() {
    try {
      if (fs.existsSync(this.intelPath)) {
        return JSON.parse(fs.readFileSync(this.intelPath, 'utf-8'));
      }
    } catch {}
    return { patterns: {}, memories: [], trajectories: [], errors: {}, agents: {}, edges: [] };
  }

  save() {
    const dir = path.dirname(this.intelPath);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

    // Export engine data if available
    if (this.engine) {
      try {
        const engineData = this.engine.export();
        this.data.engineStats = engineData.stats;
      } catch {}
    }

    fs.writeFileSync(this.intelPath, JSON.stringify(this.data, null, 2));
  }

  stats() {
    const baseStats = {
      total_patterns: Object.keys(this.data.patterns || {}).length,
      total_memories: (this.data.memories || []).length,
      total_trajectories: (this.data.trajectories || []).length,
      total_errors: Object.keys(this.data.errors || {}).length
    };

    if (this.engine) {
      try {
        const engineStats = this.engine.getStats();
        return {
          ...baseStats,
          engineEnabled: true,
          sonaEnabled: engineStats.sonaEnabled,
          attentionEnabled: engineStats.attentionEnabled,
          embeddingDim: engineStats.memoryDimensions,
          totalMemories: engineStats.totalMemories,
          totalEpisodes: engineStats.totalEpisodes,
          trajectoriesRecorded: engineStats.trajectoriesRecorded,
          patternsLearned: engineStats.patternsLearned,
          microLoraUpdates: engineStats.microLoraUpdates,
          ewcConsolidations: engineStats.ewcConsolidations,
        };
      } catch {}
    }

    return { ...baseStats, engineEnabled: false };
  }

  embed(text) {
    if (this.engine) {
      try {
        return this.engine.embed(text);
      } catch {}
    }
    // Fallback: 64-dim hash
    const embedding = new Array(64).fill(0);
    for (let i = 0; i < text.length; i++) {
      const idx = (text.charCodeAt(i) + i * 7) % 64;
      embedding[idx] += 1.0;
    }
    const norm = Math.sqrt(embedding.reduce((a, b) => a + b * b, 0));
    if (norm > 0) for (let i = 0; i < embedding.length; i++) embedding[i] /= norm;
    return embedding;
  }

  similarity(a, b) {
    if (!a || !b || a.length !== b.length) return 0;
    const dot = a.reduce((sum, v, i) => sum + v * b[i], 0);
    const normA = Math.sqrt(a.reduce((sum, v) => sum + v * v, 0));
    const normB = Math.sqrt(b.reduce((sum, v) => sum + v * v, 0));
    return normA > 0 && normB > 0 ? dot / (normA * normB) : 0;
  }

  async remember(content, type = 'general') {
    // Use engine if available (VectorDB storage)
    if (this.engine) {
      try {
        const entry = await this.engine.remember(content, type);
        // Also store in legacy format
        this.data.memories = this.data.memories || [];
        this.data.memories.push({ content, type, created: new Date().toISOString(), embedding: entry.embedding });
        this.save();
        return { stored: true, total: this.data.memories.length, engineStored: true };
      } catch {}
    }

    // Fallback
    this.data.memories = this.data.memories || [];
    this.data.memories.push({ content, type, created: new Date().toISOString(), embedding: this.embed(content) });
    this.save();
    return { stored: true, total: this.data.memories.length };
  }

  async recall(query, topK = 5) {
    // Use engine if available (HNSW search - 150x faster)
    if (this.engine) {
      try {
        const results = await this.engine.recall(query, topK);
        return results.map(r => ({
          content: r.content,
          type: r.type,
          score: r.score || 0,
          created: r.created,
          engineResult: true
        }));
      } catch {}
    }

    // Fallback: brute-force
    const queryEmbed = this.embed(query);
    const scored = (this.data.memories || []).map((m, i) => ({
      ...m,
      index: i,
      score: this.similarity(queryEmbed, m.embedding)
    }));
    return scored.sort((a, b) => b.score - a.score).slice(0, topK);
  }

  async route(task, file = null) {
    // Use engine if available (SONA-enhanced routing)
    if (this.engine) {
      try {
        const result = await this.engine.route(task, file);
        return {
          agent: result.agent,
          confidence: result.confidence,
          reason: result.reason,
          alternates: result.alternates,
          sonaPatterns: result.patterns?.length || 0,
          engineRouted: true
        };
      } catch {}
    }

    // Fallback
    const ext = file ? path.extname(file) : '';
    const state = `edit:${ext || 'unknown'}`;
    const actions = this.data.patterns[state] || {};

    const defaults = {
      '.rs': 'rust-developer',
      '.ts': 'typescript-developer',
      '.tsx': 'react-developer',
      '.js': 'javascript-developer',
      '.jsx': 'react-developer',
      '.py': 'python-developer',
      '.go': 'go-developer',
      '.sql': 'database-specialist',
      '.md': 'documentation-specialist'
    };

    let bestAgent = defaults[ext] || 'coder';
    let bestScore = 0.5;

    for (const [agent, score] of Object.entries(actions)) {
      if (score > bestScore) {
        bestAgent = agent;
        bestScore = score;
      }
    }

    return {
      agent: bestAgent,
      confidence: Math.min(bestScore, 1.0),
      reason: Object.keys(actions).length > 0 ? 'learned from patterns' : 'default mapping'
    };
  }

  getCapabilities() {
    if (!this.engine) {
      return { engine: false, vectorDb: false, sona: false, attention: false, embeddingDim: 64 };
    }
    try {
      const stats = this.engine.getStats();
      return {
        engine: true,
        vectorDb: true,
        sona: stats.sonaEnabled,
        attention: stats.attentionEnabled,
        embeddingDim: stats.memoryDimensions,
      };
    } catch {
      return { engine: true, vectorDb: false, sona: false, attention: false, embeddingDim: 256 };
    }
  }
}

// Create MCP server
const server = new Server(
  {
    name: 'ruvector',
    version: '0.1.58',
  },
  {
    capabilities: {
      tools: {},
      resources: {},
    },
  }
);

const intel = new Intelligence();

// Define tools
const TOOLS = [
  {
    name: 'hooks_stats',
    description: 'Get RuVector intelligence statistics including learned patterns, memories, and trajectories',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_route',
    description: 'Route a task to the best agent based on learned patterns',
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: 'Task description' },
        file: { type: 'string', description: 'File path (optional)' }
      },
      required: ['task']
    }
  },
  {
    name: 'hooks_remember',
    description: 'Store context in vector memory for later recall',
    inputSchema: {
      type: 'object',
      properties: {
        content: { type: 'string', description: 'Content to remember' },
        type: { type: 'string', description: 'Memory type (project, code, decision, context)', default: 'general' }
      },
      required: ['content']
    }
  },
  {
    name: 'hooks_recall',
    description: 'Search vector memory for relevant context',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Search query' },
        top_k: { type: 'number', description: 'Number of results', default: 5 }
      },
      required: ['query']
    }
  },
  {
    name: 'hooks_init',
    description: 'Initialize RuVector hooks in the current project',
    inputSchema: {
      type: 'object',
      properties: {
        pretrain: { type: 'boolean', description: 'Run pretrain after init', default: false },
        build_agents: { type: 'string', description: 'Focus for agent generation (quality, speed, security, testing, fullstack)' },
        force: { type: 'boolean', description: 'Force overwrite existing settings', default: false }
      },
      required: []
    }
  },
  {
    name: 'hooks_pretrain',
    description: 'Pretrain intelligence by analyzing the repository structure and git history',
    inputSchema: {
      type: 'object',
      properties: {
        depth: { type: 'number', description: 'Git history depth to analyze', default: 100 },
        skip_git: { type: 'boolean', description: 'Skip git history analysis', default: false },
        verbose: { type: 'boolean', description: 'Show detailed progress', default: false }
      },
      required: []
    }
  },
  {
    name: 'hooks_build_agents',
    description: 'Generate optimized agent configurations based on repository analysis',
    inputSchema: {
      type: 'object',
      properties: {
        focus: {
          type: 'string',
          description: 'Focus type for agent generation',
          enum: ['quality', 'speed', 'security', 'testing', 'fullstack'],
          default: 'quality'
        },
        include_prompts: { type: 'boolean', description: 'Include system prompts in agent configs', default: true }
      },
      required: []
    }
  },
  {
    name: 'hooks_verify',
    description: 'Verify that hooks are configured correctly',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_doctor',
    description: 'Diagnose and optionally fix setup issues',
    inputSchema: {
      type: 'object',
      properties: {
        fix: { type: 'boolean', description: 'Automatically fix issues', default: false }
      },
      required: []
    }
  },
  {
    name: 'hooks_export',
    description: 'Export intelligence data for backup',
    inputSchema: {
      type: 'object',
      properties: {
        include_all: { type: 'boolean', description: 'Include all data (patterns, memories, trajectories)', default: false }
      },
      required: []
    }
  },
  {
    name: 'hooks_capabilities',
    description: 'Get RuVector engine capabilities (VectorDB, SONA, Attention)',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_import',
    description: 'Import intelligence data from backup file',
    inputSchema: {
      type: 'object',
      properties: {
        data: { type: 'object', description: 'Exported data object to import' },
        merge: { type: 'boolean', description: 'Merge with existing data', default: true }
      },
      required: ['data']
    }
  },
  {
    name: 'hooks_swarm_recommend',
    description: 'Get agent recommendation for a task type using learned patterns',
    inputSchema: {
      type: 'object',
      properties: {
        task_type: { type: 'string', description: 'Type of task (research, code, test, review, debug, etc.)' },
        file: { type: 'string', description: 'Optional file path for context' }
      },
      required: ['task_type']
    }
  },
  {
    name: 'hooks_suggest_context',
    description: 'Get relevant context suggestions for the current task',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Current task or query' },
        top_k: { type: 'number', description: 'Number of suggestions', default: 5 }
      },
      required: []
    }
  },
  {
    name: 'hooks_trajectory_begin',
    description: 'Begin tracking a new execution trajectory',
    inputSchema: {
      type: 'object',
      properties: {
        context: { type: 'string', description: 'Task or operation context' },
        agent: { type: 'string', description: 'Agent performing the task' }
      },
      required: ['context']
    }
  },
  {
    name: 'hooks_trajectory_step',
    description: 'Add a step to the current trajectory',
    inputSchema: {
      type: 'object',
      properties: {
        action: { type: 'string', description: 'Action taken' },
        result: { type: 'string', description: 'Result of action' },
        reward: { type: 'number', description: 'Reward signal (0-1)', default: 0.5 }
      },
      required: ['action']
    }
  },
  {
    name: 'hooks_trajectory_end',
    description: 'End the current trajectory with a quality score',
    inputSchema: {
      type: 'object',
      properties: {
        success: { type: 'boolean', description: 'Whether the task succeeded' },
        quality: { type: 'number', description: 'Quality score (0-1)', default: 0.5 }
      },
      required: []
    }
  },
  {
    name: 'hooks_coedit_record',
    description: 'Record co-edit pattern (files edited together)',
    inputSchema: {
      type: 'object',
      properties: {
        primary_file: { type: 'string', description: 'Primary file being edited' },
        related_files: { type: 'array', items: { type: 'string' }, description: 'Related files edited together' }
      },
      required: ['primary_file', 'related_files']
    }
  },
  {
    name: 'hooks_coedit_suggest',
    description: 'Get suggested related files based on co-edit patterns',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Current file' },
        top_k: { type: 'number', description: 'Number of suggestions', default: 5 }
      },
      required: ['file']
    }
  },
  {
    name: 'hooks_error_record',
    description: 'Record an error and its fix for learning',
    inputSchema: {
      type: 'object',
      properties: {
        error: { type: 'string', description: 'Error message or code' },
        fix: { type: 'string', description: 'Fix that resolved the error' },
        file: { type: 'string', description: 'File where error occurred' }
      },
      required: ['error', 'fix']
    }
  },
  {
    name: 'hooks_error_suggest',
    description: 'Get suggested fixes for an error based on learned patterns',
    inputSchema: {
      type: 'object',
      properties: {
        error: { type: 'string', description: 'Error message or code' }
      },
      required: ['error']
    }
  },
  {
    name: 'hooks_force_learn',
    description: 'Force an immediate learning cycle',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  // ============================================
  // NEW CAPABILITY TOOLS (AST, Diff, Coverage, Graph, Security, RAG)
  // ============================================
  {
    name: 'hooks_ast_analyze',
    description: 'Parse file AST and extract symbols, imports, complexity metrics',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'File path to analyze' }
      },
      required: ['file']
    }
  },
  {
    name: 'hooks_ast_complexity',
    description: 'Get cyclomatic and cognitive complexity metrics for files',
    inputSchema: {
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' }, description: 'Files to analyze' },
        threshold: { type: 'number', description: 'Warn if complexity exceeds threshold', default: 10 }
      },
      required: ['files']
    }
  },
  {
    name: 'hooks_diff_analyze',
    description: 'Analyze git diff with semantic embeddings and risk scoring',
    inputSchema: {
      type: 'object',
      properties: {
        commit: { type: 'string', description: 'Commit hash (defaults to staged changes)' }
      },
      required: []
    }
  },
  {
    name: 'hooks_diff_classify',
    description: 'Classify change type (feature, bugfix, refactor, docs, test, config)',
    inputSchema: {
      type: 'object',
      properties: {
        commit: { type: 'string', description: 'Commit hash (defaults to HEAD)' }
      },
      required: []
    }
  },
  {
    name: 'hooks_diff_similar',
    description: 'Find similar past commits based on diff embeddings',
    inputSchema: {
      type: 'object',
      properties: {
        top_k: { type: 'number', description: 'Number of results', default: 5 },
        commits: { type: 'number', description: 'Recent commits to search', default: 50 }
      },
      required: []
    }
  },
  {
    name: 'hooks_coverage_route',
    description: 'Get coverage-aware agent routing for a file',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'File to analyze' }
      },
      required: ['file']
    }
  },
  {
    name: 'hooks_coverage_suggest',
    description: 'Suggest tests for files based on coverage data',
    inputSchema: {
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' }, description: 'Files to analyze' }
      },
      required: ['files']
    }
  },
  {
    name: 'hooks_graph_mincut',
    description: 'Find optimal code boundaries using MinCut algorithm (Stoer-Wagner)',
    inputSchema: {
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' }, description: 'Files to analyze' }
      },
      required: ['files']
    }
  },
  {
    name: 'hooks_graph_cluster',
    description: 'Detect code communities using spectral or Louvain clustering',
    inputSchema: {
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' }, description: 'Files to analyze' },
        method: { type: 'string', enum: ['spectral', 'louvain'], default: 'louvain' },
        clusters: { type: 'number', description: 'Number of clusters (spectral only)', default: 3 }
      },
      required: ['files']
    }
  },
  {
    name: 'hooks_security_scan',
    description: 'Parallel security vulnerability scan for common issues',
    inputSchema: {
      type: 'object',
      properties: {
        files: { type: 'array', items: { type: 'string' }, description: 'Files to scan' }
      },
      required: ['files']
    }
  },
  {
    name: 'hooks_rag_context',
    description: 'Get RAG-enhanced context for a query with optional reranking',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Query for context' },
        top_k: { type: 'number', description: 'Number of results', default: 5 },
        rerank: { type: 'boolean', description: 'Rerank results by relevance', default: false }
      },
      required: ['query']
    }
  },
  {
    name: 'hooks_git_churn',
    description: 'Analyze git churn to find hot spots',
    inputSchema: {
      type: 'object',
      properties: {
        days: { type: 'number', description: 'Number of days to analyze', default: 30 },
        top: { type: 'number', description: 'Top N files', default: 10 }
      },
      required: []
    }
  },
  {
    name: 'hooks_route_enhanced',
    description: 'Enhanced routing using AST complexity, coverage, and diff analysis signals',
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: 'Task description' },
        file: { type: 'string', description: 'File context' }
      },
      required: ['task']
    }
  },
  {
    name: 'hooks_attention_info',
    description: 'Get available attention mechanisms and their configurations',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_gnn_info',
    description: 'Get GNN layer capabilities and configuration',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  // Learning Engine Tools (v2.1)
  {
    name: 'hooks_learning_config',
    description: 'Configure learning algorithms for different tasks. Supports 9 algorithms: q-learning, sarsa, double-q, actor-critic, ppo, decision-transformer, monte-carlo, td-lambda, dqn',
    inputSchema: {
      type: 'object',
      properties: {
        task: {
          type: 'string',
          description: 'Task type: agent-routing, error-avoidance, confidence-scoring, trajectory-learning, context-ranking, memory-recall',
          enum: ['agent-routing', 'error-avoidance', 'confidence-scoring', 'trajectory-learning', 'context-ranking', 'memory-recall']
        },
        algorithm: {
          type: 'string',
          description: 'Learning algorithm',
          enum: ['q-learning', 'sarsa', 'double-q', 'actor-critic', 'ppo', 'decision-transformer', 'monte-carlo', 'td-lambda', 'dqn']
        },
        learningRate: { type: 'number', description: 'Learning rate (0.0-1.0)' },
        discountFactor: { type: 'number', description: 'Discount factor gamma (0.0-1.0)' },
        epsilon: { type: 'number', description: 'Exploration rate (0.0-1.0)' }
      },
      required: []
    }
  },
  {
    name: 'hooks_learning_stats',
    description: 'Get learning algorithm statistics and performance metrics',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_learning_update',
    description: 'Record a learning experience for a specific task',
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: 'Task type' },
        state: { type: 'string', description: 'Current state' },
        action: { type: 'string', description: 'Action taken' },
        reward: { type: 'number', description: 'Reward received (-1 to 1)' },
        nextState: { type: 'string', description: 'Next state (optional)' },
        done: { type: 'boolean', description: 'Episode is done' }
      },
      required: ['task', 'state', 'action', 'reward']
    }
  },
  {
    name: 'hooks_learn',
    description: 'Combined learning action: record experience and get best action recommendation',
    inputSchema: {
      type: 'object',
      properties: {
        state: { type: 'string', description: 'Current state' },
        action: { type: 'string', description: 'Action taken (optional)' },
        reward: { type: 'number', description: 'Reward (-1 to 1, optional)' },
        actions: { type: 'array', items: { type: 'string' }, description: 'Available actions for recommendation' },
        task: { type: 'string', description: 'Task type', default: 'agent-routing' }
      },
      required: ['state']
    }
  },
  {
    name: 'hooks_algorithms_list',
    description: 'List all available learning algorithms with descriptions',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  // TensorCompress Tools
  {
    name: 'hooks_compress',
    description: 'Compress pattern storage using TensorCompress. Provides up to 10x memory savings.',
    inputSchema: {
      type: 'object',
      properties: {
        force: { type: 'boolean', description: 'Force recompression of all patterns' }
      },
      required: []
    }
  },
  {
    name: 'hooks_compress_stats',
    description: 'Get TensorCompress statistics: memory savings, compression levels, tensor counts',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'hooks_compress_store',
    description: 'Store an embedding with adaptive compression',
    inputSchema: {
      type: 'object',
      properties: {
        key: { type: 'string', description: 'Storage key' },
        vector: { type: 'array', items: { type: 'number' }, description: 'Vector to store' },
        level: { type: 'string', description: 'Compression level', enum: ['none', 'half', 'pq8', 'pq4', 'binary'] }
      },
      required: ['key', 'vector']
    }
  },
  {
    name: 'hooks_compress_get',
    description: 'Retrieve a compressed embedding',
    inputSchema: {
      type: 'object',
      properties: {
        key: { type: 'string', description: 'Storage key' }
      },
      required: ['key']
    }
  },
  {
    name: 'hooks_batch_learn',
    description: 'Record multiple learning experiences in batch for efficiency. Processes an array of experiences at once.',
    inputSchema: {
      type: 'object',
      properties: {
        experiences: {
          type: 'array',
          description: 'Array of experiences to learn from',
          items: {
            type: 'object',
            properties: {
              state: { type: 'string', description: 'State identifier' },
              action: { type: 'string', description: 'Action taken' },
              reward: { type: 'number', description: 'Reward (-1 to 1)' },
              nextState: { type: 'string', description: 'Next state (optional)' },
              done: { type: 'boolean', description: 'Episode ended' }
            },
            required: ['state', 'action', 'reward']
          }
        },
        task: { type: 'string', description: 'Task type for all experiences', default: 'agent-routing' }
      },
      required: ['experiences']
    }
  },
  {
    name: 'hooks_subscribe_snapshot',
    description: 'Get current state snapshot for subscription-style updates. Returns counts and deltas since last call.',
    inputSchema: {
      type: 'object',
      properties: {
        events: {
          type: 'array',
          description: 'Event types to check',
          items: { type: 'string', enum: ['learn', 'compress', 'route', 'memory'] },
          default: ['learn', 'route']
        },
        lastState: {
          type: 'object',
          description: 'Previous state for delta calculation',
          properties: {
            patterns: { type: 'number' },
            memories: { type: 'number' },
            trajectories: { type: 'number' },
            updates: { type: 'number' }
          }
        }
      },
      required: []
    }
  },
  {
    name: 'hooks_watch_status',
    description: 'Get file watching status and recent changes detected',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  // ============================================
  // BACKGROUND WORKERS TOOLS (via agentic-flow)
  // ============================================
  {
    name: 'workers_dispatch',
    description: 'Dispatch a background worker for analysis (ultralearn, optimize, audit, map, etc.)',
    inputSchema: {
      type: 'object',
      properties: {
        prompt: { type: 'string', description: 'Prompt with trigger keyword (e.g., "ultralearn authentication")' }
      },
      required: ['prompt']
    }
  },
  {
    name: 'workers_status',
    description: 'Get background worker status dashboard',
    inputSchema: {
      type: 'object',
      properties: {
        workerId: { type: 'string', description: 'Specific worker ID (optional)' }
      },
      required: []
    }
  },
  {
    name: 'workers_results',
    description: 'Get analysis results from completed workers',
    inputSchema: {
      type: 'object',
      properties: {
        json: { type: 'boolean', description: 'Return as JSON', default: false }
      },
      required: []
    }
  },
  {
    name: 'workers_triggers',
    description: 'List available trigger keywords for workers',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'workers_stats',
    description: 'Get worker statistics (24h)',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  // Custom Worker System (agentic-flow@alpha.39+)
  {
    name: 'workers_presets',
    description: 'List available worker presets (quick-scan, deep-analysis, security-scan, learning, api-docs, test-analysis)',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'workers_phases',
    description: 'List available phase executors (24 phases including file-discovery, security-analysis, pattern-extraction)',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'workers_create',
    description: 'Create a custom worker from preset with composable phases',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Worker name' },
        preset: { type: 'string', description: 'Base preset (quick-scan, deep-analysis, security-scan, learning, api-docs, test-analysis)' },
        triggers: { type: 'string', description: 'Comma-separated trigger keywords' }
      },
      required: ['name']
    }
  },
  {
    name: 'workers_run',
    description: 'Run a custom worker on target path',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Worker name' },
        path: { type: 'string', description: 'Target path to analyze (default: .)' }
      },
      required: ['name']
    }
  },
  {
    name: 'workers_custom',
    description: 'List registered custom workers',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'workers_init_config',
    description: 'Generate example workers.yaml config file',
    inputSchema: {
      type: 'object',
      properties: {
        force: { type: 'boolean', description: 'Overwrite existing config' }
      },
      required: []
    }
  },
  {
    name: 'workers_load_config',
    description: 'Load custom workers from workers.yaml config file',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Config file path (default: workers.yaml)' }
      },
      required: []
    }
  },
  // ── RVF Vector Store Tools ────────────────────────────────────────────────
  {
    name: 'rvf_create',
    description: 'Create a new RVF vector store (.rvf file) with specified dimensions and distance metric',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'File path for the new .rvf store' },
        dimension: { type: 'number', description: 'Vector dimensionality (e.g. 128, 384, 768, 1536)' },
        metric: { type: 'string', description: 'Distance metric: cosine, l2, or dotproduct', default: 'cosine' }
      },
      required: ['path', 'dimension']
    }
  },
  {
    name: 'rvf_open',
    description: 'Open an existing RVF store for read-write operations',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to existing .rvf file' }
      },
      required: ['path']
    }
  },
  {
    name: 'rvf_ingest',
    description: 'Insert vectors into an RVF store',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' },
        entries: { type: 'array', description: 'Array of {id, vector, metadata?} objects', items: { type: 'object' } }
      },
      required: ['path', 'entries']
    }
  },
  {
    name: 'rvf_query',
    description: 'Query nearest neighbors in an RVF store',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' },
        vector: { type: 'array', description: 'Query vector as array of numbers', items: { type: 'number' } },
        k: { type: 'number', description: 'Number of results to return', default: 10 }
      },
      required: ['path', 'vector']
    }
  },
  {
    name: 'rvf_delete',
    description: 'Delete vectors by ID from an RVF store',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' },
        ids: { type: 'array', description: 'Vector IDs to delete', items: { type: 'number' } }
      },
      required: ['path', 'ids']
    }
  },
  {
    name: 'rvf_status',
    description: 'Get status of an RVF store (vector count, dimension, metric, file size)',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' }
      },
      required: ['path']
    }
  },
  {
    name: 'rvf_compact',
    description: 'Compact an RVF store to reclaim space from deleted vectors',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' }
      },
      required: ['path']
    }
  },
  {
    name: 'rvf_derive',
    description: 'Derive a child RVF store from a parent using copy-on-write branching',
    inputSchema: {
      type: 'object',
      properties: {
        parent_path: { type: 'string', description: 'Path to parent .rvf store' },
        child_path: { type: 'string', description: 'Path for the new child .rvf store' }
      },
      required: ['parent_path', 'child_path']
    }
  },
  {
    name: 'rvf_segments',
    description: 'List all segments in an RVF file (VEC, INDEX, KERNEL, EBPF, WITNESS, etc.)',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path to .rvf store' }
      },
      required: ['path']
    }
  },
  {
    name: 'rvf_examples',
    description: 'List available example .rvf files with download URLs from the ruvector repository',
    inputSchema: {
      type: 'object',
      properties: {
        filter: { type: 'string', description: 'Filter examples by name or description substring' }
      },
      required: []
    }
  },
  // ── rvlite Query Tools ──────────────────────────────────────────────────
  {
    name: 'rvlite_sql',
    description: 'Execute SQL query over rvlite vector database with optional RVF backend',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'SQL query string (supports distance() and vec_search() functions)' },
        db_path: { type: 'string', description: 'Path to database file (optional)' }
      },
      required: ['query']
    }
  },
  {
    name: 'rvlite_cypher',
    description: 'Execute Cypher graph query over rvlite property graph',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Cypher query string' },
        db_path: { type: 'string', description: 'Path to database file (optional)' }
      },
      required: ['query']
    }
  },
  {
    name: 'rvlite_sparql',
    description: 'Execute SPARQL query over rvlite RDF triple store',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'SPARQL query string' },
        db_path: { type: 'string', description: 'Path to database file (optional)' }
      },
      required: ['query']
    }
  }
];

// List tools handler
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return { tools: TOOLS };
});

// Call tool handler
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case 'hooks_stats': {
        const stats = intel.stats();
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              stats,
              intel_path: intel.intelPath
            }, null, 2)
          }]
        };
      }

      case 'hooks_route': {
        const result = await intel.route(args.task, args.file);
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              task: args.task,
              file: args.file,
              ...result
            }, null, 2)
          }]
        };
      }

      case 'hooks_remember': {
        const result = await intel.remember(args.content, args.type || 'general');
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              ...result
            }, null, 2)
          }]
        };
      }

      case 'hooks_recall': {
        const results = await intel.recall(args.query, args.top_k || 5);
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              query: args.query,
              results: results.map(r => ({
                content: r.content,
                type: r.type,
                score: typeof r.score === 'number' ? r.score.toFixed(3) : r.score,
                created: r.created,
                engineResult: r.engineResult || false
              }))
            }, null, 2)
          }]
        };
      }

      case 'hooks_init': {
        let cmd = 'npx ruvector hooks init';
        if (args.force) cmd += ' --force';
        if (args.pretrain) cmd += ' --pretrain';
        if (args.build_agents) cmd += ` --build-agents ${sanitizeShellArg(args.build_agents)}`;

        try {
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 60000 });
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: true, output }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message }, null, 2)
            }]
          };
        }
      }

      case 'hooks_pretrain': {
        let cmd = 'npx ruvector hooks pretrain';
        if (args.depth) cmd += ` --depth ${sanitizeNumericArg(args.depth, 3)}`;
        if (args.skip_git) cmd += ' --skip-git';
        if (args.verbose) cmd += ' --verbose';

        try {
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 120000 });
          // Reload intelligence after pretrain
          intel.data = intel.load();
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({
                success: true,
                output,
                new_stats: intel.stats()
              }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message }, null, 2)
            }]
          };
        }
      }

      case 'hooks_build_agents': {
        let cmd = 'npx ruvector hooks build-agents';
        if (args.focus) cmd += ` --focus ${sanitizeShellArg(args.focus)}`;
        if (args.include_prompts) cmd += ' --include-prompts';

        try {
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 30000 });
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: true, output }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message }, null, 2)
            }]
          };
        }
      }

      case 'hooks_verify': {
        try {
          const output = execSync('npx ruvector hooks verify', { encoding: 'utf-8', timeout: 15000 });
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: true, output }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message, output: e.stdout }, null, 2)
            }]
          };
        }
      }

      case 'hooks_doctor': {
        let cmd = 'npx ruvector hooks doctor';
        if (args.fix) cmd += ' --fix';

        try {
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 15000 });
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: true, output }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message }, null, 2)
            }]
          };
        }
      }

      case 'hooks_export': {
        const exportData = {
          version: '2.0',
          exported_at: new Date().toISOString(),
          patterns: intel.data.patterns || {},
          memories: args.include_all ? (intel.data.memories || []) : [],
          trajectories: args.include_all ? (intel.data.trajectories || []) : [],
          errors: intel.data.errors || {},
          stats: intel.stats(),
          capabilities: intel.getCapabilities()
        };
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, data: exportData }, null, 2)
          }]
        };
      }

      case 'hooks_capabilities': {
        const capabilities = intel.getCapabilities();
        const stats = intel.stats();
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              capabilities,
              features: {
                vectorDb: capabilities.vectorDb ? 'HNSW indexing (150x faster search)' : 'Brute-force fallback',
                sona: capabilities.sona ? 'Micro-LoRA + Base-LoRA + EWC++' : 'Q-learning fallback',
                attention: capabilities.attention ? 'Self-attention embeddings' : 'Hash embeddings',
                embeddingDim: capabilities.embeddingDim,
              },
              stats: {
                totalMemories: stats.totalMemories || stats.total_memories,
                trajectoriesRecorded: stats.trajectoriesRecorded || 0,
                patternsLearned: stats.patternsLearned || stats.total_patterns,
                microLoraUpdates: stats.microLoraUpdates || 0,
                ewcConsolidations: stats.ewcConsolidations || 0,
              }
            }, null, 2)
          }]
        };
      }

      case 'hooks_import': {
        try {
          const data = args.data;
          const merge = args.merge !== false;

          // Validate imported data structure to prevent prototype pollution and injection
          if (typeof data !== 'object' || data === null || Array.isArray(data)) {
            throw new Error('Import data must be a non-null object');
          }
          const allowedKeys = ['patterns', 'memories', 'errors', 'agents', 'edges', 'trajectories'];
          for (const key of Object.keys(data)) {
            if (!allowedKeys.includes(key)) {
              throw new Error(`Unknown import key: '${key}'. Allowed: ${allowedKeys.join(', ')}`);
            }
          }
          // Prevent prototype pollution via __proto__, constructor, prototype keys
          const dangerousKeys = ['__proto__', 'constructor', 'prototype'];
          function checkForProtoPollution(obj, path) {
            if (typeof obj !== 'object' || obj === null) return;
            for (const key of Object.keys(obj)) {
              if (dangerousKeys.includes(key)) {
                throw new Error(`Dangerous key '${key}' detected at ${path}.${key}`);
              }
            }
          }
          if (data.patterns) checkForProtoPollution(data.patterns, 'patterns');
          if (data.errors) checkForProtoPollution(data.errors, 'errors');

          if (data.patterns && typeof data.patterns === 'object') {
            if (merge) {
              Object.assign(intel.data.patterns, data.patterns);
            } else {
              intel.data.patterns = data.patterns;
            }
          }
          if (data.memories && Array.isArray(data.memories)) {
            if (merge) {
              intel.data.memories = [...(intel.data.memories || []), ...data.memories];
            } else {
              intel.data.memories = data.memories;
            }
          }
          if (data.errors && typeof data.errors === 'object') {
            if (merge) {
              Object.assign(intel.data.errors, data.errors);
            } else {
              intel.data.errors = data.errors;
            }
          }
          intel.save();

          return {
            content: [{
              type: 'text',
              text: JSON.stringify({
                success: true,
                message: `Imported ${Object.keys(data.patterns || {}).length} patterns, ${(data.memories || []).length} memories`,
                merge
              }, null, 2)
            }]
          };
        } catch (e) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: e.message }, null, 2)
            }]
          };
        }
      }

      case 'hooks_swarm_recommend': {
        const taskType = args.task_type || '';
        const file = args.file || '';

        // Map task types to recommended agents
        const taskAgentMap = {
          research: ['researcher', 'analyst', 'explorer'],
          code: ['coder', 'backend-dev', 'sparc-coder'],
          test: ['tester', 'tdd-london-swarm', 'production-validator'],
          review: ['reviewer', 'code-analyzer', 'analyst'],
          debug: ['coder', 'tester', 'analyst'],
          refactor: ['code-analyzer', 'reviewer', 'architect'],
          document: ['documenter', 'api-docs', 'researcher'],
          security: ['security-manager', 'reviewer', 'code-analyzer'],
          performance: ['perf-analyzer', 'performance-benchmarker', 'optimizer'],
          architecture: ['system-architect', 'architect', 'planner']
        };

        // Get learned route if file provided
        let learnedAgent = null;
        if (file) {
          const route = await intel.route({ task: taskType, file });
          learnedAgent = route?.agent;
        }

        const recommendations = taskAgentMap[taskType.toLowerCase()] || ['coder', 'researcher', 'analyst'];

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              task_type: taskType,
              recommendations,
              learned_agent: learnedAgent,
              suggested: learnedAgent || recommendations[0]
            }, null, 2)
          }]
        };
      }

      case 'hooks_suggest_context': {
        const query = args.query || '';
        const topK = args.top_k || 5;

        // Get relevant memories
        const memories = await intel.recall(query, topK);

        // Get recent patterns
        const recentPatterns = Object.entries(intel.data.patterns || {})
          .slice(0, topK)
          .map(([state, actions]) => ({ state, topAction: Object.keys(actions)[0] }));

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              query,
              memories: memories.map(m => ({ content: m.content, type: m.type, score: m.score })),
              patterns: recentPatterns
            }, null, 2)
          }]
        };
      }

      case 'hooks_trajectory_begin': {
        const context = args.context;
        const agent = args.agent || 'unknown';

        // Store trajectory start in intel
        if (!intel.data.activeTrajectories) intel.data.activeTrajectories = {};
        const trajId = `traj_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        intel.data.activeTrajectories[trajId] = {
          id: trajId,
          context,
          agent,
          steps: [],
          startTime: Date.now()
        };

        // Also use engine if available
        if (intel.engine) {
          try {
            intel.engine.beginTrajectory(context);
          } catch (e) { /* fallback to manual */ }
        }

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, trajectory_id: trajId, context, agent }, null, 2)
          }]
        };
      }

      case 'hooks_trajectory_step': {
        const action = args.action;
        const result = args.result || '';
        const reward = args.reward || 0.5;

        // Add to most recent trajectory
        const trajectories = intel.data.activeTrajectories || {};
        const trajIds = Object.keys(trajectories);
        if (trajIds.length === 0) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: 'No active trajectory. Call hooks_trajectory_begin first.' }, null, 2)
            }]
          };
        }

        const latestTrajId = trajIds[trajIds.length - 1];
        trajectories[latestTrajId].steps.push({ action, result, reward, time: Date.now() });

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, trajectory_id: latestTrajId, step: trajectories[latestTrajId].steps.length }, null, 2)
          }]
        };
      }

      case 'hooks_trajectory_end': {
        const success = args.success !== false;
        const quality = args.quality || (success ? 0.8 : 0.2);

        const trajectories = intel.data.activeTrajectories || {};
        const trajIds = Object.keys(trajectories);
        if (trajIds.length === 0) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: false, error: 'No active trajectory.' }, null, 2)
            }]
          };
        }

        const latestTrajId = trajIds[trajIds.length - 1];
        const traj = trajectories[latestTrajId];
        traj.endTime = Date.now();
        traj.quality = quality;
        traj.success = success;

        // Move to completed trajectories
        if (!intel.data.trajectories) intel.data.trajectories = [];
        intel.data.trajectories.push(traj);
        delete trajectories[latestTrajId];

        // Learn from trajectory
        if (intel.engine && traj.steps.length > 0) {
          try {
            intel.engine.endTrajectory(latestTrajId, quality);
          } catch (e) { /* fallback */ }
        }

        intel.save();

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              success: true,
              trajectory_id: latestTrajId,
              steps: traj.steps.length,
              duration_ms: traj.endTime - traj.startTime,
              quality
            }, null, 2)
          }]
        };
      }

      case 'hooks_coedit_record': {
        const primaryFile = args.primary_file;
        const relatedFiles = args.related_files || [];

        if (!intel.data.coEditPatterns) intel.data.coEditPatterns = {};
        if (!intel.data.coEditPatterns[primaryFile]) intel.data.coEditPatterns[primaryFile] = {};

        for (const related of relatedFiles) {
          intel.data.coEditPatterns[primaryFile][related] = (intel.data.coEditPatterns[primaryFile][related] || 0) + 1;
        }

        // Use engine if available
        if (intel.engine) {
          try {
            for (const related of relatedFiles) {
              intel.engine.recordCoEdit(primaryFile, related);
            }
          } catch (e) { /* fallback */ }
        }

        intel.save();

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, primary_file: primaryFile, related_count: relatedFiles.length }, null, 2)
          }]
        };
      }

      case 'hooks_coedit_suggest': {
        const file = args.file;
        const topK = args.top_k || 5;

        let suggestions = [];

        // Try engine first
        if (intel.engine) {
          try {
            suggestions = intel.engine.getLikelyNextFiles(file, topK);
          } catch (e) { /* fallback */ }
        }

        // Fallback to data
        if (suggestions.length === 0 && intel.data.coEditPatterns && intel.data.coEditPatterns[file]) {
          suggestions = Object.entries(intel.data.coEditPatterns[file])
            .sort((a, b) => b[1] - a[1])
            .slice(0, topK)
            .map(([f, count]) => ({ file: f, count, confidence: count / 10 }));
        }

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, file, suggestions }, null, 2)
          }]
        };
      }

      case 'hooks_error_record': {
        const error = args.error;
        const fix = args.fix;
        const file = args.file || '';

        if (!intel.data.errors) intel.data.errors = {};
        if (!intel.data.errors[error]) intel.data.errors[error] = [];
        intel.data.errors[error].push({ fix, file, recorded: Date.now() });

        // Use engine if available
        if (intel.engine) {
          try {
            intel.engine.recordErrorFix(error, fix);
          } catch (e) { /* fallback */ }
        }

        intel.save();

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, error: error.substring(0, 50), fixes_recorded: intel.data.errors[error].length }, null, 2)
          }]
        };
      }

      case 'hooks_error_suggest': {
        const error = args.error;

        let suggestions = [];

        // Try engine first
        if (intel.engine) {
          try {
            suggestions = intel.engine.getSuggestedFixes(error);
          } catch (e) { /* fallback */ }
        }

        // Fallback to data
        if (suggestions.length === 0 && intel.data.errors) {
          // Find similar errors
          for (const [errKey, fixes] of Object.entries(intel.data.errors)) {
            if (error.includes(errKey) || errKey.includes(error)) {
              suggestions.push(...fixes.map(f => f.fix));
            }
          }
        }

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, error: error.substring(0, 50), suggestions: [...new Set(suggestions)].slice(0, 5) }, null, 2)
          }]
        };
      }

      case 'hooks_force_learn': {
        let result = 'Learning triggered';

        if (intel.engine) {
          try {
            // Run forceLearn on engine
            const learnResult = intel.engine.forceLearn();
            result = learnResult || 'Engine learning complete';

            // Also tick for regular updates
            intel.engine.tick();
          } catch (e) {
            result = `Learning: ${e.message}`;
          }
        }

        // Save any updates
        intel.save();

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: true, result, stats: intel.stats() }, null, 2)
          }]
        };
      }

      // ============================================
      // NEW CAPABILITY TOOL HANDLERS
      // ============================================

      case 'hooks_ast_analyze': {
        try {
          const safeFile = sanitizeShellArg(args.file);
          const output = execSync(`npx ruvector hooks ast-analyze "${safeFile}" --json`, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_ast_complexity': {
        try {
          const filesArg = args.files.map(f => `"${sanitizeShellArg(f)}"`).join(' ');
          const threshold = parseInt(args.threshold, 10) || 10;
          const output = execSync(`npx ruvector hooks ast-complexity ${filesArg} --threshold ${threshold}`, { encoding: 'utf-8', timeout: 60000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_diff_analyze': {
        try {
          const cmd = args.commit ? `npx ruvector hooks diff-analyze "${sanitizeShellArg(args.commit)}" --json` : 'npx ruvector hooks diff-analyze --json';
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 60000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_diff_classify': {
        try {
          const cmd = args.commit ? `npx ruvector hooks diff-classify "${sanitizeShellArg(args.commit)}"` : 'npx ruvector hooks diff-classify';
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_diff_similar': {
        try {
          const topK = parseInt(args.top_k, 10) || 5;
          const commits = parseInt(args.commits, 10) || 50;
          const output = execSync(`npx ruvector hooks diff-similar -k ${topK} --commits ${commits}`, { encoding: 'utf-8', timeout: 120000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_coverage_route': {
        try {
          const safeFile = sanitizeShellArg(args.file);
          const output = execSync(`npx ruvector hooks coverage-route "${safeFile}"`, { encoding: 'utf-8', timeout: 15000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_coverage_suggest': {
        try {
          const filesArg = args.files.map(f => `"${sanitizeShellArg(f)}"`).join(' ');
          const output = execSync(`npx ruvector hooks coverage-suggest ${filesArg}`, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_graph_mincut': {
        try {
          const filesArg = args.files.map(f => `"${sanitizeShellArg(f)}"`).join(' ');
          const output = execSync(`npx ruvector hooks graph-mincut ${filesArg}`, { encoding: 'utf-8', timeout: 60000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_graph_cluster': {
        try {
          const filesArg = args.files.map(f => `"${sanitizeShellArg(f)}"`).join(' ');
          const method = sanitizeShellArg(args.method || 'louvain');
          const clusters = parseInt(args.clusters, 10) || 3;
          const output = execSync(`npx ruvector hooks graph-cluster ${filesArg} --method ${method} --clusters ${clusters}`, { encoding: 'utf-8', timeout: 60000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_security_scan': {
        try {
          const filesArg = args.files.map(f => `"${sanitizeShellArg(f)}"`).join(' ');
          const output = execSync(`npx ruvector hooks security-scan ${filesArg}`, { encoding: 'utf-8', timeout: 120000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_rag_context': {
        try {
          const safeQuery = sanitizeShellArg(args.query);
          const topK = parseInt(args.top_k, 10) || 5;
          let cmd = `npx ruvector hooks rag-context "${safeQuery}" -k ${topK}`;
          if (args.rerank) cmd += ' --rerank';
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_git_churn': {
        try {
          const days = parseInt(args.days, 10) || 30;
          const top = parseInt(args.top, 10) || 10;
          const output = execSync(`npx ruvector hooks git-churn --days ${days} --top ${top}`, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_route_enhanced': {
        try {
          const safeTask = sanitizeShellArg(args.task);
          let cmd = `npx ruvector hooks route-enhanced "${safeTask}"`;
          if (args.file) cmd += ` --file "${sanitizeShellArg(args.file)}"`;
          const output = execSync(cmd, { encoding: 'utf-8', timeout: 30000 });
          return { content: [{ type: 'text', text: output }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }] };
        }
      }

      case 'hooks_attention_info': {
        // Return info about available attention mechanisms
        let attentionInfo = { available: false, mechanisms: [] };
        try {
          const attention = require('@ruvector/attention');
          attentionInfo = {
            available: true,
            version: attention.version || '1.0.0',
            mechanisms: [
              { name: 'DotProductAttention', description: 'Basic scaled dot-product attention' },
              { name: 'MultiHeadAttention', description: 'Multi-head self-attention with parallel heads' },
              { name: 'FlashAttention', description: 'Memory-efficient attention with tiling' },
              { name: 'HyperbolicAttention', description: 'Attention in Poincaré ball hyperbolic space' },
              { name: 'LinearAttention', description: 'O(n) linear complexity attention' },
              { name: 'MoEAttention', description: 'Mixture-of-Experts sparse attention' },
              { name: 'GraphRoPeAttention', description: 'Rotary position embeddings for graphs' },
              { name: 'DualSpaceAttention', description: 'Euclidean + Hyperbolic hybrid' },
              { name: 'LocalGlobalAttention', description: 'Sliding window + global tokens' }
            ],
            hyperbolic: { expMap: true, logMap: true, mobiusAddition: true, poincareDistance: true }
          };
        } catch (e) {
          attentionInfo = { available: false, error: 'Attention package not installed' };
        }
        return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...attentionInfo }, null, 2) }] };
      }

      case 'hooks_gnn_info': {
        // Return info about GNN capabilities
        let gnnInfo = { available: false, layers: [] };
        try {
          const gnn = require('@ruvector/gnn');
          gnnInfo = {
            available: true,
            version: gnn.version || '1.0.0',
            layers: [
              { name: 'RuvectorLayer', description: 'Differentiable vector search layer' },
              { name: 'TensorCompress', description: 'Tensor compression for embeddings' }
            ],
            features: [
              'differentiableSearch - Gradient-based vector search',
              'hierarchicalForward - Multi-scale graph processing',
              'getCompressionLevel - Adaptive compression'
            ]
          };
        } catch (e) {
          gnnInfo = { available: false, error: 'GNN package not installed' };
        }
        return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...gnnInfo }, null, 2) }] };
      }

      // Learning Engine Handlers (v2.1)
      case 'hooks_learning_config': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const engine = new LearningEngine();
        if (intel.learning) engine.import(intel.learning);

        if (args.task && args.algorithm) {
          const config = {};
          if (args.algorithm) config.algorithm = args.algorithm;
          if (args.learningRate !== undefined) config.learningRate = args.learningRate;
          if (args.discountFactor !== undefined) config.discountFactor = args.discountFactor;
          if (args.epsilon !== undefined) config.epsilon = args.epsilon;
          engine.configure(args.task, config);
          intel.learning = engine.export();
          intel.save();
        }

        const tasks = ['agent-routing', 'error-avoidance', 'confidence-scoring', 'trajectory-learning', 'context-ranking', 'memory-recall'];
        const configs = {};
        for (const task of tasks) {
          configs[task] = engine.getConfig(task);
        }
        return { content: [{ type: 'text', text: JSON.stringify({ success: true, configs }, null, 2) }] };
      }

      case 'hooks_learning_stats': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const engine = new LearningEngine();
        if (intel.learning) engine.import(intel.learning);

        const summary = engine.getStatsSummary();
        return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...summary }, null, 2) }] };
      }

      case 'hooks_learning_update': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const engine = new LearningEngine();
        if (intel.learning) engine.import(intel.learning);

        const experience = {
          state: args.state,
          action: args.action,
          reward: args.reward,
          nextState: args.nextState || args.state,
          done: args.done || false,
          timestamp: Date.now()
        };

        const delta = engine.update(args.task, experience);
        intel.learning = engine.export();
        intel.save();

        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          task: args.task,
          experience,
          delta,
          algorithm: engine.getConfig(args.task).algorithm
        }, null, 2) }] };
      }

      case 'hooks_learn': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const engine = new LearningEngine();
        if (intel.learning) engine.import(intel.learning);

        const task = args.task || 'agent-routing';
        let result = { success: true };

        if (args.action && args.reward !== undefined) {
          const experience = {
            state: args.state,
            action: args.action,
            reward: args.reward,
            nextState: args.state,
            done: true,
            timestamp: Date.now()
          };
          const delta = engine.update(task, experience);
          result.recorded = { experience, delta, algorithm: engine.getConfig(task).algorithm };
        }

        if (args.actions && args.actions.length > 0) {
          const best = engine.getBestAction(task, args.state, args.actions);
          result.recommendation = best;
        }

        intel.learning = engine.export();
        intel.save();

        return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
      }

      case 'hooks_algorithms_list': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const algorithms = LearningEngine.getAlgorithms();
        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          algorithms: algorithms.map(a => ({
            name: a.algorithm,
            description: a.description,
            bestFor: a.bestFor
          }))
        }, null, 2) }] };
      }

      // TensorCompress Handlers
      case 'hooks_compress': {
        let TensorCompress;
        try {
          TensorCompress = require('../dist/core/tensor-compress').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'TensorCompress not available' }) }] };
        }

        const compress = new TensorCompress({ autoCompress: false });
        if (intel.compressedPatterns) compress.import(intel.compressedPatterns);

        const stats = compress.recompressAll();
        intel.compressedPatterns = compress.export();
        intel.save();

        return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Compression complete', ...stats }, null, 2) }] };
      }

      case 'hooks_compress_stats': {
        let TensorCompress;
        try {
          TensorCompress = require('../dist/core/tensor-compress').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'TensorCompress not available' }) }] };
        }

        const compress = new TensorCompress({ autoCompress: false });
        if (intel.compressedPatterns) compress.import(intel.compressedPatterns);

        const stats = compress.getStats();
        return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...stats }, null, 2) }] };
      }

      case 'hooks_compress_store': {
        let TensorCompress;
        try {
          TensorCompress = require('../dist/core/tensor-compress').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'TensorCompress not available' }) }] };
        }

        const compress = new TensorCompress({ autoCompress: false });
        if (intel.compressedPatterns) compress.import(intel.compressedPatterns);

        compress.store(args.key, args.vector, args.level);
        intel.compressedPatterns = compress.export();
        intel.save();

        const stats = compress.getStats();
        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          key: args.key,
          level: args.level || 'auto',
          originalDim: args.vector.length,
          totalTensors: stats.totalTensors
        }, null, 2) }] };
      }

      case 'hooks_compress_get': {
        let TensorCompress;
        try {
          TensorCompress = require('../dist/core/tensor-compress').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'TensorCompress not available' }) }] };
        }

        const compress = new TensorCompress({ autoCompress: false });
        if (intel.compressedPatterns) compress.import(intel.compressedPatterns);

        const vector = compress.get(args.key);
        if (!vector) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'Key not found' }) }] };
        }

        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          key: args.key,
          vector: Array.from(vector),
          dimension: vector.length
        }, null, 2) }] };
      }

      case 'hooks_batch_learn': {
        let LearningEngine;
        try {
          LearningEngine = require('../dist/core/learning-engine').default;
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'LearningEngine not available' }) }] };
        }

        const experiences = args.experiences || [];
        if (!Array.isArray(experiences) || experiences.length === 0) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: 'experiences must be a non-empty array' }) }] };
        }

        const task = args.task || 'agent-routing';
        const engine = new LearningEngine();

        // Import existing learning data
        if (intel.data.learning) {
          engine.import(intel.data.learning);
        }

        const results = [];
        let totalReward = 0;

        for (const exp of experiences) {
          const experience = {
            state: exp.state,
            action: exp.action,
            reward: exp.reward ?? 0.5,
            nextState: exp.nextState ?? exp.state,
            done: exp.done ?? false,
            timestamp: Date.now()
          };

          const delta = engine.update(task, experience);
          totalReward += experience.reward;
          results.push({ state: exp.state, action: exp.action, reward: experience.reward, delta });
        }

        // Save
        intel.data.learning = engine.export();
        intel.save();

        const stats = engine.getStatsSummary();
        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          processed: experiences.length,
          avgReward: totalReward / experiences.length,
          results,
          stats: {
            bestAlgorithm: stats.bestAlgorithm,
            totalUpdates: stats.totalUpdates,
            avgReward: stats.avgReward
          }
        }, null, 2) }] };
      }

      case 'hooks_subscribe_snapshot': {
        const events = args.events || ['learn', 'route'];
        const lastState = args.lastState || { patterns: 0, memories: 0, trajectories: 0, updates: 0 };

        const stats = intel.data.stats || {};
        const learning = intel.data.learning?.stats || {};

        // Calculate current state
        let totalUpdates = 0;
        let bestAlgorithm = null;
        let bestAvgReward = -Infinity;

        Object.entries(learning).forEach(([algo, data]) => {
          if (data.updates) {
            totalUpdates += data.updates;
            if (data.avgReward > bestAvgReward) {
              bestAvgReward = data.avgReward;
              bestAlgorithm = algo;
            }
          }
        });

        const currentState = {
          patterns: stats.total_patterns || 0,
          memories: stats.total_memories || 0,
          trajectories: stats.total_trajectories || 0,
          updates: totalUpdates
        };

        // Calculate deltas
        const deltas = {
          patterns: currentState.patterns - (lastState.patterns || 0),
          memories: currentState.memories - (lastState.memories || 0),
          trajectories: currentState.trajectories - (lastState.trajectories || 0),
          updates: currentState.updates - (lastState.updates || 0)
        };

        const hasChanges = Object.values(deltas).some(d => d > 0);

        // Build events array
        const eventsList = [];
        if (events.includes('learn') && deltas.patterns > 0) {
          eventsList.push({ type: 'learn', subtype: 'pattern', delta: deltas.patterns, total: currentState.patterns });
        }
        if (events.includes('learn') && deltas.updates > 0) {
          eventsList.push({ type: 'learn', subtype: 'algorithm', delta: deltas.updates, total: currentState.updates, bestAlgorithm });
        }
        if (events.includes('memory') && deltas.memories > 0) {
          eventsList.push({ type: 'memory', delta: deltas.memories, total: currentState.memories });
        }
        if (events.includes('route') && deltas.trajectories > 0) {
          eventsList.push({ type: 'route', delta: deltas.trajectories, total: currentState.trajectories });
        }

        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          hasChanges,
          currentState,
          deltas,
          events: eventsList,
          bestAlgorithm,
          timestamp: Date.now()
        }, null, 2) }] };
      }

      case 'hooks_watch_status': {
        // Return current intelligence state as a "watch" status
        const stats = intel.data.stats || {};
        const patterns = Object.keys(intel.data.patterns || {});
        const recentPatterns = patterns.slice(-5);

        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          watching: true,
          stats: {
            totalPatterns: stats.total_patterns || 0,
            totalMemories: stats.total_memories || 0,
            totalTrajectories: stats.total_trajectories || 0,
            sessionCount: stats.session_count || 0
          },
          recentPatterns,
          lastUpdate: stats.last_session || Date.now(),
          tip: 'Use hooks_subscribe_snapshot with lastState for delta tracking'
        }, null, 2) }] };
      }

      // ============================================
      // BACKGROUND WORKERS HANDLERS (via agentic-flow)
      // ============================================
      case 'workers_dispatch': {
        const prompt = sanitizeShellArg(args.prompt);
        try {
          const result = execSync(`npx agentic-flow@alpha workers dispatch "${prompt.replace(/"/g, '\\"')}"`, {
            encoding: 'utf-8',
            timeout: 30000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            message: 'Worker dispatched',
            output: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            message: 'Worker dispatch attempted',
            note: 'Check workers status for progress'
          }, null, 2) }] };
        }
      }

      case 'workers_status': {
        try {
          const cmdArgs = args.workerId ? `workers status ${sanitizeShellArg(args.workerId)}` : 'workers status';
          const result = execSync(`npx agentic-flow@alpha ${cmdArgs}`, {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            status: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: 'Could not get worker status',
            message: e.message
          }, null, 2) }] };
        }
      }

      case 'workers_results': {
        try {
          const cmdArgs = args.json ? 'workers results --json' : 'workers results';
          const result = execSync(`npx agentic-flow@alpha ${cmdArgs}`, {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          if (args.json) {
            try {
              return { content: [{ type: 'text', text: JSON.stringify({
                success: true,
                results: JSON.parse(result.trim())
              }, null, 2) }] };
            } catch {
              return { content: [{ type: 'text', text: result.trim() }] };
            }
          }
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            results: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: 'Could not get worker results',
            message: e.message
          }, null, 2) }] };
        }
      }

      case 'workers_triggers': {
        try {
          const result = execSync('npx agentic-flow@alpha workers triggers', {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            triggers: result.trim()
          }, null, 2) }] };
        } catch (e) {
          // Return hardcoded list as fallback
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            triggers: ['ultralearn', 'optimize', 'consolidate', 'predict', 'audit', 'map', 'preload', 'deepdive', 'document', 'refactor', 'benchmark', 'testgaps']
          }, null, 2) }] };
        }
      }

      case 'workers_stats': {
        try {
          const result = execSync('npx agentic-flow@alpha workers stats', {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            stats: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: 'Could not get worker stats',
            message: e.message
          }, null, 2) }] };
        }
      }

      // Custom Worker System handlers (agentic-flow@alpha.39+)
      case 'workers_presets': {
        try {
          const result = execSync('npx agentic-flow@alpha workers presets', {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            presets: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            presets: ['quick-scan', 'deep-analysis', 'security-scan', 'learning', 'api-docs', 'test-analysis'],
            note: 'Hardcoded fallback - install agentic-flow@alpha for full support'
          }, null, 2) }] };
        }
      }

      case 'workers_phases': {
        try {
          const result = execSync('npx agentic-flow@alpha workers phases', {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            phases: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            phases: ['file-discovery', 'static-analysis', 'security-analysis', 'pattern-extraction', 'dependency-analysis', 'complexity-analysis', 'test-coverage', 'api-extraction', 'secret-detection', 'report-generation'],
            note: 'Partial list - install agentic-flow@alpha for all 24 phases'
          }, null, 2) }] };
        }
      }

      case 'workers_create': {
        const name = args.name;
        const preset = args.preset || 'quick-scan';
        const triggers = args.triggers;
        try {
          let cmd = `npx agentic-flow@alpha workers create "${name}" --preset ${preset}`;
          if (triggers) cmd += ` --triggers "${triggers}"`;
          const result = execSync(cmd, {
            encoding: 'utf-8',
            timeout: 30000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            message: `Worker '${name}' created with preset '${preset}'`,
            output: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: 'Worker creation failed',
            message: e.message
          }, null, 2) }] };
        }
      }

      case 'workers_run': {
        const name = sanitizeShellArg(args.name);
        const targetPath = sanitizeShellArg(args.path || '.');
        try {
          const result = execSync(`npx agentic-flow@alpha workers run "${name}" --path "${targetPath}"`, {
            encoding: 'utf-8',
            timeout: 120000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            worker: name,
            path: targetPath,
            output: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: `Worker '${name}' execution failed`,
            message: e.message
          }, null, 2) }] };
        }
      }

      case 'workers_custom': {
        try {
          const result = execSync('npx agentic-flow@alpha workers custom', {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            workers: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            workers: [],
            note: 'No custom workers registered'
          }, null, 2) }] };
        }
      }

      case 'workers_init_config': {
        try {
          let cmd = 'npx agentic-flow@alpha workers init-config';
          if (args.force) cmd += ' --force';
          const result = execSync(cmd, {
            encoding: 'utf-8',
            timeout: 15000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            message: 'workers.yaml config file created',
            output: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: 'Config init failed',
            message: e.message
          }, null, 2) }] };
        }
      }

      case 'workers_load_config': {
        const configFile = sanitizeShellArg(args.file || 'workers.yaml');
        try {
          const result = execSync(`npx agentic-flow@alpha workers load-config --file "${configFile}"`, {
            encoding: 'utf-8',
            timeout: 30000,
            stdio: ['pipe', 'pipe', 'pipe']
          });
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            file: configFile,
            output: result.trim()
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: `Config load failed from '${configFile}'`,
            message: e.message
          }, null, 2) }] };
        }
      }

      // ── RVF Tool Handlers ─────────────────────────────────────────────────
      case 'rvf_create': {
        try {
          const safePath = validateRvfPath(args.path);
          const { createRvfStore } = require('../dist/core/rvf-wrapper.js');
          const store = await createRvfStore(safePath, { dimension: args.dimension, metric: args.metric || 'cosine' });
          const status = store.status ? await store.status() : { dimension: args.dimension };
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, path: safePath, ...status }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message, hint: 'Install @ruvector/rvf: npm install @ruvector/rvf' }, null, 2) }], isError: true };
        }
      }

      case 'rvf_open': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfStatus } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const status = await rvfStatus(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, path: safePath, ...status }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_ingest': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfIngest, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const result = await rvfIngest(store, args.entries);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...result }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_query': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfQuery, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const results = await rvfQuery(store, args.vector, args.k || 10);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, results }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_delete': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfDelete, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const result = await rvfDelete(store, args.ids);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...result }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_status': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfStatus, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const status = await rvfStatus(store);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...status }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_compact': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfCompact, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const result = await rvfCompact(store);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, ...result }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_derive': {
        try {
          const safeParent = validateRvfPath(args.parent_path);
          const safeChild = validateRvfPath(args.child_path);
          const { openRvfStore, rvfDerive, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safeParent);
          await rvfDerive(store, safeChild);
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, parent: safeParent, child: safeChild }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_segments': {
        try {
          const safePath = validateRvfPath(args.path);
          const { openRvfStore, rvfClose } = require('../dist/core/rvf-wrapper.js');
          const store = await openRvfStore(safePath);
          const segs = await store.segments();
          await rvfClose(store);
          return { content: [{ type: 'text', text: JSON.stringify({ success: true, segments: segs }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({ success: false, error: e.message }, null, 2) }], isError: true };
        }
      }

      case 'rvf_examples': {
        const BASE_URL = 'https://raw.githubusercontent.com/ruvnet/ruvector/main/examples/rvf/output';
        const examples = [
          { name: 'basic_store', size: '152 KB', desc: '1,000 vectors, dim 128' },
          { name: 'semantic_search', size: '755 KB', desc: 'Semantic search with HNSW' },
          { name: 'rag_pipeline', size: '303 KB', desc: 'RAG pipeline embeddings' },
          { name: 'agent_memory', size: '32 KB', desc: 'AI agent episodic memory' },
          { name: 'swarm_knowledge', size: '86 KB', desc: 'Multi-agent knowledge base' },
          { name: 'self_booting', size: '31 KB', desc: 'Self-booting with kernel' },
          { name: 'ebpf_accelerator', size: '153 KB', desc: 'eBPF distance accelerator' },
          { name: 'tee_attestation', size: '102 KB', desc: 'TEE attestation + witnesses' },
          { name: 'lineage_parent', size: '52 KB', desc: 'COW parent file' },
          { name: 'lineage_child', size: '26 KB', desc: 'COW child (derived)' },
          { name: 'claude_code_appliance', size: '17 KB', desc: 'Claude Code appliance' },
          { name: 'progressive_index', size: '2.5 MB', desc: 'Large-scale HNSW index' },
        ];
        let filtered = examples;
        if (args.filter) {
          const f = args.filter.toLowerCase();
          filtered = examples.filter(e => e.name.includes(f) || e.desc.toLowerCase().includes(f));
        }
        return { content: [{ type: 'text', text: JSON.stringify({
          success: true,
          total: 45,
          shown: filtered.length,
          examples: filtered.map(e => ({ ...e, url: `${BASE_URL}/${e.name}.rvf` })),
          catalog: 'https://github.com/ruvnet/ruvector/tree/main/examples/rvf/output'
        }, null, 2) }] };
      }

      // ── rvlite Query Tool Handlers ──────────────────────────────────────
      case 'rvlite_sql': {
        try {
          let rvlite;
          try {
            rvlite = require('rvlite');
          } catch (_e) {
            return { content: [{ type: 'text', text: JSON.stringify({
              success: false,
              error: 'rvlite package not installed',
              hint: 'Install with: npm install rvlite'
            }, null, 2) }] };
          }
          const safeQuery = sanitizeShellArg(args.query);
          const dbOpts = args.db_path ? { path: validateRvfPath(args.db_path) } : {};
          const db = new rvlite.Database(dbOpts);
          const results = db.sql(safeQuery);
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            query_type: 'sql',
            results,
            row_count: Array.isArray(results) ? results.length : 0
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: e.message
          }, null, 2) }], isError: true };
        }
      }

      case 'rvlite_cypher': {
        try {
          let rvlite;
          try {
            rvlite = require('rvlite');
          } catch (_e) {
            return { content: [{ type: 'text', text: JSON.stringify({
              success: false,
              error: 'rvlite package not installed',
              hint: 'Install with: npm install rvlite'
            }, null, 2) }] };
          }
          const safeQuery = sanitizeShellArg(args.query);
          const dbOpts = args.db_path ? { path: validateRvfPath(args.db_path) } : {};
          const db = new rvlite.Database(dbOpts);
          const results = db.cypher(safeQuery);
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            query_type: 'cypher',
            results,
            row_count: Array.isArray(results) ? results.length : 0
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: e.message
          }, null, 2) }], isError: true };
        }
      }

      case 'rvlite_sparql': {
        try {
          let rvlite;
          try {
            rvlite = require('rvlite');
          } catch (_e) {
            return { content: [{ type: 'text', text: JSON.stringify({
              success: false,
              error: 'rvlite package not installed',
              hint: 'Install with: npm install rvlite'
            }, null, 2) }] };
          }
          const safeQuery = sanitizeShellArg(args.query);
          const dbOpts = args.db_path ? { path: validateRvfPath(args.db_path) } : {};
          const db = new rvlite.Database(dbOpts);
          const results = db.sparql(safeQuery);
          return { content: [{ type: 'text', text: JSON.stringify({
            success: true,
            query_type: 'sparql',
            results,
            row_count: Array.isArray(results) ? results.length : 0
          }, null, 2) }] };
        } catch (e) {
          return { content: [{ type: 'text', text: JSON.stringify({
            success: false,
            error: e.message
          }, null, 2) }], isError: true };
        }
      }

      default:
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ success: false, error: `Unknown tool: ${name}` }, null, 2)
          }],
          isError: true
        };
    }
  } catch (error) {
    return {
      content: [{
        type: 'text',
        text: JSON.stringify({ success: false, error: error.message }, null, 2)
      }],
      isError: true
    };
  }
});

// Resources - expose intelligence data
server.setRequestHandler(ListResourcesRequestSchema, async () => {
  return {
    resources: [
      {
        uri: 'ruvector://intelligence/stats',
        name: 'Intelligence Stats',
        description: 'Current RuVector intelligence statistics',
        mimeType: 'application/json'
      },
      {
        uri: 'ruvector://intelligence/patterns',
        name: 'Learned Patterns',
        description: 'Q-learning patterns for agent routing',
        mimeType: 'application/json'
      },
      {
        uri: 'ruvector://intelligence/memories',
        name: 'Vector Memories',
        description: 'Stored context memories',
        mimeType: 'application/json'
      }
    ]
  };
});

server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
  const { uri } = request.params;

  switch (uri) {
    case 'ruvector://intelligence/stats':
      return {
        contents: [{
          uri,
          mimeType: 'application/json',
          text: JSON.stringify(intel.stats(), null, 2)
        }]
      };

    case 'ruvector://intelligence/patterns':
      return {
        contents: [{
          uri,
          mimeType: 'application/json',
          text: JSON.stringify(intel.data.patterns || {}, null, 2)
        }]
      };

    case 'ruvector://intelligence/memories':
      return {
        contents: [{
          uri,
          mimeType: 'application/json',
          text: JSON.stringify((intel.data.memories || []).map(m => ({
            content: m.content,
            type: m.type,
            created: m.created
          })), null, 2)
        }]
      };

    default:
      throw new Error(`Unknown resource: ${uri}`);
  }
});

// Start server
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error('RuVector MCP server running on stdio');
}

main().catch(console.error);
