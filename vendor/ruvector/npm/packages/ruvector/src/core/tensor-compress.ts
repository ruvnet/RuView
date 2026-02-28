/**
 * TensorCompress - Adaptive tensor compression for intelligence storage
 * Provides 10x memory savings with access-frequency based compression
 */

export type CompressionLevel = 'none' | 'half' | 'pq8' | 'pq4' | 'binary';

export interface CompressedTensor {
  data: number[] | Uint8Array | Uint16Array | Float32Array;
  level: CompressionLevel;
  originalDim: number;
  accessCount: number;
  lastAccess: number;
  created: number;
}

export interface CompressionStats {
  totalTensors: number;
  byLevel: Record<CompressionLevel, number>;
  originalBytes: number;
  compressedBytes: number;
  savingsPercent: number;
}

export interface CompressionConfig {
  hotThreshold: number;      // >0.8 access freq = none
  warmThreshold: number;     // >0.4 access freq = half
  coolThreshold: number;     // >0.1 access freq = pq8
  coldThreshold: number;     // >0.01 access freq = pq4
  // <=0.01 = binary
  autoCompress: boolean;
  compressIntervalMs: number;
}

const DEFAULT_CONFIG: CompressionConfig = {
  hotThreshold: 0.8,
  warmThreshold: 0.4,
  coolThreshold: 0.1,
  coldThreshold: 0.01,
  autoCompress: true,
  compressIntervalMs: 60000, // 1 minute
};

export class TensorCompress {
  private config: CompressionConfig;
  private tensors: Map<string, CompressedTensor> = new Map();
  private totalAccesses: number = 0;
  private compressTimer: NodeJS.Timeout | null = null;

  constructor(config: Partial<CompressionConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    if (this.config.autoCompress) {
      this.startAutoCompress();
    }
  }

  /**
   * Store a tensor with automatic compression based on access patterns
   */
  store(id: string, tensor: Float32Array | number[], level?: CompressionLevel): void {
    const data = tensor instanceof Float32Array ? Array.from(tensor) : tensor;
    const now = Date.now();

    // Check if updating existing tensor
    const existing = this.tensors.get(id);
    const accessCount = existing ? existing.accessCount : 0;

    const compressed = this.compress(data, level || 'none');

    this.tensors.set(id, {
      data: compressed.data,
      level: compressed.level,
      originalDim: data.length,
      accessCount,
      lastAccess: now,
      created: existing?.created || now,
    });
  }

  /**
   * Retrieve and decompress a tensor
   */
  get(id: string): Float32Array | null {
    const tensor = this.tensors.get(id);
    if (!tensor) return null;

    // Update access stats
    tensor.accessCount++;
    tensor.lastAccess = Date.now();
    this.totalAccesses++;

    // Decompress and return
    return this.decompress(tensor);
  }

  /**
   * Check if tensor exists
   */
  has(id: string): boolean {
    return this.tensors.has(id);
  }

  /**
   * Delete a tensor
   */
  delete(id: string): boolean {
    return this.tensors.delete(id);
  }

  /**
   * Get all tensor IDs
   */
  keys(): string[] {
    return Array.from(this.tensors.keys());
  }

  /**
   * Compress tensor to specified level
   */
  private compress(data: number[], level: CompressionLevel): { data: any; level: CompressionLevel } {
    switch (level) {
      case 'none':
        return { data: new Float32Array(data), level };

      case 'half':
        // Float16 simulation using Uint16Array
        return { data: this.toFloat16(data), level };

      case 'pq8':
        // Product quantization to 8-bit
        return { data: this.toPQ8(data), level };

      case 'pq4':
        // Product quantization to 4-bit (packed into Uint8)
        return { data: this.toPQ4(data), level };

      case 'binary':
        // Binary quantization (1-bit per value)
        return { data: this.toBinary(data), level };

      default:
        return { data: new Float32Array(data), level: 'none' };
    }
  }

  /**
   * Decompress tensor back to Float32Array
   */
  private decompress(tensor: CompressedTensor): Float32Array {
    const { data, level, originalDim } = tensor;

    switch (level) {
      case 'none':
        return data instanceof Float32Array ? data : new Float32Array(data as number[]);

      case 'half':
        return this.fromFloat16(data as Uint16Array, originalDim);

      case 'pq8':
        return this.fromPQ8(data as Uint8Array, originalDim);

      case 'pq4':
        return this.fromPQ4(data as Uint8Array, originalDim);

      case 'binary':
        return this.fromBinary(data as Uint8Array, originalDim);

      default:
        return new Float32Array(data as number[]);
    }
  }

  /**
   * Float16 conversion (approximate)
   */
  private toFloat16(data: number[]): Uint16Array {
    const result = new Uint16Array(data.length);
    for (let i = 0; i < data.length; i++) {
      result[i] = this.floatToHalf(data[i]);
    }
    return result;
  }

  private fromFloat16(data: Uint16Array, dim: number): Float32Array {
    const result = new Float32Array(dim);
    for (let i = 0; i < dim; i++) {
      result[i] = this.halfToFloat(data[i]);
    }
    return result;
  }

