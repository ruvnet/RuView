/**
 * Test utilities and helpers for psycho-symbolic-reasoner tests
 */

import { performance } from 'perf_hooks';
import { spawn, ChildProcess } from 'child_process';
import * as fs from 'fs/promises';
import * as path from 'path';

// Type definitions for WASM modules
export interface WasmModule {
  memory: WebAssembly.Memory;
  exports: Record<string, any>;
}

export interface TestMetrics {
  executionTime: number;
  memoryUsage: number;
  peakMemoryUsage: number;
  operationCount: number;
}

export interface MockAgent {
  id: string;
  type: string;
  capabilities: string[];
  state: Record<string, any>;
  messageHistory: Array<{ timestamp: number; message: any }>;
}

/**
 * WASM Module Manager for tests
 */
export class WasmTestManager {
  private modules: Map<string, WasmModule> = new Map();
  private loadPromises: Map<string, Promise<WasmModule>> = new Map();

  async loadModule(name: string, wasmPath: string): Promise<WasmModule> {
    if (this.modules.has(name)) {
      return this.modules.get(name)!;
    }

    if (this.loadPromises.has(name)) {
      return this.loadPromises.get(name)!;
    }

    const loadPromise = this.loadWasmModule(wasmPath);
    this.loadPromises.set(name, loadPromise);

    try {
      const module = await loadPromise;
      this.modules.set(name, module);
      return module;
    } catch (error) {
      this.loadPromises.delete(name);
      throw error;
    }
  }

  private async loadWasmModule(wasmPath: string): Promise<WasmModule> {
    const wasmBytes = await fs.readFile(wasmPath);
    const wasmModule = await WebAssembly.compile(wasmBytes);
    const instance = await WebAssembly.instantiate(wasmModule);

    return {
      memory: instance.exports.memory as WebAssembly.Memory,
      exports: instance.exports,
    };
  }

  unloadModule(name: string): void {
    this.modules.delete(name);
    this.loadPromises.delete(name);
  }

  unloadAll(): void {
    this.modules.clear();
    this.loadPromises.clear();
  }

  getModule(name: string): WasmModule | undefined {
    return this.modules.get(name);
  }

  isLoaded(name: string): boolean {
    return this.modules.has(name);
  }
}

/**
 * Performance Metrics Collector
 */
export class PerformanceCollector {
  private startTime: number = 0;
  private startMemory: number = 0;
  private peakMemory: number = 0;
  private operationCount: number = 0;
  private memoryCheckInterval?: NodeJS.Timeout;

  start(): void {
    this.startTime = performance.now();
    this.startMemory = this.getMemoryUsage();
    this.peakMemory = this.startMemory;
    this.operationCount = 0;

    // Monitor memory usage every 100ms
    this.memoryCheckInterval = setInterval(() => {
      const currentMemory = this.getMemoryUsage();
      if (currentMemory > this.peakMemory) {
        this.peakMemory = currentMemory;
      }
    }, 100);
  }

  incrementOperation(): void {
    this.operationCount++;
  }

  stop(): TestMetrics {
    if (this.memoryCheckInterval) {
      clearInterval(this.memoryCheckInterval);
    }

    const executionTime = performance.now() - this.startTime;
    const finalMemory = this.getMemoryUsage();

    return {
      executionTime,
      memoryUsage: finalMemory - this.startMemory,
      peakMemoryUsage: this.peakMemory - this.startMemory,
      operationCount: this.operationCount,
    };
  }

  private getMemoryUsage(): number {
    if (typeof process !== 'undefined' && process.memoryUsage) {
      return process.memoryUsage().heapUsed;
    }
    return 0;
  }
}

/**
 * Mock Agent Factory
 */
export class MockAgentFactory {
  private agents: Map<string, MockAgent> = new Map();
  private messageQueue: Array<{ agentId: string; message: any }> = [];

  createAgent(id: string, type: string, capabilities: string[] = []): MockAgent {
    const agent: MockAgent = {
      id,
      type,
      capabilities,
      state: {},
      messageHistory: [],
    };

    this.agents.set(id, agent);
    return agent;
  }

  getAgent(id: string): MockAgent | undefined {
    return this.agents.get(id);
  }

  sendMessage(agentId: string, message: any): void {
    const agent = this.agents.get(agentId);
    if (agent) {
      agent.messageHistory.push({
        timestamp: Date.now(),
        message,
      });
    }
    this.messageQueue.push({ agentId, message });
  }

  processMessages(): Array<{ agentId: string; message: any }> {
    const messages = [...this.messageQueue];
    this.messageQueue.length = 0;
    return messages;
  }

