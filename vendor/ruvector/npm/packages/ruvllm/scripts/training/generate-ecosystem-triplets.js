#!/usr/bin/env node
/**
 * Ecosystem Training Data Generator for RuvLTRA
 *
 * Generates comprehensive triplet training data for the Claude Flow ecosystem:
 * - claude-flow: Multi-agent coordination and swarm orchestration
 * - agentic-flow: AI workflow orchestration and ONNX embeddings
 * - ruvector: High-performance vector database
 *
 * Features:
 * - Reads capability definitions from JSON files
 * - Generates 5-10 natural language prompts per capability
 * - Creates hard negatives for contrastive learning
 * - Outputs combined JSONL dataset for fine-tuning
 */

const fs = require('fs');
const path = require('path');

// ============================================================================
// CAPABILITY DEFINITIONS
// ============================================================================

/**
 * Claude Flow V3 Capabilities
 * Multi-agent swarm coordination, memory, hooks, workflows
 */
const CLAUDE_FLOW_CAPABILITIES = {
  name: 'claude-flow',
  description: 'Multi-agent swarm coordination and orchestration framework',
  version: '3.0.0',
  categories: {
    swarm: {
      description: 'Multi-agent swarm coordination and topology management',
      commands: {
        'swarm init': {
          description: 'Initialize a swarm with specified topology',
          keywords: ['swarm', 'init', 'initialize', 'topology', 'multi-agent', 'coordination'],
          parameters: ['--topology', '--max-agents', '--strategy'],
        },
        'swarm status': {
          description: 'Get current swarm status and agent health',
          keywords: ['swarm', 'status', 'health', 'agents', 'monitoring'],
          parameters: ['--verbose'],
        },
        'swarm shutdown': {
          description: 'Gracefully shutdown the swarm',
          keywords: ['swarm', 'shutdown', 'stop', 'terminate', 'graceful'],
          parameters: ['--graceful', '--force'],
        },
      },
    },
    agent: {
      description: 'Agent lifecycle management',
      commands: {
        'agent spawn': {
          description: 'Spawn a new agent with specified type',
          keywords: ['agent', 'spawn', 'create', 'start', 'worker'],
          parameters: ['-t', '--type', '--name', '--model'],
        },
        'agent list': {
          description: 'List all active agents',
          keywords: ['agent', 'list', 'show', 'active', 'running'],
          parameters: ['--status', '--domain'],
        },
        'agent terminate': {
          description: 'Terminate a specific agent',
          keywords: ['agent', 'terminate', 'kill', 'stop', 'remove'],
          parameters: ['--force'],
        },
        'agent status': {
          description: 'Get status of a specific agent',
          keywords: ['agent', 'status', 'info', 'details', 'health'],
          parameters: ['--agentId'],
        },
      },
    },
    memory: {
      description: 'Persistent memory with vector search',
      commands: {
        'memory store': {
          description: 'Store a value in memory with optional namespace',
          keywords: ['memory', 'store', 'save', 'persist', 'key-value'],
          parameters: ['--key', '--value', '--namespace', '--ttl'],
        },
        'memory retrieve': {
          description: 'Retrieve a value from memory by key',
          keywords: ['memory', 'retrieve', 'get', 'fetch', 'read'],
          parameters: ['--key', '--namespace'],
        },
        'memory search': {
          description: 'Semantic vector search in memory',
          keywords: ['memory', 'search', 'query', 'find', 'semantic', 'vector'],
          parameters: ['--query', '--namespace', '--limit', '--threshold'],
        },
        'memory list': {
          description: 'List memory entries',
          keywords: ['memory', 'list', 'entries', 'keys', 'show'],
          parameters: ['--namespace', '--limit'],
        },
        'memory delete': {
          description: 'Delete a memory entry',
          keywords: ['memory', 'delete', 'remove', 'clear'],
          parameters: ['--key', '--namespace'],
        },
      },
    },
    hooks: {
      description: 'Self-learning hooks and background workers',
      commands: {
        'hooks pre-task': {
          description: 'Get agent suggestions before starting a task',
          keywords: ['hooks', 'pre-task', 'routing', 'suggestions', 'before'],
          parameters: ['--description', '--taskId'],
        },
        'hooks post-task': {
          description: 'Record task completion for learning',
          keywords: ['hooks', 'post-task', 'completion', 'learning', 'after'],
          parameters: ['--taskId', '--success', '--quality'],
        },
        'hooks route': {
          description: 'Route task to optimal agent',
          keywords: ['hooks', 'route', 'routing', 'optimal', 'agent'],
          parameters: ['--task', '--context'],
        },
        'hooks worker dispatch': {
          description: 'Dispatch a background worker',
          keywords: ['hooks', 'worker', 'dispatch', 'background', 'trigger'],
          parameters: ['--trigger', '--context', '--priority'],
        },
        'hooks metrics': {
          description: 'View learning metrics dashboard',
          keywords: ['hooks', 'metrics', 'dashboard', 'stats', 'learning'],
          parameters: ['--period', '--format'],
        },
        'hooks pretrain': {
          description: 'Bootstrap intelligence from repository',
          keywords: ['hooks', 'pretrain', 'bootstrap', 'intelligence', 'analyze'],
          parameters: ['--path', '--depth'],
        },
      },
    },
    workflow: {
      description: 'Workflow execution and templates',
      commands: {
        'workflow create': {
          description: 'Create a new workflow',
          keywords: ['workflow', 'create', 'new', 'define'],
          parameters: ['--name', '--steps', '--description'],
        },
        'workflow execute': {
          description: 'Execute a workflow',
          keywords: ['workflow', 'execute', 'run', 'start'],
          parameters: ['--workflowId', '--variables'],
        },
        'workflow status': {
          description: 'Get workflow execution status',
          keywords: ['workflow', 'status', 'progress', 'state'],
          parameters: ['--workflowId', '--verbose'],
        },
      },
    },
    hivemind: {
      description: 'Hive-mind collective consensus',
      commands: {
        'hive-mind init': {
          description: 'Initialize hive-mind collective',
          keywords: ['hive-mind', 'init', 'collective', 'consensus'],
          parameters: ['--topology', '--queenId'],
        },
        'hive-mind spawn': {
          description: 'Spawn workers and join to hive-mind',
          keywords: ['hive-mind', 'spawn', 'workers', 'join'],
          parameters: ['--count', '--prefix', '--role'],
        },
        'hive-mind consensus': {
          description: 'Propose or vote on consensus',
          keywords: ['hive-mind', 'consensus', 'vote', 'propose'],
          parameters: ['--action', '--proposalId', '--vote'],
        },
        'hive-mind broadcast': {
          description: 'Broadcast message to all workers',
          keywords: ['hive-mind', 'broadcast', 'message', 'all'],
          parameters: ['--message', '--priority'],
        },
      },
    },
    task: {
      description: 'Task creation and management',
      commands: {
        'task create': {
          description: 'Create a new task',
          keywords: ['task', 'create', 'new', 'add'],
          parameters: ['--type', '--description', '--priority'],
        },
        'task list': {
          description: 'List all tasks',
          keywords: ['task', 'list', 'show', 'all'],
          parameters: ['--status', '--priority'],
        },
        'task complete': {
          description: 'Mark task as complete',
          keywords: ['task', 'complete', 'done', 'finish'],
          parameters: ['--taskId', '--result'],
        },
      },
    },
    session: {
      description: 'Session state management',
      commands: {
        'session save': {
          description: 'Save current session state',
          keywords: ['session', 'save', 'persist', 'state'],
          parameters: ['--name', '--description'],
        },
        'session restore': {
          description: 'Restore a saved session',
          keywords: ['session', 'restore', 'load', 'resume'],
          parameters: ['--sessionId', '--name'],
        },
        'session list': {
          description: 'List saved sessions',
          keywords: ['session', 'list', 'saved', 'history'],
          parameters: ['--limit'],
        },
      },
    },
    neural: {
      description: 'Neural pattern training and prediction',
      commands: {
        'neural train': {
          description: 'Train a neural model',
          keywords: ['neural', 'train', 'model', 'learning'],
          parameters: ['--modelType', '--epochs', '--learningRate'],
        },
        'neural predict': {
          description: 'Make predictions using neural model',
          keywords: ['neural', 'predict', 'inference', 'model'],
          parameters: ['--input', '--modelId'],
        },
        'neural patterns': {
          description: 'Manage neural patterns',
          keywords: ['neural', 'patterns', 'store', 'search'],
          parameters: ['--action', '--patternId'],
        },
      },
    },
    security: {
      description: 'Security scanning and threat detection',
      commands: {
        'aidefence scan': {
          description: 'Scan input for AI manipulation threats',
          keywords: ['aidefence', 'scan', 'security', 'threats', 'injection'],
          parameters: ['--input', '--quick'],
        },
        'aidefence analyze': {
          description: 'Deep analysis for threat types',
          keywords: ['aidefence', 'analyze', 'deep', 'threats'],
          parameters: ['--input', '--searchSimilar'],
        },
        'aidefence is_safe': {
          description: 'Quick boolean safety check',
          keywords: ['aidefence', 'safe', 'check', 'validate'],
          parameters: ['--input'],
        },
      },
    },
    performance: {
      description: 'Performance profiling and optimization',
      commands: {
        'performance benchmark': {
          description: 'Run performance benchmarks',
          keywords: ['performance', 'benchmark', 'speed', 'test'],
          parameters: ['--suite', '--iterations'],
        },
        'performance profile': {
          description: 'Profile specific component',
          keywords: ['performance', 'profile', 'analyze', 'bottleneck'],
          parameters: ['--target', '--duration'],
        },
        'performance optimize': {
          description: 'Apply performance optimizations',
          keywords: ['performance', 'optimize', 'improve', 'speed'],
          parameters: ['--target', '--aggressive'],
        },
      },
    },
    embeddings: {
      description: 'Vector embeddings with ONNX',
      commands: {
        'embeddings generate': {
          description: 'Generate embeddings for text',
          keywords: ['embeddings', 'generate', 'embed', 'vector'],
          parameters: ['--text', '--hyperbolic'],
        },
        'embeddings compare': {
          description: 'Compare similarity between texts',
          keywords: ['embeddings', 'compare', 'similarity', 'distance'],
          parameters: ['--text1', '--text2', '--metric'],
        },
        'embeddings search': {
          description: 'Semantic search across stored embeddings',
          keywords: ['embeddings', 'search', 'semantic', 'query'],
          parameters: ['--query', '--topK', '--threshold'],
        },
      },
    },
    claims: {
      description: 'Issue claiming and coordination',
      commands: {
        'claims claim': {
          description: 'Claim an issue for work',
          keywords: ['claims', 'claim', 'issue', 'work', 'assign'],
          parameters: ['--issueId', '--claimant'],
        },
        'claims release': {
          description: 'Release a claim on an issue',
          keywords: ['claims', 'release', 'unclaim', 'free'],
          parameters: ['--issueId', '--reason'],
        },
        'claims handoff': {
          description: 'Handoff issue to another claimant',
          keywords: ['claims', 'handoff', 'transfer', 'pass'],
          parameters: ['--issueId', '--from', '--to'],
        },
        'claims board': {
          description: 'View claims board',
          keywords: ['claims', 'board', 'view', 'overview'],
          parameters: [],
        },
      },
    },
  },
};

