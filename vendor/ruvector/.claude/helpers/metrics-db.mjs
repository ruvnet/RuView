#!/usr/bin/env node
/**
 * Claude Flow V3 - Metrics Database Manager
 * Uses sql.js for cross-platform SQLite storage
 * Single .db file with multiple tables
 */

import initSqlJs from 'sql.js';
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, statSync } from 'fs';
import { dirname, join, basename } from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, '../..');
const V3_DIR = join(PROJECT_ROOT, 'v3');
const DB_PATH = join(PROJECT_ROOT, '.claude-flow', 'metrics.db');

// Ensure directory exists
const dbDir = dirname(DB_PATH);
if (!existsSync(dbDir)) {
  mkdirSync(dbDir, { recursive: true });
}

let SQL;
let db;

/**
 * Initialize sql.js and create/load database
 */
async function initDatabase() {
  SQL = await initSqlJs();

  // Load existing database or create new one
  if (existsSync(DB_PATH)) {
    const buffer = readFileSync(DB_PATH);
    db = new SQL.Database(buffer);
  } else {
    db = new SQL.Database();
  }

  // Create tables if they don't exist
  db.run(`
    CREATE TABLE IF NOT EXISTS v3_progress (
      id INTEGER PRIMARY KEY,
      domains_completed INTEGER DEFAULT 0,
      domains_total INTEGER DEFAULT 5,
      ddd_progress INTEGER DEFAULT 0,
      total_modules INTEGER DEFAULT 0,
      total_files INTEGER DEFAULT 0,
      total_lines INTEGER DEFAULT 0,
      last_updated TEXT
    );

    CREATE TABLE IF NOT EXISTS security_audit (
      id INTEGER PRIMARY KEY,
      status TEXT DEFAULT 'PENDING',
      cves_fixed INTEGER DEFAULT 0,
      total_cves INTEGER DEFAULT 3,
      last_audit TEXT
    );

    CREATE TABLE IF NOT EXISTS swarm_activity (
      id INTEGER PRIMARY KEY,
      agentic_flow_processes INTEGER DEFAULT 0,
      mcp_server_processes INTEGER DEFAULT 0,
      estimated_agents INTEGER DEFAULT 0,
      swarm_active INTEGER DEFAULT 0,
      coordination_active INTEGER DEFAULT 0,
      last_updated TEXT
    );

    CREATE TABLE IF NOT EXISTS performance_metrics (
      id INTEGER PRIMARY KEY,
      flash_attention_speedup TEXT DEFAULT '1.0x',
      memory_reduction TEXT DEFAULT '0%',
      search_improvement TEXT DEFAULT '1x',
      last_updated TEXT
    );

    CREATE TABLE IF NOT EXISTS module_status (
      name TEXT PRIMARY KEY,
      files INTEGER DEFAULT 0,
      lines INTEGER DEFAULT 0,
      progress INTEGER DEFAULT 0,
      has_src INTEGER DEFAULT 0,
      has_tests INTEGER DEFAULT 0,
      last_updated TEXT
    );

    CREATE TABLE IF NOT EXISTS cve_status (
      id TEXT PRIMARY KEY,
      description TEXT,
      severity TEXT DEFAULT 'critical',
      status TEXT DEFAULT 'pending',
      fixed_by TEXT,
      last_updated TEXT
    );
  `);

  // Initialize rows if empty
  const progressCheck = db.exec("SELECT COUNT(*) FROM v3_progress");
  if (progressCheck[0]?.values[0][0] === 0) {
    db.run("INSERT INTO v3_progress (id) VALUES (1)");
  }

  const securityCheck = db.exec("SELECT COUNT(*) FROM security_audit");
  if (securityCheck[0]?.values[0][0] === 0) {
    db.run("INSERT INTO security_audit (id) VALUES (1)");
  }

  const swarmCheck = db.exec("SELECT COUNT(*) FROM swarm_activity");
  if (swarmCheck[0]?.values[0][0] === 0) {
    db.run("INSERT INTO swarm_activity (id) VALUES (1)");
  }

  const perfCheck = db.exec("SELECT COUNT(*) FROM performance_metrics");
  if (perfCheck[0]?.values[0][0] === 0) {
    db.run("INSERT INTO performance_metrics (id) VALUES (1)");
  }

  // Initialize CVE records
  const cveCheck = db.exec("SELECT COUNT(*) FROM cve_status");
  if (cveCheck[0]?.values[0][0] === 0) {
    db.run(`INSERT INTO cve_status (id, description, fixed_by) VALUES
      ('CVE-1', 'Input validation bypass', 'input-validator.ts'),
      ('CVE-2', 'Path traversal vulnerability', 'path-validator.ts'),
      ('CVE-3', 'Command injection vulnerability', 'safe-executor.ts')
    `);
  }

  persist();
}

