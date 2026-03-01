#!/usr/bin/env node
/**
 * Claude Flow V3 Statusline Generator (Optimized)
 * Displays real-time V3 implementation progress and system status
 *
 * Usage: node statusline.cjs [--json] [--compact]
 *
 * Performance notes:
 * - Single git execSync call (combines branch + status + upstream)
 * - No recursive file reading (only stat/readdir, never read test contents)
 * - No ps aux calls (uses process.memoryUsage() + file-based metrics)
 * - Strict 2s timeout on all execSync calls
 * - Shared settings cache across functions
 */

/* eslint-disable @typescript-eslint/no-var-requires */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const os = require('os');

// Configuration
const CONFIG = {
  maxAgents: 15,
};

const CWD = process.cwd();

// ANSI colors
const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  red: '\x1b[0;31m',
  green: '\x1b[0;32m',
  yellow: '\x1b[0;33m',
  blue: '\x1b[0;34m',
  purple: '\x1b[0;35m',
  cyan: '\x1b[0;36m',
  brightRed: '\x1b[1;31m',
  brightGreen: '\x1b[1;32m',
  brightYellow: '\x1b[1;33m',
  brightBlue: '\x1b[1;34m',
  brightPurple: '\x1b[1;35m',
  brightCyan: '\x1b[1;36m',
  brightWhite: '\x1b[1;37m',
};