/**
 * Agentic Flow Capabilities
 * AI workflow orchestration and ONNX embeddings
 */
const AGENTIC_FLOW_CAPABILITIES = {
  name: 'agentic-flow',
  description: 'AI workflow orchestration with ONNX runtime and vector embeddings',
  version: '1.0.0',
  categories: {
    embeddings: {
      description: 'High-performance ONNX embeddings',
      commands: {
        'embed': {
          description: 'Generate embeddings for text using ONNX models',
          keywords: ['embed', 'embedding', 'vector', 'encode', 'onnx'],
          parameters: ['--text', '--model', '--normalize'],
        },
        'batch-embed': {
          description: 'Batch embed multiple texts efficiently',
          keywords: ['batch', 'embed', 'multiple', 'parallel'],
          parameters: ['--texts', '--concurrency'],
        },
        'similarity': {
          description: 'Compute similarity between embeddings',
          keywords: ['similarity', 'cosine', 'distance', 'compare'],
          parameters: ['--a', '--b', '--metric'],
        },
      },
    },
    models: {
      description: 'ONNX model management',
      commands: {
        'model load': {
          description: 'Load an ONNX model for inference',
          keywords: ['model', 'load', 'onnx', 'initialize'],
          parameters: ['--path', '--name'],
        },
        'model list': {
          description: 'List available models',
          keywords: ['model', 'list', 'available', 'show'],
          parameters: [],
        },
        'model info': {
          description: 'Get model information and metadata',
          keywords: ['model', 'info', 'metadata', 'details'],
          parameters: ['--name'],
        },
        'model quantize': {
          description: 'Quantize model for faster inference',
          keywords: ['model', 'quantize', 'compress', 'optimize'],
          parameters: ['--input', '--output', '--bits'],
        },
      },
    },
    pipeline: {
      description: 'Workflow pipeline orchestration',
      commands: {
        'pipeline create': {
          description: 'Create a new processing pipeline',
          keywords: ['pipeline', 'create', 'workflow', 'chain'],
          parameters: ['--name', '--steps'],
        },
        'pipeline run': {
          description: 'Execute a pipeline with input data',
          keywords: ['pipeline', 'run', 'execute', 'process'],
          parameters: ['--name', '--input'],
        },
        'pipeline visualize': {
          description: 'Visualize pipeline structure',
          keywords: ['pipeline', 'visualize', 'graph', 'diagram'],
          parameters: ['--name', '--format'],
        },
      },
    },
    cache: {
      description: 'Embedding cache management',
      commands: {
        'cache set': {
          description: 'Store embedding in cache',
          keywords: ['cache', 'set', 'store', 'save'],
          parameters: ['--key', '--embedding'],
        },
        'cache get': {
          description: 'Retrieve embedding from cache',
          keywords: ['cache', 'get', 'retrieve', 'fetch'],
          parameters: ['--key'],
        },
        'cache clear': {
          description: 'Clear embedding cache',
          keywords: ['cache', 'clear', 'flush', 'reset'],
          parameters: ['--namespace'],
        },
        'cache stats': {
          description: 'Get cache statistics',
          keywords: ['cache', 'stats', 'statistics', 'info'],
          parameters: [],
        },
      },
    },
    search: {
      description: 'Vector search operations',
      commands: {
        'search nearest': {
          description: 'Find nearest neighbors to query vector',
          keywords: ['search', 'nearest', 'neighbors', 'knn'],
          parameters: ['--query', '--k', '--threshold'],
        },
        'search range': {
          description: 'Range search within distance threshold',
          keywords: ['search', 'range', 'radius', 'threshold'],
          parameters: ['--query', '--radius'],
        },
        'search hybrid': {
          description: 'Hybrid search combining keyword and semantic',
          keywords: ['search', 'hybrid', 'combined', 'keyword', 'semantic'],
          parameters: ['--query', '--alpha'],
        },
      },
    },
  },
};

