#!/usr/bin/env node
/**
 * Intelligence Layer (ADR-050)
 *
 * Closes the intelligence loop by wiring PageRank-ranked memory into
 * the hook system. Pure CJS — no ESM imports of @claude-flow/memory.
 *
 * Data files (all under .claude-flow/data/):
 *   auto-memory-store.json  — written by auto-memory-hook.mjs
 *   graph-state.json        — serialized graph (nodes + edges + pageRanks)
 *   ranked-context.json     — pre-computed ranked entries for fast lookup
 *   pending-insights.jsonl  — append-only edit/task log
 */

'use strict';

const fs = require('fs');
const path = require('path');

const DATA_DIR = path.join(process.cwd(), '.claude-flow', 'data');
const STORE_PATH = path.join(DATA_DIR, 'auto-memory-store.json');
const GRAPH_PATH = path.join(DATA_DIR, 'graph-state.json');
const RANKED_PATH = path.join(DATA_DIR, 'ranked-context.json');
const PENDING_PATH = path.join(DATA_DIR, 'pending-insights.jsonl');
const SESSION_DIR = path.join(process.cwd(), '.claude-flow', 'sessions');
const SESSION_FILE = path.join(SESSION_DIR, 'current.json');

// ── Stop words for trigram matching ──────────────────────────────────────────

const STOP_WORDS = new Set([
  'the', 'a', 'an', 'is', 'are', 'was', 'were', 'be', 'been', 'being',
  'have', 'has', 'had', 'do', 'does', 'did', 'will', 'would', 'could',
  'should', 'may', 'might', 'shall', 'can', 'to', 'of', 'in', 'for',
  'on', 'with', 'at', 'by', 'from', 'as', 'into', 'through', 'during',
  'before', 'after', 'and', 'but', 'or', 'nor', 'not', 'so', 'yet',
  'both', 'either', 'neither', 'each', 'every', 'all', 'any', 'few',
  'more', 'most', 'other', 'some', 'such', 'no', 'only', 'own', 'same',
  'than', 'too', 'very', 'just', 'because', 'if', 'when', 'which',
  'who', 'whom', 'this', 'that', 'these', 'those', 'it', 'its',
]);

// ── Helpers ──────────────────────────────────────────────────────────────────

function ensureDataDir() {
  if (!fs.existsSync(DATA_DIR)) fs.mkdirSync(DATA_DIR, { recursive: true });
}

