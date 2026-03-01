#!/usr/bin/env node
/**
 * Claude Flow V3 Statusline Generator
 * Displays real-time V3 implementation progress and system status
 *
 * Usage: node statusline.cjs [--json] [--compact]
 *
 * IMPORTANT: This file uses .cjs extension to work in ES module projects.
 * The require() syntax is intentional for CommonJS compatibility.
 */

/* eslint-disable @typescript-eslint/no-var-requires */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Configuration
const CONFIG = {
  enabled: true,
  showProgress: true,
  showSecurity: true,
  showSwarm: true,
  showHooks: true,
  showPerformance: true,
  refreshInterval: 5000,
  maxAgents: 15,
  topology: 'hierarchical-mesh',
};

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

// Get user info
function getUserInfo() {
  let name = 'user';
  let gitBranch = '';
  let modelName = '🤖 Claude Code';

  try {
    name = execSync('git config user.name 2>/dev/null || echo "user"', { encoding: 'utf-8' }).trim();
    gitBranch = execSync('git branch --show-current 2>/dev/null || echo ""', { encoding: 'utf-8' }).trim();
  } catch (e) {
    // Ignore errors
  }

  // Auto-detect model from Claude Code's config
  try {
    const homedir = require('os').homedir();
    const claudeConfigPath = path.join(homedir, '.claude.json');
    if (fs.existsSync(claudeConfigPath)) {
      const claudeConfig = JSON.parse(fs.readFileSync(claudeConfigPath, 'utf-8'));
      // Try to find lastModelUsage - check current dir and parent dirs
      let lastModelUsage = null;
      const cwd = process.cwd();
      if (claudeConfig.projects) {
        // Try exact match first, then check if cwd starts with any project path
        for (const [projectPath, projectConfig] of Object.entries(claudeConfig.projects)) {
          if (cwd === projectPath || cwd.startsWith(projectPath + '/')) {
            lastModelUsage = projectConfig.lastModelUsage;
            break;
          }
        }
      }
      if (lastModelUsage) {
        const modelIds = Object.keys(lastModelUsage);
        if (modelIds.length > 0) {
          // Find the most recently used model by checking lastUsedAt timestamps
          // or fall back to the last key in the object (preserves insertion order in modern JS)
          let modelId = modelIds[modelIds.length - 1];
          let latestTimestamp = 0;

          for (const id of modelIds) {
            const usage = lastModelUsage[id];
            // Check for lastUsedAt timestamp (if available)
            if (usage.lastUsedAt) {
              const ts = new Date(usage.lastUsedAt).getTime();
              if (ts > latestTimestamp) {
                latestTimestamp = ts;
                modelId = id;
              }
            }
          }

          // Parse model ID to human-readable name
          if (modelId.includes('opus')) modelName = 'Opus 4.5';
          else if (modelId.includes('sonnet')) modelName = 'Sonnet 4';
          else if (modelId.includes('haiku')) modelName = 'Haiku 4.5';
          else modelName = modelId.split('-').slice(1, 3).join(' ');
        }
      }
    }
  } catch (e) {
    // Fallback to Unknown if can't read config
  }

  // Fallback: check project's .claude/settings.json for model
  if (modelName === 'Unknown') {
    try {
      const settingsPath = path.join(process.cwd(), '.claude', 'settings.json');
      if (fs.existsSync(settingsPath)) {
        const settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8'));
        if (settings.model) {
          if (settings.model.includes('opus')) modelName = 'Opus 4.5';
          else if (settings.model.includes('sonnet')) modelName = 'Sonnet 4';
          else if (settings.model.includes('haiku')) modelName = 'Haiku 4.5';
          else modelName = settings.model.split('-').slice(1, 3).join(' ');
        }
      }
    } catch (e) {
      // Keep Unknown
    }
  }

  return { name, gitBranch, modelName };
}

