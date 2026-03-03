/**
 * Optimized matrix operations with memory pooling and SIMD-friendly patterns
 * Target: 50% memory reduction and improved cache locality
 */

import { Matrix, Vector, SparseMatrix, DenseMatrix } from './types.js';

// Memory pool for vector allocations
class VectorPool {
  private pools: Map<number, Vector[]> = new Map();
  private maxPoolSize = 100;

  acquire(size: number): Vector {
    const pool = this.pools.get(size);
    if (pool && pool.length > 0) {
      return pool.pop()!;
    }
    return new Array(size);
  }

  release(vector: Vector): void {
    const size = vector.length;
    vector.fill(0); // Clear for reuse

    let pool = this.pools.get(size);
    if (!pool) {
      pool = [];
      this.pools.set(size, pool);
    }

    if (pool.length < this.maxPoolSize) {
      pool.push(vector);
    }
  }

  clear(): void {
    this.pools.clear();
  }

  getStats(): { poolSizes: Record<number, number>; totalVectors: number } {
    const poolSizes: Record<number, number> = {};
    let totalVectors = 0;

    for (const [size, pool] of this.pools) {
      poolSizes[size] = pool.length;
      totalVectors += pool.length;
    }

    return { poolSizes, totalVectors };
  }
}

// Compressed Sparse Row (CSR) format for JavaScript
export class CSRMatrix {
  public values: Float64Array;
  public colIndices: Uint32Array;
  public rowPtr: Uint32Array;
  private rows: number;
  private cols: number;

  constructor(rows: number, cols: number, nnz: number) {
    this.rows = rows;
    this.cols = cols;
    this.values = new Float64Array(nnz);
    this.colIndices = new Uint32Array(nnz);
    this.rowPtr = new Uint32Array(rows + 1);
  }

  static fromCOO(matrix: SparseMatrix): CSRMatrix {
    const { values, rowIndices, colIndices } = matrix;
    const nnz = values.length;
    const csr = new CSRMatrix(matrix.rows, matrix.cols, nnz);

    // Sort by row, then column
    const triplets = Array.from({ length: nnz }, (_, i) => ({
      row: rowIndices[i],
      col: colIndices[i],
      val: values[i],
      index: i
    }));

    triplets.sort((a, b) => a.row - b.row || a.col - b.col);

    // Build CSR structure
    let currentRow = 0;
    let nnzCount = 0;

    for (const triplet of triplets) {
      // Skip zeros
      if (triplet.val === 0) continue;

      // Update row pointers
      while (currentRow < triplet.row) {
        csr.rowPtr[++currentRow] = nnzCount;
      }

      csr.values[nnzCount] = triplet.val;
      csr.colIndices[nnzCount] = triplet.col;
      nnzCount++;
    }

    // Finalize row pointers
    while (currentRow < matrix.rows) {
      csr.rowPtr[++currentRow] = nnzCount;
    }

    return csr;
  }

  // Cache-friendly matrix-vector multiplication with SIMD hints
  multiplyVector(x: Vector, result: Vector): void {
    result.fill(0);

    // Process 4 rows at a time for better cache locality
    const blockSize = 4;
    let rowBlock = 0;

    while (rowBlock < this.rows) {
      const endBlock = Math.min(rowBlock + blockSize, this.rows);

      for (let row = rowBlock; row < endBlock; row++) {
        const start = this.rowPtr[row];
        const end = this.rowPtr[row + 1];
        let sum = 0;

        // Unroll loop for SIMD optimization hints
        let i = start;
        for (; i < end - 3; i += 4) {
          sum += this.values[i] * x[this.colIndices[i]] +
                 this.values[i + 1] * x[this.colIndices[i + 1]] +
                 this.values[i + 2] * x[this.colIndices[i + 2]] +
                 this.values[i + 3] * x[this.colIndices[i + 3]];
        }

        // Handle remaining elements
        for (; i < end; i++) {
          sum += this.values[i] * x[this.colIndices[i]];
        }

        result[row] = sum;
      }

      rowBlock = endBlock;
    }
  }