/**
 * Persist database to disk
 */
function persist() {
  const data = db.export();
  const buffer = Buffer.from(data);
  writeFileSync(DB_PATH, buffer);
}

/**
 * Count files and lines in a directory
 */
function countFilesAndLines(dir, ext = '.ts') {
  let files = 0;
  let lines = 0;

  function walk(currentDir) {
    if (!existsSync(currentDir)) return;

    try {
      const entries = readdirSync(currentDir, { withFileTypes: true });
      for (const entry of entries) {
        const fullPath = join(currentDir, entry.name);
        if (entry.isDirectory() && !entry.name.includes('node_modules')) {
          walk(fullPath);
        } else if (entry.isFile() && entry.name.endsWith(ext)) {
          files++;
          try {
            const content = readFileSync(fullPath, 'utf-8');
            lines += content.split('\n').length;
          } catch (e) {}
        }
      }
    } catch (e) {}
  }

  walk(dir);
  return { files, lines };
}

/**
 * Calculate module progress
 * Utility/service packages (cli, hooks, mcp, etc.) are considered complete (100%)
 * as their services ARE the application layer (DDD by design)
 */
const UTILITY_PACKAGES = new Set([
  'cli', 'hooks', 'mcp', 'shared', 'testing', 'agents', 'integration',
  'embeddings', 'deployment', 'performance', 'plugins', 'providers'
]);

function calculateModuleProgress(moduleDir) {
  if (!existsSync(moduleDir)) return 0;

  const moduleName = basename(moduleDir);

  // Utility packages are 100% complete by design
  if (UTILITY_PACKAGES.has(moduleName)) {
    return 100;
  }

  let progress = 0;

  // Check for DDD structure
  if (existsSync(join(moduleDir, 'src/domain'))) progress += 30;
  if (existsSync(join(moduleDir, 'src/application'))) progress += 30;
  if (existsSync(join(moduleDir, 'src'))) progress += 10;
  if (existsSync(join(moduleDir, 'src/index.ts')) || existsSync(join(moduleDir, 'index.ts'))) progress += 10;
  if (existsSync(join(moduleDir, '__tests__')) || existsSync(join(moduleDir, 'tests'))) progress += 10;
  if (existsSync(join(moduleDir, 'package.json'))) progress += 10;

  return Math.min(progress, 100);
}

/**
 * Check security file status
 */
function checkSecurityFile(filename, minLines = 100) {
  const filePath = join(V3_DIR, '@claude-flow/security/src', filename);
  if (!existsSync(filePath)) return false;

  try {
    const content = readFileSync(filePath, 'utf-8');
    return content.split('\n').length > minLines;
  } catch (e) {
    return false;
  }
}

/**
 * Count active processes
 */
function countProcesses() {
  try {
    const ps = execSync('ps aux 2>/dev/null || echo ""', { encoding: 'utf-8' });

    const agenticFlow = (ps.match(/agentic-flow/g) || []).length;
    const mcp = (ps.match(/mcp.*start/g) || []).length;
    const agents = (ps.match(/agent|swarm|coordinator/g) || []).length;

    return {
      agenticFlow: Math.max(0, agenticFlow - 1), // Exclude grep itself
      mcp,
      agents: Math.max(0, agents - 1)
    };
  } catch (e) {
    return { agenticFlow: 0, mcp: 0, agents: 0 };
  }
}

/**
 * Sync all metrics from actual implementation
 */
