/**
 * @ruvector/edge-full - Complete WASM toolkit for edge AI
 */

// Module namespaces
export * as edge from './edge/ruvector_edge';
export * as graph from './graph/ruvector_graph_wasm';
export * as rvlite from './rvlite/rvlite';
export * as sona from './sona/ruvector_sona';
export * as dag from './dag/ruvector_dag_wasm';

// ONNX init function
export { default as onnxInit } from './onnx/ruvector_onnx_embeddings_wasm';

// Module info interface
export interface ModuleInfo {
  name: string;
  size: string;
  features: string[];
}

export interface ModulesMap {
  edge: ModuleInfo;
  graph: ModuleInfo;
  rvlite: ModuleInfo;
  sona: ModuleInfo;
  dag: ModuleInfo;
  onnx: ModuleInfo;
}

export const modules: ModulesMap;

export interface TotalSize {
  core: string;
  withOnnx: string;
}

export const totalSize: TotalSize;

/**
 * Initialize all core modules (excludes ONNX due to size)
 */
export function initAll(): Promise<{
  edge: typeof import('./edge/ruvector_edge');
  graph: typeof import('./graph/ruvector_graph_wasm');
  rvlite: typeof import('./rvlite/rvlite');
  sona: typeof import('./sona/ruvector_sona');
  dag: typeof import('./dag/ruvector_dag_wasm');
}>;

/**
 * Initialize only specific modules
 * @param moduleNames - Array of module names to init
 */
export function initModules(moduleNames: Array<'edge' | 'graph' | 'rvlite' | 'sona' | 'dag' | 'onnx'>): Promise<{
  edge?: typeof import('./edge/ruvector_edge');
  graph?: typeof import('./graph/ruvector_graph_wasm');
  rvlite?: typeof import('./rvlite/rvlite');
  sona?: typeof import('./sona/ruvector_sona');
  dag?: typeof import('./dag/ruvector_dag_wasm');
  onnx?: typeof import('./onnx/ruvector_onnx_embeddings_wasm');
}>;

export const quickStart: string;
