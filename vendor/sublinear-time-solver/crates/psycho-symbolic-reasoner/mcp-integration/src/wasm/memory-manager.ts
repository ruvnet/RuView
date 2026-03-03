import { v4 as uuidv4 } from 'uuid';
import {
  GraphReasonerInstance,
  TextExtractorInstance,
  PlannerSystemInstance,
  WasmInstanceManager,
  PsychoSymbolicError
} from '../types/index

export class WasmMemoryManager implements WasmInstanceManager {
  private static instance: WasmMemoryManager;
  public instances = new Map<string, any>();
  private instanceTypes = new Map<string, string>();
  private creationTimes = new Map<string, number>();
  private lastAccess = new Map<string, number>();
  private maxInstances: number;
  private maxIdleTime: number;
  private cleanupInterval: NodeJS.Timeout | null = null;

  private constructor(
    maxInstances: number = 100,
    maxIdleTimeMs: number = 5 * 60 * 1000 // 5 minutes
  ) {
    this.maxInstances = maxInstances;
    this.maxIdleTime = maxIdleTimeMs;
    this.startCleanupTimer();
  }

  public static getInstance(
    maxInstances?: number,
    maxIdleTimeMs?: number
  ): WasmMemoryManager {
    if (!WasmMemoryManager.instance) {
      WasmMemoryManager.instance = new WasmMemoryManager(
        maxInstances,
        maxIdleTimeMs
      );
    }
    return WasmMemoryManager.instance;
  }

  /**
   * Create and register a new WASM instance
   */
  public createInstance<T>(
    factory: () => T,
    type: string,
    customId?: string
  ): { id: string; instance: T } {
    const id = customId || uuidv4();
    
    if (this.instances.has(id)) {
      throw new PsychoSymbolicError(
        `Instance with ID ${id} already exists`,
        'DUPLICATE_INSTANCE_ID',
        { id, type }
      );
    }

    // Check instance limit
    if (this.instances.size >= this.maxInstances) {
      this.performEmergencyCleanup();
      if (this.instances.size >= this.maxInstances) {
        throw new PsychoSymbolicError(
          `Maximum instances limit reached: ${this.maxInstances}`,
          'MAX_INSTANCES_EXCEEDED',
          { maxInstances: this.maxInstances, currentCount: this.instances.size }
        );
      }
    }

    try {
      const instance = factory();
      const now = Date.now();
      
      this.instances.set(id, instance);
      this.instanceTypes.set(id, type);
      this.creationTimes.set(id, now);
      this.lastAccess.set(id, now);
      
      return { id, instance };
    } catch (error) {
      throw new PsychoSymbolicError(
        `Failed to create WASM instance of type ${type}`,
        'INSTANCE_CREATION_FAILED',
        { type, originalError: error }
      );
    }
  }

  /**
   * Get an instance by ID
   */
  public getInstance<T>(id: string): T | null {
    const instance = this.instances.get(id) as T;
    if (instance) {
      this.lastAccess.set(id, Date.now());
      return instance;
    }
    return null;
  }

  /**
   * Set an instance (mainly for external management)
   */
  public setInstance<T>(id: string, instance: T, type: string = 'unknown'): void {
    if (this.instances.has(id)) {
      this.removeInstance(id);
    }
    
    const now = Date.now();
    this.instances.set(id, instance);
    this.instanceTypes.set(id, type);
    this.creationTimes.set(id, now);
    this.lastAccess.set(id, now);
  }

  /**
   * Remove and cleanup an instance
   */
  public removeInstance(id: string): boolean {
    const instance = this.instances.get(id);
    if (!instance) {
      return false;
    }

    // Call free() method if available
    if (typeof instance.free === 'function') {
      try {
        instance.free();
      } catch (error) {
        console.warn(`Failed to free WASM instance ${id}:`, error);
      }
    }

    this.instances.delete(id);
    this.instanceTypes.delete(id);
    this.creationTimes.delete(id);
    this.lastAccess.delete(id);
    
    return true;
  }

  /**
   * Remove all instances of a specific type
   */
  public removeInstancesByType(type: string): number {
    let removed = 0;
    const toRemove: string[] = [];
    
    for (const [id, instanceType] of this.instanceTypes.entries()) {
      if (instanceType === type) {
        toRemove.push(id);
      }
    }
    
    for (const id of toRemove) {
      if (this.removeInstance(id)) {
        removed++;
      }
    }
    
    return removed;
  }

  /**
   * Get all instances of a specific type
   */
  public getInstancesByType<T>(type: string): Map<string, T> {
    const result = new Map<string, T>();
    
    for (const [id, instanceType] of this.instanceTypes.entries()) {
      if (instanceType === type) {
        const instance = this.instances.get(id) as T;
        if (instance) {
          result.set(id, instance);
          this.lastAccess.set(id, Date.now());
        }
      }
    }
    
    return result;
  }

  /**
   * Cleanup all instances
   */
  public cleanup(): void {
    const instances = Array.from(this.instances.keys());
    for (const id of instances) {
      this.removeInstance(id);
    }
  }