/**
 * RuVector Capabilities
 * High-performance vector database
 */
const RUVECTOR_CAPABILITIES = {
  name: 'ruvector',
  description: 'High-performance vector database with HNSW indexing',
  version: '0.1.0',
  categories: {
    collections: {
      description: 'Vector collection management',
      commands: {
        'collection create': {
          description: 'Create a new vector collection',
          keywords: ['collection', 'create', 'new', 'database'],
          parameters: ['--name', '--dimension', '--metric'],
        },
        'collection delete': {
          description: 'Delete a vector collection',
          keywords: ['collection', 'delete', 'drop', 'remove'],
          parameters: ['--name', '--confirm'],
        },
        'collection info': {
          description: 'Get collection information',
          keywords: ['collection', 'info', 'stats', 'details'],
          parameters: ['--name'],
        },
        'collection list': {
          description: 'List all collections',
          keywords: ['collection', 'list', 'all', 'show'],
          parameters: [],
        },
      },
    },
    vectors: {
      description: 'Vector CRUD operations',
      commands: {
        'vector insert': {
          description: 'Insert vectors into collection',
          keywords: ['vector', 'insert', 'add', 'upsert'],
          parameters: ['--collection', '--vectors', '--ids'],
        },
        'vector delete': {
          description: 'Delete vectors from collection',
          keywords: ['vector', 'delete', 'remove', 'drop'],
          parameters: ['--collection', '--ids'],
        },
        'vector get': {
          description: 'Get vectors by ID',
          keywords: ['vector', 'get', 'fetch', 'retrieve'],
          parameters: ['--collection', '--ids'],
        },
        'vector update': {
          description: 'Update existing vectors',
          keywords: ['vector', 'update', 'modify', 'change'],
          parameters: ['--collection', '--id', '--vector'],
        },
      },
    },
    search: {
      description: 'Vector search with HNSW',
      commands: {
        'search knn': {
          description: 'K-nearest neighbor search',
          keywords: ['search', 'knn', 'nearest', 'similar'],
          parameters: ['--collection', '--query', '--k'],
        },
        'search filter': {
          description: 'Filtered vector search with metadata',
          keywords: ['search', 'filter', 'metadata', 'conditional'],
          parameters: ['--collection', '--query', '--filter'],
        },
        'search batch': {
          description: 'Batch search multiple queries',
          keywords: ['search', 'batch', 'multiple', 'parallel'],
          parameters: ['--collection', '--queries', '--k'],
        },
      },
    },
    index: {
      description: 'HNSW index management',
      commands: {
        'index build': {
          description: 'Build HNSW index for collection',
          keywords: ['index', 'build', 'create', 'hnsw'],
          parameters: ['--collection', '--ef', '--m'],
        },
        'index rebuild': {
          description: 'Rebuild existing index',
          keywords: ['index', 'rebuild', 'refresh', 'reindex'],
          parameters: ['--collection'],
        },
        'index stats': {
          description: 'Get index statistics',
          keywords: ['index', 'stats', 'info', 'metrics'],
          parameters: ['--collection'],
        },
        'index optimize': {
          description: 'Optimize index for search performance',
          keywords: ['index', 'optimize', 'tune', 'improve'],
          parameters: ['--collection', '--target'],
        },
      },
    },
    persistence: {
      description: 'Data persistence and backup',
      commands: {
        'snapshot create': {
          description: 'Create a snapshot of collection',
          keywords: ['snapshot', 'create', 'backup', 'save'],
          parameters: ['--collection', '--path'],
        },
        'snapshot restore': {
          description: 'Restore collection from snapshot',
          keywords: ['snapshot', 'restore', 'load', 'recover'],
          parameters: ['--path', '--collection'],
        },
        'snapshot list': {
          description: 'List available snapshots',
          keywords: ['snapshot', 'list', 'backups', 'show'],
          parameters: ['--collection'],
        },
      },
    },
    quantization: {
      description: 'Vector quantization for memory efficiency',
      commands: {
        'quantize apply': {
          description: 'Apply quantization to collection',
          keywords: ['quantize', 'apply', 'compress', 'reduce'],
          parameters: ['--collection', '--type', '--bits'],
        },
        'quantize info': {
          description: 'Get quantization info',
          keywords: ['quantize', 'info', 'status', 'details'],
          parameters: ['--collection'],
        },
      },
    },
  },
};

// ============================================================================
// PROMPT TEMPLATES
// ============================================================================

/**
 * Natural language prompt variations for generating diverse training data
 * Each template has placeholders for action, object, and description
 */
const PROMPT_TEMPLATES = {
  // Direct commands
  direct: [
    '{action} {object}',
    '{action} the {object}',
    'Please {action} {object}',
    'I need to {action} {object}',
  ],
  // Request style
  request: [
    'Can you {action} {object}?',
    'Help me {action} {object}',
    'I want to {action} {object}',
    'Could you {action} {object} for me?',
  ],
  // Question style
  question: [
    'How do I {action} {object}?',
    'What\'s the best way to {action} {object}?',
    'How can I {action} {object}?',
    'Which command {desc}?',
  ],
  // Contextual
  contextual: [
    'I\'m trying to {desc}',
    'I need a way to {desc}',
    'My goal is to {desc}',
    'For this project, I need to {desc}',
  ],
  // Descriptive
  descriptive: [
    '{desc}',
    'Help with {desc}',
    'I want {desc}',
    'Need {desc}',
  ],
};

/**
 * Action verbs mapped to capability types
 */
