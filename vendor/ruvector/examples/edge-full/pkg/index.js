/**
 * @ruvector/edge-full - Complete WASM toolkit for edge AI
 *
 * This package bundles all RuVector WASM modules for comprehensive
 * edge computing capabilities:
 *
 * - edge: Cryptographic identity, P2P, vector search, neural networks
 * - graph: Neo4j-style graph database with Cypher queries
 * - rvlite: SQL/SPARQL/Cypher vector database
 * - sona: Self-optimizing neural architecture with LoRA
 * - dag: Directed acyclic graph for workflow orchestration
 * - onnx: ONNX inference with HuggingFace embedding models
 *
 * Each module can be imported separately:
 *
 *   import { WasmIdentity, WasmHnswIndex } from '@ruvector/edge-full/edge';
 *   import { WasmGraphStore } from '@ruvector/edge-full/graph';
 *   import { Database } from '@ruvector/edge-full/rvlite';
 *   import { SonaEngine } from '@ruvector/edge-full/sona';
 *   import { Dag } from '@ruvector/edge-full/dag';
 *   import { OnnxEmbedder } from '@ruvector/edge-full/onnx';
 *
 * Or use the unified init for quick setup:
 *
 *   import { initAll, edge, graph, rvlite, sona, dag } from '@ruvector/edge-full';
 *   await initAll();
 */

// Re-export all modules for convenience
export * as edge from './edge/ruvector_edge.js';
export * as graph from './graph/ruvector_graph_wasm.js';
export * as rvlite from './rvlite/rvlite.js';
export * as sona from './sona/ruvector_sona.js';
export * as dag from './dag/ruvector_dag_wasm.js';

// ONNX is large (7MB), export separately
export { default as onnxInit } from './onnx/ruvector_onnx_embeddings_wasm.js';

// Module info
export const modules = {
  edge: {
    name: 'RuVector Edge',
    size: '364KB',
    features: ['Ed25519 identity', 'AES-256-GCM encryption', 'HNSW vector search',
               'Raft consensus', 'Neural networks', 'Post-quantum crypto']
  },
  graph: {
    name: 'Graph Database',
    size: '288KB',
    features: ['Neo4j-style API', 'Cypher queries', 'Hypergraph support',
               'Worker threads', 'IndexedDB persistence']
  },
  rvlite: {
    name: 'RVLite Vector DB',
    size: '260KB',
    features: ['SQL queries', 'SPARQL queries', 'Cypher queries',
               'Multi-index', 'Browser/Node/Edge']
  },
  sona: {
    name: 'SONA Neural',
    size: '238KB',
    features: ['Two-tier LoRA', 'EWC++', 'ReasoningBank',
               'Adaptive learning', 'Router optimization']
  },
  dag: {
    name: 'DAG Workflows',
    size: '132KB',
    features: ['Workflow orchestration', 'Dependency tracking',
               'Topological sort', 'Minimal footprint']
  },
  onnx: {
    name: 'ONNX Embeddings',
    size: '7.1MB',
    features: ['HuggingFace models', '6 pre-trained models',
               'Parallel workers', 'SIMD acceleration']
  }
};

// Calculate total size
export const totalSize = {
  core: '1.28MB', // edge + graph + rvlite + sona + dag
  withOnnx: '8.4MB' // including ONNX
};

/**
 * Initialize all core modules (excludes ONNX due to size)
 * For ONNX, import and init separately:
 *   import onnxInit from '@ruvector/edge-full/onnx';
 *   await onnxInit();
 */
export async function initAll() {
  const { default: edgeInit } = await import('./edge/ruvector_edge.js');
  const { default: graphInit } = await import('./graph/ruvector_graph_wasm.js');
  const { default: rvliteInit } = await import('./rvlite/rvlite.js');
  const { default: sonaInit } = await import('./sona/ruvector_sona.js');
  const { default: dagInit } = await import('./dag/ruvector_dag_wasm.js');

  await Promise.all([
    edgeInit(),
    graphInit(),
    rvliteInit(),
    sonaInit(),
    dagInit()
  ]);

  return { edge, graph, rvlite, sona, dag };
}

/**
 * Initialize only specific modules
 * @param {string[]} moduleNames - Array of module names to init
 */
export async function initModules(moduleNames) {
  const results = {};

  for (const name of moduleNames) {
    switch (name) {
      case 'edge':
        const { default: edgeInit } = await import('./edge/ruvector_edge.js');
        await edgeInit();
        results.edge = await import('./edge/ruvector_edge.js');
        break;
      case 'graph':
        const { default: graphInit } = await import('./graph/ruvector_graph_wasm.js');
        await graphInit();
        results.graph = await import('./graph/ruvector_graph_wasm.js');
        break;
      case 'rvlite':
        const { default: rvliteInit } = await import('./rvlite/rvlite.js');
        await rvliteInit();
        results.rvlite = await import('./rvlite/rvlite.js');
        break;
      case 'sona':
        const { default: sonaInit } = await import('./sona/ruvector_sona.js');
        await sonaInit();
        results.sona = await import('./sona/ruvector_sona.js');
        break;
      case 'dag':
        const { default: dagInit } = await import('./dag/ruvector_dag_wasm.js');
        await dagInit();
        results.dag = await import('./dag/ruvector_dag_wasm.js');
        break;
      case 'onnx':
        const { default: onnxInit } = await import('./onnx/ruvector_onnx_embeddings_wasm.js');
        await onnxInit();
        results.onnx = await import('./onnx/ruvector_onnx_embeddings_wasm.js');
        break;
    }
  }

  return results;
}

// Quick start example
export const quickStart = `
// Initialize all core modules (1.28MB total)
import { initAll } from '@ruvector/edge-full';

const { edge, graph, rvlite, sona, dag } from await initAll();

// Create cryptographic identity
const identity = new edge.WasmIdentity.generate();
console.log('Agent:', identity.agent_id());

// Build a graph
const graphStore = new graph.WasmGraphStore();
graphStore.run_cypher("CREATE (a:Agent {id: 'agent-1'})");

// Vector search with SQL
const db = new rvlite.Database();
db.execute("INSERT INTO vectors (embedding) VALUES ([1,2,3])");

// Self-learning neural routing
const sonaEngine = new sona.SonaEngine();
sonaEngine.route_request({ task: "analyze" });

// Workflow orchestration
const workflow = new dag.Dag();
workflow.add_node("start");
workflow.add_node("process");
workflow.add_edge("start", "process");
`;
