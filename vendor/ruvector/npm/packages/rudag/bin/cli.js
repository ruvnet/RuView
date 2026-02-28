#!/usr/bin/env node

/**
 * rudag CLI - Command-line interface for DAG operations
 *
 * @security File paths are validated to prevent reading arbitrary files
 */

const path = require('path');
const fs = require('fs');

// Lazy load to improve startup time
let RuDag, DagOperator, AttentionMechanism;

const args = process.argv.slice(2);
const command = args[0];

const help = `
rudag - Self-learning DAG query optimization CLI

Usage: rudag <command> [options]

Commands:
  create <name>             Create a new DAG and output to stdout
  load <file>               Load DAG from file (must be .dag or .json)
  info <file>               Show DAG information
  topo <file>               Print topological sort
  critical <file>           Find critical path
  attention <file> [type]   Compute attention scores (type: topo|critical|uniform)
  convert <in> <out>        Convert between JSON and binary formats
  help                      Show this help message

Examples:
  rudag create my-query > my-query.dag
  rudag info ./data/my-query.dag
  rudag critical ./queries/query.dag
  rudag attention query.dag critical

Options:
  --json                    Output in JSON format
  --verbose                 Verbose output

Security:
  - Only .dag and .json files are allowed
  - Paths are restricted to current directory and subdirectories
`;

/**
 * Validate file path for security
 * @security Prevents path traversal and restricts to allowed extensions
 */
function validateFilePath(filePath) {
  if (!filePath || typeof filePath !== 'string') {
    throw new Error('File path is required');
  }

  // Check extension
  const ext = path.extname(filePath).toLowerCase();
  if (ext !== '.dag' && ext !== '.json') {
    throw new Error(`Invalid file extension: ${ext}. Only .dag and .json files are allowed.`);
  }

  // Resolve to absolute path
  const resolved = path.resolve(filePath);
  const cwd = process.cwd();

  // Ensure path is within current directory (prevents traversal)
  if (!resolved.startsWith(cwd + path.sep) && resolved !== cwd) {
    // Allow absolute paths within cwd or relative paths
    const normalized = path.normalize(filePath);
    if (normalized.startsWith('..') || path.isAbsolute(normalized)) {
      // Check if absolute path is within cwd
      if (!resolved.startsWith(cwd)) {
        throw new Error('Access denied: file path must be within current directory');
      }
    }
  }

  // Additional check: no null bytes
  if (filePath.includes('\0')) {
    throw new Error('Invalid file path: contains null bytes');
  }

  return resolved;
}

/**
 * Validate output file path
 */
function validateOutputPath(filePath) {
  if (!filePath || typeof filePath !== 'string') {
    throw new Error('Output file path is required');
  }

  const ext = path.extname(filePath).toLowerCase();
  if (ext !== '.dag' && ext !== '.json') {
    throw new Error(`Invalid output extension: ${ext}. Only .dag and .json files are allowed.`);
  }

  const resolved = path.resolve(filePath);
  const cwd = process.cwd();

  // Must be within current directory
  if (!resolved.startsWith(cwd + path.sep) && resolved !== cwd) {
    throw new Error('Access denied: output path must be within current directory');
  }

  if (filePath.includes('\0')) {
    throw new Error('Invalid file path: contains null bytes');
  }

  return resolved;
}

/**
 * Lazy load dependencies
 */
async function loadDependencies() {
  if (!RuDag) {
    const mod = require('../dist/index.js');
    RuDag = mod.RuDag;
    DagOperator = mod.DagOperator;
    AttentionMechanism = mod.AttentionMechanism;
  }
}