  getEntry(row: number, col: number): number {
    const start = this.rowPtr[row];
    const end = this.rowPtr[row + 1];

    // Binary search for column
    let left = start;
    let right = end - 1;

    while (left <= right) {
      const mid = Math.floor((left + right) / 2);
      const midCol = this.colIndices[mid];

      if (midCol === col) {
        return this.values[mid];
      } else if (midCol < col) {
        left = mid + 1;
      } else {
        right = mid - 1;
      }
    }

    return 0;
  }

  // Memory-efficient row iteration
  *rowEntries(row: number): Generator<{ col: number; val: number }> {
    const start = this.rowPtr[row];
    const end = this.rowPtr[row + 1];

    for (let i = start; i < end; i++) {
      yield { col: this.colIndices[i], val: this.values[i] };
    }
  }

  getMemoryUsage(): number {
    return this.values.byteLength +
           this.colIndices.byteLength +
           this.rowPtr.byteLength;
  }

  getNnz(): number {
    return this.values.length;
  }

  getRows(): number {
    return this.rows;
  }

  getCols(): number {
    return this.cols;
  }
}

// Compressed Sparse Column (CSC) format for column-wise operations
export class CSCMatrix {
  public values: Float64Array;
  public rowIndices: Uint32Array;
  public colPtr: Uint32Array;
  private rows: number;
  private cols: number;

  constructor(rows: number, cols: number, nnz: number) {
    this.rows = rows;
    this.cols = cols;
    this.values = new Float64Array(nnz);
    this.rowIndices = new Uint32Array(nnz);
    this.colPtr = new Uint32Array(cols + 1);
  }

  static fromCSR(csr: CSRMatrix): CSCMatrix {
    const nnz = csr.getNnz();
    const csc = new CSCMatrix(csr.getRows(), csr.getCols(), nnz);

    // Convert CSR to triplets, then sort by column
    const triplets: Array<{ row: number; col: number; val: number }> = [];

    for (let row = 0; row < csr.getRows(); row++) {
      for (const entry of csr.rowEntries(row)) {
        triplets.push({ row, col: entry.col, val: entry.val });
      }
    }

    triplets.sort((a, b) => a.col - b.col || a.row - b.row);

    // Build CSC structure
    let currentCol = 0;
    let nnzCount = 0;

    for (const triplet of triplets) {
      while (currentCol < triplet.col) {
        csc.colPtr[++currentCol] = nnzCount;
      }

      csc.values[nnzCount] = triplet.val;
      csc.rowIndices[nnzCount] = triplet.row;
      nnzCount++;
    }

    while (currentCol < csc.cols) {
      csc.colPtr[++currentCol] = nnzCount;
    }

    return csc;
  }

  // Column-wise matrix-vector multiplication
  multiplyVector(x: Vector, result: Vector): void {
    result.fill(0);

    for (let col = 0; col < this.cols; col++) {
      const xCol = x[col];
      if (xCol === 0) continue;

      const start = this.colPtr[col];
      const end = this.colPtr[col + 1];

      // Vectorized accumulation
      for (let i = start; i < end; i++) {
        result[this.rowIndices[i]] += this.values[i] * xCol;
      }
    }
  }

  getMemoryUsage(): number {
    return this.values.byteLength +
           this.rowIndices.byteLength +
           this.colPtr.byteLength;
  }

  getNnz(): number {
    return this.values.length;
  }

  getRows(): number {
    return this.rows;
  }

  getCols(): number {
    return this.cols;
  }
}

// Memory streaming for large matrices
export class StreamingMatrix {
  private chunks: Map<number, CSRMatrix> = new Map();
  private chunkSize: number;
  private rows: number;
  private cols: number;
  private maxCachedChunks: number;