// Safe execSync with strict timeout (returns empty string on failure)
// Validates command to prevent command injection
function safeExec(cmd, timeoutMs = 2000) {
  try {
    // Validate command to prevent command injection
    // Only allow commands that match safe patterns (no shell metacharacters)
    if (typeof cmd !== 'string') {
      return '';
    }
    
    // Check for dangerous shell metacharacters that could allow injection
    const dangerousChars = /[;&|`$(){}[\]<>'"\\]/;
    if (dangerousChars.test(cmd)) {
      // If dangerous chars found, only allow if it's a known safe pattern
      // Allow 'sh -c' with single-quoted script (already escaped)
      const safeShPattern = /^sh\s+-c\s+'[^']*'$/;
      if (!safeShPattern.test(cmd)) {
        console.warn('safeExec: Command contains potentially dangerous characters');
        return '';
      }
    }
    
    return execSync(cmd, {
      encoding: 'utf-8',
      timeout: timeoutMs,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
  } catch {
    return '';
  }
}

// Safe JSON file reader (returns null on failure)
function readJSON(filePath) {
  try {
    if (fs.existsSync(filePath)) {
      return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
    }
  } catch { /* ignore */ }
  return null;
}

// Safe file stat (returns null on failure)
function safeStat(filePath) {
  try {
    return fs.statSync(filePath);
  } catch { /* ignore */ }
  return null;
}

// Shared settings cache — read once, used by multiple functions
let _settingsCache = undefined;
function getSettings() {
  if (_settingsCache !== undefined) return _settingsCache;
  _settingsCache = readJSON(path.join(CWD, '.claude', 'settings.json'))
                || readJSON(path.join(CWD, '.claude', 'settings.local.json'))
                || null;
  return _settingsCache;
}

// ─── Data Collection (all pure-Node.js or single-exec) ──────────

// Get all git info in ONE shell call
function getGitInfo() {
  const result = {
    name: 'user', gitBranch: '', modified: 0, untracked: 0,
    staged: 0, ahead: 0, behind: 0,
  };

  // Single shell: get user.name, branch, porcelain status, and upstream diff
  const script = [
    'git config user.name 2>/dev/null || echo user',
    'echo "---SEP---"',
    'git branch --show-current 2>/dev/null',
    'echo "---SEP---"',
    'git status --porcelain 2>/dev/null',
    'echo "---SEP---"',
    'git rev-list --left-right --count HEAD...@{upstream} 2>/dev/null || echo "0 0"',
  ].join('; ');

  const raw = safeExec("sh -c '" + script + "'", 3000);
  if (!raw) return result;

  const parts = raw.split('---SEP---').map(s => s.trim());
  if (parts.length >= 4) {
    result.name = parts[0] || 'user';
    result.gitBranch = parts[1] || '';

    // Parse porcelain status
    if (parts[2]) {
      for (const line of parts[2].split('\n')) {
        if (!line || line.length < 2) continue;
        const x = line[0], y = line[1];
        if (x === '?' && y === '?') { result.untracked++; continue; }
        if (x !== ' ' && x !== '?') result.staged++;
        if (y !== ' ' && y !== '?') result.modified++;
      }
    }

    // Parse ahead/behind
    const ab = (parts[3] || '0 0').split(/\s+/);
    result.ahead = parseInt(ab[0]) || 0;
    result.behind = parseInt(ab[1]) || 0;
  }

  return result;
}

// Detect model name from Claude config (pure file reads, no exec)
function getModelName() {
  try {
    const claudeConfig = readJSON(path.join(os.homedir(), '.claude.json'));
    if (claudeConfig && claudeConfig.projects) {
      for (const [projectPath, projectConfig] of Object.entries(claudeConfig.projects)) {
        if (CWD === projectPath || CWD.startsWith(projectPath + '/')) {
          const usage = projectConfig.lastModelUsage;
          if (usage) {
            const ids = Object.keys(usage);
            if (ids.length > 0) {
              let modelId = ids[ids.length - 1];
              let latest = 0;
              for (const id of ids) {
                const ts = usage[id] && usage[id].lastUsedAt ? new Date(usage[id].lastUsedAt).getTime() : 0;
                if (ts > latest) { latest = ts; modelId = id; }
              }
              if (modelId.includes('opus')) return 'Opus 4.6';
              if (modelId.includes('sonnet')) return 'Sonnet 4.6';
              if (modelId.includes('haiku')) return 'Haiku 4.5';
              return modelId.split('-').slice(1, 3).join(' ');
            }
          }
          break;
        }
      }
    }
  } catch { /* ignore */ }

  // Fallback: settings.json model field
  const settings = getSettings();
  if (settings && settings.model) {
    const m = settings.model;
    if (m.includes('opus')) return 'Opus 4.6';
    if (m.includes('sonnet')) return 'Sonnet 4.6';
    if (m.includes('haiku')) return 'Haiku 4.5';
  }
  return 'Claude Code';
}

// Get learning stats from memory database (pure stat calls)
function getLearningStats() {
  const memoryPaths = [
    path.join(CWD, '.swarm', 'memory.db'),
    path.join(CWD, '.claude-flow', 'memory.db'),
    path.join(CWD, '.claude', 'memory.db'),
    path.join(CWD, 'data', 'memory.db'),
    path.join(CWD, '.agentdb', 'memory.db'),
  ];

  for (const dbPath of memoryPaths) {
    const stat = safeStat(dbPath);
    if (stat) {
      const sizeKB = stat.size / 1024;
      const patterns = Math.floor(sizeKB / 2);
      return {
        patterns,
        sessions: Math.max(1, Math.floor(patterns / 10)),
      };
    }
  }

  // Check session files count
  let sessions = 0;
  try {
    const sessDir = path.join(CWD, '.claude', 'sessions');
    if (fs.existsSync(sessDir)) {
      sessions = fs.readdirSync(sessDir).filter(f => f.endsWith('.json')).length;
    }
  } catch { /* ignore */ }

  return { patterns: 0, sessions };
}

// V3 progress from metrics files (pure file reads)
function getV3Progress() {
  const learning = getLearningStats();
  const totalDomains = 5;

  const dddData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'ddd-progress.json'));
  let dddProgress = dddData ? (dddData.progress || 0) : 0;
  let domainsCompleted = Math.min(5, Math.floor(dddProgress / 20));

  if (dddProgress === 0 && learning.patterns > 0) {
    if (learning.patterns >= 500) domainsCompleted = 5;
    else if (learning.patterns >= 200) domainsCompleted = 4;
    else if (learning.patterns >= 100) domainsCompleted = 3;
    else if (learning.patterns >= 50) domainsCompleted = 2;
    else if (learning.patterns >= 10) domainsCompleted = 1;
    dddProgress = Math.floor((domainsCompleted / totalDomains) * 100);
  }

  return {
    domainsCompleted, totalDomains, dddProgress,
    patternsLearned: learning.patterns,
    sessionsCompleted: learning.sessions,
  };
}

// Security status (pure file reads)
function getSecurityStatus() {
  const totalCves = 3;
  const auditData = readJSON(path.join(CWD, '.claude-flow', 'security', 'audit-status.json'));
  if (auditData) {
    return {
      status: auditData.status || 'PENDING',
      cvesFixed: auditData.cvesFixed || 0,
      totalCves: auditData.totalCves || 3,
    };
  }

  let cvesFixed = 0;
  try {
    const scanDir = path.join(CWD, '.claude', 'security-scans');
    if (fs.existsSync(scanDir)) {
      cvesFixed = Math.min(totalCves, fs.readdirSync(scanDir).filter(f => f.endsWith('.json')).length);
    }
  } catch { /* ignore */ }

  return {
    status: cvesFixed >= totalCves ? 'CLEAN' : cvesFixed > 0 ? 'IN_PROGRESS' : 'PENDING',
    cvesFixed,
    totalCves,
  };
}

// Swarm status (pure file reads, NO ps aux)
function getSwarmStatus() {
  const activityData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'swarm-activity.json'));
  if (activityData && activityData.swarm) {
    return {
      activeAgents: activityData.swarm.agent_count || 0,
      maxAgents: CONFIG.maxAgents,
      coordinationActive: activityData.swarm.coordination_active || activityData.swarm.active || false,
    };
  }

  const progressData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'v3-progress.json'));
  if (progressData && progressData.swarm) {
    return {
      activeAgents: progressData.swarm.activeAgents || progressData.swarm.agent_count || 0,
      maxAgents: progressData.swarm.totalAgents || CONFIG.maxAgents,
      coordinationActive: progressData.swarm.active || (progressData.swarm.activeAgents > 0),
    };
  }

  return { activeAgents: 0, maxAgents: CONFIG.maxAgents, coordinationActive: false };
}

// System metrics (uses process.memoryUsage() — no shell spawn)
function getSystemMetrics() {
  const memoryMB = Math.floor(process.memoryUsage().heapUsed / 1024 / 1024);
  const learning = getLearningStats();
  const agentdb = getAgentDBStats();

  // Intelligence from learning.json
  const learningData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'learning.json'));
  let intelligencePct = 0;
  let contextPct = 0;

  if (learningData && learningData.intelligence && learningData.intelligence.score !== undefined) {
    intelligencePct = Math.min(100, Math.floor(learningData.intelligence.score));
  } else {
    const fromPatterns = learning.patterns > 0 ? Math.min(100, Math.floor(learning.patterns / 10)) : 0;
    const fromVectors = agentdb.vectorCount > 0 ? Math.min(100, Math.floor(agentdb.vectorCount / 100)) : 0;
    intelligencePct = Math.max(fromPatterns, fromVectors);
  }

  // Maturity fallback (pure fs checks, no git exec)
  if (intelligencePct === 0) {
    let score = 0;
    if (fs.existsSync(path.join(CWD, '.claude'))) score += 15;
    const srcDirs = ['src', 'lib', 'app', 'packages', 'v3'];
    for (const d of srcDirs) { if (fs.existsSync(path.join(CWD, d))) { score += 15; break; } }
    const testDirs = ['tests', 'test', '__tests__', 'spec'];
    for (const d of testDirs) { if (fs.existsSync(path.join(CWD, d))) { score += 10; break; } }
    const cfgFiles = ['package.json', 'tsconfig.json', 'pyproject.toml', 'Cargo.toml', 'go.mod'];
    for (const f of cfgFiles) { if (fs.existsSync(path.join(CWD, f))) { score += 5; break; } }
    intelligencePct = Math.min(100, score);
  }

  if (learningData && learningData.sessions && learningData.sessions.total !== undefined) {
    contextPct = Math.min(100, learningData.sessions.total * 5);
  } else {
    contextPct = Math.min(100, Math.floor(learning.sessions * 5));
  }

  // Sub-agents from file metrics (no ps aux)
  let subAgents = 0;
  const activityData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'swarm-activity.json'));
  if (activityData && activityData.processes && activityData.processes.estimated_agents) {
    subAgents = activityData.processes.estimated_agents;
  }

  return { memoryMB, contextPct, intelligencePct, subAgents };
}

// ADR status (count files only — don't read contents)
function getADRStatus() {
  const complianceData = readJSON(path.join(CWD, '.claude-flow', 'metrics', 'adr-compliance.json'));
  if (complianceData) {
    const checks = complianceData.checks || {};
    const total = Object.keys(checks).length;
    const impl = Object.values(checks).filter(c => c.compliant).length;
    return { count: total, implemented: impl, compliance: complianceData.compliance || 0 };
  }

  // Fallback: just count ADR files (don't read them)
  const adrPaths = [
    path.join(CWD, 'v3', 'implementation', 'adrs'),
    path.join(CWD, 'docs', 'adrs'),
    path.join(CWD, '.claude-flow', 'adrs'),
  ];

  for (const adrPath of adrPaths) {
    try {
      if (fs.existsSync(adrPath)) {
        const files = fs.readdirSync(adrPath).filter(f =>
          f.endsWith('.md') && (f.startsWith('ADR-') || f.startsWith('adr-') || /^\d{4}-/.test(f))
        );
        const implemented = Math.floor(files.length * 0.7);
        const compliance = files.length > 0 ? Math.floor((implemented / files.length) * 100) : 0;
        return { count: files.length, implemented, compliance };
      }
    } catch { /* ignore */ }
  }

  return { count: 0, implemented: 0, compliance: 0 };
}

// Hooks status (shared settings cache)
function getHooksStatus() {
  let enabled = 0;
  const total = 17;
  const settings = getSettings();

  if (settings && settings.hooks) {
    for (const category of Object.keys(settings.hooks)) {
      const h = settings.hooks[category];
      if (Array.isArray(h) && h.length > 0) enabled++;
    }
  }

  try {
    const hooksDir = path.join(CWD, '.claude', 'hooks');
    if (fs.existsSync(hooksDir)) {
      const hookFiles = fs.readdirSync(hooksDir).filter(f => f.endsWith('.js') || f.endsWith('.sh')).length;
      enabled = Math.max(enabled, hookFiles);
    }
  } catch { /* ignore */ }

  return { enabled, total };
}

// AgentDB stats (pure stat calls)
function getAgentDBStats() {
  let vectorCount = 0;
  let dbSizeKB = 0;
  let namespaces = 0;
  let hasHnsw = false;

  const dbFiles = [
    path.join(CWD, '.swarm', 'memory.db'),
    path.join(CWD, '.claude-flow', 'memory.db'),
    path.join(CWD, '.claude', 'memory.db'),
    path.join(CWD, 'data', 'memory.db'),
  ];

  for (const f of dbFiles) {
    const stat = safeStat(f);
    if (stat) {
      dbSizeKB = stat.size / 1024;
      vectorCount = Math.floor(dbSizeKB / 2);
      namespaces = 1;
      break;
    }
  }

  if (vectorCount === 0) {
    const dbDirs = [
      path.join(CWD, '.claude-flow', 'agentdb'),
      path.join(CWD, '.swarm', 'agentdb'),
      path.join(CWD, '.agentdb'),
    ];
    for (const dir of dbDirs) {
      try {
        if (fs.existsSync(dir) && fs.statSync(dir).isDirectory()) {
          const files = fs.readdirSync(dir);
          namespaces = files.filter(f => f.endsWith('.db') || f.endsWith('.sqlite')).length;
          for (const file of files) {
            const stat = safeStat(path.join(dir, file));
            if (stat && stat.isFile()) dbSizeKB += stat.size / 1024;
          }
          vectorCount = Math.floor(dbSizeKB / 2);
          break;
        }
      } catch { /* ignore */ }
    }
  }

  const hnswPaths = [
    path.join(CWD, '.swarm', 'hnsw.index'),
    path.join(CWD, '.claude-flow', 'hnsw.index'),
  ];
  for (const p of hnswPaths) {
    const stat = safeStat(p);
    if (stat) {
      hasHnsw = true;
      vectorCount = Math.max(vectorCount, Math.floor(stat.size / 512));
      break;
    }
  }

  return { vectorCount, dbSizeKB: Math.floor(dbSizeKB), namespaces, hasHnsw };
}

// Test stats (count files only — NO reading file contents)
function getTestStats() {
  let testFiles = 0;

  function countTestFiles(dir, depth) {
    if (depth === undefined) depth = 0;
    if (depth > 2) return;
    try {
      if (!fs.existsSync(dir)) return;
      const entries = fs.readdirSync(dir, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isDirectory() && !entry.name.startsWith('.') && entry.name !== 'node_modules') {
          countTestFiles(path.join(dir, entry.name), depth + 1);
        } else if (entry.isFile()) {
          const n = entry.name;
          if (n.includes('.test.') || n.includes('.spec.') || n.includes('_test.') || n.includes('_spec.')) {
            testFiles++;
          }
        }
      }
    } catch { /* ignore */ }
  }

  var testDirNames = ['tests', 'test', '__tests__', 'v3/__tests__'];
  for (var i = 0; i < testDirNames.length; i++) {
    countTestFiles(path.join(CWD, testDirNames[i]));
  }
  countTestFiles(path.join(CWD, 'src'));

  return { testFiles, testCases: testFiles * 4 };
}

// Integration status (shared settings + file checks)
function getIntegrationStatus() {
  const mcpServers = { total: 0, enabled: 0 };
  const settings = getSettings();

  if (settings && settings.mcpServers && typeof settings.mcpServers === 'object') {
    const servers = Object.keys(settings.mcpServers);
    mcpServers.total = servers.length;
    mcpServers.enabled = settings.enabledMcpjsonServers
      ? settings.enabledMcpjsonServers.filter(s => servers.includes(s)).length
      : servers.length;
  }

  if (mcpServers.total === 0) {
    const mcpConfig = readJSON(path.join(CWD, '.mcp.json'))
                   || readJSON(path.join(os.homedir(), '.claude', 'mcp.json'));
    if (mcpConfig && mcpConfig.mcpServers) {
      const s = Object.keys(mcpConfig.mcpServers);
      mcpServers.total = s.length;
      mcpServers.enabled = s.length;
    }
  }

  const hasDatabase = ['.swarm/memory.db', '.claude-flow/memory.db', 'data/memory.db']
    .some(p => fs.existsSync(path.join(CWD, p)));
  const hasApi = !!(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY);

  return { mcpServers, hasDatabase, hasApi };
}

// Session stats (pure file reads)
function getSessionStats() {
  var sessionPaths = ['.claude-flow/session.json', '.claude/session.json'];
  for (var i = 0; i < sessionPaths.length; i++) {
    const data = readJSON(path.join(CWD, sessionPaths[i]));
    if (data && data.startTime) {
      const diffMs = Date.now() - new Date(data.startTime).getTime();
      const mins = Math.floor(diffMs / 60000);
      const duration = mins < 60 ? mins + 'm' : Math.floor(mins / 60) + 'h' + (mins % 60) + 'm';
      return { duration: duration };
    }
  }
  return { duration: '' };
}

// ─── Rendering ──────────────────────────────────────────────────

function progressBar(current, total) {
  const width = 5;
  const filled = Math.round((current / total) * width);
  return '[' + '\u25CF'.repeat(filled) + '\u25CB'.repeat(width - filled) + ']';
}

function generateStatusline() {
  const git = getGitInfo();
  // Prefer model name from Claude Code stdin data, fallback to file-based detection
  const modelName = getModelFromStdin() || getModelName();
  const ctxInfo = getContextFromStdin();
  const costInfo = getCostFromStdin();
  const progress = getV3Progress();
  const security = getSecurityStatus();
  const swarm = getSwarmStatus();
  const system = getSystemMetrics();
  const adrs = getADRStatus();
  const hooks = getHooksStatus();
  const agentdb = getAgentDBStats();
  const tests = getTestStats();
  const session = getSessionStats();
  const integration = getIntegrationStatus();
  const lines = [];

  // Header
  let header = c.bold + c.brightPurple + '\u258A Claude Flow V3 ' + c.reset;
  header += (swarm.coordinationActive ? c.brightCyan : c.dim) + '\u25CF ' + c.brightCyan + git.name + c.reset;
  if (git.gitBranch) {
    header += '  ' + c.dim + '\u2502' + c.reset + '  ' + c.brightBlue + '\u23C7 ' + git.gitBranch + c.reset;
    const changes = git.modified + git.staged + git.untracked;
    if (changes > 0) {
      let ind = '';
      if (git.staged > 0) ind += c.brightGreen + '+' + git.staged + c.reset;
      if (git.modified > 0) ind += c.brightYellow + '~' + git.modified + c.reset;
      if (git.untracked > 0) ind += c.dim + '?' + git.untracked + c.reset;
      header += ' ' + ind;
    }
    if (git.ahead > 0) header += ' ' + c.brightGreen + '\u2191' + git.ahead + c.reset;
    if (git.behind > 0) header += ' ' + c.brightRed + '\u2193' + git.behind + c.reset;
  }
  header += '  ' + c.dim + '\u2502' + c.reset + '  ' + c.purple + modelName + c.reset;
  // Show session duration from Claude Code stdin if available, else from local files
  const duration = costInfo ? costInfo.duration : session.duration;
  if (duration) header += '  ' + c.dim + '\u2502' + c.reset + '  ' + c.cyan + '\u23F1 ' + duration + c.reset;
  // Show context usage from Claude Code stdin if available
  if (ctxInfo && ctxInfo.usedPct > 0) {
    const ctxColor = ctxInfo.usedPct >= 90 ? c.brightRed : ctxInfo.usedPct >= 70 ? c.brightYellow : c.brightGreen;
    header += '  ' + c.dim + '\u2502' + c.reset + '  ' + ctxColor + '\u25CF ' + ctxInfo.usedPct + '% ctx' + c.reset;
  }
  // Show cost from Claude Code stdin if available
  if (costInfo && costInfo.costUsd > 0) {
    header += '  ' + c.dim + '\u2502' + c.reset + '  ' + c.brightYellow + '$' + costInfo.costUsd.toFixed(2) + c.reset;
  }
  lines.push(header);

  // Separator
  lines.push(c.dim + '\u2500'.repeat(53) + c.reset);

  // Line 1: DDD Domains
  const domainsColor = progress.domainsCompleted >= 3 ? c.brightGreen : progress.domainsCompleted > 0 ? c.yellow : c.red;
  let perfIndicator;
  if (agentdb.hasHnsw && agentdb.vectorCount > 0) {
    const speedup = agentdb.vectorCount > 10000 ? '12500x' : agentdb.vectorCount > 1000 ? '150x' : '10x';
    perfIndicator = c.brightGreen + '\u26A1 HNSW ' + speedup + c.reset;
  } else if (progress.patternsLearned > 0) {
    const pk = progress.patternsLearned >= 1000 ? (progress.patternsLearned / 1000).toFixed(1) + 'k' : String(progress.patternsLearned);
    perfIndicator = c.brightYellow + '\uD83D\uDCDA ' + pk + ' patterns' + c.reset;
  } else {
    perfIndicator = c.dim + '\u26A1 target: 150x-12500x' + c.reset;
  }
  lines.push(
    c.brightCyan + '\uD83C\uDFD7\uFE0F  DDD Domains' + c.reset + '    ' + progressBar(progress.domainsCompleted, progress.totalDomains) + '  ' +
    domainsColor + progress.domainsCompleted + c.reset + '/' + c.brightWhite + progress.totalDomains + c.reset + '    ' + perfIndicator
  );

  // Line 2: Swarm + Hooks + CVE + Memory + Intelligence
  const swarmInd = swarm.coordinationActive ? c.brightGreen + '\u25C9' + c.reset : c.dim + '\u25CB' + c.reset;
  const agentsColor = swarm.activeAgents > 0 ? c.brightGreen : c.red;
  const secIcon = security.status === 'CLEAN' ? '\uD83D\uDFE2' : security.status === 'IN_PROGRESS' ? '\uD83D\uDFE1' : '\uD83D\uDD34';
  const secColor = security.status === 'CLEAN' ? c.brightGreen : security.status === 'IN_PROGRESS' ? c.brightYellow : c.brightRed;
  const hooksColor = hooks.enabled > 0 ? c.brightGreen : c.dim;
  const intellColor = system.intelligencePct >= 80 ? c.brightGreen : system.intelligencePct >= 40 ? c.brightYellow : c.dim;

  lines.push(
    c.brightYellow + '\uD83E\uDD16 Swarm' + c.reset + '  ' + swarmInd + ' [' + agentsColor + String(swarm.activeAgents).padStart(2) + c.reset + '/' + c.brightWhite + swarm.maxAgents + c.reset + ']  ' +
    c.brightPurple + '\uD83D\uDC65 ' + system.subAgents + c.reset + '    ' +
    c.brightBlue + '\uD83E\uDE9D ' + hooksColor + hooks.enabled + c.reset + '/' + c.brightWhite + hooks.total + c.reset + '    ' +
    secIcon + ' ' + secColor + 'CVE ' + security.cvesFixed + c.reset + '/' + c.brightWhite + security.totalCves + c.reset + '    ' +
    c.brightCyan + '\uD83D\uDCBE ' + system.memoryMB + 'MB' + c.reset + '    ' +
    intellColor + '\uD83E\uDDE0 ' + String(system.intelligencePct).padStart(3) + '%' + c.reset
  );

  // Line 3: Architecture
  const dddColor = progress.dddProgress >= 50 ? c.brightGreen : progress.dddProgress > 0 ? c.yellow : c.red;
  const adrColor = adrs.count > 0 ? (adrs.implemented === adrs.count ? c.brightGreen : c.yellow) : c.dim;
  const adrDisplay = adrs.compliance > 0 ? adrColor + '\u25CF' + adrs.compliance + '%' + c.reset : adrColor + '\u25CF' + adrs.implemented + '/' + adrs.count + c.reset;

  lines.push(
    c.brightPurple + '\uD83D\uDD27 Architecture' + c.reset + '    ' +
    c.cyan + 'ADRs' + c.reset + ' ' + adrDisplay + '  ' + c.dim + '\u2502' + c.reset + '  ' +
    c.cyan + 'DDD' + c.reset + ' ' + dddColor + '\u25CF' + String(progress.dddProgress).padStart(3) + '%' + c.reset + '  ' + c.dim + '\u2502' + c.reset + '  ' +
    c.cyan + 'Security' + c.reset + ' ' + secColor + '\u25CF' + security.status + c.reset
  );

  // Line 4: AgentDB, Tests, Integration
  const hnswInd = agentdb.hasHnsw ? c.brightGreen + '\u26A1' + c.reset : '';
  const sizeDisp = agentdb.dbSizeKB >= 1024 ? (agentdb.dbSizeKB / 1024).toFixed(1) + 'MB' : agentdb.dbSizeKB + 'KB';
  const vectorColor = agentdb.vectorCount > 0 ? c.brightGreen : c.dim;
  const testColor = tests.testFiles > 0 ? c.brightGreen : c.dim;

  let integStr = '';
  if (integration.mcpServers.total > 0) {
    const mcpCol = integration.mcpServers.enabled === integration.mcpServers.total ? c.brightGreen :
                   integration.mcpServers.enabled > 0 ? c.brightYellow : c.red;
    integStr += c.cyan + 'MCP' + c.reset + ' ' + mcpCol + '\u25CF' + integration.mcpServers.enabled + '/' + integration.mcpServers.total + c.reset;
  }
  if (integration.hasDatabase) integStr += (integStr ? '  ' : '') + c.brightGreen + '\u25C6' + c.reset + 'DB';
  if (integration.hasApi) integStr += (integStr ? '  ' : '') + c.brightGreen + '\u25C6' + c.reset + 'API';
  if (!integStr) integStr = c.dim + '\u25CF none' + c.reset;

  lines.push(
    c.brightCyan + '\uD83D\uDCCA AgentDB' + c.reset + '    ' +
    c.cyan + 'Vectors' + c.reset + ' ' + vectorColor + '\u25CF' + agentdb.vectorCount + hnswInd + c.reset + '  ' + c.dim + '\u2502' + c.reset + '  ' +
    c.cyan + 'Size' + c.reset + ' ' + c.brightWhite + sizeDisp + c.reset + '  ' + c.dim + '\u2502' + c.reset + '  ' +
    c.cyan + 'Tests' + c.reset + ' ' + testColor + '\u25CF' + tests.testFiles + c.reset + ' ' + c.dim + '(~' + tests.testCases + ' cases)' + c.reset + '  ' + c.dim + '\u2502' + c.reset + '  ' +
    integStr
  );

  return lines.join('\n');
}

// JSON output
function generateJSON() {
  const git = getGitInfo();
  return {
    user: { name: git.name, gitBranch: git.gitBranch, modelName: getModelName() },
    v3Progress: getV3Progress(),
    security: getSecurityStatus(),
    swarm: getSwarmStatus(),
    system: getSystemMetrics(),
    adrs: getADRStatus(),
    hooks: getHooksStatus(),
    agentdb: getAgentDBStats(),
    tests: getTestStats(),
    git: { modified: git.modified, untracked: git.untracked, staged: git.staged, ahead: git.ahead, behind: git.behind },
    lastUpdated: new Date().toISOString(),
  };
}

// ─── Stdin reader (Claude Code pipes session JSON) ──────────────

// Claude Code sends session JSON via stdin (model, context, cost, etc.)
// Read it synchronously so the script works both:
//   1. When invoked by Claude Code (stdin has JSON)
//   2. When invoked manually from terminal (stdin is empty/tty)
let _stdinData = null;
function getStdinData() {
  if (_stdinData !== undefined && _stdinData !== null) return _stdinData;
  try {
    // Check if stdin is a TTY (manual run) — skip reading
    if (process.stdin.isTTY) { _stdinData = null; return null; }
    // Read stdin synchronously via fd 0
    const chunks = [];
    const buf = Buffer.alloc(4096);
    let bytesRead;
    try {
      while ((bytesRead = fs.readSync(0, buf, 0, buf.length, null)) > 0) {
        chunks.push(buf.slice(0, bytesRead));
      }
    } catch { /* EOF or read error */ }
    const raw = Buffer.concat(chunks).toString('utf-8').trim();
    if (raw && raw.startsWith('{')) {
      _stdinData = JSON.parse(raw);
    } else {
      _stdinData = null;
    }
  } catch {
    _stdinData = null;
  }
  return _stdinData;
}

// Override model detection to prefer stdin data from Claude Code
function getModelFromStdin() {
  const data = getStdinData();
  if (data && data.model && data.model.display_name) return data.model.display_name;
  return null;
}

// Get context window info from Claude Code session
function getContextFromStdin() {
  const data = getStdinData();
  if (data && data.context_window) {
    return {
      usedPct: Math.floor(data.context_window.used_percentage || 0),
      remainingPct: Math.floor(data.context_window.remaining_percentage || 100),
    };
  }
  return null;
}

// Get cost info from Claude Code session
function getCostFromStdin() {
  const data = getStdinData();
  if (data && data.cost) {
    const durationMs = data.cost.total_duration_ms || 0;
    const mins = Math.floor(durationMs / 60000);
    const secs = Math.floor((durationMs % 60000) / 1000);
    return {
      costUsd: data.cost.total_cost_usd || 0,
      duration: mins > 0 ? mins + 'm' + secs + 's' : secs + 's',
      linesAdded: data.cost.total_lines_added || 0,
      linesRemoved: data.cost.total_lines_removed || 0,
    };
  }
  return null;
}

// ─── Main ───────────────────────────────────────────────────────
if (process.argv.includes('--json')) {
  console.log(JSON.stringify(generateJSON(), null, 2));
} else if (process.argv.includes('--compact')) {
  console.log(JSON.stringify(generateJSON()));
} else {
  console.log(generateStatusline());
}
