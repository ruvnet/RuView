// TypeScript definitions for RuVector Graph WASM

export function init(input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module): Promise<void>;

export function version(): string;

export class GraphDB {
  constructor(metric?: string);

  query(cypher: string): Promise<QueryResult>;
  createNode(labels: string[], properties: object): string;
  getNode(id: string): JsNode | null;
  deleteNode(id: string): boolean;

  createEdge(from: string, to: string, type: string, properties: object): string;
  getEdge(id: string): JsEdge | null;
  deleteEdge(id: string): boolean;

  createHyperedge(nodes: string[], description: string, embedding?: number[], confidence?: number): string;
  getHyperedge(id: string): JsHyperedge | null;

  importCypher(statements: string[]): Promise<number>;
  exportCypher(): string;

  stats(): GraphStats;
}

export class JsNode {
  readonly id: string;
  readonly labels: string[];
  readonly properties: object;
  readonly embedding?: number[];

  getProperty(key: string): any;
  hasLabel(label: string): boolean;
}

export class JsEdge {
  readonly id: string;
  readonly from: string;
  readonly to: string;
  readonly type: string;
  readonly properties: object;

  getProperty(key: string): any;
}

export class JsHyperedge {
  readonly id: string;
  readonly nodes: string[];
  readonly description: string;
  readonly embedding: number[];
  readonly confidence: number;
  readonly properties: object;
  readonly order: number;
}

export class QueryResult {
  readonly nodes: JsNode[];
  readonly edges: JsEdge[];
  readonly hyperedges: JsHyperedge[];
  readonly data: object[];
  readonly count: number;

  isEmpty(): boolean;
}

export class AsyncQueryExecutor {
  constructor(batchSize?: number);

  executeStreaming(query: string): Promise<any>;
  executeInWorker(query: string): Promise<any>;

  batchSize: number;
}

export class AsyncTransaction {
  constructor();

  addOperation(operation: string): void;
  commit(): Promise<any>;
  rollback(): void;

  readonly operationCount: number;
  readonly isCommitted: boolean;
}

export class BatchOperations {
  constructor(maxBatchSize?: number);

  executeBatch(statements: string[]): Promise<any>;

  readonly maxBatchSize: number;
}

export class ResultStream {
  constructor(chunkSize?: number);

  nextChunk(): Promise<any>;
  reset(): void;

  readonly offset: number;
  readonly chunkSize: number;
}

export interface GraphStats {
  nodeCount: number;
  edgeCount: number;
  hyperedgeCount: number;
  hypergraphEntities: number;
  hypergraphEdges: number;
  avgEntityDegree: number;
}
