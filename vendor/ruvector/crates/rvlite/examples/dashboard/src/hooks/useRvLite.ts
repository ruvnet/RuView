import { useState, useEffect, useCallback, useRef } from 'react';

// Import types from the WASM package
// The actual module will be loaded dynamically to avoid bundler issues

// Types matching RvLite WASM API
export interface RvLiteConfig {
  dimensions: number;
  distance_metric: string;
}

export interface VectorEntry {
  id: string;
  vector: number[];
  metadata?: Record<string, unknown>;
}

export interface SearchResult {
  id: string;
  score: number;
  metadata?: Record<string, unknown>;
}

export interface CypherResult {
  nodes?: Array<{
    id: string;
    labels: string[];
    properties: Record<string, unknown>;
  }>;
  relationships?: Array<{
    id: string;
    type: string;
    start: string;
    end: string;
    properties: Record<string, unknown>;
  }>;
  message?: string;
}

export interface SparqlResult {
  type: 'select' | 'ask' | 'construct' | 'describe' | 'update';
  variables?: string[];
  bindings?: Array<Record<string, string>>;
  result?: boolean;
  triples?: Array<{
    subject: string;
    predicate: string;
    object: string;
  }>;
  success?: boolean;
}

export interface SqlResult {
  rows?: Array<Record<string, unknown>>;
  rowsAffected?: number;
  message?: string;
}

export interface RvLiteStats {
  vectorCount: number;
  dimensions: number;
  distanceMetric: string;
  tripleCount: number;
  graphNodeCount: number;
  graphEdgeCount: number;
  features: string[];
  version: string;
  memoryUsage: string;
}

// Internal interface for WASM module
interface WasmRvLite {
  is_ready: () => boolean;
  get_version: () => string;
  get_features: () => string[];
  get_config: () => { get_dimensions: () => number; get_distance_metric: () => string };

  insert: (vector: Float32Array, metadata: unknown) => string;
  insert_with_id: (id: string, vector: Float32Array, metadata: unknown) => void;
  search: (queryVector: Float32Array, k: number) => SearchResult[];
  search_with_filter: (queryVector: Float32Array, k: number, filter: unknown) => SearchResult[];
  get: (id: string) => VectorEntry | null;
  delete: (id: string) => boolean;
  len: () => number;
  is_empty: () => boolean;

  sql: (query: string) => SqlResult;

  cypher: (query: string) => CypherResult;
  cypher_stats: () => { nodes: number; relationships: number };
  cypher_clear: () => void;

  sparql: (query: string) => SparqlResult;
  add_triple: (subject: string, predicate: string, object: string) => void;
  triple_count: () => number;
  clear_triples: () => void;

  save: () => Promise<unknown>;
  init_storage: () => Promise<unknown>;
  export_json: () => Record<string, unknown>;
  import_json: (json: unknown) => void;
}

interface WasmRvLiteConfig {
  get_dimensions: () => number;
  get_distance_metric: () => string;
  with_distance_metric: (metric: string) => WasmRvLiteConfig;
}

interface WasmModule {
  default: (path?: string) => Promise<unknown>;
  init: () => void;
  RvLite: {
    new(config: WasmRvLiteConfig): WasmRvLite;
    default: () => WasmRvLite;
    clear_storage: () => Promise<unknown>;
    has_saved_state: () => Promise<boolean>;
    is_storage_available: () => boolean;
  };
  RvLiteConfig: {
    new(dimensions: number): WasmRvLiteConfig;
  };
}

// Wrapper to normalize WASM API
interface RvLiteInstance {
  is_ready: () => boolean;
  get_version: () => string;
  get_features: () => string[];
  get_config: () => RvLiteConfig;

  insert: (vector: number[], metadata?: Record<string, unknown>) => string;
  insert_with_id: (id: string, vector: number[], metadata?: Record<string, unknown>) => void;
  search: (queryVector: number[], k: number) => SearchResult[];
  search_with_filter: (queryVector: number[], k: number, filter: Record<string, unknown>) => SearchResult[];
  get: (id: string) => VectorEntry | null;
  delete: (id: string) => boolean;
  len: () => number;
  is_empty: () => boolean;

  sql: (query: string) => SqlResult;

