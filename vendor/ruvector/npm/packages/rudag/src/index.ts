/**
 * @ruvector/rudag - Self-learning DAG query optimization
 *
 * Provides WASM-accelerated DAG operations with IndexedDB persistence
 * for browser environments.
 */

export {
  RuDag,
  DagOperator,
  AttentionMechanism,
  type DagNode,
  type DagEdge,
  type CriticalPath,
  type RuDagOptions,
} from './dag';

export {
  DagStorage,
  MemoryStorage,
  createStorage,
  isIndexedDBAvailable,
  type StoredDag,
  type DagStorageOptions,
} from './storage';

// Version info
export const VERSION = '0.1.0';

/**
 * Quick start example:
 *
 * ```typescript
 * import { RuDag, DagOperator, AttentionMechanism } from '@ruvector/rudag';
 *
 * // Create and initialize a DAG
 * const dag = await new RuDag({ name: 'my-query' }).init();
 *
 * // Add nodes (query operators)
 * const scan = dag.addNode(DagOperator.SCAN, 10.0);
 * const filter = dag.addNode(DagOperator.FILTER, 2.0);
 * const project = dag.addNode(DagOperator.PROJECT, 1.0);
 *
 * // Connect nodes
 * dag.addEdge(scan, filter);
 * dag.addEdge(filter, project);
 *
 * // Get critical path
 * const { path, cost } = dag.criticalPath();
 * console.log(`Critical path: ${path.join(' -> ')}, total cost: ${cost}`);
 *
 * // Compute attention scores
 * const scores = dag.attention(AttentionMechanism.CRITICAL_PATH);
 * console.log('Attention scores:', scores);
 *
 * // DAG is auto-saved to IndexedDB
 * // Load it later
 * const loadedDag = await RuDag.load(dag.getId());
 * ```
 */