function readJSON(filePath) {
  try {
    if (fs.existsSync(filePath)) return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch { /* corrupt file — start fresh */ }
  return null;
}

function writeJSON(filePath, data) {
  ensureDataDir();
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf-8');
}

function tokenize(text) {
  if (!text) return [];
  return text.toLowerCase()
    .replace(/[^a-z0-9\s-]/g, ' ')
    .split(/\s+/)
    .filter(w => w.length > 2 && !STOP_WORDS.has(w));
}

function trigrams(words) {
  const t = new Set();
  for (const w of words) {
    for (let i = 0; i <= w.length - 3; i++) t.add(w.slice(i, i + 3));
  }
  return t;
}

function jaccardSimilarity(setA, setB) {
  if (setA.size === 0 && setB.size === 0) return 0;
  let intersection = 0;
  for (const item of setA) { if (setB.has(item)) intersection++; }
  return intersection / (setA.size + setB.size - intersection);
}

// ── Session state helpers ────────────────────────────────────────────────────

function sessionGet(key) {
  try {
    if (!fs.existsSync(SESSION_FILE)) return null;
    const session = JSON.parse(fs.readFileSync(SESSION_FILE, 'utf-8'));
    return key ? (session.context || {})[key] : session.context;
  } catch { return null; }
}

function sessionSet(key, value) {
  try {
    if (!fs.existsSync(SESSION_DIR)) fs.mkdirSync(SESSION_DIR, { recursive: true });
    let session = {};
    if (fs.existsSync(SESSION_FILE)) {
      session = JSON.parse(fs.readFileSync(SESSION_FILE, 'utf-8'));
    }
    if (!session.context) session.context = {};
    session.context[key] = value;
    session.updatedAt = new Date().toISOString();
    fs.writeFileSync(SESSION_FILE, JSON.stringify(session, null, 2), 'utf-8');
  } catch { /* best effort */ }
}

// ── PageRank ─────────────────────────────────────────────────────────────────

function computePageRank(nodes, edges, damping, maxIter) {
  damping = damping || 0.85;
  maxIter = maxIter || 30;

  const ids = Object.keys(nodes);
  const n = ids.length;
  if (n === 0) return {};

  // Build adjacency: outgoing edges per node
  const outLinks = {};
  const inLinks = {};
  for (const id of ids) { outLinks[id] = []; inLinks[id] = []; }
  for (const edge of edges) {
    if (outLinks[edge.sourceId]) outLinks[edge.sourceId].push(edge.targetId);
    if (inLinks[edge.targetId]) inLinks[edge.targetId].push(edge.sourceId);
  }

  // Initialize ranks
  const ranks = {};
  for (const id of ids) ranks[id] = 1 / n;

  // Power iteration (with dangling node redistribution)
  for (let iter = 0; iter < maxIter; iter++) {
    const newRanks = {};
    let diff = 0;

    // Collect rank from dangling nodes (no outgoing edges)
    let danglingSum = 0;
    for (const id of ids) {
      if (outLinks[id].length === 0) danglingSum += ranks[id];
    }

    for (const id of ids) {
      let sum = 0;
      for (const src of inLinks[id]) {
        const outCount = outLinks[src].length;
        if (outCount > 0) sum += ranks[src] / outCount;
      }
      // Dangling rank distributed evenly + teleport
      newRanks[id] = (1 - damping) / n + damping * (sum + danglingSum / n);
      diff += Math.abs(newRanks[id] - ranks[id]);
    }

    for (const id of ids) ranks[id] = newRanks[id];
    if (diff < 1e-6) break; // converged
  }

  return ranks;
}

// ── Edge building ────────────────────────────────────────────────────────────

function buildEdges(entries) {
  const edges = [];
  const byCategory = {};

  for (const entry of entries) {
    const cat = entry.category || entry.namespace || 'default';
    if (!byCategory[cat]) byCategory[cat] = [];
    byCategory[cat].push(entry);
  }

  // Temporal edges: entries from same sourceFile
  const byFile = {};
  for (const entry of entries) {
    const file = (entry.metadata && entry.metadata.sourceFile) || null;
    if (file) {
      if (!byFile[file]) byFile[file] = [];
      byFile[file].push(entry);
    }
  }
  for (const file of Object.keys(byFile)) {
    const group = byFile[file];
    for (let i = 0; i < group.length - 1; i++) {
      edges.push({
        sourceId: group[i].id,
        targetId: group[i + 1].id,
        type: 'temporal',
        weight: 0.5,
      });
    }
  }

  // Similarity edges within categories (Jaccard > 0.3)
  for (const cat of Object.keys(byCategory)) {
    const group = byCategory[cat];
    for (let i = 0; i < group.length; i++) {
      const triA = trigrams(tokenize(group[i].content || group[i].summary || ''));
      for (let j = i + 1; j < group.length; j++) {
        const triB = trigrams(tokenize(group[j].content || group[j].summary || ''));
        const sim = jaccardSimilarity(triA, triB);
        if (sim > 0.3) {
          edges.push({
            sourceId: group[i].id,
            targetId: group[j].id,
            type: 'similar',
            weight: sim,
          });
        }
      }
    }
  }

  return edges;
}

// ── Bootstrap from MEMORY.md files ───────────────────────────────────────────

/**
 * If auto-memory-store.json is empty, bootstrap by parsing MEMORY.md and
 * topic files from the auto-memory directory. This removes the dependency
 * on @claude-flow/memory for the initial seed.
 */
function bootstrapFromMemoryFiles() {
  const entries = [];
  const cwd = process.cwd();

  // Search for auto-memory directories
  const candidates = [
    // Claude Code auto-memory (project-scoped)
    path.join(require('os').homedir(), '.claude', 'projects'),
    // Local project memory
    path.join(cwd, '.claude-flow', 'memory'),
    path.join(cwd, '.claude', 'memory'),
  ];

  // Find MEMORY.md in project-scoped dirs
  for (const base of candidates) {
    if (!fs.existsSync(base)) continue;

    // For the projects dir, scan subdirectories for memory/
    if (base.endsWith('projects')) {
      try {
        const projectDirs = fs.readdirSync(base);
        for (const pdir of projectDirs) {
          const memDir = path.join(base, pdir, 'memory');
          if (fs.existsSync(memDir)) {
            parseMemoryDir(memDir, entries);
          }
        }
      } catch { /* skip */ }
    } else if (fs.existsSync(base)) {
      parseMemoryDir(base, entries);
    }
  }

  return entries;
}

function parseMemoryDir(dir, entries) {
  try {
    const files = fs.readdirSync(dir).filter(f => f.endsWith('.md'));
    for (const file of files) {
      const filePath = path.join(dir, file);
      const content = fs.readFileSync(filePath, 'utf-8');
      if (!content.trim()) continue;

      // Parse markdown sections as separate entries
      const sections = content.split(/^##?\s+/m).filter(Boolean);
      for (const section of sections) {
        const lines = section.trim().split('\n');
        const title = lines[0].trim();
        const body = lines.slice(1).join('\n').trim();
        if (!body || body.length < 10) continue;

        const id = `mem-${file.replace('.md', '')}-${title.replace(/[^a-z0-9]/gi, '-').toLowerCase().slice(0, 30)}`;
        entries.push({
          id,
          key: title.toLowerCase().replace(/[^a-z0-9]+/g, '-').slice(0, 50),
          content: body.slice(0, 500),
          summary: title,
          namespace: file === 'MEMORY.md' ? 'core' : file.replace('.md', ''),
          type: 'semantic',
          metadata: { sourceFile: filePath, bootstrapped: true },
          createdAt: Date.now(),
        });
      }
    }
  } catch { /* skip unreadable dirs */ }
}

// ── Exported functions ───────────────────────────────────────────────────────

/**
 * init() — Called from session-restore. Budget: <200ms.
 * Reads auto-memory-store.json, builds graph, computes PageRank, writes caches.
 * If store is empty, bootstraps from MEMORY.md files directly.
 */
function init() {
  ensureDataDir();

  // Check if graph-state.json is fresh (within 60s of store)
  const graphState = readJSON(GRAPH_PATH);
  let store = readJSON(STORE_PATH);

  // Bootstrap from MEMORY.md files if store is empty
  if (!store || !Array.isArray(store) || store.length === 0) {
    const bootstrapped = bootstrapFromMemoryFiles();
    if (bootstrapped.length > 0) {
      store = bootstrapped;
      writeJSON(STORE_PATH, store);
    } else {
      return { nodes: 0, edges: 0, message: 'No memory entries to index' };
    }
  }

  // Skip rebuild if graph is fresh and store hasn't changed
  if (graphState && graphState.nodeCount === store.length) {
    const age = Date.now() - (graphState.updatedAt || 0);
    if (age < 60000) {
      return {
        nodes: graphState.nodeCount || Object.keys(graphState.nodes || {}).length,
        edges: (graphState.edges || []).length,
        message: 'Graph cache hit',
      };
    }
  }

  // Build nodes
  const nodes = {};
  for (const entry of store) {
    const id = entry.id || entry.key || `entry-${Math.random().toString(36).slice(2, 8)}`;
    nodes[id] = {
      id,
      category: entry.namespace || entry.type || 'default',
      confidence: (entry.metadata && entry.metadata.confidence) || 0.5,
      accessCount: (entry.metadata && entry.metadata.accessCount) || 0,
      createdAt: entry.createdAt || Date.now(),
    };
    // Ensure entry has id for edge building
    entry.id = id;
  }

  // Build edges
  const edges = buildEdges(store);

  // Compute PageRank
  const pageRanks = computePageRank(nodes, edges, 0.85, 30);

  // Write graph state
  const graph = {
    version: 1,
    updatedAt: Date.now(),
    nodeCount: Object.keys(nodes).length,
    nodes,
    edges,
    pageRanks,
  };
  writeJSON(GRAPH_PATH, graph);

  // Build ranked context for fast lookup
  const rankedEntries = store.map(entry => {
    const id = entry.id;
    const content = entry.content || entry.value || '';
    const summary = entry.summary || entry.key || '';
    const words = tokenize(content + ' ' + summary);
    return {
      id,
      content,
      summary,
      category: entry.namespace || entry.type || 'default',
      confidence: nodes[id] ? nodes[id].confidence : 0.5,
      pageRank: pageRanks[id] || 0,
      accessCount: nodes[id] ? nodes[id].accessCount : 0,
      words,
    };
  }).sort((a, b) => {
    const scoreA = 0.6 * a.pageRank + 0.4 * a.confidence;
    const scoreB = 0.6 * b.pageRank + 0.4 * b.confidence;
    return scoreB - scoreA;
  });

  const ranked = {
    version: 1,
    computedAt: Date.now(),
    entries: rankedEntries,
  };
  writeJSON(RANKED_PATH, ranked);

  return {
    nodes: Object.keys(nodes).length,
    edges: edges.length,
    message: 'Graph built and ranked',
  };
}

/**
 * getContext(prompt) — Called from route. Budget: <15ms.
 * Matches prompt to ranked entries, returns top-5 formatted context.
 */
function getContext(prompt) {
  if (!prompt) return null;

  const ranked = readJSON(RANKED_PATH);
  if (!ranked || !ranked.entries || ranked.entries.length === 0) return null;

  const promptWords = tokenize(prompt);
  if (promptWords.length === 0) return null;
  const promptTrigrams = trigrams(promptWords);

  const ALPHA = 0.6; // content match weight
  const MIN_THRESHOLD = 0.05;
  const TOP_K = 5;

  // Score each entry
  const scored = [];
  for (const entry of ranked.entries) {
    const entryTrigrams = trigrams(entry.words || []);
    const contentMatch = jaccardSimilarity(promptTrigrams, entryTrigrams);
    const score = ALPHA * contentMatch + (1 - ALPHA) * (entry.pageRank || 0);
    if (score >= MIN_THRESHOLD) {
      scored.push({ ...entry, score });
    }
  }

  if (scored.length === 0) return null;

  // Sort by score descending, take top-K
  scored.sort((a, b) => b.score - a.score);
  const topEntries = scored.slice(0, TOP_K);

  // Boost previously matched patterns (implicit success: user continued working)
  const prevMatched = sessionGet('lastMatchedPatterns');

  // Store NEW matched IDs in session state for feedback
  const matchedIds = topEntries.map(e => e.id);
  sessionSet('lastMatchedPatterns', matchedIds);

  // Only boost previous if they differ from current (avoid double-boosting)
  if (prevMatched && Array.isArray(prevMatched)) {
    const newSet = new Set(matchedIds);
    const toBoost = prevMatched.filter(id => !newSet.has(id));
    if (toBoost.length > 0) boostConfidence(toBoost, 0.03);
  }

  // Format output
  const lines = ['[INTELLIGENCE] Relevant patterns for this task:'];
  for (let i = 0; i < topEntries.length; i++) {
    const e = topEntries[i];
    const display = (e.summary || e.content || '').slice(0, 80);
    const accessed = e.accessCount || 0;
    lines.push(`  * (${e.score.toFixed(2)}) ${display} [rank #${i + 1}, ${accessed}x accessed]`);
  }

  return lines.join('\n');
}

/**
 * recordEdit(file) — Called from post-edit. Budget: <2ms.
 * Appends to pending-insights.jsonl.
 */
function recordEdit(file) {
  ensureDataDir();
  const entry = JSON.stringify({
    type: 'edit',
    file: file || 'unknown',
    timestamp: Date.now(),
    sessionId: sessionGet('sessionId') || null,
  });
  fs.appendFileSync(PENDING_PATH, entry + '\n', 'utf-8');
}

/**
 * feedback(success) — Called from post-task. Budget: <10ms.
 * Boosts or decays confidence for last-matched patterns.
 */
function feedback(success) {
  const matchedIds = sessionGet('lastMatchedPatterns');
  if (!matchedIds || !Array.isArray(matchedIds)) return;

  const amount = success ? 0.05 : -0.02;
  boostConfidence(matchedIds, amount);
}

function boostConfidence(ids, amount) {
  const ranked = readJSON(RANKED_PATH);
  if (!ranked || !ranked.entries) return;

  let changed = false;
  for (const entry of ranked.entries) {
    if (ids.includes(entry.id)) {
      entry.confidence = Math.max(0, Math.min(1, (entry.confidence || 0.5) + amount));
      entry.accessCount = (entry.accessCount || 0) + 1;
      changed = true;
    }
  }

  if (changed) writeJSON(RANKED_PATH, ranked);

  // Also update graph-state confidence
  const graph = readJSON(GRAPH_PATH);
  if (graph && graph.nodes) {
    for (const id of ids) {
      if (graph.nodes[id]) {
        graph.nodes[id].confidence = Math.max(0, Math.min(1, (graph.nodes[id].confidence || 0.5) + amount));
        graph.nodes[id].accessCount = (graph.nodes[id].accessCount || 0) + 1;
      }
    }
    writeJSON(GRAPH_PATH, graph);
  }
}

/**
 * consolidate() — Called from session-end. Budget: <500ms.
 * Processes pending insights, rebuilds edges, recomputes PageRank.
 */
function consolidate() {
  ensureDataDir();

  const store = readJSON(STORE_PATH);
  if (!store || !Array.isArray(store)) {
    return { entries: 0, edges: 0, newEntries: 0, message: 'No store to consolidate' };
  }

  // 1. Process pending insights
  let newEntries = 0;
  if (fs.existsSync(PENDING_PATH)) {
    const lines = fs.readFileSync(PENDING_PATH, 'utf-8').trim().split('\n').filter(Boolean);
    const editCounts = {};
    for (const line of lines) {
      try {
        const insight = JSON.parse(line);
        if (insight.file) {
          editCounts[insight.file] = (editCounts[insight.file] || 0) + 1;
        }
      } catch { /* skip malformed */ }
    }

    // Create entries for frequently-edited files (3+ edits)
    for (const [file, count] of Object.entries(editCounts)) {
      if (count >= 3) {
        const exists = store.some(e =>
          (e.metadata && e.metadata.sourceFile === file && e.metadata.autoGenerated)
        );
        if (!exists) {
          store.push({
            id: `insight-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
            key: `frequent-edit-${path.basename(file)}`,
            content: `File ${file} was edited ${count} times this session — likely a hot path worth monitoring.`,
            summary: `Frequently edited: ${path.basename(file)} (${count}x)`,
            namespace: 'insights',
            type: 'procedural',
            metadata: { sourceFile: file, editCount: count, autoGenerated: true },
            createdAt: Date.now(),
          });
          newEntries++;
        }
      }
    }

    // Clear pending
    fs.writeFileSync(PENDING_PATH, '', 'utf-8');
  }

  // 2. Confidence decay for unaccessed entries
  const graph = readJSON(GRAPH_PATH);
  if (graph && graph.nodes) {
    const now = Date.now();
    for (const id of Object.keys(graph.nodes)) {
      const node = graph.nodes[id];
      const hoursSinceCreation = (now - (node.createdAt || now)) / (1000 * 60 * 60);
      if (node.accessCount === 0 && hoursSinceCreation > 24) {
        node.confidence = Math.max(0.05, (node.confidence || 0.5) - 0.005 * Math.floor(hoursSinceCreation / 24));
      }
    }
  }

  // 3. Rebuild edges with updated store
  for (const entry of store) {
    if (!entry.id) entry.id = `entry-${Math.random().toString(36).slice(2, 8)}`;
  }
  const edges = buildEdges(store);

  // 4. Build updated nodes
  const nodes = {};
  for (const entry of store) {
    nodes[entry.id] = {
      id: entry.id,
      category: entry.namespace || entry.type || 'default',
      confidence: (graph && graph.nodes && graph.nodes[entry.id])
        ? graph.nodes[entry.id].confidence
        : (entry.metadata && entry.metadata.confidence) || 0.5,
      accessCount: (graph && graph.nodes && graph.nodes[entry.id])
        ? graph.nodes[entry.id].accessCount
        : (entry.metadata && entry.metadata.accessCount) || 0,
      createdAt: entry.createdAt || Date.now(),
    };
  }

  // 5. Recompute PageRank
  const pageRanks = computePageRank(nodes, edges, 0.85, 30);

  // 6. Write updated graph
  writeJSON(GRAPH_PATH, {
    version: 1,
    updatedAt: Date.now(),
    nodeCount: Object.keys(nodes).length,
    nodes,
    edges,
    pageRanks,
  });

  // 7. Write updated ranked context
  const rankedEntries = store.map(entry => {
    const id = entry.id;
    const content = entry.content || entry.value || '';
    const summary = entry.summary || entry.key || '';
    const words = tokenize(content + ' ' + summary);
    return {
      id,
      content,
      summary,
      category: entry.namespace || entry.type || 'default',
      confidence: nodes[id] ? nodes[id].confidence : 0.5,
      pageRank: pageRanks[id] || 0,
      accessCount: nodes[id] ? nodes[id].accessCount : 0,
      words,
    };
  }).sort((a, b) => {
    const scoreA = 0.6 * a.pageRank + 0.4 * a.confidence;
    const scoreB = 0.6 * b.pageRank + 0.4 * b.confidence;
    return scoreB - scoreA;
  });

  writeJSON(RANKED_PATH, {
    version: 1,
    computedAt: Date.now(),
    entries: rankedEntries,
  });

  // 8. Persist updated store (with new insight entries)
  if (newEntries > 0) writeJSON(STORE_PATH, store);

  // 9. Save snapshot for delta tracking
  const updatedGraph = readJSON(GRAPH_PATH);
  const updatedRanked = readJSON(RANKED_PATH);
  saveSnapshot(updatedGraph, updatedRanked);

  return {
    entries: store.length,
    edges: edges.length,
    newEntries,
    message: 'Consolidated',
  };
}

// ── Snapshot for delta tracking ─────────────────────────────────────────────

const SNAPSHOT_PATH = path.join(DATA_DIR, 'intelligence-snapshot.json');

function saveSnapshot(graph, ranked) {
  const snap = {
    timestamp: Date.now(),
    nodes: graph ? Object.keys(graph.nodes || {}).length : 0,
    edges: graph ? (graph.edges || []).length : 0,
    pageRankSum: 0,
    confidences: [],
    accessCounts: [],
    topPatterns: [],
  };

  if (graph && graph.pageRanks) {
    for (const v of Object.values(graph.pageRanks)) snap.pageRankSum += v;
  }
  if (graph && graph.nodes) {
    for (const n of Object.values(graph.nodes)) {
      snap.confidences.push(n.confidence || 0.5);
      snap.accessCounts.push(n.accessCount || 0);
    }
  }
  if (ranked && ranked.entries) {
    snap.topPatterns = ranked.entries.slice(0, 10).map(e => ({
      id: e.id,
      summary: (e.summary || '').slice(0, 60),
      confidence: e.confidence || 0.5,
      pageRank: e.pageRank || 0,
      accessCount: e.accessCount || 0,
    }));
  }

  // Keep history: append to array, cap at 50
  let history = readJSON(SNAPSHOT_PATH);
  if (!Array.isArray(history)) history = [];
  history.push(snap);
  if (history.length > 50) history = history.slice(-50);
  writeJSON(SNAPSHOT_PATH, history);
}

/**
 * stats() — Diagnostic report showing intelligence health and improvement.
 * Can be called as: node intelligence.cjs stats [--json]
 */
function stats(outputJson) {
  const graph = readJSON(GRAPH_PATH);
  const ranked = readJSON(RANKED_PATH);
  const history = readJSON(SNAPSHOT_PATH) || [];
  const pending = fs.existsSync(PENDING_PATH)
    ? fs.readFileSync(PENDING_PATH, 'utf-8').trim().split('\n').filter(Boolean).length
    : 0;

  // Current state
  const nodes = graph ? Object.keys(graph.nodes || {}).length : 0;
  const edges = graph ? (graph.edges || []).length : 0;
  const density = nodes > 1 ? (2 * edges) / (nodes * (nodes - 1)) : 0;

  // Confidence distribution
  const confidences = [];
  const accessCounts = [];
  if (graph && graph.nodes) {
    for (const n of Object.values(graph.nodes)) {
      confidences.push(n.confidence || 0.5);
      accessCounts.push(n.accessCount || 0);
    }
  }
  confidences.sort((a, b) => a - b);
  const confMin = confidences.length ? confidences[0] : 0;
  const confMax = confidences.length ? confidences[confidences.length - 1] : 0;
  const confMean = confidences.length ? confidences.reduce((s, c) => s + c, 0) / confidences.length : 0;
  const confMedian = confidences.length ? confidences[Math.floor(confidences.length / 2)] : 0;

  // Access stats
  const totalAccess = accessCounts.reduce((s, c) => s + c, 0);
  const accessedCount = accessCounts.filter(c => c > 0).length;

  // PageRank stats
  let prSum = 0, prMax = 0, prMaxId = '';
  if (graph && graph.pageRanks) {
    for (const [id, pr] of Object.entries(graph.pageRanks)) {
      prSum += pr;
      if (pr > prMax) { prMax = pr; prMaxId = id; }
    }
  }

  // Top patterns by composite score
  const topPatterns = (ranked && ranked.entries || []).slice(0, 10).map((e, i) => ({
    rank: i + 1,
    summary: (e.summary || '').slice(0, 60),
    confidence: (e.confidence || 0.5).toFixed(3),
    pageRank: (e.pageRank || 0).toFixed(4),
    accessed: e.accessCount || 0,
    score: (0.6 * (e.pageRank || 0) + 0.4 * (e.confidence || 0.5)).toFixed(4),
  }));

  // Edge type breakdown
  const edgeTypes = {};
  if (graph && graph.edges) {
    for (const e of graph.edges) {
      edgeTypes[e.type || 'unknown'] = (edgeTypes[e.type || 'unknown'] || 0) + 1;
    }
  }

  // Delta from previous snapshot
  let delta = null;
  if (history.length >= 2) {
    const prev = history[history.length - 2];
    const curr = history[history.length - 1];
    const elapsed = (curr.timestamp - prev.timestamp) / 1000;
    const prevConfMean = prev.confidences.length
      ? prev.confidences.reduce((s, c) => s + c, 0) / prev.confidences.length : 0;
    const currConfMean = curr.confidences.length
      ? curr.confidences.reduce((s, c) => s + c, 0) / curr.confidences.length : 0;
    const prevAccess = prev.accessCounts.reduce((s, c) => s + c, 0);
    const currAccess = curr.accessCounts.reduce((s, c) => s + c, 0);

    delta = {
      elapsed: elapsed < 3600 ? `${Math.round(elapsed / 60)}m` : `${(elapsed / 3600).toFixed(1)}h`,
      nodes: curr.nodes - prev.nodes,
      edges: curr.edges - prev.edges,
      confidenceMean: currConfMean - prevConfMean,
      totalAccess: currAccess - prevAccess,
    };
  }

  // Trend over all history
  let trend = null;
  if (history.length >= 3) {
    const first = history[0];
    const last = history[history.length - 1];
    const sessions = history.length;
    const firstConfMean = first.confidences.length
      ? first.confidences.reduce((s, c) => s + c, 0) / first.confidences.length : 0;
    const lastConfMean = last.confidences.length
      ? last.confidences.reduce((s, c) => s + c, 0) / last.confidences.length : 0;
    trend = {
      sessions,
      nodeGrowth: last.nodes - first.nodes,
      edgeGrowth: last.edges - first.edges,
      confidenceDrift: lastConfMean - firstConfMean,
      direction: lastConfMean > firstConfMean ? 'improving' :
                 lastConfMean < firstConfMean ? 'declining' : 'stable',
    };
  }

  const report = {
    graph: { nodes, edges, density: +density.toFixed(4) },
    confidence: {
      min: +confMin.toFixed(3), max: +confMax.toFixed(3),
      mean: +confMean.toFixed(3), median: +confMedian.toFixed(3),
    },
    access: { total: totalAccess, patternsAccessed: accessedCount, patternsNeverAccessed: nodes - accessedCount },
    pageRank: { sum: +prSum.toFixed(4), topNode: prMaxId, topNodeRank: +prMax.toFixed(4) },
    edgeTypes,
    pendingInsights: pending,
    snapshots: history.length,
    topPatterns,
    delta,
    trend,
  };

  if (outputJson) {
    console.log(JSON.stringify(report, null, 2));
    return report;
  }

  // Human-readable output
  const bar = '+' + '-'.repeat(62) + '+';
  console.log(bar);
  console.log('|' + '  Intelligence Diagnostics (ADR-050)'.padEnd(62) + '|');
  console.log(bar);
  console.log('');

  console.log('  Graph');
  console.log(`    Nodes:    ${nodes}`);
  console.log(`    Edges:    ${edges} (${Object.entries(edgeTypes).map(([t,c]) => `${c} ${t}`).join(', ') || 'none'})`);
  console.log(`    Density:  ${(density * 100).toFixed(1)}%`);
  console.log('');

  console.log('  Confidence');
  console.log(`    Min:      ${confMin.toFixed(3)}`);
  console.log(`    Max:      ${confMax.toFixed(3)}`);
  console.log(`    Mean:     ${confMean.toFixed(3)}`);
  console.log(`    Median:   ${confMedian.toFixed(3)}`);
  console.log('');

  console.log('  Access');
  console.log(`    Total accesses:     ${totalAccess}`);
  console.log(`    Patterns used:      ${accessedCount}/${nodes}`);
  console.log(`    Never accessed:     ${nodes - accessedCount}`);
  console.log(`    Pending insights:   ${pending}`);
  console.log('');

  console.log('  PageRank');
  console.log(`    Sum:      ${prSum.toFixed(4)} (should be ~1.0)`);
  console.log(`    Top node: ${prMaxId || '(none)'} (${prMax.toFixed(4)})`);
  console.log('');

  if (topPatterns.length > 0) {
    console.log('  Top Patterns (by composite score)');
    console.log('  ' + '-'.repeat(60));
    for (const p of topPatterns) {
      console.log(`    #${p.rank}  ${p.summary}`);
      console.log(`         conf=${p.confidence}  pr=${p.pageRank}  score=${p.score}  accessed=${p.accessed}x`);
    }
    console.log('');
  }

  if (delta) {
    console.log(`  Last Delta (${delta.elapsed} ago)`);
    const sign = v => v > 0 ? `+${v}` : `${v}`;
    console.log(`    Nodes:      ${sign(delta.nodes)}`);
    console.log(`    Edges:      ${sign(delta.edges)}`);
    console.log(`    Confidence: ${delta.confidenceMean >= 0 ? '+' : ''}${delta.confidenceMean.toFixed(4)}`);
    console.log(`    Accesses:   ${sign(delta.totalAccess)}`);
    console.log('');
  }

  if (trend) {
    console.log(`  Trend (${trend.sessions} snapshots)`);
    console.log(`    Node growth:       ${trend.nodeGrowth >= 0 ? '+' : ''}${trend.nodeGrowth}`);
    console.log(`    Edge growth:       ${trend.edgeGrowth >= 0 ? '+' : ''}${trend.edgeGrowth}`);
    console.log(`    Confidence drift:  ${trend.confidenceDrift >= 0 ? '+' : ''}${trend.confidenceDrift.toFixed(4)}`);
    console.log(`    Direction:         ${trend.direction.toUpperCase()}`);
    console.log('');
  }

  if (!delta && !trend) {
    console.log('  No history yet — run more sessions to see deltas and trends.');
    console.log('');
  }

  console.log(bar);
  return report;
}

module.exports = { init, getContext, recordEdit, feedback, consolidate, stats };

// ── CLI entrypoint ──────────────────────────────────────────────────────────
if (require.main === module) {
  const cmd = process.argv[2];
  const jsonFlag = process.argv.includes('--json');

  const cmds = {
    init: () => { const r = init(); console.log(JSON.stringify(r)); },
    stats: () => { stats(jsonFlag); },
    consolidate: () => { const r = consolidate(); console.log(JSON.stringify(r)); },
  };

  if (cmd && cmds[cmd]) {
    cmds[cmd]();
  } else {
    console.log('Usage: intelligence.cjs <stats|init|consolidate> [--json]');
    console.log('');
    console.log('  stats         Show intelligence diagnostics and trends');
    console.log('  stats --json  Output as JSON for programmatic use');
    console.log('  init          Build graph and rank entries');
    console.log('  consolidate   Process pending insights and recompute');
  }
}