const ACTION_MAPPINGS = {
  // Swarm/Agent actions
  swarm: ['initialize', 'start', 'create', 'set up', 'configure', 'launch'],
  agent: ['spawn', 'create', 'start', 'launch', 'deploy', 'run'],
  terminate: ['stop', 'kill', 'terminate', 'shutdown', 'end', 'close'],
  status: ['check', 'get', 'view', 'show', 'monitor', 'inspect'],
  list: ['list', 'show', 'display', 'enumerate', 'get all'],

  // Memory actions
  store: ['store', 'save', 'persist', 'write', 'put', 'cache'],
  retrieve: ['retrieve', 'get', 'fetch', 'read', 'load'],
  search: ['search', 'find', 'query', 'look up', 'discover'],
  delete: ['delete', 'remove', 'clear', 'drop', 'erase'],

  // Workflow actions
  create: ['create', 'make', 'build', 'define', 'set up'],
  execute: ['execute', 'run', 'start', 'trigger', 'launch'],

  // Vector operations
  embed: ['embed', 'encode', 'vectorize', 'generate embedding for'],
  insert: ['insert', 'add', 'upsert', 'put', 'store'],
  knn: ['find similar', 'search for nearest', 'query neighbors', 'find k-nearest'],

  // Index operations
  build: ['build', 'create', 'construct', 'generate'],
  optimize: ['optimize', 'tune', 'improve', 'speed up'],
  rebuild: ['rebuild', 'regenerate', 'refresh', 'recreate'],
};

// ============================================================================
// HARD NEGATIVE PATTERNS
// ============================================================================

/**
 * Confusing command pairs for hard negative generation
 * These are commands that sound similar but have different purposes
 */
const CONFUSING_PAIRS = [
  // Claude Flow internal confusion
  { cmd1: 'memory store', cmd2: 'memory search', reason: 'both involve memory' },
  { cmd1: 'agent spawn', cmd2: 'hive-mind spawn', reason: 'both spawn workers' },
  { cmd1: 'swarm init', cmd2: 'hive-mind init', reason: 'both initialize coordination' },
  { cmd1: 'hooks route', cmd2: 'hooks pre-task', reason: 'both involve task routing' },
  { cmd1: 'workflow execute', cmd2: 'task create', reason: 'both start work' },
  { cmd1: 'session save', cmd2: 'memory store', reason: 'both persist data' },
  { cmd1: 'neural train', cmd2: 'hooks pretrain', reason: 'both involve training' },
  { cmd1: 'embeddings search', cmd2: 'memory search', reason: 'both search semantically' },
  { cmd1: 'performance profile', cmd2: 'performance benchmark', reason: 'both analyze performance' },
  { cmd1: 'claims claim', cmd2: 'task create', reason: 'both assign work' },

  // Cross-tool confusion
  { cmd1: 'claude-flow memory search', cmd2: 'ruvector search knn', reason: 'both do vector search' },
  { cmd1: 'claude-flow embeddings generate', cmd2: 'agentic-flow embed', reason: 'both generate embeddings' },
  { cmd1: 'ruvector collection create', cmd2: 'claude-flow memory store', reason: 'both store data' },
  { cmd1: 'agentic-flow cache set', cmd2: 'claude-flow memory store', reason: 'both cache data' },
  { cmd1: 'ruvector index build', cmd2: 'claude-flow hooks pretrain', reason: 'both build indexes' },
  { cmd1: 'agentic-flow search hybrid', cmd2: 'ruvector search filter', reason: 'both filtered search' },
  { cmd1: 'claude-flow swarm init', cmd2: 'agentic-flow pipeline create', reason: 'both orchestrate work' },

  // Category confusion
  { cmd1: 'ruvector vector insert', cmd2: 'agentic-flow cache set', reason: 'both store vectors' },
  { cmd1: 'claude-flow agent list', cmd2: 'agentic-flow model list', reason: 'both list resources' },
  { cmd1: 'ruvector snapshot create', cmd2: 'claude-flow session save', reason: 'both create backups' },
  { cmd1: 'agentic-flow model quantize', cmd2: 'ruvector quantize apply', reason: 'both quantize' },
];

// ============================================================================
// TRIPLET GENERATION
// ============================================================================

/**
 * Generate natural language prompts for a capability
 */
function generatePrompts(capability, command, config) {
  const prompts = [];
  const { description, keywords } = config;

  // Extract action and object from command
  const parts = command.split(' ');
  const category = parts[0];
  const action = parts[1] || parts[0];

  // Get action variations - but avoid repeating category name as action
  const actionVariations = ACTION_MAPPINGS[action] || [action];
  const primaryAction = actionVariations[0];

  // Create clean description for prompts
  const descLower = description.toLowerCase();

  // Avoid redundant phrases like "search search" or "status status"
  const isActionSameAsCategory = primaryAction.toLowerCase() === category.toLowerCase();

  // Direct commands: "{action} {object}"
  if (!isActionSameAsCategory) {
    prompts.push(`${primaryAction} ${category}`);
    prompts.push(`${primaryAction} the ${category}`);
    prompts.push(`I need to ${primaryAction} ${category}`);
    prompts.push(`Can you ${primaryAction} ${category}?`);
    prompts.push(`Help me ${primaryAction} ${category}`);
  } else {
    // When action == category, use description instead
    prompts.push(`${primaryAction}`);
    prompts.push(`I need to ${primaryAction}`);
    prompts.push(`Help me ${primaryAction}`);
  }

  // Use description-based prompts (always good quality)
  prompts.push(`I want to ${descLower}`);
  prompts.push(`How do I ${descLower}?`);
  prompts.push(`I need a way to ${descLower}`);

  // Action variations (skip if redundant)
  for (const actionVar of actionVariations.slice(1, 3)) {
    if (actionVar.toLowerCase() !== category.toLowerCase()) {
      prompts.push(`${actionVar} ${category}`);
      prompts.push(`I want to ${actionVar} ${category}`);
    }
  }

  // Keyword-based prompts (only use unique keywords not in action/category)
  const usedWords = new Set([primaryAction.toLowerCase(), category.toLowerCase(),
    ...actionVariations.map(a => a.toLowerCase())]);
  for (const keyword of keywords) {
    const kwLower = keyword.toLowerCase();
    if (!usedWords.has(kwLower) && kwLower !== category.toLowerCase()) {
      prompts.push(`${keyword} in ${category}`);
      prompts.push(`I need ${keyword} functionality`);
      usedWords.add(kwLower);
      if (usedWords.size > keywords.length + 3) break;
    }
  }

  // Tool-specific technical prompts
  prompts.push(`run ${command}`);
  prompts.push(`${capability} ${command}`);

  // Clean up prompts: remove duplicates, fix spacing, validate
  const cleanPrompts = [...new Set(prompts)]
    .map(p => p.trim().replace(/\s+/g, ' '))
    .filter(p => {
      // Filter out bad prompts
      if (p.length < 5) return false;
      if (p.includes('undefined')) return false;
      // Check for redundant word repetition (e.g., "status status")
      const words = p.toLowerCase().split(' ');
      for (let i = 0; i < words.length - 1; i++) {
        if (words[i] === words[i + 1] && words[i].length > 2) return false;
      }
      return true;
    })
    .slice(0, 10);

  return cleanPrompts;
}