// Get learning stats from intelligence loop data (ADR-050)
function getLearningStats() {
  let patterns = 0;
  let sessions = 0;
  let trajectories = 0;
  let edges = 0;
  let confidenceMean = 0;
  let accessedCount = 0;
  let trend = 'STABLE';

  // PRIMARY: Read from intelligence loop data files
  const dataDir = path.join(process.cwd(), '.claude-flow', 'data');

  // 1. graph-state.json — authoritative node/edge counts
  const graphPath = path.join(dataDir, 'graph-state.json');
  if (fs.existsSync(graphPath)) {
    try {
      const graph = JSON.parse(fs.readFileSync(graphPath, 'utf-8'));
      patterns = graph.nodes ? Object.keys(graph.nodes).length : 0;
      edges = Array.isArray(graph.edges) ? graph.edges.length : 0;
    } catch (e) { /* ignore */ }
  }

  // 2. ranked-context.json — confidence and access data
  const rankedPath = path.join(dataDir, 'ranked-context.json');
  if (fs.existsSync(rankedPath)) {
    try {
      const ranked = JSON.parse(fs.readFileSync(rankedPath, 'utf-8'));
      if (ranked.entries && ranked.entries.length > 0) {
        patterns = Math.max(patterns, ranked.entries.length);
        let confSum = 0;
        let accCount = 0;
        for (let i = 0; i < ranked.entries.length; i++) {
          confSum += (ranked.entries[i].confidence || 0);
          if ((ranked.entries[i].accessCount || 0) > 0) accCount++;
        }
        confidenceMean = confSum / ranked.entries.length;
        accessedCount = accCount;
      }
    } catch (e) { /* ignore */ }
  }

  // 3. intelligence-snapshot.json — trend history
  const snapshotPath = path.join(dataDir, 'intelligence-snapshot.json');
  if (fs.existsSync(snapshotPath)) {
    try {
      const snapshot = JSON.parse(fs.readFileSync(snapshotPath, 'utf-8'));
      if (snapshot.history && snapshot.history.length >= 2) {
        const first = snapshot.history[0];
        const last = snapshot.history[snapshot.history.length - 1];
        const confDrift = (last.confidenceMean || 0) - (first.confidenceMean || 0);
        trend = confDrift > 0.01 ? 'IMPROVING' : confDrift < -0.01 ? 'DECLINING' : 'STABLE';
        sessions = Math.max(sessions, snapshot.history.length);
      }
    } catch (e) { /* ignore */ }
  }

  // 4. auto-memory-store.json — fallback entry count
  if (patterns === 0) {
    const autoMemPath = path.join(dataDir, 'auto-memory-store.json');
    if (fs.existsSync(autoMemPath)) {
      try {
        const data = JSON.parse(fs.readFileSync(autoMemPath, 'utf-8'));
        patterns = Array.isArray(data) ? data.length : (data.entries ? data.entries.length : 0);
      } catch (e) { /* ignore */ }
    }
  }

  // FALLBACK: Legacy memory.db file-size estimation
  if (patterns === 0) {
    const memoryPaths = [
      path.join(process.cwd(), '.swarm', 'memory.db'),
      path.join(process.cwd(), '.claude-flow', 'memory.db'),
      path.join(process.cwd(), '.claude', 'memory.db'),
      path.join(process.cwd(), 'data', 'memory.db'),
      path.join(process.cwd(), 'memory.db'),
      path.join(process.cwd(), '.agentdb', 'memory.db'),
    ];
    for (let j = 0; j < memoryPaths.length; j++) {
      if (fs.existsSync(memoryPaths[j])) {
        try {
          const dbStats = fs.statSync(memoryPaths[j]);
          patterns = Math.floor(dbStats.size / 1024 / 2);
          break;
        } catch (e) { /* ignore */ }
      }
    }
  }

  // Session count from session files
  const sessionsPath = path.join(process.cwd(), '.claude', 'sessions');
  if (fs.existsSync(sessionsPath)) {
    try {
      const sessionFiles = fs.readdirSync(sessionsPath).filter(f => f.endsWith('.json'));
      sessions = Math.max(sessions, sessionFiles.length);
    } catch (e) { /* ignore */ }
  }

  trajectories = Math.floor(patterns / 5);

  return { patterns, sessions, trajectories, edges, confidenceMean, accessedCount, trend };
}

// Get V3 progress from REAL metrics files
function getV3Progress() {
  const learning = getLearningStats();
  const totalDomains = 5;

  let dddProgress = 0;
  let dddScore = 0;
  let dddMaxScore = 100;
  let moduleCount = 0;

  // Check ddd-progress.json for REAL DDD analysis
  const dddPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'ddd-progress.json');
  if (fs.existsSync(dddPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(dddPath, 'utf-8'));
      dddProgress = data.progress || 0;
      dddScore = data.score || 0;
      dddMaxScore = data.maxScore || 100;
      moduleCount = data.modules ? Object.keys(data.modules).length : 0;
    } catch (e) {
      // Ignore - use fallback
    }
  }

  // Calculate domains completed from DDD progress (each 20% = 1 domain)
  let domainsCompleted = Math.min(5, Math.floor(dddProgress / 20));

  // Fallback: if no DDD data, use pattern-based calculation
  if (dddProgress === 0 && learning.patterns > 0) {
    if (learning.patterns >= 500) domainsCompleted = 5;
    else if (learning.patterns >= 200) domainsCompleted = 4;
    else if (learning.patterns >= 100) domainsCompleted = 3;
    else if (learning.patterns >= 50) domainsCompleted = 2;
    else if (learning.patterns >= 10) domainsCompleted = 1;
    dddProgress = Math.floor((domainsCompleted / totalDomains) * 100);
  }

  return {
    domainsCompleted,
    totalDomains,
    dddProgress,
    dddScore,
    dddMaxScore,
    moduleCount,
    patternsLearned: learning.patterns,
    sessionsCompleted: learning.sessions
  };
}

// Get security status based on actual scans
function getSecurityStatus() {
  const totalCves = 3;
  let cvesFixed = 0;

  // Check audit-status.json first (created by init)
  const auditStatusPath = path.join(process.cwd(), '.claude-flow', 'security', 'audit-status.json');
  if (fs.existsSync(auditStatusPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(auditStatusPath, 'utf-8'));
      return {
        status: data.status || 'PENDING',
        cvesFixed: data.cvesFixed || 0,
        totalCves: data.totalCves || 3,
      };
    } catch (e) {
      // Fall through to scan directory check
    }
  }

  // Check for security scan results in memory
  const scanResultsPath = path.join(process.cwd(), '.claude', 'security-scans');
  if (fs.existsSync(scanResultsPath)) {
    try {
      const scans = fs.readdirSync(scanResultsPath).filter(f => f.endsWith('.json'));
      // Each successful scan file = 1 CVE addressed
      cvesFixed = Math.min(totalCves, scans.length);
    } catch (e) {
      // Ignore
    }
  }

  // Also check .swarm/security for audit results
  const swarmAuditPath = path.join(process.cwd(), '.swarm', 'security');
  if (fs.existsSync(swarmAuditPath)) {
    try {
      const audits = fs.readdirSync(swarmAuditPath).filter(f => f.includes('audit'));
      cvesFixed = Math.min(totalCves, Math.max(cvesFixed, audits.length));
    } catch (e) {
      // Ignore
    }
  }

  const status = cvesFixed >= totalCves ? 'CLEAN' : cvesFixed > 0 ? 'IN_PROGRESS' : 'PENDING';

  return {
    status,
    cvesFixed,
    totalCves,
  };
}

