import React, { useState, useEffect, useCallback } from 'react';
import { WorkerPool } from '../../crates/ruvector-wasm/src/worker-pool.js';
import { IndexedDBPersistence } from '../../crates/ruvector-wasm/src/indexeddb.js';

const DIMENSIONS = 384;
const WORKER_URL = '../../crates/ruvector-wasm/src/worker.js';
const WASM_URL = '../../crates/ruvector-wasm/pkg/ruvector_wasm.js';

function App() {
  const [workerPool, setWorkerPool] = useState(null);
  const [persistence, setPersistence] = useState(null);
  const [status, setStatus] = useState({ type: 'info', message: 'Initializing...' });
  const [stats, setStats] = useState({
    vectorCount: 0,
    poolSize: 0,
    busyWorkers: 0,
    cacheSize: 0,
    simdEnabled: false
  });
  const [searchResults, setSearchResults] = useState([]);
  const [benchmarkResults, setBenchmarkResults] = useState(null);

  // Initialize worker pool and persistence
  useEffect(() => {
    async function init() {
      try {
        // Initialize worker pool
        const pool = new WorkerPool(WORKER_URL, WASM_URL, {
          poolSize: navigator.hardwareConcurrency || 4,
          dimensions: DIMENSIONS,
          metric: 'cosine',
          useHnsw: true
        });

        await pool.init();
        setWorkerPool(pool);

        // Initialize persistence
        const persist = new IndexedDBPersistence();
        await persist.open();
        setPersistence(persist);

        setStatus({
          type: 'success',
          message: `Initialized with ${pool.poolSize} workers`
        });

        updateStats(pool, persist);
      } catch (error) {
        setStatus({
          type: 'error',
          message: `Initialization failed: ${error.message}`
        });
        console.error(error);
      }
    }

    init();

    // Cleanup on unmount
    return () => {
      if (workerPool) {
        workerPool.terminate();
      }
      if (persistence) {
        persistence.close();
      }
    };
  }, []);

  // Update statistics
  const updateStats = useCallback(async (pool, persist) => {
    if (!pool || !persist) return;

    try {
      const poolStats = pool.getStats();
      const dbStats = await persist.getStats();
      const count = await pool.len();

      setStats({
        vectorCount: count,
        poolSize: poolStats.poolSize,
        busyWorkers: poolStats.busyWorkers,
        cacheSize: dbStats.cacheSize,
        simdEnabled: false // Would need to detect from worker
      });
    } catch (error) {
      console.error('Failed to update stats:', error);
    }
  }, []);

  // Generate random vector
  const randomVector = useCallback((dimensions) => {
    const vector = new Float32Array(dimensions);
    for (let i = 0; i < dimensions; i++) {
      vector[i] = Math.random() * 2 - 1;
    }
    // Normalize
    const norm = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    for (let i = 0; i < dimensions; i++) {
      vector[i] /= norm;
    }
    return vector;
  }, []);

  // Insert random vectors
  const insertVectors = useCallback(async (count = 100) => {
    if (!workerPool || !persistence) return;

    const startTime = performance.now();
    setStatus({ type: 'info', message: `Inserting ${count} vectors...` });

    try {
      const entries = [];
      for (let i = 0; i < count; i++) {
        entries.push({
          vector: Array.from(randomVector(DIMENSIONS)),
          id: `vec_${Date.now()}_${i}`,
          metadata: { index: i, timestamp: Date.now() }
        });
      }

      // Insert via worker pool
      const ids = await workerPool.insertBatch(entries);

      // Save to IndexedDB
      await persistence.saveBatch(entries.map((e, i) => ({
        id: ids[i],
        vector: new Float32Array(e.vector),
        metadata: e.metadata
      })));

      const duration = performance.now() - startTime;
      const throughput = (count / (duration / 1000)).toFixed(0);

      setStatus({
        type: 'success',
        message: `Inserted ${ids.length} vectors in ${duration.toFixed(2)}ms (${throughput} ops/sec)`
      });

      updateStats(workerPool, persistence);
    } catch (error) {
      setStatus({
        type: 'error',
        message: `Insert failed: ${error.message}`
      });
      console.error(error);
    }
  }, [workerPool, persistence, randomVector, updateStats]);

  // Search for similar vectors
  const searchVectors = useCallback(async (k = 10) => {
    if (!workerPool) return;

    const startTime = performance.now();
    setStatus({ type: 'info', message: 'Searching...' });

    try {
      const query = Array.from(randomVector(DIMENSIONS));
      const results = await workerPool.search(query, k, null);

      const duration = performance.now() - startTime;

      setSearchResults(results);
      setStatus({
        type: 'success',
        message: `Found ${results.length} results in ${duration.toFixed(2)}ms`
      });
    } catch (error) {
      setStatus({
        type: 'error',
        message: `Search failed: ${error.message}`
      });
      console.error(error);
    }
  }, [workerPool, randomVector]);

  // Run benchmark
  const runBenchmark = useCallback(async () => {
    if (!workerPool) return;

    setStatus({ type: 'info', message: 'Running benchmark...' });
    setBenchmarkResults(null);

    try {
      const iterations = 1000;
      const queries = 100;

      // Benchmark insert
      const insertStart = performance.now();
      await insertVectors(iterations);
      const insertDuration = performance.now() - insertStart;
      const insertThroughput = (iterations / (insertDuration / 1000)).toFixed(0);

      // Benchmark search
      const searchStart = performance.now();
      const searchPromises = [];
      for (let i = 0; i < queries; i++) {
        const query = Array.from(randomVector(DIMENSIONS));
        searchPromises.push(workerPool.search(query, 10, null));
      }
      await Promise.all(searchPromises);
      const searchDuration = performance.now() - searchStart;
      const searchThroughput = (queries / (searchDuration / 1000)).toFixed(0);

      setBenchmarkResults({
        insertOpsPerSec: insertThroughput,
        searchOpsPerSec: searchThroughput,
        insertDuration: insertDuration.toFixed(2),
        searchDuration: searchDuration.toFixed(2)
      });

      setStatus({
        type: 'success',
        message: `Benchmark complete: Insert ${insertThroughput} ops/sec, Search ${searchThroughput} ops/sec`
      });
    } catch (error) {
      setStatus({
        type: 'error',
        message: `Benchmark failed: ${error.message}`
      });
      console.error(error);
    }
  }, [workerPool, insertVectors, randomVector]);

  // Save to IndexedDB
  const saveToIndexedDB = useCallback(async () => {
    if (!persistence) return;

    setStatus({ type: 'info', message: 'Saving to IndexedDB...' });

    try {
      const dbStats = await persistence.getStats();
      setStatus({
        type: 'success',
        message: `Saved ${dbStats.totalVectors} vectors to IndexedDB`
      });
    } catch (error) {
      setStatus({
        type: 'error',
        message: `Save failed: ${error.message}`
      });
      console.error(error);
    }
  }, [persistence]);

  // Load from IndexedDB
  const loadFromIndexedDB = useCallback(async () => {
    if (!persistence || !workerPool) return;

    setStatus({ type: 'info', message: 'Loading from IndexedDB...' });

    try {
      let totalLoaded = 0;

      await persistence.loadAll((progress) => {
        totalLoaded = progress.loaded;
        setStatus({
          type: 'info',
          message: `Loading... ${totalLoaded} vectors loaded`
        });

        // Insert batch into worker pool
        if (progress.vectors && progress.vectors.length > 0) {
          workerPool.insertBatch(progress.vectors).catch(console.error);
        }

        if (progress.complete) {
          setStatus({
            type: 'success',
            message: `Loaded ${totalLoaded} vectors from IndexedDB`
          });
          updateStats(workerPool, persistence);
        }
      });
    } catch (error) {
      setStatus({
        type: 'error',
        message: `Load failed: ${error.message}`
      });
      console.error(error);
    }
  }, [persistence, workerPool, updateStats]);

  return (
    <div style={styles.container}>
      <h1 style={styles.title}>🚀 Ruvector WASM + React</h1>
      <p style={styles.subtitle}>
        High-performance vector database with Web Workers
      </p>

      <div style={{ ...styles.status, ...styles[status.type] }}>
        {status.message}
      </div>

      <div style={styles.stats}>
        <StatCard label="Vectors" value={stats.vectorCount} />
        <StatCard label="Workers" value={`${stats.busyWorkers}/${stats.poolSize}`} />
        <StatCard label="Cache" value={stats.cacheSize} />
        <StatCard label="SIMD" value={stats.simdEnabled ? '✅' : '❌'} />
      </div>

      <div style={styles.controls}>
        <button style={styles.button} onClick={() => insertVectors(100)}>
          Insert 100 Vectors
        </button>
        <button style={styles.button} onClick={() => searchVectors(10)}>
          Search Similar
        </button>
        <button style={styles.button} onClick={runBenchmark}>
          Run Benchmark
        </button>
        <button style={styles.button} onClick={saveToIndexedDB}>
          Save to IndexedDB
        </button>
        <button style={styles.button} onClick={loadFromIndexedDB}>
          Load from IndexedDB
        </button>
      </div>

      {benchmarkResults && (
        <div style={styles.results}>
          <h3>Benchmark Results</h3>
          <div style={styles.resultGrid}>
            <div style={styles.resultItem}>
              <strong>Insert Throughput:</strong> {benchmarkResults.insertOpsPerSec} ops/sec
            </div>
            <div style={styles.resultItem}>
              <strong>Search Throughput:</strong> {benchmarkResults.searchOpsPerSec} ops/sec
            </div>
            <div style={styles.resultItem}>
              <strong>Insert Duration:</strong> {benchmarkResults.insertDuration}ms
            </div>
            <div style={styles.resultItem}>
              <strong>Search Duration:</strong> {benchmarkResults.searchDuration}ms
            </div>
          </div>
        </div>
      )}

      {searchResults.length > 0 && (
        <div style={styles.results}>
          <h3>Search Results</h3>
          {searchResults.map((result, i) => (
            <div key={i} style={styles.resultItem}>
              <strong>#{i + 1}:</strong> {result.id} - Score: {result.score.toFixed(6)}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function StatCard({ label, value }) {
  return (
    <div style={styles.statCard}>
      <div style={styles.statValue}>{value}</div>
      <div style={styles.statLabel}>{label}</div>
    </div>
  );
}

const styles = {
  container: {
    maxWidth: '1200px',
    margin: '0 auto',
    padding: '20px',
    fontFamily: 'system-ui, -apple-system, sans-serif'
  },
  title: {
    fontSize: '2.5em',
    color: '#667eea',
    marginBottom: '10px'
  },
  subtitle: {
    fontSize: '1.1em',
    color: '#666',
    marginBottom: '30px'
  },
  status: {
    padding: '15px',
    borderRadius: '8px',
    marginBottom: '20px',
    fontWeight: '500'
  },
  info: {
    background: '#e3f2fd',
    color: '#1976d2'
  },
  success: {
    background: '#e8f5e9',
    color: '#388e3c'
  },
  error: {
    background: '#ffebee',
    color: '#c62828'
  },
  stats: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
    gap: '15px',
    marginBottom: '30px'
  },
  statCard: {
    background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    color: 'white',
    padding: '20px',
    borderRadius: '8px',
    textAlign: 'center'
  },
  statValue: {
    fontSize: '2em',
    fontWeight: 'bold',
    marginBottom: '5px'
  },
  statLabel: {
    fontSize: '0.9em',
    opacity: 0.9
  },
  controls: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
    gap: '10px',
    marginBottom: '30px'
  },
  button: {
    padding: '12px 24px',
    border: 'none',
    borderRadius: '6px',
    fontSize: '14px',
    fontWeight: '600',
    cursor: 'pointer',
    background: '#667eea',
    color: 'white',
    transition: 'all 0.3s ease'
  },
  results: {
    background: '#f8f9fa',
    borderRadius: '8px',
    padding: '20px',
    marginTop: '20px'
  },
  resultGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
    gap: '10px',
    marginTop: '15px'
  },
  resultItem: {
    background: 'white',
    padding: '12px',
    borderRadius: '6px',
    borderLeft: '4px solid #667eea'
  }
};

export default App;