/**
 * Find hard negatives for a command
 */
function findHardNegatives(tool, command, allCapabilities) {
  const negatives = [];
  const fullCommand = `${tool} ${command}`;

  // Find from predefined confusing pairs
  for (const pair of CONFUSING_PAIRS) {
    if (pair.cmd1.includes(command) || pair.cmd2.includes(command)) {
      const negative = pair.cmd1.includes(command) ? pair.cmd2 : pair.cmd1;
      negatives.push({
        command: negative,
        reason: pair.reason,
      });
    }
  }

  // Find similar commands from other tools
  for (const cap of allCapabilities) {
    if (cap.name === tool) continue;

    for (const [catName, category] of Object.entries(cap.categories)) {
      for (const [cmdName, cmdConfig] of Object.entries(category.commands)) {
        // Check for keyword overlap
        const cmdKeywords = cmdConfig.keywords || [];
        const sourceConfig = getCommandConfig(tool, command, allCapabilities);
        const sourceKeywords = sourceConfig?.keywords || [];

        const overlap = cmdKeywords.filter((k) => sourceKeywords.includes(k));
        if (overlap.length >= 2) {
          negatives.push({
            command: `${cap.name} ${cmdName}`,
            reason: `keyword overlap: ${overlap.join(', ')}`,
          });
        }
      }
    }
  }

  // Find similar commands within same tool (different category)
  const sourceCapability = allCapabilities.find((c) => c.name === tool);
  if (sourceCapability) {
    const [sourceCategory] = command.split(' ');
    for (const [catName, category] of Object.entries(sourceCapability.categories)) {
      if (catName === sourceCategory) continue;

      for (const [cmdName, cmdConfig] of Object.entries(category.commands)) {
        // Similar action words
        const cmdAction = cmdName.split(' ')[1] || cmdName.split(' ')[0];
        const sourceAction = command.split(' ')[1] || command.split(' ')[0];

        if (cmdAction === sourceAction) {
          negatives.push({
            command: `${tool} ${cmdName}`,
            reason: `same action '${cmdAction}' different category`,
          });
        }
      }
    }
  }

  // Limit and deduplicate
  const seen = new Set();
  return negatives.filter((n) => {
    if (seen.has(n.command)) return false;
    seen.add(n.command);
    return true;
  }).slice(0, 5);
}

/**
 * Get command config from capabilities
 */
function getCommandConfig(tool, command, allCapabilities) {
  const capability = allCapabilities.find((c) => c.name === tool);
  if (!capability) return null;

  for (const category of Object.values(capability.categories)) {
    if (category.commands[command]) {
      return category.commands[command];
    }
  }
  return null;
}

/**
 * Generate triplets for all capabilities
 */
function generateTriplets(capabilities) {
  const triplets = [];

  for (const cap of capabilities) {
    for (const [catName, category] of Object.entries(cap.categories)) {
      for (const [cmdName, cmdConfig] of Object.entries(category.commands)) {
        const fullCommand = `${cap.name} ${cmdName}`;
        const prompts = generatePrompts(cap.name, cmdName, cmdConfig);
        const negatives = findHardNegatives(cap.name, cmdName, capabilities);

        // Create triplets
        for (const prompt of prompts) {
          // Skip malformed prompts
          if (!prompt || prompt.length < 5) continue;

          // For each prompt, create triplets with each negative
          if (negatives.length > 0) {
            for (const negative of negatives) {
              // Ensure negative has full tool prefix
              let negCommand = negative.command;
              if (!negCommand.includes(' ') || (!negCommand.startsWith('claude-flow') &&
                  !negCommand.startsWith('agentic-flow') && !negCommand.startsWith('ruvector'))) {
                continue; // Skip incomplete negatives
              }

              // Skip if negative equals positive
              if (negCommand === fullCommand) continue;

              triplets.push({
                anchor: prompt,
                positive: fullCommand,
                negative: negCommand,
                isHard: true,
                category: catName,
                tool: cap.name,
              });
            }
          } else {
            // Create triplet with random different tool command as negative
            const otherCaps = capabilities.filter((c) => c.name !== cap.name);
            if (otherCaps.length > 0) {
              const randomCap = otherCaps[Math.floor(Math.random() * otherCaps.length)];
              const randomCatName = Object.keys(randomCap.categories)[0];
              const randomCmdName = Object.keys(randomCap.categories[randomCatName].commands)[0];
              const negCommand = `${randomCap.name} ${randomCmdName}`;

              // Skip if somehow equals positive
              if (negCommand === fullCommand) continue;

              triplets.push({
                anchor: prompt,
                positive: fullCommand,
                negative: negCommand,
                isHard: false,
                category: catName,
                tool: cap.name,
              });
            }
          }
        }
      }
    }
  }

  return triplets;
}

/**
 * Generate category-specific examples with rich diversity
 */