  /**
   * Cleanup idle instances
   */
  public cleanupIdleInstances(): number {
    const now = Date.now();
    let cleaned = 0;
    const toRemove: string[] = [];
    
    for (const [id, lastAccessTime] of this.lastAccess.entries()) {
      if (now - lastAccessTime > this.maxIdleTime) {
        toRemove.push(id);
      }
    }
    
    for (const id of toRemove) {
      if (this.removeInstance(id)) {
        cleaned++;
      }
    }
    
    return cleaned;
  }

  /**
   * Emergency cleanup - remove oldest instances
   */
  private performEmergencyCleanup(): void {
    const instances = Array.from(this.creationTimes.entries())
      .sort(([, a], [, b]) => a - b) // Sort by creation time (oldest first)
      .slice(0, Math.floor(this.maxInstances * 0.25)) // Remove 25% of instances
      .map(([id]) => id);
    
    for (const id of instances) {
      this.removeInstance(id);
    }
  }

  /**
   * Start automatic cleanup timer
   */
  private startCleanupTimer(): void {
    if (this.cleanupInterval) {
      clearInterval(this.cleanupInterval);
    }
    
    this.cleanupInterval = setInterval(() => {
      this.cleanupIdleInstances();
    }, Math.min(this.maxIdleTime / 4, 60000)); // Check every 1 minute or 1/4 of idle time
  }

  /**
   * Stop automatic cleanup timer
   */
  public stopCleanupTimer(): void {
    if (this.cleanupInterval) {
      clearInterval(this.cleanupInterval);
      this.cleanupInterval = null;
    }
  }

  /**
   * Get memory usage statistics
   */
  public getMemoryStats(): {
    totalInstances: number;
    instancesByType: Record<string, number>;
    oldestInstanceAge: number;
    newestInstanceAge: number;
    averageAge: number;
    memoryUsageMB: number;
  } {
    const now = Date.now();
    const ages: number[] = [];
    const typeCount: Record<string, number> = {};
    
    for (const [id, creationTime] of this.creationTimes.entries()) {
      const age = now - creationTime;
      ages.push(age);
      
      const type = this.instanceTypes.get(id) || 'unknown';
      typeCount[type] = (typeCount[type] || 0) + 1;
    }
    
    // Estimate memory usage (rough approximation)
    const estimatedMemoryPerInstance = 5; // MB
    const memoryUsageMB = this.instances.size * estimatedMemoryPerInstance;
    
    return {
      totalInstances: this.instances.size,
      instancesByType: typeCount,
      oldestInstanceAge: ages.length > 0 ? Math.max(...ages) : 0,
      newestInstanceAge: ages.length > 0 ? Math.min(...ages) : 0,
      averageAge: ages.length > 0 ? ages.reduce((a, b) => a + b, 0) / ages.length : 0,
      memoryUsageMB
    };
  }

  /**
   * Update configuration
   */
  public updateConfig(maxInstances?: number, maxIdleTimeMs?: number): void {
    if (maxInstances !== undefined) {
      this.maxInstances = maxInstances;
    }
    if (maxIdleTimeMs !== undefined) {
      this.maxIdleTime = maxIdleTimeMs;
      this.startCleanupTimer(); // Restart timer with new interval
    }
  }

  /**
   * Create a scoped instance manager for temporary instances
   */
  public createScope(): WasmInstanceScope {
    return new WasmInstanceScope(this);
  }
}

/**
 * Scoped instance manager for automatic cleanup
 */
export class WasmInstanceScope {
  private instanceIds = new Set<string>();
  private disposed = false;

  constructor(private manager: WasmMemoryManager) {}

  /**
   * Create an instance within this scope
   */
  public createInstance<T>(
    factory: () => T,
    type: string,
    customId?: string
  ): { id: string; instance: T } {
    if (this.disposed) {
      throw new PsychoSymbolicError(
        'Cannot create instance in disposed scope',
        'SCOPE_DISPOSED'
      );
    }

    const result = this.manager.createInstance(factory, type, customId);
    this.instanceIds.add(result.id);
    return result;
  }

  /**
   * Get an instance from 'this scope
   */
  public getInstance<T>(id: string): T | null {
    if (this.disposed || !this.instanceIds.has(id)) {
      return null;
    }
    return this.manager.getInstance<T>(id);
  }

  /**
   * Remove an instance from 'this scope
   */
  public removeInstance(id: string): boolean {
    if (!this.instanceIds.has(id)) {
      return false;
    }
    
    this.instanceIds.delete(id);
    return this.manager.removeInstance(id);
  }

  /**
   * Dispose all instances in this scope
   */
  public dispose(): void {
    if (this.disposed) {
      return;
    }

    for (const id of this.instanceIds) {
      this.manager.removeInstance(id);
    }
    
    this.instanceIds.clear();
    this.disposed = true;
  }

  /**
   * Get the number of instances in this scope
   */
  public get size(): number {
    return this.instanceIds.size;
  }

  /**
   * Check if the scope is disposed
   */
  public get isDisposed(): boolean {
    return this.disposed;
  }';
}';