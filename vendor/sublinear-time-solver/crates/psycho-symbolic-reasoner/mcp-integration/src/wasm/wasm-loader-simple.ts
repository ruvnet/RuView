import { join } from 'path';
import { pathToFileURL } from 'url';
import {
  GraphReasonerWasm,
  TextExtractorWasm,
  PlannerSystemWasm,
  WasmLoadError,
  WasmModuleConfig
} from '../types/index

/**
 * Simplified WASM loader that works with wasm-pack generated modules
 */
export class SimpleWasmLoader {
  private static instance: SimpleWasmLoader;
  private loadedModules = new Map<string, any>();

  private constructor() {}

  public static getInstance(): SimpleWasmLoader {
    if (!SimpleWasmLoader.instance) {
      SimpleWasmLoader.instance = new SimpleWasmLoader();
    }
    return SimpleWasmLoader.instance;
  }

  /**
   * Load Graph Reasoner WASM module
   */
  public async loadGraphReasoner(config: WasmModuleConfig): Promise<any> {
    if (this.loadedModules.has('graph_reasoner')) {
      return this.loadedModules.get('graph_reasoner');
    }

    try {
      // Import the generated JS file
      const modulePath = pathToFileURL(join(config.wasmPath, 'graph_reasoner.js')).href;
      const module = await import(modulePath);
      
      // Initialize the WASM module
      if (typeof module.default === 'function') {
        await module.default();
      }
      
      this.loadedModules.set('graph_reasoner', module);
      return module;
    } catch (error) {
      throw new WasmLoadError('graph_reasoner', {
        originalError: error,
        wasmPath: config.wasmPath
      });
    }
  }

  /**
   * Load Text Extractor WASM module
   */
  public async loadTextExtractor(config: WasmModuleConfig): Promise<any> {
    if (this.loadedModules.has('text_extractor')) {
      return this.loadedModules.get('text_extractor');
    }

    try {
      // Import the generated JS file
      const modulePath = pathToFileURL(join(config.wasmPath, 'extractors.js')).href;
      const module = await import(modulePath);
      
      // Initialize the WASM module
      if (typeof module.default === 'function') {
        await module.default();
      }
      
      this.loadedModules.set('text_extractor', module);
      return module;
    } catch (error) {
      throw new WasmLoadError('text_extractor', {
        originalError: error,
        wasmPath: config.wasmPath
      });
    }
  }

  /**
   * Load Planner System WASM module
   */
  public async loadPlannerSystem(config: WasmModuleConfig): Promise<any> {
    if (this.loadedModules.has('planner_system')) {
      return this.loadedModules.get('planner_system');
    }

    try {
      // Import the generated JS file
      const modulePath = pathToFileURL(join(config.wasmPath, 'planner.js')).href;
      const module = await import(modulePath);
      
      // Initialize the WASM module
      if (typeof module.default === 'function') {
        await module.default();
      }
      
      this.loadedModules.set('planner_system', module);
      return module;
    } catch (error) {
      throw new WasmLoadError('planner_system', {
        originalError: error,
        wasmPath: config.wasmPath
      });
    }
  }

  /**
   * Get a loaded module
   */
  public getModule<T>(moduleName: string): T | null {
    return this.loadedModules.get(moduleName) as T || null;
  }

  /**
   * Check if a module is loaded
   */
  public isModuleLoaded(moduleName: string): boolean {
    return this.loadedModules.has(moduleName);
  }

  /**
   * Unload all modules
   */
  public unloadAllModules(): void {
    this.loadedModules.clear();
  }

  /**
   * Get memory usage statistics
   */
  public getMemoryStats(): {
    loadedModules: number;
    moduleNames: string[];
  } {
    return {
      loadedModules: this.loadedModules.size,
      moduleNames: Array.from(this.loadedModules.keys())
    };
  }
}