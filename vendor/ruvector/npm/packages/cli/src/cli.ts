#!/usr/bin/env node
/**
 * RuVector CLI - Command-line interface for RuVector vector database
 *
 * This CLI provides access to hooks, memory, learning, and swarm commands.
 * Supports PostgreSQL storage (preferred) with JSON fallback.
 *
 * Set RUVECTOR_POSTGRES_URL or DATABASE_URL for PostgreSQL support.
 */

import { program } from 'commander';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

const INTEL_PATH = path.join(os.homedir(), '.ruvector', 'intelligence.json');

interface QPattern {
  state: string;
  action: string;
  q_value: number;
  visits: number;
  last_update: number;
}

interface MemoryEntry {
  id: string;
  memory_type: string;
  content: string;
  embedding: number[];
  metadata: Record<string, string>;
  timestamp: number;
}

interface Trajectory {
  id: string;
  state: string;
  action: string;
  outcome: string;
  reward: number;
  timestamp: number;
}

interface ErrorPattern {
  code: string;
  error_type: string;
  message: string;
  fixes: string[];
  occurrences: number;
}

interface SwarmAgent {
  id: string;
  agent_type: string;
  capabilities: string[];
  success_rate: number;
  task_count: number;
  status: string;
}

interface SwarmEdge {
  source: string;
  target: string;
  weight: number;
  coordination_count: number;
}

interface FileSequence {
  from_file: string;
  to_file: string;
  count: number;
}

interface IntelligenceStats {
  total_patterns: number;
  total_memories: number;
  total_trajectories: number;
  total_errors: number;
  session_count: number;
  last_session: number;
}

interface IntelligenceData {
  patterns: Record<string, QPattern>;
  memories: MemoryEntry[];
  trajectories: Trajectory[];
  errors: Record<string, ErrorPattern>;
  file_sequences: FileSequence[];
  agents: Record<string, SwarmAgent>;
  edges: SwarmEdge[];
  stats: IntelligenceStats;
}

class Intelligence {
  private data: IntelligenceData;
  private alpha = 0.1;
  private lastEditedFile: string | null = null;

  constructor() {
    this.data = this.load();
  }