  updateAgentState(agentId: string, state: Record<string, any>): void {
    const agent = this.agents.get(agentId);
    if (agent) {
      Object.assign(agent.state, state);
    }
  }

  getAllAgents(): MockAgent[] {
    return Array.from(this.agents.values());
  }

  clearAgents(): void {
    this.agents.clear();
    this.messageQueue.length = 0;
  }
}

/**
 * CLI Test Runner
 */
export class CLITestRunner {
  private processes: ChildProcess[] = [];

  async runCommand(command: string, args: string[] = [], options: any = {}): Promise<{
    stdout: string;
    stderr: string;
    exitCode: number;
    executionTime: number;
  }> {
    const startTime = performance.now();

    return new Promise((resolve, reject) => {
      const process = spawn(command, args, {
        stdio: ['pipe', 'pipe', 'pipe'],
        ...options,
      });

      this.processes.push(process);

      let stdout = '';
      let stderr = '';

      process.stdout?.on('data', (data) => {
        stdout += data.toString();
      });

      process.stderr?.on('data', (data) => {
        stderr += data.toString();
      });

      process.on('close', (code) => {
        const executionTime = performance.now() - startTime;
        const index = this.processes.indexOf(process);
        if (index > -1) {
          this.processes.splice(index, 1);
        }

        resolve({
          stdout,
          stderr,
          exitCode: code || 0,
          executionTime,
        });
      });

      process.on('error', (error) => {
        reject(error);
      });

      // Set timeout
      const timeout = options.timeout || 30000;
      setTimeout(() => {
        if (!process.killed) {
          process.kill('SIGTERM');
          reject(new Error(`Process timed out after ${timeout}ms`));
        }
      }, timeout);
    });
  }

  cleanup(): void {
    this.processes.forEach(process => {
      if (!process.killed) {
        process.kill('SIGTERM');
      }
    });
    this.processes.length = 0;
  }
}

/**
 * Memory Leak Detector
 */
export class MemoryLeakDetector {
  private initialMemory: number = 0;
  private snapshots: Array<{ time: number; memory: number }> = [];
  private threshold: number;

  constructor(threshold: number = 10 * 1024 * 1024) { // 10MB default
    this.threshold = threshold;
  }

  start(): void {
    // Force garbage collection if available
    if (global.gc) {
      global.gc();
    }

    this.initialMemory = this.getMemoryUsage();
    this.snapshots = [{ time: Date.now(), memory: this.initialMemory }];
  }

  snapshot(): void {
    if (global.gc) {
      global.gc();
    }

    const memory = this.getMemoryUsage();
    this.snapshots.push({ time: Date.now(), memory });
  }

  checkForLeaks(): {
    hasLeak: boolean;
    memoryIncrease: number;
    leakRate: number; // bytes per second
    snapshots: Array<{ time: number; memory: number }>;
  } {
    if (this.snapshots.length < 2) {
      return {
        hasLeak: false,
        memoryIncrease: 0,
        leakRate: 0,
        snapshots: this.snapshots,
      };
    }

    const latest = this.snapshots[this.snapshots.length - 1];
    const memoryIncrease = latest.memory - this.initialMemory;
    const timeElapsed = (latest.time - this.snapshots[0].time) / 1000; // seconds
    const leakRate = memoryIncrease / timeElapsed;

    return {
      hasLeak: memoryIncrease > this.threshold,
      memoryIncrease,
      leakRate,
      snapshots: this.snapshots,
    };
  }

  private getMemoryUsage(): number {
    if (typeof process !== 'undefined' && process.memoryUsage) {
      return process.memoryUsage().heapUsed;
    }
    return 0;
  }
}

/**
 * Test Data Generator
 */
export class TestDataGenerator {
  static generateRandomString(length: number): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    let result = '';
    for (let i = 0; i < length; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
  }

  static generateGraphData(nodeCount: number, edgeCount: number): {
    nodes: Array<{ id: string; type: string; properties: Record<string, any> }>;
    edges: Array<{ source: string; target: string; relation: string; weight: number }>;
  } {
    const nodes = [];
    const edges = [];

    // Generate nodes
    for (let i = 0; i < nodeCount; i++) {
      nodes.push({
        id: `node_${i}`,
        type: ['person', 'object', 'concept'][Math.floor(Math.random() * 3)],
        properties: {
          name: this.generateRandomString(8),
          value: Math.random() * 100,
        },
      });
    }

    // Generate edges
    for (let i = 0; i < edgeCount; i++) {
      const source = nodes[Math.floor(Math.random() * nodes.length)].id;
      const target = nodes[Math.floor(Math.random() * nodes.length)].id;

      if (source !== target) {
        edges.push({
          source,
          target,
          relation: ['likes', 'knows', 'related_to', 'part_of'][Math.floor(Math.random() * 4)],
          weight: Math.random(),
        });
      }
    }

    return { nodes, edges };
  }