  cypher: (query: string) => CypherResult;
  cypher_stats: () => { nodes: number; relationships: number };
  cypher_clear: () => void;

  sparql: (query: string) => SparqlResult;
  add_triple: (subject: string, predicate: string, object: string) => void;
  triple_count: () => number;
  clear_triples: () => void;

  save: () => Promise<boolean>;
  has_saved_state: () => Promise<boolean>;
  clear_storage: () => Promise<boolean>;
  export_json: () => Record<string, unknown>;
  import_json: (json: Record<string, unknown>) => void;
}

// Wrapper for the real WASM module
function createWasmWrapper(wasm: WasmRvLite, WasmModule: WasmModule['RvLite']): RvLiteInstance {
  return {
    is_ready: () => wasm.is_ready(),
    get_version: () => wasm.get_version(),
    get_features: () => {
      const features = wasm.get_features();
      return Array.isArray(features) ? features : [];
    },
    get_config: () => {
      const config = wasm.get_config();
      // Config may return an object with getter methods or a plain JSON object depending on WASM version
      // Try getter methods first, fallback to direct property access
      const dims = typeof config?.get_dimensions === 'function'
        ? config.get_dimensions()
        : (config as unknown as { dimensions?: number })?.dimensions ?? 128;
      const metric = typeof config?.get_distance_metric === 'function'
        ? config.get_distance_metric()
        : (config as unknown as { distance_metric?: string })?.distance_metric ?? 'cosine';
      return {
        dimensions: dims,
        distance_metric: metric,
      };
    },

    insert: (vector, metadata) => {
      return wasm.insert(new Float32Array(vector), metadata || null);
    },
    insert_with_id: (id, vector, metadata) => {
      wasm.insert_with_id(id, new Float32Array(vector), metadata || null);
    },
    search: (queryVector, k) => {
      const results = wasm.search(new Float32Array(queryVector), k);
      return Array.isArray(results) ? results : [];
    },
    search_with_filter: (queryVector, k, filter) => {
      const results = wasm.search_with_filter(new Float32Array(queryVector), k, filter);
      return Array.isArray(results) ? results : [];
    },
    get: (id) => wasm.get(id),
    delete: (id) => wasm.delete(id),
    len: () => wasm.len(),
    is_empty: () => wasm.is_empty(),

    sql: (query) => wasm.sql(query) || { message: 'No result' },

    cypher: (query) => wasm.cypher(query) || { message: 'No result' },
    cypher_stats: () => {
      const stats = wasm.cypher_stats();
      // WASM returns { node_count, edge_count }, normalize to { nodes, relationships }
      if (stats && typeof stats === 'object') {
        const s = stats as Record<string, unknown>;
        return {
          nodes: (s.node_count as number) ?? (s.nodes as number) ?? 0,
          relationships: (s.edge_count as number) ?? (s.relationships as number) ?? 0,
        };
      }
      return { nodes: 0, relationships: 0 };
    },
    cypher_clear: () => wasm.cypher_clear(),

    sparql: (query) => wasm.sparql(query) || { type: 'select' as const },
    add_triple: (subject, predicate, object) => wasm.add_triple(subject, predicate, object),
    triple_count: () => wasm.triple_count(),
    clear_triples: () => wasm.clear_triples(),

    save: async () => {
      try {
        await wasm.init_storage();
        await wasm.save();
        return true;
      } catch (e) {
        console.error('Save failed:', e);
        return false;
      }
    },
    has_saved_state: async () => {
      try {
        return await WasmModule.has_saved_state();
      } catch {
        return false;
      }
    },
    clear_storage: async () => {
      try {
        await WasmModule.clear_storage();
        return true;
      } catch {
        return false;
      }
    },
    export_json: () => wasm.export_json() || {},
    import_json: (json) => wasm.import_json(json),
  };
}

// No mock implementation - WASM is required

