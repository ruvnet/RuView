/**
 * @ruvector/node/gnn - GNN-specific exports
 *
 * Import GNN capabilities directly:
 * ```typescript
 * import { RuvectorLayer, TensorCompress } from '@ruvector/node/gnn';
 * ```
 */

export {
  RuvectorLayer,
  TensorCompress,
  differentiableSearch,
  hierarchicalForward,
  getCompressionLevel,
  init,
  type CompressionLevelConfig,
  type SearchResult
} from '@ruvector/gnn';