  static generatePlanningScenario(): {
    initialState: Record<string, any>;
    goalState: Record<string, any>;
    actions: Array<{
      id: string;
      preconditions: Record<string, any>;
      effects: Record<string, any>;
      cost: number;
    }>;
  } {
    const locations = ['home', 'work', 'store', 'gym'];
    const items = ['key', 'money', 'phone', 'laptop'];

    const initialState: Record<string, any> = {
      location: locations[0],
      has_key: true,
      has_money: Math.random() > 0.5,
      has_phone: true,
      energy: Math.floor(Math.random() * 100),
    };

    const goalState: Record<string, any> = {
      location: locations[Math.floor(Math.random() * locations.length)],
      has_laptop: true,
      energy: Math.max(50, Math.floor(Math.random() * 100)),
    };

    const actions = [
      {
        id: 'travel',
        preconditions: { has_key: true },
        effects: { location: 'work' },
        cost: 10,
      },
      {
        id: 'buy_laptop',
        preconditions: { location: 'store', has_money: true },
        effects: { has_laptop: true, has_money: false },
        cost: 100,
      },
      {
        id: 'rest',
        preconditions: { location: 'home' },
        effects: { energy: 100 },
        cost: 20,
      },
    ];

    return { initialState, goalState, actions };
  }

  static generateSentimentTestCases(): Array<{ text: string; expectedSentiment: string; expectedScore: number }> {
    return [
      { text: "I love this product! It's amazing!", expectedSentiment: "positive", expectedScore: 0.8 },
      { text: "This is terrible. I hate it.", expectedSentiment: "negative", expectedScore: -0.8 },
      { text: "It's okay, nothing special.", expectedSentiment: "neutral", expectedScore: 0.0 },
      { text: "Great quality but expensive.", expectedSentiment: "mixed", expectedScore: 0.2 },
      { text: "Absolutely fantastic! Best purchase ever!", expectedSentiment: "positive", expectedScore: 0.9 },
    ];
  }
}

/**
 * Async Test Utilities
 */
export class AsyncTestUtils {
  static async waitFor(condition: () => boolean, timeout: number = 5000, interval: number = 100): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (condition()) {
        return;
      }
      await this.sleep(interval);
    }

    throw new Error(`Condition not met within ${timeout}ms`);
  }

  static async sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  static async withTimeout<T>(promise: Promise<T>, timeout: number): Promise<T> {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`Operation timed out after ${timeout}ms`)), timeout);
    });

    return Promise.race([promise, timeoutPromise]);
  }

  static async retry<T>(
    operation: () => Promise<T>,
    maxRetries: number = 3,
    delay: number = 1000
  ): Promise<T> {
    let lastError: Error;

    for (let i = 0; i <= maxRetries; i++) {
      try {
        return await operation();
      } catch (error) {
        lastError = error as Error;
        if (i < maxRetries) {
          await this.sleep(delay);
        }
      }
    }

    throw lastError!;
  }
}

/**
 * File System Test Utilities
 */
export class FileSystemTestUtils {
  static async createTempFile(content: string, extension: string = '.tmp'): Promise<string> {
    const tempDir = await fs.mkdtemp(path.join(process.cwd(), 'temp-'));
    const tempFile = path.join(tempDir, `test-${Date.now()}${extension}`);
    await fs.writeFile(tempFile, content);
    return tempFile;
  }

  static async createTempDir(): Promise<string> {
    return fs.mkdtemp(path.join(process.cwd(), 'temp-test-'));
  }

  static async cleanup(paths: string[]): Promise<void> {
    for (const path of paths) {
      try {
        const stat = await fs.stat(path);
        if (stat.isDirectory()) {
          await fs.rmdir(path, { recursive: true });
        } else {
          await fs.unlink(path);
        }
      } catch (error) {
        // Ignore cleanup errors
      }
    }
  }
}

// Global test utilities instance
export const testUtils = {
  wasmManager: new WasmTestManager(),
  performanceCollector: new PerformanceCollector(),
  mockAgentFactory: new MockAgentFactory(),
  cliRunner: new CLITestRunner(),
  memoryLeakDetector: new MemoryLeakDetector(),
  dataGenerator: TestDataGenerator,
  asyncUtils: AsyncTestUtils,
  fsUtils: FileSystemTestUtils,
};