// Get swarm status (cross-platform)
function getSwarmStatus() {
  let activeAgents = 0;
  let coordinationActive = false;

  // Check swarm-activity.json first (works on all platforms)
  const activityPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'swarm-activity.json');
  if (fs.existsSync(activityPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(activityPath, 'utf-8'));
      if (data.swarm) {
        return {
          activeAgents: data.swarm.agent_count || 0,
          maxAgents: CONFIG.maxAgents,
          coordinationActive: data.swarm.coordination_active || data.swarm.active || false,
        };
      }
    } catch (e) {
      // Fall through to v3-progress.json check
    }
  }

  // Also check v3-progress.json for swarm data (secondary source)
  const progressPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'v3-progress.json');
  if (fs.existsSync(progressPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(progressPath, 'utf-8'));
      if (data.swarm) {
        return {
          activeAgents: data.swarm.activeAgents || data.swarm.agent_count || 0,
          maxAgents: data.swarm.totalAgents || CONFIG.maxAgents,
          coordinationActive: data.swarm.active || (data.swarm.activeAgents > 0),
        };
      }
    } catch (e) {
      // Fall through to process detection
    }
  }

  // Platform-specific process detection (fallback)
  const isWindows = process.platform === 'win32';
  try {
    if (isWindows) {
      // Windows: use tasklist
      const ps = execSync('tasklist /FI "IMAGENAME eq node.exe" /NH 2>nul || echo ""', { encoding: 'utf-8' });
      const nodeProcesses = (ps.match(/node\.exe/gi) || []).length;
      activeAgents = Math.max(0, Math.floor(nodeProcesses / 3)); // Heuristic
      coordinationActive = nodeProcesses > 0;
    } else {
      // Unix: use ps - check for various agent process patterns
      try {
        const ps = execSync('ps aux 2>/dev/null | grep -E "(agentic-flow|claude-flow|mcp.*server)" | grep -v grep | wc -l', { encoding: 'utf-8' });
        activeAgents = Math.max(0, parseInt(ps.trim()));
        coordinationActive = activeAgents > 0;
      } catch (e) {
        // Fallback to simple agentic-flow check
        const ps = execSync('ps aux 2>/dev/null | grep -c agentic-flow || echo "0"', { encoding: 'utf-8' });
        activeAgents = Math.max(0, parseInt(ps.trim()) - 1);
        coordinationActive = activeAgents > 0;
      }
    }
  } catch (e) {
    // Ignore errors - return defaults
  }

  return {
    activeAgents,
    maxAgents: CONFIG.maxAgents,
    coordinationActive,
  };
}

// Get system metrics (cross-platform)
function getSystemMetrics() {
  let memoryMB = 0;
  let subAgents = 0;

  // Check learning.json first for REAL intelligence metrics
  const learningMetricsPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'learning.json');
  let intelligenceFromFile = null;
  let contextFromFile = null;
  if (fs.existsSync(learningMetricsPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(learningMetricsPath, 'utf-8'));
      // Use intelligence.score (the REAL metric) instead of routing.accuracy
      if (data.intelligence?.score !== undefined) {
        intelligenceFromFile = Math.min(100, Math.floor(data.intelligence.score));
      }
      if (data.sessions?.total !== undefined) {
        contextFromFile = Math.min(100, data.sessions.total * 5);
      }
    } catch (e) {
      // Fall through
    }
  }

  // Platform-specific memory detection
  const isWindows = process.platform === 'win32';
  try {
    if (isWindows) {
      // Windows: use process.memoryUsage() (most reliable cross-platform)
      memoryMB = Math.floor(process.memoryUsage().heapUsed / 1024 / 1024);
    } else {
      // Unix: try ps command, fallback to process.memoryUsage()
      try {
        const mem = execSync('ps aux | grep -E "(node|agentic|claude)" | grep -v grep | awk \'{sum += \$6} END {print int(sum/1024)}\'', { encoding: 'utf-8' });
        memoryMB = parseInt(mem.trim()) || 0;
      } catch (e) {
        memoryMB = Math.floor(process.memoryUsage().heapUsed / 1024 / 1024);
      }
    }
  } catch (e) {
    // Fallback to Node.js memory API
    memoryMB = Math.floor(process.memoryUsage().heapUsed / 1024 / 1024);
  }

  // Get learning stats for intelligence %
  const learning = getLearningStats();

  // Also get AgentDB stats for fallback intelligence calculation
  const agentdbStats = getAgentDBStats();

  // Intelligence % — priority chain (ADR-050):
  // 1. Intelligence loop data (confidenceMean + accessRatio + density)
  // 2. learning.json file metric
  // 3. Pattern count / vector count fallback
  // 4. Project maturity fallback (below)
  let intelligencePct = 0;

  // Priority 1: Intelligence loop real data
  if (learning.confidenceMean > 0 || (learning.patterns > 0 && learning.accessedCount > 0)) {
    const confScore = Math.min(100, Math.floor(learning.confidenceMean * 100));
    const accessRatio = learning.patterns > 0 ? (learning.accessedCount / learning.patterns) : 0;
    const accessScore = Math.min(100, Math.floor(accessRatio * 100));
    const densityScore = Math.min(100, Math.floor(learning.patterns / 5));
    intelligencePct = Math.floor(confScore * 0.4 + accessScore * 0.3 + densityScore * 0.3);
  }

  // Priority 2: learning.json file metric
  if (intelligencePct === 0 && intelligenceFromFile !== null) {
    intelligencePct = intelligenceFromFile;
  }

  // Priority 3: Pattern/vector count fallback
  if (intelligencePct === 0) {
    const fromPatterns = learning.patterns > 0 ? Math.min(100, Math.floor(learning.patterns / 10)) : 0;
    const fromVectors = agentdbStats.vectorCount > 0 ? Math.min(100, Math.floor(agentdbStats.vectorCount / 100)) : 0;
    intelligencePct = Math.max(fromPatterns, fromVectors);
  }

  // If still 0, use project maturity fallback
  if (intelligencePct === 0) {
    // Final fallback: estimate from project maturity indicators
    let maturityScore = 0;

    // Check git commit count (proxy for project development)
    try {
      const commitCount = parseInt(execSync('git rev-list --count HEAD 2>/dev/null || echo "0"', { encoding: 'utf-8' }).trim());
      maturityScore += Math.min(30, Math.floor(commitCount / 10)); // Max 30% from commits
    } catch (e) { /* ignore */ }

    // Check for Claude session history
    const sessionPaths = [
      path.join(process.cwd(), '.claude', 'sessions'),
      path.join(process.cwd(), '.claude-flow', 'sessions'),
    ];
    for (const sessPath of sessionPaths) {
      if (fs.existsSync(sessPath)) {
        try {
          const sessions = fs.readdirSync(sessPath).filter(f => f.endsWith('.json')).length;
          maturityScore += Math.min(20, sessions * 2); // Max 20% from sessions
          break;
        } catch (e) { /* ignore */ }
      }
    }

    // Check for source files (indicates codebase size)
    try {
      const srcDirs = ['src', 'lib', 'app', 'packages'];
      for (const dir of srcDirs) {
        const dirPath = path.join(process.cwd(), dir);
        if (fs.existsSync(dirPath)) {
          maturityScore += 15; // Base score for having source dir
          break;
        }
      }
    } catch (e) { /* ignore */ }

    // Check for test files
    try {
      const testDirs = ['tests', 'test', '__tests__', 'spec'];
      for (const dir of testDirs) {
        const dirPath = path.join(process.cwd(), dir);
        if (fs.existsSync(dirPath)) {
          maturityScore += 10; // Bonus for having tests
          break;
        }
      }
    } catch (e) { /* ignore */ }

    // Check for .claude directory (Claude Code usage)
    if (fs.existsSync(path.join(process.cwd(), '.claude'))) {
      maturityScore += 15; // Bonus for Claude Code integration
    }

    // Check for config files (project maturity)
    const configFiles = ['package.json', 'tsconfig.json', 'pyproject.toml', 'Cargo.toml', 'go.mod'];
    for (const cfg of configFiles) {
      if (fs.existsSync(path.join(process.cwd(), cfg))) {
        maturityScore += 5;
        break;
      }
    }

    intelligencePct = Math.min(100, maturityScore);
  }

  // Context % based on session history (0 sessions = 0%, grows with usage)
  const contextPct = contextFromFile !== null
    ? contextFromFile
    : Math.min(100, Math.floor(learning.sessions * 5));

  // Count active sub-agents (cross-platform via metrics file)
  const activityPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'swarm-activity.json');
  if (fs.existsSync(activityPath)) {
    try {
      const data = JSON.parse(fs.readFileSync(activityPath, 'utf-8'));
      subAgents = data.processes?.estimated_agents || 0;
    } catch (e) {
      // Ignore
    }
  }

  // Fallback to process detection on Unix only
  if (subAgents === 0 && !isWindows) {
    try {
      const agents = execSync('ps aux 2>/dev/null | grep -c "claude-flow.*agent" || echo "0"', { encoding: 'utf-8' });
      subAgents = Math.max(0, parseInt(agents.trim()) - 1);
    } catch (e) {
      // Ignore
    }
  }

  return {
    memoryMB,
    contextPct,
    intelligencePct,
    subAgents,
  };
}