async function main() {
  if (!command || command === 'help' || command === '--help') {
    console.log(help);
    process.exit(0);
  }

  const isJson = args.includes('--json');
  const verbose = args.includes('--verbose');

  try {
    await loadDependencies();

    switch (command) {
      case 'create': {
        const name = args[1] || 'untitled';

        // Validate name (alphanumeric only)
        if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
          throw new Error('Invalid name: must be alphanumeric with dashes/underscores only');
        }

        const dag = new RuDag({ name, storage: null, autoSave: false });
        await dag.init();

        // Create a simple example DAG
        const scan = dag.addNode(DagOperator.SCAN, 10.0);
        const filter = dag.addNode(DagOperator.FILTER, 2.0);
        const project = dag.addNode(DagOperator.PROJECT, 1.0);

        dag.addEdge(scan, filter);
        dag.addEdge(filter, project);

        if (isJson) {
          console.log(dag.toJSON());
        } else {
          const bytes = dag.toBytes();
          process.stdout.write(Buffer.from(bytes));
        }

        dag.dispose();
        break;
      }

      case 'load': {
        const file = validateFilePath(args[1]);

        if (!fs.existsSync(file)) {
          throw new Error(`File not found: ${args[1]}`);
        }

        const data = fs.readFileSync(file);
        let dag;

        if (file.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        console.log(`Loaded DAG with ${dag.nodeCount} nodes and ${dag.edgeCount} edges`);
        dag.dispose();
        break;
      }

      case 'info': {
        const file = validateFilePath(args[1]);

        if (!fs.existsSync(file)) {
          throw new Error(`File not found: ${args[1]}`);
        }

        const data = fs.readFileSync(file);
        let dag;

        if (file.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        const critPath = dag.criticalPath();
        const info = {
          file: args[1],
          nodes: dag.nodeCount,
          edges: dag.edgeCount,
          criticalPath: critPath,
        };

        if (isJson) {
          console.log(JSON.stringify(info, null, 2));
        } else {
          console.log(`File: ${info.file}`);
          console.log(`Nodes: ${info.nodes}`);
          console.log(`Edges: ${info.edges}`);
          console.log(`Critical Path: ${info.criticalPath.path.join(' -> ')}`);
          console.log(`Total Cost: ${info.criticalPath.cost}`);
        }

        dag.dispose();
        break;
      }

      case 'topo': {
        const file = validateFilePath(args[1]);

        if (!fs.existsSync(file)) {
          throw new Error(`File not found: ${args[1]}`);
        }

        const data = fs.readFileSync(file);
        let dag;

        if (file.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        const topo = dag.topoSort();

        if (isJson) {
          console.log(JSON.stringify(topo));
        } else {
          console.log('Topological order:', topo.join(' -> '));
        }

        dag.dispose();
        break;
      }

      case 'critical': {
        const file = validateFilePath(args[1]);

        if (!fs.existsSync(file)) {
          throw new Error(`File not found: ${args[1]}`);
        }

        const data = fs.readFileSync(file);
        let dag;

        if (file.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        const result = dag.criticalPath();

        if (isJson) {
          console.log(JSON.stringify(result));
        } else {
          console.log('Critical Path:', result.path.join(' -> '));
          console.log('Total Cost:', result.cost);
        }

        dag.dispose();
        break;
      }

      case 'attention': {
        const file = validateFilePath(args[1]);
        const type = args[2] || 'critical';

        if (!fs.existsSync(file)) {
          throw new Error(`File not found: ${args[1]}`);
        }

        const data = fs.readFileSync(file);
        let dag;

        if (file.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        let mechanism;
        switch (type) {
          case 'topo':
          case 'topological':
            mechanism = AttentionMechanism.TOPOLOGICAL;
            break;
          case 'critical':
          case 'critical_path':
            mechanism = AttentionMechanism.CRITICAL_PATH;
            break;
          case 'uniform':
            mechanism = AttentionMechanism.UNIFORM;
            break;
          default:
            dag.dispose();
            throw new Error(`Unknown attention type: ${type}. Use: topo, critical, or uniform`);
        }

        const scores = dag.attention(mechanism);

        if (isJson) {
          console.log(JSON.stringify({ type, scores }));
        } else {
          console.log(`Attention type: ${type}`);
          scores.forEach((score, i) => {
            console.log(`  Node ${i}: ${score.toFixed(4)}`);
          });
        }

        dag.dispose();
        break;
      }

      case 'convert': {
        const inFile = validateFilePath(args[1]);
        const outFile = validateOutputPath(args[2]);

        if (!fs.existsSync(inFile)) {
          throw new Error(`Input file not found: ${args[1]}`);
        }

        const data = fs.readFileSync(inFile);
        let dag;

        if (inFile.endsWith('.json')) {
          dag = await RuDag.fromJSON(data.toString(), { storage: null });
        } else {
          dag = await RuDag.fromBytes(new Uint8Array(data), { storage: null });
        }

        if (outFile.endsWith('.json')) {
          fs.writeFileSync(outFile, dag.toJSON());
        } else {
          fs.writeFileSync(outFile, Buffer.from(dag.toBytes()));
        }

        console.log(`Converted ${args[1]} -> ${args[2]}`);
        dag.dispose();
        break;
      }

      default:
        console.error(`Unknown command: ${command}`);
        console.log('Run "rudag help" for usage information');
        process.exit(1);
    }
  } catch (error) {
    console.error('Error:', error.message);
    if (verbose) {
      console.error(error.stack);
    }
    process.exit(1);
  }
}

main();