  constructor(rows: number, cols: number, chunkSize = 1000, maxCachedChunks = 10) {
    this.rows = rows;
    this.cols = cols;
    this.chunkSize = chunkSize;
    this.maxCachedChunks = maxCachedChunks;
  }

  static fromMatrix(matrix: Matrix, chunkSize = 1000): StreamingMatrix {
    const streaming = new StreamingMatrix(matrix.rows, matrix.cols, chunkSize);

    if (matrix.format === 'coo') {
      const sparse = matrix as SparseMatrix;
      const chunkData = new Map<number, Array<{ col: number; val: number }>>();

      for (let i = 0; i < sparse.values.length; i++) {
        const row = sparse.rowIndices[i];
        const chunkId = Math.floor(row / chunkSize);

        if (!chunkData.has(chunkId)) {
          chunkData.set(chunkId, []);
        }

        chunkData.get(chunkId)!.push({
          col: sparse.colIndices[i],
          val: sparse.values[i]
        });
      }

      // Convert each chunk to CSR
      for (const [chunkId, entries] of chunkData) {
        const chunkRows = Math.min(chunkSize, streaming.rows - chunkId * chunkSize);
        const chunkCSR = new CSRMatrix(chunkRows, streaming.cols, entries.length);

        // Build CSR for this chunk
        const rowData = new Map<number, Array<{ col: number; val: number }>>();

        for (const entry of entries) {
          const localRow = (chunkId * chunkSize) % chunkSize;
          if (!rowData.has(localRow)) {
            rowData.set(localRow, []);
          }
          rowData.get(localRow)!.push(entry);
        }

        // Fill CSR arrays
        let nnzCount = 0;
        for (let row = 0; row < chunkRows; row++) {
          chunkCSR.rowPtr[row] = nnzCount;
          const rowEntries = rowData.get(row) || [];

          rowEntries.sort((a, b) => a.col - b.col);

          for (const entry of rowEntries) {
            chunkCSR.values[nnzCount] = entry.val;
            chunkCSR.colIndices[nnzCount] = entry.col;
            nnzCount++;
          }
        }
        chunkCSR.rowPtr[chunkRows] = nnzCount;

        streaming.chunks.set(chunkId, chunkCSR);
      }
    }

    return streaming;
  }

  getChunk(chunkId: number): CSRMatrix | null {
    return this.chunks.get(chunkId) || null;
  }

  // Streaming matrix-vector multiplication
  multiplyVector(x: Vector, result: Vector): void {
    result.fill(0);

    const totalChunks = Math.ceil(this.rows / this.chunkSize);

    for (let chunkId = 0; chunkId < totalChunks; chunkId++) {
      const chunk = this.getChunk(chunkId);
      if (!chunk) continue;

      const startRow = chunkId * this.chunkSize;
      const chunkResult = new Array(chunk.getRows()).fill(0);

      chunk.multiplyVector(x, chunkResult);

      // Copy back to result
      for (let i = 0; i < chunkResult.length && startRow + i < this.rows; i++) {
        result[startRow + i] = chunkResult[i];
      }

      // Memory management: remove old chunks if cache is full
      if (this.chunks.size > this.maxCachedChunks) {
        const oldestChunk = Math.max(0, chunkId - this.maxCachedChunks);
        this.chunks.delete(oldestChunk);
      }
    }
  }

  getMemoryUsage(): number {
    let total = 0;
    for (const chunk of this.chunks.values()) {
      total += chunk.getMemoryUsage();
    }
    return total;
  }
}

// Optimized matrix operations with memory pooling
export class OptimizedMatrixOperations {
  private static vectorPool = new VectorPool();

  static getVectorPool(): VectorPool {
    return this.vectorPool;
  }