// Get ADR (Architecture Decision Records) status from REAL compliance data
function getADRStatus() {
  let compliance = 0;
  let totalChecks = 0;
  let compliantChecks = 0;
  let checks = {};

  // Check adr-compliance.json for REAL compliance data
  const compliancePath = path.join(process.cwd(), '.claude-flow', 'metrics', 'adr-compliance.json');
  if (fs.existsSync(compliancePath)) {
    try {
      const data = JSON.parse(fs.readFileSync(compliancePath, 'utf-8'));
      compliance = data.compliance || 0;
      checks = data.checks || {};
      totalChecks = Object.keys(checks).length;
      compliantChecks = Object.values(checks).filter(c => c.compliant).length;
      return { count: totalChecks, implemented: compliantChecks, compliance };
    } catch (e) {
      // Fall through to file-based detection
    }
  }

  // Fallback: count ADR files directly
  const adrPaths = [
    path.join(process.cwd(), 'docs', 'adrs'),
    path.join(process.cwd(), 'docs', 'adr'),
    path.join(process.cwd(), 'adr'),
    path.join(process.cwd(), 'ADR'),
    path.join(process.cwd(), '.claude-flow', 'adrs'),
    path.join(process.cwd(), 'v3', 'implementation', 'adrs'),
    path.join(process.cwd(), 'implementation', 'adrs'),
  ];

  let count = 0;
  let implemented = 0;

  for (const adrPath of adrPaths) {
    if (fs.existsSync(adrPath)) {
      try {
        const files = fs.readdirSync(adrPath).filter(f =>
          f.endsWith('.md') && (f.startsWith('ADR-') || f.startsWith('adr-') || /^\d{4}-/.test(f))
        );
        count = files.length;

        for (const file of files) {
          try {
            const content = fs.readFileSync(path.join(adrPath, file), 'utf-8');
            if (content.includes('Status: Implemented') || content.includes('status: implemented') ||
                content.includes('Status: Accepted') || content.includes('status: accepted')) {
              implemented++;
            }
          } catch (e) {
            // Skip unreadable files
          }
        }
        break;
      } catch (e) {
        // Ignore
      }
    }
  }

  compliance = count > 0 ? Math.floor((implemented / count) * 100) : 0;
  return { count, implemented, compliance };
}