  private floatToHalf(val: number): number {
    const floatView = new Float32Array(1);
    const int32View = new Int32Array(floatView.buffer);
    floatView[0] = val;
    const x = int32View[0];

    let bits = (x >> 16) & 0x8000;
    let m = (x >> 12) & 0x07ff;
    const e = (x >> 23) & 0xff;

    if (e < 103) return bits;
    if (e > 142) {
      bits |= 0x7c00;
      bits |= ((e === 255) ? 0 : 1) && (x & 0x007fffff);
      return bits;
    }
    if (e < 113) {
      m |= 0x0800;
      bits |= (m >> (114 - e)) + ((m >> (113 - e)) & 1);
      return bits;
    }
    bits |= ((e - 112) << 10) | (m >> 1);
    bits += (m & 1);
    return bits;
  }

  private halfToFloat(val: number): number {
    const s = (val & 0x8000) >> 15;
    const e = (val & 0x7C00) >> 10;
    const f = val & 0x03FF;

    if (e === 0) {
      return (s ? -1 : 1) * Math.pow(2, -14) * (f / Math.pow(2, 10));
    } else if (e === 0x1F) {
      return f ? NaN : ((s ? -1 : 1) * Infinity);
    }
    return (s ? -1 : 1) * Math.pow(2, e - 15) * (1 + f / Math.pow(2, 10));
  }

  /**
   * Product Quantization 8-bit
   */
  private toPQ8(data: number[]): Uint8Array {
    const result = new Uint8Array(data.length);
    const min = Math.min(...data);
    const max = Math.max(...data);
    const range = max - min || 1;

    for (let i = 0; i < data.length; i++) {
      result[i] = Math.round(((data[i] - min) / range) * 255);
    }

    // Store min/max in first 8 bytes for reconstruction
    const output = new Uint8Array(data.length + 8);
    const view = new DataView(output.buffer);
    view.setFloat32(0, min, true);
    view.setFloat32(4, max, true);
    output.set(result, 8);

    return output;
  }

  private fromPQ8(data: Uint8Array, dim: number): Float32Array {
    const view = new DataView(data.buffer, data.byteOffset);
    const min = view.getFloat32(0, true);
    const max = view.getFloat32(4, true);
    const range = max - min || 1;

    const result = new Float32Array(dim);
    for (let i = 0; i < dim; i++) {
      result[i] = (data[i + 8] / 255) * range + min;
    }
    return result;
  }

  /**
   * Product Quantization 4-bit (packed)
   */
  private toPQ4(data: number[]): Uint8Array {
    const packedLen = Math.ceil(data.length / 2);
    const result = new Uint8Array(packedLen + 8);

    const min = Math.min(...data);
    const max = Math.max(...data);
    const range = max - min || 1;

    const view = new DataView(result.buffer);
    view.setFloat32(0, min, true);
    view.setFloat32(4, max, true);

    for (let i = 0; i < data.length; i += 2) {
      const v1 = Math.round(((data[i] - min) / range) * 15);
      const v2 = i + 1 < data.length ? Math.round(((data[i + 1] - min) / range) * 15) : 0;
      result[8 + i / 2] = (v1 << 4) | v2;
    }

    return result;
  }

  private fromPQ4(data: Uint8Array, dim: number): Float32Array {
    const view = new DataView(data.buffer, data.byteOffset);
    const min = view.getFloat32(0, true);
    const max = view.getFloat32(4, true);
    const range = max - min || 1;

    const result = new Float32Array(dim);
    for (let i = 0; i < dim; i += 2) {
      const packed = data[8 + i / 2];
      result[i] = ((packed >> 4) / 15) * range + min;
      if (i + 1 < dim) {
        result[i + 1] = ((packed & 0x0F) / 15) * range + min;
      }
    }
    return result;
  }

  /**
   * Binary quantization (1-bit per value)
   */
  private toBinary(data: number[]): Uint8Array {
    const packedLen = Math.ceil(data.length / 8);
    const result = new Uint8Array(packedLen + 4);

    // Store mean for reconstruction
    const mean = data.reduce((a, b) => a + b, 0) / data.length;
    const view = new DataView(result.buffer);
    view.setFloat32(0, mean, true);

    for (let i = 0; i < data.length; i++) {
      if (data[i] >= mean) {
        const byteIdx = 4 + Math.floor(i / 8);
        const bitIdx = i % 8;
        result[byteIdx] |= (1 << bitIdx);
      }
    }

    return result;
  }

  private fromBinary(data: Uint8Array, dim: number): Float32Array {
    const view = new DataView(data.buffer, data.byteOffset);
    const mean = view.getFloat32(0, true);

    // Use fixed deviation for reconstruction
    const deviation = 0.5;

    const result = new Float32Array(dim);
    for (let i = 0; i < dim; i++) {
      const byteIdx = 4 + Math.floor(i / 8);
      const bitIdx = i % 8;
      const bit = (data[byteIdx] >> bitIdx) & 1;
      result[i] = bit ? mean + deviation : mean - deviation;
    }
    return result;
  }