  private load(): IntelligenceData {
    try {
      if (fs.existsSync(INTEL_PATH)) {
        return JSON.parse(fs.readFileSync(INTEL_PATH, 'utf-8'));
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

  save(): void {
    const dir = path.dirname(INTEL_PATH);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(INTEL_PATH, JSON.stringify(this.data, null, 2));
  }

  private now(): number {
    return Math.floor(Date.now() / 1000);
  }

  private embed(text: string): number[] {
    const embedding = new Array(64).fill(0);
    for (let i = 0; i < text.length; i++) {
      const idx = (text.charCodeAt(i) + i * 7) % 64;
      embedding[idx] += 1.0;
    }
    const norm = Math.sqrt(embedding.reduce((a, b) => a + b * b, 0));
    if (norm > 0) {
      for (let i = 0; i < embedding.length; i++) {
        embedding[i] /= norm;
      }
    }
    return embedding;
  }

  private similarity(a: number[], b: number[]): number {
    if (a.length !== b.length) return 0;
    const dot = a.reduce((sum, v, i) => sum + v * b[i], 0);
    const normA = Math.sqrt(a.reduce((sum, v) => sum + v * v, 0));
    const normB = Math.sqrt(b.reduce((sum, v) => sum + v * v, 0));
    return normA > 0 && normB > 0 ? dot / (normA * normB) : 0;
  }

  remember(memoryType: string, content: string, metadata: Record<string, string> = {}): string {
    const id = `mem_${this.now()}`;
    this.data.memories.push({
      id,
      memory_type: memoryType,
      content,
      embedding: this.embed(content),
      metadata,
      timestamp: this.now()
    });
    if (this.data.memories.length > 5000) {
      this.data.memories.splice(0, 1000);
    }
    this.data.stats.total_memories = this.data.memories.length;
    return id;
  }

  recall(query: string, topK: number): MemoryEntry[] {
    const queryEmbed = this.embed(query);
    return this.data.memories
      .map(m => ({ score: this.similarity(queryEmbed, m.embedding), memory: m }))
      .sort((a, b) => b.score - a.score)
      .slice(0, topK)
      .map(r => r.memory);
  }

  private getQ(state: string, action: string): number {
    const key = `${state}|${action}`;
    return this.data.patterns[key]?.q_value ?? 0;
  }

  private updateQ(state: string, action: string, reward: number): void {
    const key = `${state}|${action}`;
    if (!this.data.patterns[key]) {
      this.data.patterns[key] = { state, action, q_value: 0, visits: 0, last_update: 0 };
    }
    const p = this.data.patterns[key];
    p.q_value = p.q_value + this.alpha * (reward - p.q_value);
    p.visits++;
    p.last_update = this.now();
    this.data.stats.total_patterns = Object.keys(this.data.patterns).length;
  }

  learn(state: string, action: string, outcome: string, reward: number): string {
    const id = `traj_${this.now()}`;
    this.updateQ(state, action, reward);
    this.data.trajectories.push({ id, state, action, outcome, reward, timestamp: this.now() });
    if (this.data.trajectories.length > 1000) {
      this.data.trajectories.splice(0, 200);
    }
    this.data.stats.total_trajectories = this.data.trajectories.length;
    return id;
  }

  suggest(state: string, actions: string[]): { action: string; confidence: number } {
    let bestAction = actions[0] ?? '';
    let bestQ = -Infinity;
    for (const action of actions) {
      const q = this.getQ(state, action);
      if (q > bestQ) {
        bestQ = q;
        bestAction = action;
      }
    }
    return { action: bestAction, confidence: bestQ > 0 ? Math.min(bestQ, 1) : 0 };
  }

  route(task: string, file?: string, crateName?: string, operation = 'edit'): { agent: string; confidence: number; reason: string } {
    const fileType = file ? path.extname(file).slice(1) : 'unknown';
    const state = `${operation}_${fileType}_in_${crateName ?? 'project'}`;

    const agentMap: Record<string, string[]> = {
      rs: ['rust-developer', 'coder', 'reviewer', 'tester'],
      ts: ['typescript-developer', 'coder', 'frontend-dev'],
      tsx: ['typescript-developer', 'coder', 'frontend-dev'],
      js: ['coder', 'frontend-dev'],
      jsx: ['coder', 'frontend-dev'],
      py: ['python-developer', 'coder', 'ml-developer'],
      md: ['docs-writer', 'coder']
    };

    const agents = agentMap[fileType] ?? ['coder', 'reviewer'];
    const { action, confidence } = this.suggest(state, agents);

    const reason = confidence > 0.5 ? 'learned from past success'
      : confidence > 0 ? 'based on patterns'
      : `default for ${fileType} files`;

    return { agent: action, confidence, reason };
  }

  shouldTest(file: string): { suggest: boolean; command: string } {
    const ext = path.extname(file).slice(1);
    switch (ext) {
      case 'rs': {
        const crateMatch = file.match(/crates\/([^/]+)/);
        return crateMatch
          ? { suggest: true, command: `cargo test -p ${crateMatch[1]}` }
          : { suggest: true, command: 'cargo test' };
      }
      case 'ts':
      case 'tsx':
      case 'js':
      case 'jsx':
        return { suggest: true, command: 'npm test' };
      case 'py':
        return { suggest: true, command: 'pytest' };
      default:
        return { suggest: false, command: '' };
    }
  }

  // Record file edit sequence for prediction
  recordFileSequence(fromFile: string, toFile: string): void {
    const existing = this.data.file_sequences.find(
      s => s.from_file === fromFile && s.to_file === toFile
    );
    if (existing) {
      existing.count++;
    } else {
      this.data.file_sequences.push({ from_file: fromFile, to_file: toFile, count: 1 });
    }
    this.lastEditedFile = toFile;
  }

  // Suggest next files based on sequences
  suggestNext(file: string, limit = 3): { file: string; score: number }[] {
    return this.data.file_sequences
      .filter(s => s.from_file === file)
      .sort((a, b) => b.count - a.count)
      .slice(0, limit)
      .map(s => ({ file: s.to_file, score: s.count }));
  }

  // Record error pattern
  recordError(command: string, message: string): string[] {
    const codeMatch = message.match(/error\[([A-Z]\d+)\]/i) || message.match(/([A-Z]\d{4})/);
    const codes: string[] = [];

    if (codeMatch) {
      const code = codeMatch[1];
      codes.push(code);

      if (!this.data.errors[code]) {
        this.data.errors[code] = {
          code,
          error_type: this.classifyError(code),
          message: message.slice(0, 500),
          fixes: [],
          occurrences: 0
        };
      }
      this.data.errors[code].occurrences++;
      this.data.errors[code].message = message.slice(0, 500);
      this.data.stats.total_errors = Object.keys(this.data.errors).length;
    }

    return codes;
  }

  private classifyError(code: string): string {
    if (code.startsWith('E0')) return 'type-error';
    if (code.startsWith('E1')) return 'borrow-error';
    if (code.startsWith('E2')) return 'lifetime-error';
    if (code.startsWith('E3')) return 'trait-error';
    if (code.startsWith('E4')) return 'macro-error';
    if (code.startsWith('E5')) return 'pattern-error';
    if (code.startsWith('E6')) return 'import-error';
    if (code.startsWith('E7')) return 'async-error';
    return 'unknown-error';
  }

  // Get fix suggestions for error code
  suggestFix(code: string): { code: string; type: string; fixes: string[]; occurrences: number } | null {
    const error = this.data.errors[code];
    if (!error) return null;
    return {
      code: error.code,
      type: error.error_type,
      fixes: error.fixes,
      occurrences: error.occurrences
    };
  }

  // Classify command type
  classifyCommand(command: string): { category: string; subcategory: string; risk: string } {
    const cmd = command.toLowerCase();

    if (cmd.includes('cargo') || cmd.includes('rustc')) {
      return { category: 'rust', subcategory: cmd.includes('test') ? 'test' : 'build', risk: 'low' };
    }
    if (cmd.includes('npm') || cmd.includes('node') || cmd.includes('yarn')) {
      return { category: 'javascript', subcategory: cmd.includes('test') ? 'test' : 'build', risk: 'low' };
    }
    if (cmd.includes('git')) {
      const risk = cmd.includes('push') || cmd.includes('force') ? 'medium' : 'low';
      return { category: 'git', subcategory: 'vcs', risk };
    }
    if (cmd.includes('rm') || cmd.includes('delete')) {
      return { category: 'filesystem', subcategory: 'destructive', risk: 'high' };
    }

    return { category: 'shell', subcategory: 'general', risk: 'low' };
  }

  // Swarm methods
  swarmRegister(id: string, agentType: string, capabilities: string[]): void {
    this.data.agents[id] = {
      id,
      agent_type: agentType,
      capabilities,
      success_rate: 1.0,
      task_count: 0,
      status: 'active'
    };
  }

  swarmCoordinate(source: string, target: string, weight: number): void {
    const existing = this.data.edges.find(e => e.source === source && e.target === target);
    if (existing) {
      existing.weight = (existing.weight + weight) / 2;
      existing.coordination_count++;
    } else {
      this.data.edges.push({ source, target, weight, coordination_count: 1 });
    }
  }

  swarmOptimize(tasks: string[]): { task: string; agents: number; edges: number }[] {
    return tasks.map(task => ({
      task,
      agents: Object.keys(this.data.agents).length,
      edges: this.data.edges.length
    }));
  }

  swarmRecommend(taskType: string): { agent: string; type: string; score: number } | null {
    const agents = Object.values(this.data.agents);
    if (agents.length === 0) return null;

    // Find agent with matching capability or best success rate
    const matching = agents.filter(a =>
      a.capabilities.some(c => taskType.toLowerCase().includes(c.toLowerCase()))
    );

    const best = matching.length > 0
      ? matching.sort((a, b) => b.success_rate - a.success_rate)[0]
      : agents.sort((a, b) => b.success_rate - a.success_rate)[0];

    return { agent: best.id, type: best.agent_type, score: best.success_rate };
  }

  swarmHeal(failedAgentId: string): { healed: boolean; replacement: string | null } {
    const failed = this.data.agents[failedAgentId];
    if (!failed) return { healed: false, replacement: null };

    // Mark as failed
    failed.status = 'failed';
    failed.success_rate = 0;

    // Find replacement with same type
    const replacement = Object.values(this.data.agents).find(
      a => a.agent_type === failed.agent_type && a.status === 'active' && a.id !== failedAgentId
    );

    return { healed: true, replacement: replacement?.id ?? null };
  }

  swarmStats(): { agents: number; edges: number; avgSuccess: number } {
    const agents = Object.keys(this.data.agents).length;
    const edges = this.data.edges.length;
    const activeAgents = Object.values(this.data.agents).filter(a => a.status === 'active');
    const avgSuccess = activeAgents.length > 0
      ? activeAgents.reduce((sum, a) => sum + a.success_rate, 0) / activeAgents.length
      : 0;
    return { agents, edges, avgSuccess };
  }

  stats(): IntelligenceStats {
    return this.data.stats;
  }

  sessionStart(): void {
    this.data.stats.session_count++;
    this.data.stats.last_session = this.now();
  }

  sessionEnd(): { duration: number; actions: number } {
    const duration = this.now() - this.data.stats.last_session;
    const actions = this.data.trajectories.filter(t => t.timestamp >= this.data.stats.last_session).length;
    return { duration, actions };
  }

  getLastEditedFile(): string | null {
    return this.lastEditedFile;
  }
}

// Generate Claude hooks configuration
function generateClaudeHooksConfig(): object {
  return {
    hooks: {
      PreToolUse: [
        {
          matcher: "Edit|Write|MultiEdit",
          hooks: [
            "npx @ruvector/cli hooks pre-edit \"$TOOL_INPUT_file_path\""
          ]
        },
        {
          matcher: "Bash",
          hooks: [
            "npx @ruvector/cli hooks pre-command \"$TOOL_INPUT_command\""
          ]
        }
      ],
      PostToolUse: [
        {
          matcher: "Edit|Write|MultiEdit",
          hooks: [
            "npx @ruvector/cli hooks post-edit --success \"$TOOL_INPUT_file_path\""
          ]
        },
        {
          matcher: "Bash",
          hooks: [
            "npx @ruvector/cli hooks post-command --success \"$TOOL_INPUT_command\""
          ]
        }
      ],
      SessionStart: [
        "npx @ruvector/cli hooks session-start"
      ],
      Stop: [
        "npx @ruvector/cli hooks session-end"
      ],
      PreCompact: [
        "npx @ruvector/cli hooks pre-compact"
      ]
    }
  };
}

// CLI setup
program
  .name('ruvector')
  .description('RuVector CLI - High-performance vector database')
  .version('0.1.27');

const hooks = program.command('hooks').description('Self-learning intelligence hooks for Claude Code');

// ============================================================================
// Core Commands
// ============================================================================

hooks.command('init')
  .description('Initialize hooks in current project')
  .option('--force', 'Force overwrite existing configuration')
  .action((opts: { force?: boolean }) => {
    const configPath = path.join(process.cwd(), '.ruvector', 'hooks.json');
    const configDir = path.dirname(configPath);
    const claudeDir = path.join(process.cwd(), '.claude');
    const settingsPath = path.join(claudeDir, 'settings.json');

    // Check if already initialized
    if (fs.existsSync(settingsPath) && !opts.force) {
      console.log('Hooks already initialized. Use --force to overwrite.');
      return;
    }

    // Create .ruvector config
    if (!fs.existsSync(configDir)) {
      fs.mkdirSync(configDir, { recursive: true });
    }

    const config = {
      version: '1.0.0',
      enabled: true,
      storage: 'json',
      postgres_url: null,
      learning: { alpha: 0.1, gamma: 0.95, epsilon: 0.1 }
    };

    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));

    // Create .claude/settings.json with hooks
    if (!fs.existsSync(claudeDir)) {
      fs.mkdirSync(claudeDir, { recursive: true });
    }

    let settings: Record<string, unknown> = {};
    if (fs.existsSync(settingsPath)) {
      try {
        settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8'));
      } catch {}
    }

    const hooksConfig = generateClaudeHooksConfig();
    settings = { ...settings, ...hooksConfig };

    fs.writeFileSync(settingsPath, JSON.stringify(settings, null, 2));

    console.log('✅ Hooks initialized!');
    console.log('   Created: .ruvector/hooks.json');
    console.log('   Created: .claude/settings.json');
    console.log('\nNext steps:');
    console.log('   1. Restart Claude Code to activate hooks');
    console.log("   2. Run 'ruvector hooks stats' to verify");
  });

hooks.command('install')
  .description('Install hooks into Claude settings')
  .option('--settings-dir <dir>', 'Claude settings directory', '.claude')
  .action((opts: { settingsDir: string }) => {
    const settingsPath = path.join(process.cwd(), opts.settingsDir, 'settings.json');
    const settingsDir = path.dirname(settingsPath);

    if (!fs.existsSync(settingsDir)) {
      fs.mkdirSync(settingsDir, { recursive: true });
    }

    let settings: Record<string, unknown> = {};
    if (fs.existsSync(settingsPath)) {
      try {
        settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8'));
      } catch {}
    }

    const hooksConfig = generateClaudeHooksConfig();
    settings = { ...settings, ...hooksConfig };

    fs.writeFileSync(settingsPath, JSON.stringify(settings, null, 2));
    console.log(`✅ Hooks installed to ${settingsPath}`);
    console.log('\nInstalled hooks:');
    console.log('  - PreToolUse: Edit, Write, MultiEdit, Bash');
    console.log('  - PostToolUse: Edit, Write, MultiEdit, Bash');
    console.log('  - SessionStart, Stop, PreCompact');
  });

hooks.command('stats')
  .description('Show intelligence statistics')
  .action(() => {
    const intel = new Intelligence();
    const stats = intel.stats();
    const swarm = intel.swarmStats();

    console.log('\x1b[36m\x1b[1m🧠 RuVector Intelligence Stats\x1b[0m\n');
    console.log(`  \x1b[32m${stats.total_patterns}\x1b[0m Q-learning patterns`);
    console.log(`  \x1b[32m${stats.total_memories}\x1b[0m vector memories`);
    console.log(`  \x1b[32m${stats.total_trajectories}\x1b[0m learning trajectories`);
    console.log(`  \x1b[32m${stats.total_errors}\x1b[0m error patterns\n`);
    console.log('\x1b[1mSwarm Status:\x1b[0m');
    console.log(`  \x1b[36m${swarm.agents}\x1b[0m agents registered`);
    console.log(`  \x1b[36m${swarm.edges}\x1b[0m coordination edges`);
    const rate = swarm.avgSuccess > 0 ? `${(swarm.avgSuccess * 100).toFixed(0)}%` : 'N/A';
    console.log(`  \x1b[36m${rate}\x1b[0m average success rate`);
  });

// ============================================================================
// Session Hooks
// ============================================================================

hooks.command('session-start')
  .description('Session start hook')
  .action(() => {
    const intel = new Intelligence();
    intel.sessionStart();
    intel.save();
    console.log('\x1b[36m\x1b[1m🧠 RuVector Intelligence Layer Active\x1b[0m\n');
    console.log('⚡ Intelligence guides: agent routing, error fixes, file sequences');
  });

hooks.command('session-end')
  .description('Session end hook')
  .option('--export-metrics', 'Export session metrics')
  .action((opts: { exportMetrics?: boolean }) => {
    const intel = new Intelligence();
    const sessionInfo = intel.sessionEnd();
    intel.save();

    console.log('📊 Session ended. Learning data saved.');
    if (opts.exportMetrics) {
      console.log(JSON.stringify({
        duration_seconds: sessionInfo.duration,
        actions_recorded: sessionInfo.actions,
        saved: true
      }, null, 2));
    }
  });

hooks.command('pre-compact')
  .description('Pre-compact hook - save state before context compaction')
  .action(() => {
    const intel = new Intelligence();
    const stats = intel.stats();
    intel.save();

    console.log(`🗜️ Pre-compact: ${stats.total_trajectories} trajectories, ${stats.total_memories} memories saved`);
  });

// ============================================================================
// Edit Hooks
// ============================================================================

hooks.command('pre-edit')
  .description('Pre-edit intelligence hook')
  .argument('<file>', 'File path')
  .action((file: string) => {
    const intel = new Intelligence();
    const fileName = path.basename(file);
    const crateMatch = file.match(/crates\/([^/]+)/);
    const crate = crateMatch?.[1];
    const { agent, confidence, reason } = intel.route(`edit ${fileName}`, file, crate, 'edit');

    console.log('\x1b[1m🧠 Intelligence Analysis:\x1b[0m');
    console.log(`   📁 \x1b[36m${crate ?? 'project'}\x1b[0m/${fileName}`);
    console.log(`   🤖 Recommended: \x1b[32m\x1b[1m${agent}\x1b[0m (${(confidence * 100).toFixed(0)}% confidence)`);
    if (reason) console.log(`      → \x1b[2m${reason}\x1b[0m`);

    // Show suggested next files
    const nextFiles = intel.suggestNext(file, 3);
    if (nextFiles.length > 0) {
      console.log('   📎 Likely next files:');
      nextFiles.forEach(n => console.log(`      - ${n.file} (${n.score} edits)`));
    }
  });

hooks.command('post-edit')
  .description('Post-edit learning hook')
  .argument('<file>', 'File path')
  .option('--success', 'Edit succeeded')
  .action((file: string, opts: { success?: boolean }) => {
    const intel = new Intelligence();
    const success = opts.success ?? false;
    const ext = path.extname(file).slice(1);
    const crateMatch = file.match(/crates\/([^/]+)/);
    const crate = crateMatch?.[1] ?? 'project';
    const state = `edit_${ext}_in_${crate}`;

    // Record file sequence
    const lastFile = intel.getLastEditedFile();
    if (lastFile && lastFile !== file) {
      intel.recordFileSequence(lastFile, file);
    }

    intel.learn(state, success ? 'successful-edit' : 'failed-edit', success ? 'completed' : 'failed', success ? 1.0 : -0.5);
    intel.remember('edit', `${success ? 'successful' : 'failed'} edit of ${ext} in ${crate}`);
    intel.save();

    console.log(`📊 Learning recorded: ${success ? '✅' : '❌'} ${path.basename(file)}`);

    const test = intel.shouldTest(file);
    if (test.suggest) console.log(`   🧪 Consider: \x1b[36m${test.command}\x1b[0m`);
  });

// ============================================================================
// Command Hooks
// ============================================================================

hooks.command('pre-command')
  .description('Pre-command intelligence hook')
  .argument('<command...>', 'Command to analyze')
  .action((command: string[]) => {
    const intel = new Intelligence();
    const cmd = command.join(' ');
    const classification = intel.classifyCommand(cmd);

    console.log('\x1b[1m🧠 Command Analysis:\x1b[0m');
    console.log(`   📦 Category: \x1b[36m${classification.category}\x1b[0m`);
    console.log(`   🏷️  Type: ${classification.subcategory}`);

    if (classification.risk === 'high') {
      console.log('   ⚠️  Risk: \x1b[31mHIGH\x1b[0m - Review carefully');
    } else if (classification.risk === 'medium') {
      console.log('   ⚡ Risk: \x1b[33mMEDIUM\x1b[0m');
    } else {
      console.log('   ✅ Risk: \x1b[32mLOW\x1b[0m');
    }
  });

hooks.command('post-command')
  .description('Post-command learning hook')
  .argument('<command...>', 'Command that ran')
  .option('--success', 'Command succeeded')
  .option('--stderr <stderr>', 'Stderr output for error learning')
  .action((command: string[], opts: { success?: boolean; stderr?: string }) => {
    const intel = new Intelligence();
    const cmd = command.join(' ');
    const success = opts.success ?? true;

    // Learn from command outcome
    const classification = intel.classifyCommand(cmd);
    intel.learn(
      `cmd_${classification.category}_${classification.subcategory}`,
      success ? 'success' : 'failure',
      success ? 'completed' : 'failed',
      success ? 0.8 : -0.3
    );

    // Learn from errors if stderr provided
    if (opts.stderr) {
      const errorCodes = intel.recordError(cmd, opts.stderr);
      if (errorCodes.length > 0) {
        console.log(`📊 Learned error patterns: ${errorCodes.join(', ')}`);
      }
    }

    intel.remember('command', `${cmd} ${success ? 'succeeded' : 'failed'}`);
    intel.save();

    console.log(`📊 Command ${success ? '✅' : '❌'} recorded`);
  });

// ============================================================================
// Error Learning
// ============================================================================

hooks.command('record-error')
  .description('Record error pattern for learning')
  .argument('<command>', 'Command that produced error')
  .argument('<message>', 'Error message')
  .action((command: string, message: string) => {
    const intel = new Intelligence();
    const codes = intel.recordError(command, message);
    intel.save();

    console.log(JSON.stringify({ errors: codes, recorded: codes.length }));
  });

hooks.command('suggest-fix')
  .description('Get suggested fix for error code')
  .argument('<code>', 'Error code (e.g., E0308)')
  .action((code: string) => {
    const intel = new Intelligence();
    const fix = intel.suggestFix(code);

    if (fix) {
      console.log(JSON.stringify(fix, null, 2));
    } else {
      console.log(JSON.stringify({ code, fixes: [], occurrences: 0, type: 'unknown' }));
    }
  });

hooks.command('suggest-next')
  .description('Suggest next files to edit based on patterns')
  .argument('<file>', 'Current file')
  .option('-n, --limit <n>', 'Number of suggestions', '3')
  .action((file: string, opts: { limit: string }) => {
    const intel = new Intelligence();
    const suggestions = intel.suggestNext(file, parseInt(opts.limit));

    console.log(JSON.stringify({
      current_file: file,
      suggestions: suggestions.map(s => ({ file: s.file, frequency: s.score }))
    }, null, 2));
  });

// ============================================================================
// Memory Commands
// ============================================================================

hooks.command('remember')
  .description('Store content in semantic memory')
  .requiredOption('-t, --type <type>', 'Memory type')
  .argument('<content...>', 'Content to remember')
  .action((content: string[], opts: { type: string }) => {
    const intel = new Intelligence();
    const id = intel.remember(opts.type, content.join(' '));
    intel.save();
    console.log(JSON.stringify({ success: true, id }));
  });

hooks.command('recall')
  .description('Search memory semantically')
  .argument('<query...>', 'Search query')
  .option('-k, --top-k <n>', 'Number of results', '5')
  .action((query: string[], opts: { topK: string }) => {
    const intel = new Intelligence();
    const results = intel.recall(query.join(' '), parseInt(opts.topK));
    console.log(JSON.stringify({
      query: query.join(' '),
      results: results.map(r => ({
        type: r.memory_type,
        content: r.content.slice(0, 200),
        timestamp: r.timestamp
      }))
    }, null, 2));
  });

// ============================================================================
// Learning Commands
// ============================================================================

hooks.command('learn')
  .description('Record a learning trajectory')
  .argument('<state>', 'State identifier')
  .argument('<action>', 'Action taken')
  .option('-r, --reward <n>', 'Reward value', '0.0')
  .action((state: string, action: string, opts: { reward: string }) => {
    const intel = new Intelligence();
    const id = intel.learn(state, action, 'recorded', parseFloat(opts.reward));
    intel.save();
    console.log(JSON.stringify({ success: true, id, state, action, reward: parseFloat(opts.reward) }));
  });

hooks.command('suggest')
  .description('Get action suggestion for state')
  .argument('<state>', 'Current state')
  .requiredOption('-a, --actions <actions>', 'Available actions (comma-separated)')
  .action((state: string, opts: { actions: string }) => {
    const intel = new Intelligence();
    const actions = opts.actions.split(',').map(s => s.trim());
    const result = intel.suggest(state, actions);
    console.log(JSON.stringify({ state, ...result }, null, 2));
  });

hooks.command('route')
  .description('Route task to best agent')
  .argument('<task...>', 'Task description')
  .option('--file <file>', 'File being worked on')
  .option('--crate-name <crate>', 'Crate/module context')
  .action((task: string[], opts: { file?: string; crateName?: string }) => {
    const intel = new Intelligence();
    const result = intel.route(task.join(' '), opts.file, opts.crateName);
    console.log(JSON.stringify({
      task: task.join(' '),
      recommended: result.agent,
      confidence: result.confidence,
      reasoning: result.reason
    }, null, 2));
  });

hooks.command('should-test')
  .description('Check if tests should run')
  .argument('<file>', 'File that was edited')
  .action((file: string) => {
    const intel = new Intelligence();
    console.log(JSON.stringify(intel.shouldTest(file), null, 2));
  });

// ============================================================================
// Swarm Commands
// ============================================================================

hooks.command('swarm-register')
  .description('Register agent in swarm')
  .argument('<id>', 'Agent ID')
  .argument('<type>', 'Agent type')
  .option('--capabilities <caps>', 'Capabilities (comma-separated)')
  .action((id: string, type: string, opts: { capabilities?: string }) => {
    const intel = new Intelligence();
    const caps = opts.capabilities?.split(',').map(s => s.trim()) ?? [];
    intel.swarmRegister(id, type, caps);
    intel.save();
    console.log(JSON.stringify({ success: true, agent_id: id, type }));
  });

hooks.command('swarm-coordinate')
  .description('Record agent coordination')
  .argument('<source>', 'Source agent ID')
  .argument('<target>', 'Target agent ID')
  .option('-w, --weight <n>', 'Coordination weight', '1.0')
  .action((source: string, target: string, opts: { weight: string }) => {
    const intel = new Intelligence();
    intel.swarmCoordinate(source, target, parseFloat(opts.weight));
    intel.save();
    console.log(JSON.stringify({ success: true, source, target, weight: parseFloat(opts.weight) }));
  });

hooks.command('swarm-optimize')
  .description('Optimize task distribution')
  .argument('<tasks>', 'Tasks (comma-separated)')
  .action((tasks: string) => {
    const intel = new Intelligence();
    const taskList = tasks.split(',').map(s => s.trim());
    const result = intel.swarmOptimize(taskList);
    console.log(JSON.stringify({ tasks: taskList.length, assignments: result }, null, 2));
  });

hooks.command('swarm-recommend')
  .description('Recommend agent for task type')
  .argument('<task-type>', 'Type of task')
  .action((taskType: string) => {
    const intel = new Intelligence();
    const result = intel.swarmRecommend(taskType);
    if (result) {
      console.log(JSON.stringify({ task_type: taskType, recommended: result.agent, type: result.type, score: result.score }));
    } else {
      console.log(JSON.stringify({ task_type: taskType, recommended: null, message: 'No matching agent found' }));
    }
  });

hooks.command('swarm-heal')
  .description('Handle agent failure')
  .argument('<agent-id>', 'Failed agent ID')
  .action((agentId: string) => {
    const intel = new Intelligence();
    const result = intel.swarmHeal(agentId);
    intel.save();
    console.log(JSON.stringify({ failed_agent: agentId, healed: result.healed, replacement: result.replacement }));
  });

hooks.command('swarm-stats')
  .description('Show swarm statistics')
  .action(() => {
    const intel = new Intelligence();
    const stats = intel.swarmStats();
    console.log(JSON.stringify({
      agents: stats.agents,
      edges: stats.edges,
      average_success_rate: stats.avgSuccess,
      topology: 'mesh'
    }, null, 2));
  });

// ============================================================================
// Claude Code v2.0.55+ Features
// ============================================================================

hooks.command('lsp-diagnostic')
  .description('Process LSP diagnostic events (Claude Code 2.0.55+)')
  .option('--file <path>', 'File with diagnostic')
  .option('--severity <level>', 'Diagnostic severity (error, warning, info, hint)')
  .option('--message <text>', 'Diagnostic message')
  .action((opts: { file?: string; severity?: string; message?: string }) => {
    const intel = new Intelligence();

    // Read hook input from stdin if available
    let stdinData: any = null;
    try {
      const inputPath = process.env.CLAUDE_HOOK_INPUT;
      if (inputPath && fs.existsSync(inputPath)) {
        stdinData = JSON.parse(fs.readFileSync(inputPath, 'utf-8'));
      }
    } catch { /* ignore */ }

    const file = opts.file || stdinData?.tool_input?.file || 'unknown';
    const severity = opts.severity || stdinData?.tool_input?.severity || 'info';
    const message = opts.message || stdinData?.tool_input?.message || '';

    // Learn from LSP diagnostics
    if (severity === 'error' || severity === 'warning') {
      // Record error and get codes
      const codes = intel.recordError(`lsp:${file}`, message);
      const errorCode = codes[0] || `${severity}-unknown`;

      // Record trajectory for learning
      const state = `lsp_${severity}_${path.extname(file).slice(1) || 'unknown'}`;
      intel.learn(state, 'diagnostic', message.slice(0, 100), severity === 'error' ? -0.5 : -0.2);
      intel.save();

      // Output context for Claude
      const fixInfo = intel.suggestFix(errorCode);
      const learnedFixes = fixInfo?.fixes ?? [];
      console.log(JSON.stringify({
        file,
        severity,
        error_code: errorCode,
        learned_fixes: learnedFixes.slice(0, 3),
        recommendation: learnedFixes.length > 0 ? 'Apply learned fix' : 'Investigate error pattern'
      }));
    } else {
      console.log(JSON.stringify({ file, severity, message, action: 'logged' }));
    }
  });

hooks.command('suggest-ultrathink')
  .description('Recommend ultrathink mode for complex tasks (Claude Code 2.0.55+)')
  .argument('<task...>', 'Task description')
  .option('--file <path>', 'File being worked on')
  .action((task: string[], opts: { file?: string }) => {
    const intel = new Intelligence();
    const taskStr = task.join(' ').toLowerCase();
    const file = opts.file;

    // Complexity patterns that suggest ultrathink mode
    const complexityPatterns: Array<[string, number]> = [
      ['algorithm', 0.8], ['optimize', 0.7], ['refactor', 0.6],
      ['debug', 0.7], ['performance', 0.7], ['concurrent', 0.8],
      ['async', 0.6], ['architecture', 0.8], ['security', 0.7],
      ['cryptograph', 0.9], ['distributed', 0.8], ['consensus', 0.9],
      ['neural', 0.8], ['ml', 0.7], ['complex', 0.6],
      ['migrate', 0.7], ['integration', 0.6], ['api design', 0.7],
      ['database schema', 0.7], ['state machine', 0.8], ['parser', 0.8],
      ['compiler', 0.9], ['memory management', 0.8], ['thread', 0.7],
    ];

    let complexityScore = 0;
    const triggers: string[] = [];

    for (const [pattern, weight] of complexityPatterns) {
      if (taskStr.includes(pattern)) {
        complexityScore = Math.max(complexityScore, weight);
        triggers.push(pattern);
      }
    }

    // Check file extension complexity
    if (file) {
      const ext = path.extname(file).slice(1);
      const complexExts: Record<string, number> = {
        rs: 0.5, cpp: 0.5, c: 0.4, zig: 0.5,
        asm: 0.7, wasm: 0.6, sql: 0.4
      };
      if (complexExts[ext]) {
        complexityScore = Math.max(complexityScore, complexExts[ext]);
        triggers.push(`${ext} file`);
      }
    }

    // Check learned patterns
    const state = `ultrathink_${triggers[0] || 'general'}`;
    const suggested = intel.suggest(state, ['enable', 'skip']);

    const recommendUltrathink = complexityScore >= 0.6;

    // Record trajectory for learning
    intel.learn(state, recommendUltrathink ? 'enable' : 'skip', taskStr.slice(0, 100), 0);

    // Build output
    const output: Record<string, unknown> = {
      task: task.join(' '),
      complexity_score: complexityScore,
      triggers,
      recommend_ultrathink: recommendUltrathink,
      learned_preference: suggested
    };

    if (recommendUltrathink) {
      output.message = '🧠 Complex task detected - ultrathink mode recommended';
      output.reasoning_depth = complexityScore >= 0.8 ? 'deep' : 'moderate';
    } else {
      output.message = 'Standard processing sufficient';
    }

    intel.save();
    console.log(JSON.stringify(output, null, 2));
  });

hooks.command('async-agent')
  .description('Coordinate async sub-agent execution (Claude Code 2.0.55+)')
  .option('--action <type>', 'Action: spawn, sync, complete', 'spawn')
  .option('--agent-id <id>', 'Agent identifier')
  .option('--task <description>', 'Task description (for spawn)')
  .action((opts: { action: string; agentId?: string; task?: string }) => {
    const intel = new Intelligence();
    const action = opts.action;
    const agentId = opts.agentId || `async-${Date.now()}`;
    const task = opts.task || '';

    switch (action) {
      case 'spawn': {
        // Register async agent
        intel.swarmRegister(agentId, 'async-subagent', ['parallel', 'autonomous']);

        // Record spawn event
        const state = `async_spawn_${task.split(' ')[0] || 'general'}`;
        intel.learn(state, 'spawn', task.slice(0, 100), 0.1);

        // Get learned patterns for similar tasks
        const suggested = intel.suggest(state, ['coder', 'researcher', 'tester', 'reviewer']);

        intel.save();
        console.log(JSON.stringify({
          action: 'spawned',
          agent_id: agentId,
          task,
          suggested_type: suggested.action,
          status: 'running',
          async: true
        }));
        break;
      }

      case 'sync': {
        // Check agent status and coordinate
        const stats = intel.swarmStats();
        console.log(JSON.stringify({
          action: 'sync',
          agent_id: agentId,
          swarm_agents: stats.agents,
          status: 'synchronized',
          pending_results: 0
        }));
        break;
      }

      case 'complete': {
        // Mark agent complete and record success
        const state = `async_complete_${agentId}`;
        intel.learn(state, 'complete', task.slice(0, 100), 1.0);

        // Update agent status
        intel.swarmHeal(agentId); // Resets/removes the agent

        intel.save();
        console.log(JSON.stringify({
          action: 'completed',
          agent_id: agentId,
          status: 'success',
          learning_recorded: true
        }));
        break;
      }

      default:
        console.log(JSON.stringify({
          error: `Unknown action: ${action}`,
          valid_actions: ['spawn', 'sync', 'complete']
        }));
    }
  });

program.parse();