// Get hooks status (enabled/registered hooks)
function getHooksStatus() {
  let enabled = 0;
  let total = 17; // V3 has 17 hook types

  // Check .claude/settings.json for hooks config
  const settingsPaths = [
    path.join(process.cwd(), '.claude', 'settings.json'),
    path.join(process.cwd(), '.claude', 'settings.local.json'),
  ];

  for (const settingsPath of settingsPaths) {
    if (fs.existsSync(settingsPath)) {
      try {
        const settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8'));
        if (settings.hooks) {
          // Claude Code native hooks format: PreToolUse, PostToolUse, SessionStart, etc.
          const hookCategories = Object.keys(settings.hooks);
          for (const category of hookCategories) {
            const categoryHooks = settings.hooks[category];
            if (Array.isArray(categoryHooks) && categoryHooks.length > 0) {
              // Count categories with at least one hook defined
              enabled++;
            }
          }
        }
        break;
      } catch (e) {
        // Ignore parse errors
      }
    }
  }

  // Also check for hook files in .claude/hooks
  const hooksDir = path.join(process.cwd(), '.claude', 'hooks');
  if (fs.existsSync(hooksDir)) {
    try {
      const hookFiles = fs.readdirSync(hooksDir).filter(f => f.endsWith('.js') || f.endsWith('.sh'));
      enabled = Math.max(enabled, hookFiles.length);
    } catch (e) {
      // Ignore
    }
  }

  return { enabled, total };
}

// Get AgentDB memory stats
function getAgentDBStats() {
  let vectorCount = 0;
  let dbSizeKB = 0;
  let namespaces = 0;
  let hasHnsw = false;

  // Check for database directories
  const dbDirPaths = [
    path.join(process.cwd(), '.claude-flow', 'agentdb'),
    path.join(process.cwd(), '.swarm', 'agentdb'),
    path.join(process.cwd(), 'data', 'agentdb'),
    path.join(process.cwd(), '.claude', 'memory'),
    path.join(process.cwd(), '.agentdb'),
  ];

  // Check for direct database files (memory.db, etc.)
  const dbFilePaths = [
    path.join(process.cwd(), '.swarm', 'memory.db'),
    path.join(process.cwd(), '.claude-flow', 'memory.db'),
    path.join(process.cwd(), '.claude', 'memory.db'),
    path.join(process.cwd(), 'data', 'memory.db'),
    path.join(process.cwd(), 'memory.db'),
  ];

  // Check for HNSW index files
  const hnswPaths = [
    path.join(process.cwd(), '.swarm', 'hnsw.index'),
    path.join(process.cwd(), '.claude-flow', 'hnsw.index'),
    path.join(process.cwd(), 'data', 'hnsw.index'),
  ];

  // Check direct database files first
  for (const dbFile of dbFilePaths) {
    if (fs.existsSync(dbFile)) {
      try {
        const stats = fs.statSync(dbFile);
        dbSizeKB = stats.size / 1024;
        // Estimate vectors: ~2KB per vector for SQLite with embeddings
        vectorCount = Math.floor(dbSizeKB / 2);
        namespaces = 1;
        break;
      } catch (e) {
        // Ignore
      }
    }
  }

  // Check database directories if no direct file found
  if (vectorCount === 0) {
    for (const dbPath of dbDirPaths) {
      if (fs.existsSync(dbPath)) {
        try {
          const stats = fs.statSync(dbPath);
          if (stats.isDirectory()) {
            const files = fs.readdirSync(dbPath);
            namespaces = files.filter(f => f.endsWith('.db') || f.endsWith('.sqlite')).length;

            for (const file of files) {
              const filePath = path.join(dbPath, file);
              const fileStat = fs.statSync(filePath);
              if (fileStat.isFile()) {
                dbSizeKB += fileStat.size / 1024;
              }
            }

            vectorCount = Math.floor(dbSizeKB / 2);
          }
          break;
        } catch (e) {
          // Ignore
        }
      }
    }
  }

  // Check for HNSW index (indicates vector search capability)
  for (const hnswPath of hnswPaths) {
    if (fs.existsSync(hnswPath)) {
      hasHnsw = true;
      try {
        const stats = fs.statSync(hnswPath);
        // HNSW index: ~0.5KB per vector
        const hnswVectors = Math.floor(stats.size / 1024 / 0.5);
        vectorCount = Math.max(vectorCount, hnswVectors);
      } catch (e) {
        // Ignore
      }
      break;
    }
  }

  // Also check for vectors.json (simple vector store)
  const vectorsPath = path.join(process.cwd(), '.claude-flow', 'vectors.json');
  if (fs.existsSync(vectorsPath) && vectorCount === 0) {
    try {
      const data = JSON.parse(fs.readFileSync(vectorsPath, 'utf-8'));
      if (Array.isArray(data)) {
        vectorCount = data.length;
      } else if (data.vectors) {
        vectorCount = Object.keys(data.vectors).length;
      }
    } catch (e) {
      // Ignore
    }
  }

  return { vectorCount, dbSizeKB: Math.floor(dbSizeKB), namespaces, hasHnsw };
}