  /**
   * Calculate access frequency for a tensor
   */
  private getAccessFrequency(tensor: CompressedTensor): number {
    if (this.totalAccesses === 0) return 1;
    return tensor.accessCount / this.totalAccesses;
  }

  /**
   * Determine optimal compression level based on access frequency
   */
  getOptimalLevel(id: string): CompressionLevel {
    const tensor = this.tensors.get(id);
    if (!tensor) return 'none';

    const freq = this.getAccessFrequency(tensor);

    if (freq > this.config.hotThreshold) return 'none';
    if (freq > this.config.warmThreshold) return 'half';
    if (freq > this.config.coolThreshold) return 'pq8';
    if (freq > this.config.coldThreshold) return 'pq4';
    return 'binary';
  }

  /**
   * Recompress all tensors based on current access patterns
   */
  recompressAll(): CompressionStats {
    const stats: CompressionStats = {
      totalTensors: this.tensors.size,
      byLevel: { none: 0, half: 0, pq8: 0, pq4: 0, binary: 0 },
      originalBytes: 0,
      compressedBytes: 0,
      savingsPercent: 0,
    };

    for (const [id, tensor] of this.tensors) {
      const optimalLevel = this.getOptimalLevel(id);

      if (optimalLevel !== tensor.level) {
        // Decompress and recompress at new level
        const decompressed = this.decompress(tensor);
        const recompressed = this.compress(Array.from(decompressed), optimalLevel);

        tensor.data = recompressed.data;
        tensor.level = recompressed.level;
      }

      stats.byLevel[tensor.level]++;
      stats.originalBytes += tensor.originalDim * 4; // Float32
      stats.compressedBytes += this.getCompressedSize(tensor);
    }

    stats.savingsPercent = stats.originalBytes > 0
      ? ((stats.originalBytes - stats.compressedBytes) / stats.originalBytes) * 100
      : 0;

    return stats;
  }

  /**
   * Get compressed size in bytes
   */
  private getCompressedSize(tensor: CompressedTensor): number {
    const { data, level, originalDim } = tensor;

    switch (level) {
      case 'none': return originalDim * 4;
      case 'half': return originalDim * 2;
      case 'pq8': return originalDim + 8;
      case 'pq4': return Math.ceil(originalDim / 2) + 8;
      case 'binary': return Math.ceil(originalDim / 8) + 4;
      default: return originalDim * 4;
    }
  }

  /**
   * Get compression statistics
   */
  getStats(): CompressionStats {
    const stats: CompressionStats = {
      totalTensors: this.tensors.size,
      byLevel: { none: 0, half: 0, pq8: 0, pq4: 0, binary: 0 },
      originalBytes: 0,
      compressedBytes: 0,
      savingsPercent: 0,
    };

    for (const tensor of this.tensors.values()) {
      stats.byLevel[tensor.level]++;
      stats.originalBytes += tensor.originalDim * 4;
      stats.compressedBytes += this.getCompressedSize(tensor);
    }

    stats.savingsPercent = stats.originalBytes > 0
      ? ((stats.originalBytes - stats.compressedBytes) / stats.originalBytes) * 100
      : 0;

    return stats;
  }

  /**
   * Start auto-compression timer
   */
  private startAutoCompress(): void {
    if (this.compressTimer) return;

    this.compressTimer = setInterval(() => {
      this.recompressAll();
    }, this.config.compressIntervalMs);
  }

  /**
   * Stop auto-compression
   */
  stopAutoCompress(): void {
    if (this.compressTimer) {
      clearInterval(this.compressTimer);
      this.compressTimer = null;
    }
  }

  /**
   * Export all tensors for persistence
   */
  export(): { tensors: Record<string, any>; totalAccesses: number } {
    const tensors: Record<string, any> = {};

    for (const [id, tensor] of this.tensors) {
      tensors[id] = {
        data: Array.from(tensor.data as any),
        level: tensor.level,
        originalDim: tensor.originalDim,
        accessCount: tensor.accessCount,
        lastAccess: tensor.lastAccess,
        created: tensor.created,
      };
    }

    return { tensors, totalAccesses: this.totalAccesses };
  }

  /**
   * Import tensors from persistence
   */
  import(data: { tensors: Record<string, any>; totalAccesses: number }): void {
    this.totalAccesses = data.totalAccesses || 0;

    for (const [id, tensor] of Object.entries(data.tensors)) {
      const t = tensor as any;
      this.tensors.set(id, {
        data: this.restoreTypedArray(t.data, t.level),
        level: t.level,
        originalDim: t.originalDim,
        accessCount: t.accessCount || 0,
        lastAccess: t.lastAccess || Date.now(),
        created: t.created || Date.now(),
      });
    }
  }

  private restoreTypedArray(data: number[], level: CompressionLevel): any {
    switch (level) {
      case 'none': return new Float32Array(data);
      case 'half': return new Uint16Array(data);
      case 'pq8':
      case 'pq4':
      case 'binary': return new Uint8Array(data);
      default: return new Float32Array(data);
    }
  }

  /**
   * Clear all tensors
   */
  clear(): void {
    this.tensors.clear();
    this.totalAccesses = 0;
  }
}

export default TensorCompress;
