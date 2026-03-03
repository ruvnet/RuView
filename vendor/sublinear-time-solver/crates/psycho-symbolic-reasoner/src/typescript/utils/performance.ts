import { PerformanceConfig } from '../types/config.js';
import { Logger } from './logger.js';

/**
 * Performance monitoring and optimization utilities
 */
export class PerformanceMonitor {
  private static instance: PerformanceMonitor;
  private config: PerformanceConfig;
  private metrics: Map<string, any> = new Map();
  private gcInterval?: NodeJS.Timeout;
  private metricsInterval?: NodeJS.Timeout;

  private constructor(config: PerformanceConfig) {
    this.config = config;
    this.startMonitoring();
  }

  /**
   * Initialize performance monitor
   */
  public static initialize(config: PerformanceConfig): PerformanceMonitor {
    if (!PerformanceMonitor.instance) {
      PerformanceMonitor.instance = new PerformanceMonitor(config);
    }
    return PerformanceMonitor.instance;
  }

  /**
   * Get performance monitor instance
   */
  public static getInstance(): PerformanceMonitor {
    if (!PerformanceMonitor.instance) {
      throw new Error('PerformanceMonitor not initialized');
    }
    return PerformanceMonitor.instance;
  }

  /**
   * Start performance monitoring
   */
  private startMonitoring(): void {
    // Set up garbage collection monitoring
    if (this.config.gcInterval > 0) {
      this.gcInterval = setInterval(() => {
        if (global.gc) {
          global.gc();
          Logger.debug('Forced garbage collection');
        }
      }, this.config.gcInterval);
    }

    // Set up metrics collection
    if (this.config.metricsInterval > 0) {
      this.metricsInterval = setInterval(() => {
        this.collectMetrics();
      }, this.config.metricsInterval);
    }

    // Monitor memory usage
    this.setupMemoryMonitoring();
  }

  /**
   * Setup memory monitoring
   */
  private setupMemoryMonitoring(): void {
    const maxMemoryBytes = this.parseMemorySize(this.config.maxMemoryUsage);

    const checkMemory = () => {
      const usage = process.memoryUsage();

      if (usage.heapUsed > maxMemoryBytes) {
        Logger.warn('Memory usage exceeds limit', {
          current: this.formatBytes(usage.heapUsed),
          limit: this.config.maxMemoryUsage,
          rss: this.formatBytes(usage.rss)
        });

        // Force garbage collection if available
        if (global.gc) {
          global.gc();
        }
      }
    };

    // Check memory every 10 seconds
    setInterval(checkMemory, 10000);
  }

  /**
   * Collect performance metrics
   */
  private collectMetrics(): void {
    const memUsage = process.memoryUsage();
    const cpuUsage = process.cpuUsage();

    const metrics = {
      timestamp: Date.now(),
      memory: {
        rss: memUsage.rss,
        heapTotal: memUsage.heapTotal,
        heapUsed: memUsage.heapUsed,
        external: memUsage.external,
        arrayBuffers: memUsage.arrayBuffers
      },
      cpu: {
        user: cpuUsage.user,
        system: cpuUsage.system
      },
      uptime: process.uptime(),
      eventLoopDelay: this.measureEventLoopDelay()
    };

    this.metrics.set('current', metrics);

    if (this.config.enableProfiling) {
      Logger.debug('Performance metrics collected', {
        heapUsed: this.formatBytes(metrics.memory.heapUsed),
        cpuUser: metrics.cpu.user,
        eventLoopDelay: metrics.eventLoopDelay
      });
    }
  }

  /**
   * Measure event loop delay
   */
  private measureEventLoopDelay(): number {
    const start = process.hrtime.bigint();
    setImmediate(() => {
      const delay = Number(process.hrtime.bigint() - start) / 1000000; // Convert to ms
      this.metrics.set('eventLoopDelay', delay);
    });
    return this.metrics.get('eventLoopDelay') || 0;
  }

  /**
   * Parse memory size string to bytes
   */
  private parseMemorySize(size: string): number {
    const units: Record<string, number> = {
      'b': 1,
      'k': 1024,
      'm': 1024 * 1024,
      'g': 1024 * 1024 * 1024
    };

    const match = size.toLowerCase().match(/^(\d+)([kmg]?)b?$/);
    if (!match) {
      throw new Error(`Invalid memory size format: ${size}`);
    }

    const num = match[1];
    const unit = match[2] || 'b';
    const unitKey = unit as keyof typeof units;
    if (!num || !units[unitKey]) {
      throw new Error(`Invalid memory size format: ${size}`);
    }
    return parseInt(num, 10) * units[unitKey];
  }

  /**
   * Format bytes to human readable string
   */
  private formatBytes(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(2)} ${units[unitIndex]}`;
  }

  /**
   * Get current metrics
   */
  public getCurrentMetrics(): any {
    return this.metrics.get('current');
  }

  /**
   * Get performance summary
   */
  public getPerformanceSummary(): any {
    const current = this.getCurrentMetrics();
    if (!current) {
      return { error: 'No metrics available' };
    }

    return {
      memory: {
        used: this.formatBytes(current.memory.heapUsed),
        total: this.formatBytes(current.memory.heapTotal),
        rss: this.formatBytes(current.memory.rss),
        usage_percent: ((current.memory.heapUsed / current.memory.heapTotal) * 100).toFixed(2)
      },
      uptime: {
        seconds: current.uptime,
        formatted: this.formatUptime(current.uptime)
      },
      eventLoopDelay: `${current.eventLoopDelay.toFixed(2)}ms`,
      timestamp: new Date(current.timestamp).toISOString()
    };
  }

  /**
   * Format uptime to human readable string
   */
  private formatUptime(seconds: number): string {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    return `${hours}h ${minutes}m ${secs}s`;
  }

  /**
   * Stop performance monitoring
   */
  public stop(): void {
    if (this.gcInterval) {
      clearInterval(this.gcInterval);
    }

    if (this.metricsInterval) {
      clearInterval(this.metricsInterval);
    }

    Logger.info('Performance monitoring stopped');
  }

  /**
   * Start timing an operation
   */
  public startTimer(name: string): () => number {
    const start = process.hrtime.bigint();

    return () => {
      const end = process.hrtime.bigint();
      const duration = Number(end - start) / 1000000; // Convert to milliseconds

      Logger.debug(`Operation '${name}' completed`, { duration: `${duration.toFixed(2)}ms` });
      return duration;
    };
  }

  /**
   * Measure async operation performance
   */
  public async measureAsync<T>(name: string, operation: () => Promise<T>): Promise<T> {
    const timer = this.startTimer(name);
    try {
      const result = await operation();
      timer();
      return result;
    } catch (error) {
      timer();
      throw error;
    }
  }
}