function generateCategoryExamples() {
  const examples = [];

  // ---- SWARM COORDINATION ----
  const swarmExamples = [
    // Initialization
    { anchor: 'Set up a multi-agent swarm for parallel processing', positive: 'claude-flow swarm init', negatives: ['agentic-flow pipeline create', 'claude-flow agent spawn'] },
    { anchor: 'Initialize hierarchical agent coordination', positive: 'claude-flow swarm init', negatives: ['claude-flow hive-mind init'] },
    { anchor: 'Create a mesh topology swarm with 10 agents', positive: 'claude-flow swarm init', negatives: ['claude-flow hive-mind spawn'] },
    { anchor: 'Configure swarm consensus using raft protocol', positive: 'claude-flow swarm init', negatives: ['claude-flow hive-mind consensus'] },
    { anchor: 'Start a queen-led hive-mind collective', positive: 'claude-flow hive-mind init', negatives: ['claude-flow swarm init'] },
    { anchor: 'Coordinate multiple AI agents on a complex task', positive: 'claude-flow swarm init', negatives: ['agentic-flow pipeline create'] },
    { anchor: 'Set up Byzantine fault-tolerant consensus', positive: 'claude-flow hive-mind init', negatives: ['claude-flow swarm init'] },
    { anchor: 'Launch a distributed agent network', positive: 'claude-flow swarm init', negatives: ['claude-flow agent spawn'] },
    { anchor: 'Create a star topology for centralized coordination', positive: 'claude-flow swarm init', negatives: ['claude-flow hive-mind init'] },
    // Status/monitoring
    { anchor: 'Check the health of all running agents', positive: 'claude-flow swarm status', negatives: ['claude-flow agent status'] },
    { anchor: 'Monitor swarm performance and throughput', positive: 'claude-flow swarm status', negatives: ['claude-flow performance benchmark'] },
    { anchor: 'Get swarm coordination status', positive: 'claude-flow swarm status', negatives: ['claude-flow agent list'] },
  ];

  // ---- AGENT MANAGEMENT ----
  const agentExamples = [
    { anchor: 'Spawn a coder agent to implement features', positive: 'claude-flow agent spawn', negatives: ['claude-flow hive-mind spawn', 'claude-flow task create'] },
    { anchor: 'Create a new worker agent for this task', positive: 'claude-flow agent spawn', negatives: ['claude-flow hive-mind spawn'] },
    { anchor: 'Start a researcher agent to investigate', positive: 'claude-flow agent spawn', negatives: ['claude-flow task create'] },
    { anchor: 'List all active agents in the system', positive: 'claude-flow agent list', negatives: ['agentic-flow model list', 'ruvector collection list'] },
    { anchor: 'Show running agent processes', positive: 'claude-flow agent list', negatives: ['claude-flow task list'] },
    { anchor: 'Kill a misbehaving agent', positive: 'claude-flow agent terminate', negatives: ['claude-flow swarm shutdown'] },
    { anchor: 'Stop the agent that is stuck', positive: 'claude-flow agent terminate', negatives: ['claude-flow task cancel'] },
    { anchor: 'Get details about a specific agent', positive: 'claude-flow agent status', negatives: ['claude-flow swarm status'] },
  ];

  // ---- MEMORY OPERATIONS ----
  const memoryExamples = [
    // Store operations
    { anchor: 'Store learned patterns for future reference', positive: 'claude-flow memory store', negatives: ['ruvector vector insert', 'agentic-flow cache set'] },
    { anchor: 'Save task completion metrics to memory', positive: 'claude-flow memory store', negatives: ['claude-flow session save'] },
    { anchor: 'Persist agent decisions for analysis', positive: 'claude-flow memory store', negatives: ['ruvector vector insert'] },
    { anchor: 'Cache successful code patterns', positive: 'claude-flow memory store', negatives: ['agentic-flow cache set'] },
    { anchor: 'Remember this debugging solution', positive: 'claude-flow memory store', negatives: ['agentic-flow cache set'] },
    { anchor: 'Store API response for later retrieval', positive: 'claude-flow memory store', negatives: ['ruvector vector insert'] },
    { anchor: 'Save this configuration for reuse', positive: 'claude-flow memory store', negatives: ['claude-flow session save'] },
    // Retrieve operations
    { anchor: 'Get the stored pattern for authentication', positive: 'claude-flow memory retrieve', negatives: ['claude-flow memory search', 'ruvector vector get'] },
    { anchor: 'Fetch previously saved configuration', positive: 'claude-flow memory retrieve', negatives: ['claude-flow session restore'] },
    { anchor: 'Load cached data from memory', positive: 'claude-flow memory retrieve', negatives: ['agentic-flow cache get'] },
    // Search operations
    { anchor: 'Search memory for similar patterns', positive: 'claude-flow memory search', negatives: ['ruvector search knn', 'agentic-flow search nearest'] },
    { anchor: 'Find relevant past solutions', positive: 'claude-flow memory search', negatives: ['ruvector search filter'] },
    { anchor: 'Query semantic memory for debugging tips', positive: 'claude-flow memory search', negatives: ['agentic-flow search hybrid'] },
    { anchor: 'Look up related patterns in storage', positive: 'claude-flow memory search', negatives: ['ruvector search knn'] },
  ];

  // ---- VECTOR DATABASE (RUVECTOR) ----
  const vectorExamples = [
    // Search operations
    { anchor: 'Find k-nearest matches to this embedding', positive: 'ruvector search knn', negatives: ['claude-flow memory search', 'agentic-flow search nearest'] },
    { anchor: 'Search vectors with metadata filters', positive: 'ruvector search filter', negatives: ['claude-flow memory search', 'agentic-flow search hybrid'] },
    { anchor: 'Perform approximate nearest neighbor search', positive: 'ruvector search knn', negatives: ['agentic-flow search nearest'] },
    { anchor: 'Query the vector database for similar items', positive: 'ruvector search knn', negatives: ['claude-flow memory search'] },
    { anchor: 'Find similar embeddings in the collection', positive: 'ruvector search knn', negatives: ['claude-flow embeddings search'] },
    { anchor: 'Batch search multiple query vectors', positive: 'ruvector search batch', negatives: ['agentic-flow batch-embed'] },
    // Collection operations
    { anchor: 'Create a new vector collection for documents', positive: 'ruvector collection create', negatives: ['claude-flow memory store'] },
    { anchor: 'Set up a database for embedding storage', positive: 'ruvector collection create', negatives: ['agentic-flow cache set'] },
    { anchor: 'Delete the old vector collection', positive: 'ruvector collection delete', negatives: ['claude-flow memory delete'] },
    { anchor: 'Get information about the collection', positive: 'ruvector collection info', negatives: ['agentic-flow model info'] },
    // Vector CRUD
    { anchor: 'Insert embeddings into the database', positive: 'ruvector vector insert', negatives: ['claude-flow memory store', 'agentic-flow cache set'] },
    { anchor: 'Add vectors to the collection', positive: 'ruvector vector insert', negatives: ['claude-flow memory store'] },
    { anchor: 'Upsert vectors with metadata', positive: 'ruvector vector insert', negatives: ['agentic-flow cache set'] },
    { anchor: 'Delete vectors by ID', positive: 'ruvector vector delete', negatives: ['claude-flow memory delete'] },
    // Index operations
    { anchor: 'Build HNSW index for faster search', positive: 'ruvector index build', negatives: ['claude-flow hooks pretrain'] },
    { anchor: 'Create search index for vectors', positive: 'ruvector index build', negatives: ['claude-flow neural train'] },
    { anchor: 'Optimize index for query performance', positive: 'ruvector index optimize', negatives: ['claude-flow performance optimize'] },
    { anchor: 'Rebuild search index after updates', positive: 'ruvector index rebuild', negatives: ['claude-flow hooks pretrain'] },
    { anchor: 'Configure HNSW parameters for accuracy', positive: 'ruvector index build', negatives: ['ruvector quantize apply'] },
    // Persistence
    { anchor: 'Create a snapshot backup of vectors', positive: 'ruvector snapshot create', negatives: ['claude-flow session save'] },
    { anchor: 'Restore vectors from backup', positive: 'ruvector snapshot restore', negatives: ['claude-flow session restore'] },
  ];

  // ---- EMBEDDINGS (AGENTIC-FLOW) ----
  const embeddingExamples = [
    { anchor: 'Generate embeddings for these documents', positive: 'agentic-flow embed', negatives: ['claude-flow embeddings generate'] },
    { anchor: 'Create vector representations of text', positive: 'agentic-flow embed', negatives: ['claude-flow embeddings generate'] },
    { anchor: 'Encode sentences into embeddings', positive: 'agentic-flow embed', negatives: ['claude-flow embeddings generate'] },
    { anchor: 'Vectorize code snippets for search', positive: 'agentic-flow embed', negatives: ['ruvector vector insert'] },
    { anchor: 'Produce semantic embeddings from descriptions', positive: 'agentic-flow embed', negatives: ['claude-flow embeddings generate'] },
    { anchor: 'Convert text to numerical vectors using ONNX', positive: 'agentic-flow embed', negatives: ['claude-flow embeddings generate'] },
    { anchor: 'Batch embed multiple documents efficiently', positive: 'agentic-flow batch-embed', negatives: ['ruvector search batch'] },
    { anchor: 'Embed large corpus in parallel', positive: 'agentic-flow batch-embed', negatives: ['ruvector vector insert'] },
    { anchor: 'Compare similarity between two texts', positive: 'agentic-flow similarity', negatives: ['claude-flow embeddings compare'] },
    { anchor: 'Calculate cosine distance between embeddings', positive: 'agentic-flow similarity', negatives: ['claude-flow embeddings compare'] },
    // Model operations
    { anchor: 'Load the ONNX model for inference', positive: 'agentic-flow model load', negatives: ['claude-flow neural train'] },
    { anchor: 'List available embedding models', positive: 'agentic-flow model list', negatives: ['claude-flow agent list'] },
    { anchor: 'Quantize the model for faster inference', positive: 'agentic-flow model quantize', negatives: ['ruvector quantize apply'] },
    // Cache operations
    { anchor: 'Cache the embedding for reuse', positive: 'agentic-flow cache set', negatives: ['claude-flow memory store'] },
    { anchor: 'Get cached embedding', positive: 'agentic-flow cache get', negatives: ['claude-flow memory retrieve'] },
    { anchor: 'Clear the embedding cache', positive: 'agentic-flow cache clear', negatives: ['claude-flow memory delete'] },
    // Search
    { anchor: 'Find nearest neighbors to query', positive: 'agentic-flow search nearest', negatives: ['ruvector search knn'] },
    { anchor: 'Hybrid keyword and semantic search', positive: 'agentic-flow search hybrid', negatives: ['ruvector search filter'] },
    // Pipeline
    { anchor: 'Create an embedding pipeline', positive: 'agentic-flow pipeline create', negatives: ['claude-flow workflow create'] },
    { anchor: 'Run the processing pipeline', positive: 'agentic-flow pipeline run', negatives: ['claude-flow workflow execute'] },
  ];

  // ---- HOOKS AND LEARNING ----
  const hookExamples = [
    { anchor: 'Route this task to the optimal agent', positive: 'claude-flow hooks route', negatives: ['claude-flow agent spawn', 'claude-flow hooks pre-task'] },
    { anchor: 'Get agent suggestions before starting work', positive: 'claude-flow hooks pre-task', negatives: ['claude-flow hooks route'] },
    { anchor: 'Record task completion for learning', positive: 'claude-flow hooks post-task', negatives: ['claude-flow hooks metrics'] },
    { anchor: 'Analyze codebase to bootstrap intelligence', positive: 'claude-flow hooks pretrain', negatives: ['claude-flow neural train', 'ruvector index build'] },
    { anchor: 'Track metrics from completed tasks', positive: 'claude-flow hooks metrics', negatives: ['claude-flow performance benchmark'] },
    { anchor: 'Pre-train routing model on repository', positive: 'claude-flow hooks pretrain', negatives: ['claude-flow neural train'] },
    { anchor: 'Dispatch a background worker for optimization', positive: 'claude-flow hooks worker dispatch', negatives: ['claude-flow agent spawn'] },
    { anchor: 'Log task before starting', positive: 'claude-flow hooks pre-task', negatives: ['claude-flow task create'] },
  ];

  // ---- WORKFLOW AND TASKS ----
  const workflowExamples = [
    { anchor: 'Create a new workflow for code review', positive: 'claude-flow workflow create', negatives: ['agentic-flow pipeline create', 'claude-flow task create'] },
    { anchor: 'Define a multi-step workflow', positive: 'claude-flow workflow create', negatives: ['agentic-flow pipeline create'] },
    { anchor: 'Execute the deployment workflow', positive: 'claude-flow workflow execute', negatives: ['agentic-flow pipeline run'] },
    { anchor: 'Run the CI/CD workflow', positive: 'claude-flow workflow execute', negatives: ['claude-flow task create'] },
    { anchor: 'Check workflow execution status', positive: 'claude-flow workflow status', negatives: ['claude-flow task status'] },
    { anchor: 'Create a task for the coder agent', positive: 'claude-flow task create', negatives: ['claude-flow agent spawn'] },
    { anchor: 'Add a new task to the queue', positive: 'claude-flow task create', negatives: ['claude-flow workflow create'] },
    { anchor: 'Mark this task as complete', positive: 'claude-flow task complete', negatives: ['claude-flow hooks post-task'] },
    { anchor: 'List pending tasks', positive: 'claude-flow task list', negatives: ['claude-flow agent list'] },
  ];

  // ---- SESSION AND STATE ----
  const sessionExamples = [
    { anchor: 'Save the current session state', positive: 'claude-flow session save', negatives: ['claude-flow memory store', 'ruvector snapshot create'] },
    { anchor: 'Persist session for later continuation', positive: 'claude-flow session save', negatives: ['claude-flow memory store'] },
    { anchor: 'Restore previous session', positive: 'claude-flow session restore', negatives: ['ruvector snapshot restore', 'claude-flow memory retrieve'] },
    { anchor: 'Continue where I left off', positive: 'claude-flow session restore', negatives: ['claude-flow memory retrieve'] },
    { anchor: 'List saved sessions', positive: 'claude-flow session list', negatives: ['claude-flow memory list'] },
  ];

  // ---- NEURAL AND ML ----
  const neuralExamples = [
    { anchor: 'Train the routing model', positive: 'claude-flow neural train', negatives: ['claude-flow hooks pretrain'] },
    { anchor: 'Train neural patterns for better routing', positive: 'claude-flow neural train', negatives: ['claude-flow hooks pretrain'] },
    { anchor: 'Make prediction with neural model', positive: 'claude-flow neural predict', negatives: ['claude-flow hooks route'] },
    { anchor: 'Get neural routing prediction', positive: 'claude-flow neural predict', negatives: ['agentic-flow embed'] },
    { anchor: 'Store learned neural patterns', positive: 'claude-flow neural patterns', negatives: ['claude-flow memory store'] },
  ];

  // ---- PERFORMANCE AND SECURITY ----
  const perfExamples = [
    { anchor: 'Benchmark system performance', positive: 'claude-flow performance benchmark', negatives: ['claude-flow hooks metrics'] },
    { anchor: 'Profile slow operations', positive: 'claude-flow performance profile', negatives: ['claude-flow performance benchmark'] },
    { anchor: 'Optimize for lower latency', positive: 'claude-flow performance optimize', negatives: ['ruvector index optimize'] },
    { anchor: 'Scan input for security threats', positive: 'claude-flow aidefence scan', negatives: ['claude-flow hooks pre-task'] },
    { anchor: 'Check if input is safe', positive: 'claude-flow aidefence is_safe', negatives: ['claude-flow aidefence scan'] },
    { anchor: 'Analyze potential prompt injection', positive: 'claude-flow aidefence analyze', negatives: ['claude-flow hooks route'] },
  ];

  // ---- CLAIMS AND COORDINATION ----
  const claimsExamples = [
    { anchor: 'Claim this issue for work', positive: 'claude-flow claims claim', negatives: ['claude-flow task create'] },
    { anchor: 'Assign this issue to me', positive: 'claude-flow claims claim', negatives: ['claude-flow task create'] },
    { anchor: 'Release my claim on this issue', positive: 'claude-flow claims release', negatives: ['claude-flow task complete'] },
    { anchor: 'Hand off issue to another agent', positive: 'claude-flow claims handoff', negatives: ['claude-flow claims release'] },
    { anchor: 'View the claims board', positive: 'claude-flow claims board', negatives: ['claude-flow task list'] },
  ];

  // Convert all examples to triplet format
  const allExampleSets = [
    swarmExamples, agentExamples, memoryExamples, vectorExamples,
    embeddingExamples, hookExamples, workflowExamples, sessionExamples,
    neuralExamples, perfExamples, claimsExamples
  ];

  for (const exampleSet of allExampleSets) {
    for (const ex of exampleSet) {
      const negatives = ex.negatives || [ex.negative];
      for (const neg of negatives) {
        examples.push({
          anchor: ex.anchor,
          positive: ex.positive,
          negative: neg,
          isHard: true,
        });
      }
    }
  }

  return examples;
}