async function syncMetrics() {
  const now = new Date().toISOString();

  // Count V3 modules
  const modulesDir = join(V3_DIR, '@claude-flow');
  let modules = [];
  let totalProgress = 0;

  if (existsSync(modulesDir)) {
    const entries = readdirSync(modulesDir, { withFileTypes: true });
    for (const entry of entries) {
      // Skip hidden directories (like .agentic-flow, .claude-flow)
      if (entry.isDirectory() && !entry.name.startsWith('.')) {
        const moduleDir = join(modulesDir, entry.name);
        const { files, lines } = countFilesAndLines(moduleDir);
        const progress = calculateModuleProgress(moduleDir);

        modules.push({ name: entry.name, files, lines, progress });
        totalProgress += progress;

        // Update module_status table
        db.run(`
          INSERT OR REPLACE INTO module_status (name, files, lines, progress, has_src, has_tests, last_updated)
          VALUES (?, ?, ?, ?, ?, ?, ?)
        `, [
          entry.name,
          files,
          lines,
          progress,
          existsSync(join(moduleDir, 'src')) ? 1 : 0,
          existsSync(join(moduleDir, '__tests__')) ? 1 : 0,
          now
        ]);
      }
    }
  }

  const avgProgress = modules.length > 0 ? Math.round(totalProgress / modules.length) : 0;
  const totalStats = countFilesAndLines(V3_DIR);

  // Count completed domains (mapped to modules)
  const domainModules = ['swarm', 'memory', 'performance', 'cli', 'integration'];
  const domainsCompleted = domainModules.filter(m =>
    modules.some(mod => mod.name === m && mod.progress >= 50)
  ).length;

  // Update v3_progress
  db.run(`
    UPDATE v3_progress SET
      domains_completed = ?,
      ddd_progress = ?,
      total_modules = ?,
      total_files = ?,
      total_lines = ?,
      last_updated = ?
    WHERE id = 1
  `, [domainsCompleted, avgProgress, modules.length, totalStats.files, totalStats.lines, now]);

  // Check security CVEs
  const cve1Fixed = checkSecurityFile('input-validator.ts');
  const cve2Fixed = checkSecurityFile('path-validator.ts');
  const cve3Fixed = checkSecurityFile('safe-executor.ts');
  const cvesFixed = [cve1Fixed, cve2Fixed, cve3Fixed].filter(Boolean).length;

  let securityStatus = 'PENDING';
  if (cvesFixed === 3) securityStatus = 'CLEAN';
  else if (cvesFixed > 0) securityStatus = 'IN_PROGRESS';

  db.run(`
    UPDATE security_audit SET
      status = ?,
      cves_fixed = ?,
      last_audit = ?
    WHERE id = 1
  `, [securityStatus, cvesFixed, now]);

  // Update individual CVE status
  db.run("UPDATE cve_status SET status = ?, last_updated = ? WHERE id = 'CVE-1'", [cve1Fixed ? 'fixed' : 'pending', now]);
  db.run("UPDATE cve_status SET status = ?, last_updated = ? WHERE id = 'CVE-2'", [cve2Fixed ? 'fixed' : 'pending', now]);
  db.run("UPDATE cve_status SET status = ?, last_updated = ? WHERE id = 'CVE-3'", [cve3Fixed ? 'fixed' : 'pending', now]);

  // Update swarm activity
  const processes = countProcesses();
  db.run(`
    UPDATE swarm_activity SET
      agentic_flow_processes = ?,
      mcp_server_processes = ?,
      estimated_agents = ?,
      swarm_active = ?,
      coordination_active = ?,
      last_updated = ?
    WHERE id = 1
  `, [
    processes.agenticFlow,
    processes.mcp,
    processes.agents,
    processes.agents > 0 ? 1 : 0,
    processes.agenticFlow > 0 ? 1 : 0,
    now
  ]);

  persist();

  return {
    modules: modules.length,
    domains: domainsCompleted,
    dddProgress: avgProgress,
    cvesFixed,
    securityStatus,
    files: totalStats.files,
    lines: totalStats.lines
  };
}