  // SIMD-optimized vector operations
  static vectorAdd(a: Vector, b: Vector, result?: Vector): Vector {
    const n = a.length;
    const out = result || this.vectorPool.acquire(n);

    // Process 4 elements at a time for SIMD
    let i = 0;
    for (; i < n - 3; i += 4) {
      out[i] = a[i] + b[i];
      out[i + 1] = a[i + 1] + b[i + 1];
      out[i + 2] = a[i + 2] + b[i + 2];
      out[i + 3] = a[i + 3] + b[i + 3];
    }

    // Handle remaining elements
    for (; i < n; i++) {
      out[i] = a[i] + b[i];
    }

    return out;
  }

  static vectorScale(vector: Vector, scalar: number, result?: Vector): Vector {
    const n = vector.length;
    const out = result || this.vectorPool.acquire(n);

    // SIMD-friendly unrolled loop
    let i = 0;
    for (; i < n - 3; i += 4) {
      out[i] = vector[i] * scalar;
      out[i + 1] = vector[i + 1] * scalar;
      out[i + 2] = vector[i + 2] * scalar;
      out[i + 3] = vector[i + 3] * scalar;
    }

    for (; i < n; i++) {
      out[i] = vector[i] * scalar;
    }

    return out;
  }

  static vectorDot(a: Vector, b: Vector): number {
    const n = a.length;
    let sum = 0;

    // Unrolled loop for SIMD optimization
    let i = 0;
    for (; i < n - 3; i += 4) {
      sum += a[i] * b[i] +
             a[i + 1] * b[i + 1] +
             a[i + 2] * b[i + 2] +
             a[i + 3] * b[i + 3];
    }

    for (; i < n; i++) {
      sum += a[i] * b[i];
    }

    return sum;
  }

  static vectorNorm2(vector: Vector): number {
    return Math.sqrt(this.vectorDot(vector, vector));
  }

  // Memory-efficient matrix format conversion
  static convertToOptimalFormat(matrix: Matrix): CSRMatrix | CSCMatrix {
    if (matrix.format === 'coo') {
      const sparse = matrix as SparseMatrix;

      // Choose format based on sparsity pattern and expected access
      const sparsity = sparse.values.length / (matrix.rows * matrix.cols);

      // CSR is generally better for row-wise access and matrix-vector multiplication
      return CSRMatrix.fromCOO(sparse);
    } else {
      // Convert dense to sparse first
      const sparse = this.denseToSparse(matrix as DenseMatrix);
      return CSRMatrix.fromCOO(sparse);
    }
  }

  private static denseToSparse(dense: DenseMatrix, tolerance = 1e-15): SparseMatrix {
    const values: number[] = [];
    const rowIndices: number[] = [];
    const colIndices: number[] = [];

    for (let i = 0; i < dense.rows; i++) {
      for (let j = 0; j < dense.cols; j++) {
        const value = dense.data[i][j];
        if (Math.abs(value) > tolerance) {
          values.push(value);
          rowIndices.push(i);
          colIndices.push(j);
        }
      }
    }

    return {
      rows: dense.rows,
      cols: dense.cols,
      values,
      rowIndices,
      colIndices,
      format: 'coo'
    };
  }

  // Memory usage profiling
  static profileMemoryUsage(matrix: CSRMatrix | CSCMatrix | StreamingMatrix): {
    matrixSize: number;
    nnz: number;
    memoryUsed: number;
    compressionRatio: number;
  } {
    const memoryUsed = matrix.getMemoryUsage();
    let nnz: number;
    let rows: number;
    let cols: number;

    if (matrix instanceof CSRMatrix || matrix instanceof CSCMatrix) {
      nnz = matrix.getNnz();
      rows = matrix.getRows();
      cols = matrix.getCols();
    } else {
      nnz = 0;
      rows = matrix['rows'];
      cols = matrix['cols'];
    }

    const denseMemory = rows * cols * 8; // 8 bytes per double
    const compressionRatio = denseMemory / memoryUsed;

    return {
      matrixSize: rows * cols,
      nnz,
      memoryUsed,
      compressionRatio
    };
  }

  // Cleanup memory pools
  static cleanup(): void {
    this.vectorPool.clear();
  }
}