// ============================================================================
// MAIN GENERATION
// ============================================================================

/**
 * Save capability definitions to JSON files
 */
function saveCapabilities(outputDir) {
  const capabilities = [
    { filename: 'claude-flow-capabilities.json', data: CLAUDE_FLOW_CAPABILITIES },
    { filename: 'agentic-flow-capabilities.json', data: AGENTIC_FLOW_CAPABILITIES },
    { filename: 'ruvector-capabilities.json', data: RUVECTOR_CAPABILITIES },
  ];

  for (const { filename, data } of capabilities) {
    const filepath = path.join(outputDir, filename);
    fs.writeFileSync(filepath, JSON.stringify(data, null, 2));
    console.log(`  Saved ${filepath}`);
  }
}

/**
 * Main entry point
 */
function main() {
  console.log('\n' + '='.repeat(80));
  console.log('          ECOSYSTEM TRAINING DATA GENERATOR FOR RUVLTRA');
  console.log('='.repeat(80) + '\n');

  const args = process.argv.slice(2);
  const outputDir = path.dirname(path.resolve(__filename));
  const outputFile = args.find((a) => a.startsWith('--output='))?.split('=')[1] ||
    path.join(outputDir, 'ecosystem-triplets.jsonl');
  const saveCapabilityFiles = args.includes('--save-capabilities');

  console.log('Configuration:');
  console.log(`  Output: ${outputFile}`);
  console.log(`  Save capability JSONs: ${saveCapabilityFiles}`);
  console.log();

  // All capabilities
  const allCapabilities = [
    CLAUDE_FLOW_CAPABILITIES,
    AGENTIC_FLOW_CAPABILITIES,
    RUVECTOR_CAPABILITIES,
  ];

  // Save capability definitions if requested
  if (saveCapabilityFiles) {
    console.log('Saving capability definitions...');
    saveCapabilities(outputDir);
    console.log();
  }

  // Generate triplets
  console.log('Generating training triplets...');

  const triplets = generateTriplets(allCapabilities);
  const categoryExamples = generateCategoryExamples();

  // Combine all triplets
  const allTriplets = [...triplets, ...categoryExamples];

  // Shuffle for better training
  allTriplets.sort(() => Math.random() - 0.5);

  // Statistics
  const stats = {
    total: allTriplets.length,
    byTool: {},
    byCategory: {},
    hardNegatives: 0,
  };

  for (const t of allTriplets) {
    if (t.tool) {
      stats.byTool[t.tool] = (stats.byTool[t.tool] || 0) + 1;
    }
    if (t.category) {
      stats.byCategory[t.category] = (stats.byCategory[t.category] || 0) + 1;
    }
    if (t.isHard) {
      stats.hardNegatives++;
    }
  }

  // Save triplets as JSONL
  const jsonlContent = allTriplets.map((t) => JSON.stringify({
    anchor: t.anchor,
    positive: t.positive,
    negative: t.negative,
  })).join('\n');

  fs.writeFileSync(outputFile, jsonlContent);

  // Print summary
  console.log('\n' + '-'.repeat(60));
  console.log('                    GENERATION SUMMARY');
  console.log('-'.repeat(60) + '\n');

  console.log(`Total triplets generated: ${stats.total}`);
  console.log(`Hard negatives: ${stats.hardNegatives} (${((stats.hardNegatives / stats.total) * 100).toFixed(1)}%)`);
  console.log();

  console.log('Triplets by tool:');
  for (const [tool, count] of Object.entries(stats.byTool)) {
    console.log(`  ${tool.padEnd(20)} ${count}`);
  }
  console.log();

  console.log('Triplets by category:');
  for (const [category, count] of Object.entries(stats.byCategory).slice(0, 10)) {
    console.log(`  ${category.padEnd(20)} ${count}`);
  }
  if (Object.keys(stats.byCategory).length > 10) {
    console.log(`  ... and ${Object.keys(stats.byCategory).length - 10} more categories`);
  }
  console.log();

  console.log(`Output saved to: ${outputFile}`);
  console.log();

  // Show sample triplets
  console.log('-'.repeat(60));
  console.log('                    SAMPLE TRIPLETS');
  console.log('-'.repeat(60) + '\n');

  for (const triplet of allTriplets.slice(0, 5)) {
    console.log(`  Anchor:   "${triplet.anchor}"`);
    console.log(`  Positive: ${triplet.positive}`);
    console.log(`  Negative: ${triplet.negative}`);
    console.log();
  }

  console.log('='.repeat(80));
  console.log('                         NEXT STEPS');
  console.log('='.repeat(80) + '\n');

  console.log('1. Merge with existing training data:');
  console.log(`   cat ~/.ruvllm/training/ruvltra-finetuned/triplets.jsonl ${outputFile} > combined.jsonl`);
  console.log();
  console.log('2. Train with contrastive loss:');
  console.log('   cargo run --example train_contrastive --release -- --triplets combined.jsonl --epochs 30');
  console.log();
  console.log('3. Evaluate routing accuracy improvement');
  console.log();
}

// Export for testing
module.exports = {
  CLAUDE_FLOW_CAPABILITIES,
  AGENTIC_FLOW_CAPABILITIES,
  RUVECTOR_CAPABILITIES,
  generatePrompts,
  findHardNegatives,
  generateTriplets,
  generateCategoryExamples,
  CONFUSING_PAIRS,
};

// Run if called directly
if (require.main === module) {
  main();
}