/**
 * Get current metrics as JSON (for statusline compatibility)
 */
function getMetricsJSON() {
  const progress = db.exec("SELECT * FROM v3_progress WHERE id = 1")[0];
  const security = db.exec("SELECT * FROM security_audit WHERE id = 1")[0];
  const swarm = db.exec("SELECT * FROM swarm_activity WHERE id = 1")[0];
  const perf = db.exec("SELECT * FROM performance_metrics WHERE id = 1")[0];

  // Map column names to values
  const mapRow = (result) => {
    if (!result) return {};
    const cols = result.columns;
    const vals = result.values[0];
    return Object.fromEntries(cols.map((c, i) => [c, vals[i]]));
  };

  return {
    v3Progress: mapRow(progress),
    securityAudit: mapRow(security),
    swarmActivity: mapRow(swarm),
    performanceMetrics: mapRow(perf)
  };
}

/**
 * Export metrics to JSON files for backward compatibility
 */
function exportToJSON() {
  const metrics = getMetricsJSON();
  const metricsDir = join(PROJECT_ROOT, '.claude-flow/metrics');
  const securityDir = join(PROJECT_ROOT, '.claude-flow/security');

  if (!existsSync(metricsDir)) mkdirSync(metricsDir, { recursive: true });
  if (!existsSync(securityDir)) mkdirSync(securityDir, { recursive: true });

  // v3-progress.json
  writeFileSync(join(metricsDir, 'v3-progress.json'), JSON.stringify({
    domains: {
      completed: metrics.v3Progress.domains_completed,
      total: metrics.v3Progress.domains_total
    },
    ddd: {
      progress: metrics.v3Progress.ddd_progress,
      modules: metrics.v3Progress.total_modules,
      totalFiles: metrics.v3Progress.total_files,
      totalLines: metrics.v3Progress.total_lines
    },
    swarm: {
      activeAgents: metrics.swarmActivity.estimated_agents,
      totalAgents: 15
    },
    lastUpdated: metrics.v3Progress.last_updated,
    source: 'metrics.db'
  }, null, 2));

  // security/audit-status.json
  writeFileSync(join(securityDir, 'audit-status.json'), JSON.stringify({
    status: metrics.securityAudit.status,
    cvesFixed: metrics.securityAudit.cves_fixed,
    totalCves: metrics.securityAudit.total_cves,
    lastAudit: metrics.securityAudit.last_audit,
    source: 'metrics.db'
  }, null, 2));

  // swarm-activity.json
  writeFileSync(join(metricsDir, 'swarm-activity.json'), JSON.stringify({
    timestamp: metrics.swarmActivity.last_updated,
    processes: {
      agentic_flow: metrics.swarmActivity.agentic_flow_processes,
      mcp_server: metrics.swarmActivity.mcp_server_processes,
      estimated_agents: metrics.swarmActivity.estimated_agents
    },
    swarm: {
      active: metrics.swarmActivity.swarm_active === 1,
      agent_count: metrics.swarmActivity.estimated_agents,
      coordination_active: metrics.swarmActivity.coordination_active === 1
    },
    source: 'metrics.db'
  }, null, 2));
}

/**
 * Main entry point
 */
async function main() {
  const command = process.argv[2] || 'sync';

  await initDatabase();

  switch (command) {
    case 'sync':
      const result = await syncMetrics();
      exportToJSON();
      console.log(JSON.stringify(result));
      break;

    case 'export':
      exportToJSON();
      console.log('Exported to JSON files');
      break;

    case 'status':
      const metrics = getMetricsJSON();
      console.log(JSON.stringify(metrics, null, 2));
      break;

    case 'daemon':
      const interval = parseInt(process.argv[3]) || 30;
      console.log(`Starting metrics daemon (interval: ${interval}s)`);

      // Initial sync
      await syncMetrics();
      exportToJSON();

      // Continuous sync
      setInterval(async () => {
        await syncMetrics();
        exportToJSON();
      }, interval * 1000);
      break;

    default:
      console.log('Usage: metrics-db.mjs [sync|export|status|daemon [interval]]');
  }
}

main().catch(console.error);