// Hook for using RvLite
export function useRvLite(initialDimensions: number = 128, initialDistanceMetric: string = 'cosine') {
  const [isReady, setIsReady] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isWasm, setIsWasm] = useState(false);
  const [currentDimensions] = useState(initialDimensions);
  const [currentMetric, setCurrentMetric] = useState(initialDistanceMetric);
  const [stats, setStats] = useState<RvLiteStats>({
    vectorCount: 0,
    dimensions: initialDimensions,
    distanceMetric: initialDistanceMetric,
    tripleCount: 0,
    graphNodeCount: 0,
    graphEdgeCount: 0,
    features: [],
    version: '',
    memoryUsage: '0 KB',
  });

  // Storage status
  const [storageStatus, setStorageStatus] = useState<{
    available: boolean;
    hasSavedState: boolean;
    estimatedSize: number;
  }>({ available: false, hasSavedState: false, estimatedSize: 0 });

  const dbRef = useRef<RvLiteInstance | null>(null);
  const wasmModuleRef = useRef<WasmModule | null>(null);
  const initRef = useRef(false);

  // Initialize RvLite
  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;

    const init = async () => {
      setIsLoading(true);
      setError(null);

      try {
        // Try to load actual WASM module using script injection
        // This avoids Vite's module transformation which breaks the WASM bindings
        const wasmJsPath = '/pkg/rvlite.js';
        const wasmBinaryPath = '/pkg/rvlite_bg.wasm';

        // BBS-style initialization display
        const bbsInit = () => {
          const cyan = 'color: #00d4ff; font-weight: bold';
          const green = 'color: #00ff88; font-weight: bold';
          const yellow = 'color: #ffcc00; font-weight: bold';
          const magenta = 'color: #ff00ff; font-weight: bold';
          const dim = 'color: #888888';

          console.log('%c╔══════════════════════════════════════════════════════════════════╗', cyan);
          console.log('%c║                                                                  ║', cyan);
          console.log('%c║  %c██████╗ ██╗   ██╗██╗     ██╗████████╗███████╗%c                 ║', cyan, green, cyan);
          console.log('%c║  %c██╔══██╗██║   ██║██║     ██║╚══██╔══╝██╔════╝%c                 ║', cyan, green, cyan);
          console.log('%c║  %c██████╔╝██║   ██║██║     ██║   ██║   █████╗%c                   ║', cyan, green, cyan);
          console.log('%c║  %c██╔══██╗╚██╗ ██╔╝██║     ██║   ██║   ██╔══╝%c                   ║', cyan, green, cyan);
          console.log('%c║  %c██║  ██║ ╚████╔╝ ███████╗██║   ██║   ███████╗%c                 ║', cyan, green, cyan);
          console.log('%c║  %c╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝   ╚═╝   ╚══════╝%c                 ║', cyan, green, cyan);
          console.log('%c║                                                                  ║', cyan);
          console.log('%c║  %cVector Database + SQL + SPARQL + Cypher%c                        ║', cyan, yellow, cyan);
          console.log('%c║  %cBrowser-Native WASM Implementation%c                             ║', cyan, dim, cyan);
          console.log('%c║                                                                  ║', cyan);
          console.log('%c╠══════════════════════════════════════════════════════════════════╣', cyan);
          console.log('%c║  %c[ SYSTEM INITIALIZATION ]%c                                      ║', cyan, magenta, cyan);
          console.log('%c╚══════════════════════════════════════════════════════════════════╝', cyan);
        };

        const bbsStatus = (label: string, status: string, ok: boolean) => {
          const cyan = 'color: #00d4ff';
          const statusColor = ok ? 'color: #00ff88; font-weight: bold' : 'color: #ff4444; font-weight: bold';
          console.log(`%c  ├─ ${label.padEnd(30)} %c[${status}]`, cyan, statusColor);
        };

        const bbsComplete = (version: string, _wasmLoaded: boolean, config: { dimensions: number; distanceMetric: string }) => {
          const cyan = 'color: #00d4ff; font-weight: bold';
          const green = 'color: #00ff88; font-weight: bold';
          const yellow = 'color: #ffcc00';
          const white = 'color: #ffffff';

          console.log('%c╔══════════════════════════════════════════════════════════════════╗', cyan);
          console.log('%c║  %c✓ RVLITE INITIALIZED SUCCESSFULLY%c                               ║', cyan, green, cyan);
          console.log('%c╠══════════════════════════════════════════════════════════════════╣', cyan);
          console.log(`%c║  %cVersion:%c      ${version.padEnd(48)}%c║`, cyan, yellow, white, cyan);
          console.log(`%c║  %cBackend:%c      ${'WebAssembly (WASM)'.padEnd(48)}%c║`, cyan, yellow, white, cyan);
          console.log(`%c║  %cDimensions:%c   ${String(config.dimensions).padEnd(48)}%c║`, cyan, yellow, white, cyan);
          console.log(`%c║  %cMetric:%c       ${config.distanceMetric.padEnd(48)}%c║`, cyan, yellow, white, cyan);
          console.log('%c╠══════════════════════════════════════════════════════════════════╣', cyan);
          console.log('%c║  %cFeatures:%c                                                       ║', cyan, yellow, cyan);
          console.log('%c║    ✓ Vector Search (k-NN)          ✓ SQL Queries%c                ║', green, cyan);
          console.log('%c║    ✓ SPARQL (RDF Triple Store)     ✓ Cypher (Graph DB)%c          ║', green, cyan);
          console.log('%c║    ✓ IndexedDB Persistence         ✓ JSON Import/Export%c         ║', green, cyan);
          console.log('%c║    ✓ Metadata Filtering            ✓ Multiple Metrics%c           ║', green, cyan);
          console.log('%c╠══════════════════════════════════════════════════════════════════╣', cyan);
          console.log('%c║  %cDistance Metrics:%c                                               ║', cyan, yellow, cyan);
          console.log('%c║    • cosine     - Cosine Similarity (angular distance)%c          ║', white, cyan);
          console.log('%c║    • euclidean  - L2 Norm (straight-line distance)%c              ║', white, cyan);
          console.log('%c║    • dotproduct - Inner Product (projection similarity)%c         ║', white, cyan);
          console.log('%c║    • manhattan  - L1 Norm (taxicab distance)%c                    ║', white, cyan);
          console.log('%c╚══════════════════════════════════════════════════════════════════╝', cyan);
        };

        bbsInit();

        let loadedIsWasm = false;
        let loadedVersion = '';

        try {
          bbsStatus('WASM Binary', 'LOADING', true);

          // Check if WASM binary exists
          const wasmResponse = await fetch(wasmBinaryPath, { method: 'HEAD' });
          if (!wasmResponse.ok) {
            throw new Error('WASM binary not found');
          }
          bbsStatus('WASM Binary', 'OK', true);

          bbsStatus('JavaScript Bindings', 'LOADING', true);
          // Dynamically import the WASM module
          // Use a blob URL to avoid Vite's module transformation
          const jsResponse = await fetch(wasmJsPath);
          if (!jsResponse.ok) {
            throw new Error(`Failed to fetch WASM JS: ${jsResponse.status}`);
          }
          bbsStatus('JavaScript Bindings', 'OK', true);

          const jsCode = await jsResponse.text();

          // Create a module from the JS code
          const blob = new Blob([jsCode], { type: 'application/javascript' });
          const blobUrl = URL.createObjectURL(blob);

          try {
            bbsStatus('WebAssembly Module', 'INSTANTIATING', true);
            const wasmModule = await import(/* @vite-ignore */ blobUrl) as WasmModule;

            // Initialize the WASM module with the correct path to the binary
            // The WASM module accepts either string path or object with module_or_path
            await (wasmModule.default as (path?: unknown) => Promise<unknown>)(wasmBinaryPath);
            wasmModule.init();
            bbsStatus('WebAssembly Module', 'OK', true);

            bbsStatus('RvLite Configuration', 'CONFIGURING', true);
            // Create config with dimensions and distance metric
            let config = new wasmModule.RvLiteConfig(currentDimensions);
            if (currentMetric && currentMetric !== 'cosine') {
              config = config.with_distance_metric(currentMetric);
            }
            // Store the WASM module for later use (distance metric changes)
            wasmModuleRef.current = wasmModule;
            bbsStatus('RvLite Configuration', 'OK', true);

            bbsStatus('Database Instance', 'CREATING', true);
            // Create RvLite instance
            const wasmDb = new wasmModule.RvLite(config);

            // Wrap it with our normalized interface
            dbRef.current = createWasmWrapper(wasmDb, wasmModule.RvLite);
            loadedIsWasm = true;
            loadedVersion = wasmDb.get_version();
            setIsWasm(true);
            bbsStatus('Database Instance', 'OK', true);

            bbsStatus('Vector Search Engine', 'READY', true);
            bbsStatus('SQL Query Engine', 'READY', true);
            bbsStatus('SPARQL Engine', 'READY', true);
            bbsStatus('Cypher Graph Engine', 'READY', true);
            bbsStatus('IndexedDB Persistence', 'AVAILABLE', true);
          } finally {
            URL.revokeObjectURL(blobUrl);
          }
        } catch (wasmError) {
          bbsStatus('WASM Module', 'FAILED TO LOAD', false);
          const errorMsg = wasmError instanceof Error ? wasmError.message : 'WASM module failed to load';
          throw new Error(`WASM required but failed to load: ${errorMsg}`);
        }

        if (dbRef.current) {
          setIsReady(true);
          // Display completion banner
          bbsComplete(loadedVersion, loadedIsWasm, { dimensions: currentDimensions, distanceMetric: currentMetric });
          // Update stats after a short delay to ensure WASM is fully initialized
          setTimeout(() => updateStatsInternal(), 100);
          // Update storage status
          checkStorageStatus();
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        setError(message);
        console.error('RvLite initialization failed:', err);
      } finally {
        setIsLoading(false);
      }
    };

    init();
  }, [currentDimensions, currentMetric]);

  // Internal stats update (not a callback to avoid dependency issues)
  const updateStatsInternal = () => {
    if (!dbRef.current) return;

    try {
      const db = dbRef.current;
      const cypherStats = db.cypher_stats();
      const config = db.get_config();

      setStats({
        vectorCount: db.len(),
        dimensions: config.dimensions,
        distanceMetric: config.distance_metric,
        tripleCount: db.triple_count(),
        graphNodeCount: cypherStats.nodes ?? 0,
        graphEdgeCount: cypherStats.relationships ?? 0,
        features: db.get_features(),
        version: db.get_version(),
        memoryUsage: `${Math.round((db.len() * currentDimensions * 4) / 1024)} KB`,
      });
    } catch (e) {
      console.error('Failed to update stats:', e);
    }
  };

  // Update stats
  const updateStats = useCallback(() => {
    updateStatsInternal();
  }, []);

  // Vector operations
  const insertVector = useCallback((vector: number[], metadata?: Record<string, unknown>) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    const id = dbRef.current.insert(vector, metadata);
    updateStatsInternal();
    return id;
  }, []);

  const insertVectorWithId = useCallback((id: string, vector: number[], metadata?: Record<string, unknown>) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    dbRef.current.insert_with_id(id, vector, metadata);
    updateStatsInternal();
  }, []);

  const searchVectors = useCallback((queryVector: number[], k: number = 10) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.search(queryVector, k);
  }, []);

  const searchVectorsWithFilter = useCallback((queryVector: number[], k: number, filter: Record<string, unknown>) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.search_with_filter(queryVector, k, filter);
  }, []);

  const getVector = useCallback((id: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.get(id);
  }, []);

  const deleteVector = useCallback((id: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    const result = dbRef.current.delete(id);
    updateStatsInternal();
    return result;
  }, []);

  const getAllVectors = useCallback(() => {
    if (!dbRef.current) return [];
    const randomVector = Array(currentDimensions).fill(0).map(() => Math.random());
    const count = dbRef.current.len();
    if (count === 0) return [];
    return dbRef.current.search(randomVector, count);
  }, [currentDimensions]);

  // SQL operations
  const executeSql = useCallback((query: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    const result = dbRef.current.sql(query);
    updateStatsInternal();
    return result;
  }, []);

  // Cypher operations
  const executeCypher = useCallback((query: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    const result = dbRef.current.cypher(query);
    updateStatsInternal();
    return result;
  }, []);

  const getCypherStats = useCallback(() => {
    if (!dbRef.current) return { nodes: 0, relationships: 0 };
    return dbRef.current.cypher_stats();
  }, []);

  const clearCypher = useCallback(() => {
    if (!dbRef.current) return;
    dbRef.current.cypher_clear();
    updateStatsInternal();
  }, []);

  // SPARQL operations
  const executeSparql = useCallback((query: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.sparql(query);
  }, []);

  const addTriple = useCallback((subject: string, predicate: string, object: string) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    dbRef.current.add_triple(subject, predicate, object);
    updateStatsInternal();
  }, []);

  const clearTriples = useCallback(() => {
    if (!dbRef.current) return;
    dbRef.current.clear_triples();
    updateStatsInternal();
  }, []);

  // Persistence operations
  const saveDatabase = useCallback(async () => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.save();
  }, []);

  const exportDatabase = useCallback(() => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    return dbRef.current.export_json();
  }, []);

  const importDatabase = useCallback((json: Record<string, unknown>) => {
    if (!dbRef.current) throw new Error('RvLite not initialized');
    dbRef.current.import_json(json);
    updateStatsInternal();
  }, []);

  const clearDatabase = useCallback(async () => {
    if (!dbRef.current) return;
    await dbRef.current.clear_storage();
    dbRef.current.cypher_clear();
    dbRef.current.clear_triples();
    updateStatsInternal();
  }, []);

  // Generate random vector
  const generateVector = useCallback((dim?: number) => {
    const d = dim || currentDimensions;
    return Array(d).fill(0).map(() => Math.random() * 2 - 1);
  }, [currentDimensions]);

  // Check storage status
  const checkStorageStatus = useCallback(async () => {
    if (!dbRef.current) return;

    try {
      const hasSaved = await dbRef.current.has_saved_state();
      const vectorCount = dbRef.current.len();
      const tripleCount = dbRef.current.triple_count();
      const cypherStats = dbRef.current.cypher_stats();

      // Estimate storage size (vectors + triples + graph)
      const vectorBytes = vectorCount * currentDimensions * 4; // float32
      const tripleBytes = tripleCount * 200; // estimate per triple
      const graphBytes = (cypherStats.nodes + cypherStats.relationships) * 100;
      const estimatedSize = vectorBytes + tripleBytes + graphBytes;

      setStorageStatus({
        available: true,
        hasSavedState: hasSaved,
        estimatedSize,
      });
    } catch {
      setStorageStatus(prev => ({ ...prev, available: false }));
    }
  }, [currentDimensions]);

  // Change distance metric (recreates the database instance)
  const changeDistanceMetric = useCallback(async (newMetric: string): Promise<boolean> => {
    if (!wasmModuleRef.current || !isWasm) {
      // WASM required - no fallback
      console.error('WASM module required for distance metric change');
      return false;
    }

    try {
      // Export current data
      const exportedData = dbRef.current?.export_json();

      // Create new config with new metric
      const wasmModule = wasmModuleRef.current;
      let config = new wasmModule.RvLiteConfig(currentDimensions);
      if (newMetric !== 'cosine') {
        config = config.with_distance_metric(newMetric);
      }

      // Create new instance
      const wasmDb = new wasmModule.RvLite(config);
      dbRef.current = createWasmWrapper(wasmDb, wasmModule.RvLite);

      // Re-import the data
      if (exportedData) {
        dbRef.current.import_json(exportedData);
      }

      setCurrentMetric(newMetric);
      updateStatsInternal();

      console.log(`%c  Distance metric changed to: ${newMetric}`, 'color: #00ff88; font-weight: bold');
      return true;
    } catch (err) {
      console.error('Failed to change distance metric:', err);
      return false;
    }
  }, [isWasm, currentDimensions]);

  // Clear IndexedDB storage
  const clearStorageData = useCallback(async (): Promise<boolean> => {
    if (!dbRef.current) return false;

    try {
      const result = await dbRef.current.clear_storage();
      await checkStorageStatus();
      return result;
    } catch {
      return false;
    }
  }, [checkStorageStatus]);

  return {
    // State
    isReady,
    isLoading,
    isWasm,
    error,
    stats,
    storageStatus,

    // Vector operations
    insertVector,
    insertVectorWithId,
    searchVectors,
    searchVectorsWithFilter,
    getVector,
    deleteVector,
    getAllVectors,

    // SQL
    executeSql,

    // Cypher
    executeCypher,
    getCypherStats,
    clearCypher,

    // SPARQL
    executeSparql,
    addTriple,
    clearTriples,

    // Persistence
    saveDatabase,
    exportDatabase,
    importDatabase,
    clearDatabase,

    // Configuration
    changeDistanceMetric,
    clearStorageData,
    checkStorageStatus,

    // Utilities
    generateVector,
    updateStats,
  };
}

export default useRvLite;