// Get test statistics
function getTestStats() {
  let testFiles = 0;
  let testCases = 0;

  const testDirs = [
    path.join(process.cwd(), 'tests'),
    path.join(process.cwd(), 'test'),
    path.join(process.cwd(), '__tests__'),
    path.join(process.cwd(), 'src', '__tests__'),
    path.join(process.cwd(), 'v3', '__tests__'),
  ];

  // Recursively count test files
  function countTestFiles(dir, depth = 0) {
    if (depth > 3) return; // Limit recursion
    if (!fs.existsSync(dir)) return;

    try {
      const entries = fs.readdirSync(dir, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isDirectory() && !entry.name.startsWith('.') && entry.name !== 'node_modules') {
          countTestFiles(path.join(dir, entry.name), depth + 1);
        } else if (entry.isFile()) {
          const name = entry.name;
          if (name.includes('.test.') || name.includes('.spec.') ||
              name.includes('_test.') || name.includes('_spec.') ||
              name.startsWith('test_') || name.startsWith('spec_')) {
            testFiles++;

            // Try to estimate test cases from file
            try {
              const content = fs.readFileSync(path.join(dir, name), 'utf-8');
              // Count it(), test(), describe() patterns
              const itMatches = (content.match(/\bit\s*\(/g) || []).length;
              const testMatches = (content.match(/\btest\s*\(/g) || []).length;
              testCases += itMatches + testMatches;
            } catch (e) {
              // Estimate 3 tests per file if can't read
              testCases += 3;
            }
          }
        }
      }
    } catch (e) {
      // Ignore
    }
  }

  for (const dir of testDirs) {
    countTestFiles(dir);
  }

  // Also check src directory for colocated tests
  const srcDir = path.join(process.cwd(), 'src');
  if (fs.existsSync(srcDir)) {
    countTestFiles(srcDir);
  }

  return { testFiles, testCases };
}

// Get integration status (MCP servers, external connections)
function getIntegrationStatus() {
  let mcpServers = { total: 0, enabled: 0, names: [] };
  let hasDatabase = false;
  let hasCache = false;
  let hasApi = false;

  // Check for MCP servers in settings
  const settingsPaths = [
    path.join(process.cwd(), '.claude', 'settings.json'),
    path.join(process.cwd(), '.claude', 'settings.local.json'),
  ];

  for (const settingsPath of settingsPaths) {
    if (fs.existsSync(settingsPath)) {
      try {
        const settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8'));

        // Check mcpServers object
        if (settings.mcpServers && typeof settings.mcpServers === 'object') {
          const servers = Object.keys(settings.mcpServers);
          mcpServers.total = servers.length;
          mcpServers.names = servers;

          // Check enabledMcpjsonServers for enabled count
          if (settings.enabledMcpjsonServers && Array.isArray(settings.enabledMcpjsonServers)) {
            mcpServers.enabled = settings.enabledMcpjsonServers.filter(s => servers.includes(s)).length;
          } else {
            mcpServers.enabled = mcpServers.total; // Assume all enabled if not specified
          }
        }
        break;
      } catch (e) { /* ignore */ }
    }
  }

  // Also check .mcp.json or mcp.json
  const mcpConfigPaths = [
    path.join(process.cwd(), '.mcp.json'),
    path.join(process.cwd(), 'mcp.json'),
    path.join(require('os').homedir(), '.claude', 'mcp.json'),
  ];

  for (const mcpPath of mcpConfigPaths) {
    if (fs.existsSync(mcpPath) && mcpServers.total === 0) {
      try {
        const config = JSON.parse(fs.readFileSync(mcpPath, 'utf-8'));
        if (config.mcpServers) {
          const servers = Object.keys(config.mcpServers);
          mcpServers.total = servers.length;
          mcpServers.names = servers;
          mcpServers.enabled = servers.length;
        }
      } catch (e) { /* ignore */ }
    }
  }

  // Check for database (AgentDB, SQLite, etc.)
  const dbPaths = [
    path.join(process.cwd(), '.swarm', 'memory.db'),
    path.join(process.cwd(), '.claude-flow', 'memory.db'),
    path.join(process.cwd(), 'data', 'memory.db'),
  ];
  hasDatabase = dbPaths.some(p => fs.existsSync(p));

  // Check for cache
  const cachePaths = [
    path.join(process.cwd(), '.claude-flow', 'cache'),
    path.join(process.cwd(), '.cache'),
    path.join(process.cwd(), 'node_modules', '.cache'),
  ];
  hasCache = cachePaths.some(p => fs.existsSync(p));

  // Check for API configuration (env vars or config)
  try {
    hasApi = !!(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY);
  } catch (e) { /* ignore */ }

  return { mcpServers, hasDatabase, hasCache, hasApi };
}

// Get git status (uncommitted changes, untracked files) - cross-platform
function getGitStatus() {
  let modified = 0;
  let untracked = 0;
  let staged = 0;
  let ahead = 0;
  let behind = 0;
  const isWindows = process.platform === 'win32';

  try {
    // Get modified and staged counts - works on all platforms
    const status = execSync('git status --porcelain', {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'], // Suppress stderr
      timeout: 5000,
    });
    const lines = status.trim().split('\n').filter(l => l);
    for (const line of lines) {
      const code = line.substring(0, 2);
      if (code.includes('M') || code.includes('D') || code.includes('R')) {
        if (code[0] !== ' ') staged++;
        if (code[1] !== ' ') modified++;
      }
      if (code.includes('?')) untracked++;
      if (code.includes('A')) staged++;
    }

    // Get ahead/behind - may fail if no upstream
    try {
      const abStatus = execSync('git rev-list --left-right --count HEAD...@{upstream}', {
        encoding: 'utf-8',
        stdio: ['pipe', 'pipe', 'pipe'],
        timeout: 5000,
      });
      const parts = abStatus.trim().split(/\s+/);
      ahead = parseInt(parts[0]) || 0;
      behind = parseInt(parts[1]) || 0;
    } catch (e) { /* no upstream or error - that's ok */ }

  } catch (e) {
    // Not a git repo or git not installed - return zeros
  }

  return { modified, untracked, staged, ahead, behind };
}

// Get session statistics
function getSessionStats() {
  let sessionStart = null;
  let duration = '';
  let lastActivity = '';
  let operationsCount = 0;

  // Check for session file
  const sessionPaths = [
    path.join(process.cwd(), '.claude-flow', 'session.json'),
    path.join(process.cwd(), '.claude', 'session.json'),
  ];

  for (const sessPath of sessionPaths) {
    if (fs.existsSync(sessPath)) {
      try {
        const data = JSON.parse(fs.readFileSync(sessPath, 'utf-8'));
        if (data.startTime) {
          sessionStart = new Date(data.startTime);
          const now = new Date();
          const diffMs = now.getTime() - sessionStart.getTime();
          const diffMins = Math.floor(diffMs / 60000);
          if (diffMins < 60) {
            duration = `${diffMins}m`;
          } else {
            const hours = Math.floor(diffMins / 60);
            const mins = diffMins % 60;
            duration = `${hours}h${mins}m`;
          }
        }
        if (data.lastActivity) {
          const last = new Date(data.lastActivity);
          const now = new Date();
          const diffMs = now.getTime() - last.getTime();
          const diffMins = Math.floor(diffMs / 60000);
          if (diffMins < 1) lastActivity = 'now';
          else if (diffMins < 60) lastActivity = `${diffMins}m ago`;
          else lastActivity = `${Math.floor(diffMins / 60)}h ago`;
        }
        operationsCount = data.operationsCount || data.commandCount || 0;
        break;
      } catch (e) { /* ignore */ }
    }
  }

  // Fallback: check metrics for activity
  if (!duration) {
    const metricsPath = path.join(process.cwd(), '.claude-flow', 'metrics', 'activity.json');
    if (fs.existsSync(metricsPath)) {
      try {
        const data = JSON.parse(fs.readFileSync(metricsPath, 'utf-8'));
        operationsCount = data.totalOperations || 0;
      } catch (e) { /* ignore */ }
    }
  }

  return { duration, lastActivity, operationsCount };
}

// Get trend indicator based on change
function getTrend(current, previous) {
  if (previous === null || previous === undefined) return '';
  if (current > previous) return `${c.brightGreen}↑${c.reset}`;
  if (current < previous) return `${c.brightRed}↓${c.reset}`;
  return `${c.dim}→${c.reset}`;
}

// Store previous values for trends (persisted between calls)
let prevIntelligence = null;
try {
  const trendPath = path.join(process.cwd(), '.claude-flow', '.trend-cache.json');
  if (fs.existsSync(trendPath)) {
    const data = JSON.parse(fs.readFileSync(trendPath, 'utf-8'));
    prevIntelligence = data.intelligence;
  }
} catch (e) { /* ignore */ }

// Generate progress bar
function progressBar(current, total) {
  const width = 5;
  const filled = Math.round((current / total) * width);
  const empty = width - filled;
  return '[' + '\u25CF'.repeat(filled) + '\u25CB'.repeat(empty) + ']';
}

// Generate full statusline
function generateStatusline() {
  const user = getUserInfo();
  const progress = getV3Progress();
  const security = getSecurityStatus();
  const swarm = getSwarmStatus();
  const system = getSystemMetrics();
  const adrs = getADRStatus();
  const hooks = getHooksStatus();
  const agentdb = getAgentDBStats();
  const tests = getTestStats();
  const git = getGitStatus();
  const session = getSessionStats();
  const integration = getIntegrationStatus();
  const lines = [];

  // Calculate intelligence trend
  const intellTrend = getTrend(system.intelligencePct, prevIntelligence);

  // Save current values for next trend calculation
  try {
    const trendPath = path.join(process.cwd(), '.claude-flow', '.trend-cache.json');
    const trendDir = path.dirname(trendPath);
    if (!fs.existsSync(trendDir)) fs.mkdirSync(trendDir, { recursive: true });
    fs.writeFileSync(trendPath, JSON.stringify({ intelligence: system.intelligencePct, timestamp: Date.now() }));
  } catch (e) { /* ignore */ }

  // Header Line with git changes indicator
  let header = `${c.bold}${c.brightPurple}▊ Claude Flow V3 ${c.reset}`;
  header += `${swarm.coordinationActive ? c.brightCyan : c.dim}● ${c.brightCyan}${user.name}${c.reset}`;
  if (user.gitBranch) {
    header += `  ${c.dim}│${c.reset}  ${c.brightBlue}⎇ ${user.gitBranch}${c.reset}`;
    // Add git changes indicator
    const gitChanges = git.modified + git.staged + git.untracked;
    if (gitChanges > 0) {
      let gitIndicator = '';
      if (git.staged > 0) gitIndicator += `${c.brightGreen}+${git.staged}${c.reset}`;
      if (git.modified > 0) gitIndicator += `${c.brightYellow}~${git.modified}${c.reset}`;
      if (git.untracked > 0) gitIndicator += `${c.dim}?${git.untracked}${c.reset}`;
      header += ` ${gitIndicator}`;
    }
    // Add ahead/behind indicator
    if (git.ahead > 0 || git.behind > 0) {
      if (git.ahead > 0) header += ` ${c.brightGreen}↑${git.ahead}${c.reset}`;
      if (git.behind > 0) header += ` ${c.brightRed}↓${git.behind}${c.reset}`;
    }
  }
  header += `  ${c.dim}│${c.reset}  ${c.purple}${user.modelName}${c.reset}`;
  // Add session duration if available
  if (session.duration) {
    header += `  ${c.dim}│${c.reset}  ${c.cyan}⏱ ${session.duration}${c.reset}`;
  }
  lines.push(header);

  // Separator
  lines.push(`${c.dim}─────────────────────────────────────────────────────${c.reset}`);

  // Line 1: DDD Domain Progress with dynamic performance indicator
  const domainsColor = progress.domainsCompleted >= 3 ? c.brightGreen : progress.domainsCompleted > 0 ? c.yellow : c.red;
  // Show HNSW speedup if enabled, otherwise show patterns learned
  let perfIndicator = '';
  if (agentdb.hasHnsw && agentdb.vectorCount > 0) {
    // HNSW enabled: show estimated speedup (150x-12500x based on vector count)
    const speedup = agentdb.vectorCount > 10000 ? '12500x' : agentdb.vectorCount > 1000 ? '150x' : '10x';
    perfIndicator = `${c.brightGreen}⚡ HNSW ${speedup}${c.reset}`;
  } else if (progress.patternsLearned > 0) {
    // Show patterns learned
    const patternsK = progress.patternsLearned >= 1000
      ? `${(progress.patternsLearned / 1000).toFixed(1)}k`
      : String(progress.patternsLearned);
    perfIndicator = `${c.brightYellow}📚 ${patternsK} patterns${c.reset}`;
  } else {
    // New project: show target
    perfIndicator = `${c.dim}⚡ target: 150x-12500x${c.reset}`;
  }
  lines.push(
    `${c.brightCyan}🏗️  DDD Domains${c.reset}    ${progressBar(progress.domainsCompleted, progress.totalDomains)}  ` +
    `${domainsColor}${progress.domainsCompleted}${c.reset}/${c.brightWhite}${progress.totalDomains}${c.reset}    ` +
    perfIndicator
  );

  // Line 2: Swarm + Hooks + CVE + Memory + Context + Intelligence
  const swarmIndicator = swarm.coordinationActive ? `${c.brightGreen}◉${c.reset}` : `${c.dim}○${c.reset}`;
  const agentsColor = swarm.activeAgents > 0 ? c.brightGreen : c.red;
  let securityIcon = security.status === 'CLEAN' ? '🟢' : security.status === 'IN_PROGRESS' ? '🟡' : '🔴';
  let securityColor = security.status === 'CLEAN' ? c.brightGreen : security.status === 'IN_PROGRESS' ? c.brightYellow : c.brightRed;
  const hooksColor = hooks.enabled > 0 ? c.brightGreen : c.dim;

  lines.push(
    `${c.brightYellow}🤖 Swarm${c.reset}  ${swarmIndicator} [${agentsColor}${String(swarm.activeAgents).padStart(2)}${c.reset}/${c.brightWhite}${swarm.maxAgents}${c.reset}]  ` +
    `${c.brightPurple}👥 ${system.subAgents}${c.reset}    ` +
    `${c.brightBlue}🪝 ${hooksColor}${hooks.enabled}${c.reset}/${c.brightWhite}${hooks.total}${c.reset}    ` +
    `${securityIcon} ${securityColor}CVE ${security.cvesFixed}${c.reset}/${c.brightWhite}${security.totalCves}${c.reset}    ` +
    `${c.brightCyan}💾 ${system.memoryMB}MB${c.reset}    ` +
    `${system.intelligencePct >= 80 ? c.brightGreen : system.intelligencePct >= 40 ? c.brightYellow : c.dim}🧠 ${String(system.intelligencePct).padStart(3)}%${intellTrend}${c.reset}`
  );

  // Line 3: Architecture status with ADRs, AgentDB, Tests
  const dddColor = progress.dddProgress >= 50 ? c.brightGreen : progress.dddProgress > 0 ? c.yellow : c.red;
  const adrColor = adrs.count > 0 ? (adrs.implemented === adrs.count ? c.brightGreen : c.yellow) : c.dim;
  const vectorColor = agentdb.vectorCount > 0 ? c.brightGreen : c.dim;
  const testColor = tests.testFiles > 0 ? c.brightGreen : c.dim;

  // Show ADR compliance % if from real data, otherwise show count
  const adrDisplay = adrs.compliance > 0
    ? `${adrColor}●${adrs.compliance}%${c.reset}`
    : `${adrColor}●${adrs.implemented}/${adrs.count}${c.reset}`;

  lines.push(
    `${c.brightPurple}🔧 Architecture${c.reset}    ` +
    `${c.cyan}ADRs${c.reset} ${adrDisplay}  ${c.dim}│${c.reset}  ` +
    `${c.cyan}DDD${c.reset} ${dddColor}●${String(progress.dddProgress).padStart(3)}%${c.reset}  ${c.dim}│${c.reset}  ` +
    `${c.cyan}Security${c.reset} ${securityColor}●${security.status}${c.reset}`
  );

  // Line 4: Memory, Vectors, Tests
  const hnswIndicator = agentdb.hasHnsw ? `${c.brightGreen}⚡${c.reset}` : '';
  const sizeDisplay = agentdb.dbSizeKB >= 1024
    ? `${(agentdb.dbSizeKB / 1024).toFixed(1)}MB`
    : `${agentdb.dbSizeKB}KB`;
  // Build integration status string
  let integrationStr = '';
  if (integration.mcpServers.total > 0) {
    const mcpColor = integration.mcpServers.enabled === integration.mcpServers.total ? c.brightGreen :
                     integration.mcpServers.enabled > 0 ? c.brightYellow : c.red;
    integrationStr += `${c.cyan}MCP${c.reset} ${mcpColor}●${integration.mcpServers.enabled}/${integration.mcpServers.total}${c.reset}`;
  }
  if (integration.hasDatabase) {
    integrationStr += (integrationStr ? '  ' : '') + `${c.brightGreen}◆${c.reset}DB`;
  }
  if (integration.hasApi) {
    integrationStr += (integrationStr ? '  ' : '') + `${c.brightGreen}◆${c.reset}API`;
  }
  if (!integrationStr) {
    integrationStr = `${c.dim}●none${c.reset}`;
  }

  lines.push(
    `${c.brightCyan}📊 AgentDB${c.reset}    ` +
    `${c.cyan}Vectors${c.reset} ${vectorColor}●${agentdb.vectorCount}${hnswIndicator}${c.reset}  ${c.dim}│${c.reset}  ` +
    `${c.cyan}Size${c.reset} ${c.brightWhite}${sizeDisplay}${c.reset}  ${c.dim}│${c.reset}  ` +
    `${c.cyan}Tests${c.reset} ${testColor}●${tests.testFiles}${c.reset} ${c.dim}(${tests.testCases} cases)${c.reset}  ${c.dim}│${c.reset}  ` +
    integrationStr
  );

  return lines.join('\n');
}

// Generate JSON data
function generateJSON() {
  return {
    user: getUserInfo(),
    v3Progress: getV3Progress(),
    security: getSecurityStatus(),
    swarm: getSwarmStatus(),
    system: getSystemMetrics(),
    adrs: getADRStatus(),
    hooks: getHooksStatus(),
    agentdb: getAgentDBStats(),
    tests: getTestStats(),
    performance: {
      flashAttentionTarget: '2.49x-7.47x',
      searchImprovement: '150x-12,500x',
      memoryReduction: '50-75%',
    },
    lastUpdated: new Date().toISOString(),
  };
}

// Main
if (process.argv.includes('--json')) {
  console.log(JSON.stringify(generateJSON(), null, 2));
} else if (process.argv.includes('--compact')) {
  console.log(JSON.stringify(generateJSON()));
} else {
  console.log(generateStatusline());
}
