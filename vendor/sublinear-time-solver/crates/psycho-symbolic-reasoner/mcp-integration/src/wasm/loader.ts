import { readFile } from 'fs/promises';
import { join } from 'path';
import {
  GraphReasonerWasm,
  TextExtractorWasm,
  PlannerSystemWasm,
  WasmLoadError,
  WasmModuleConfig
} from '../types/index

export class WasmLoader {
  private static instance: WasmLoader;
  private loadedModules = new Map<string, any>();
  private loadingPromises = new Map<string, Promise<any>>();

  private constructor() {}

  public static getInstance(): WasmLoader {
    if (!WasmLoader.instance) {
      WasmLoader.instance = new WasmLoader();
    }
    return WasmLoader.instance;
  }

  /**
   * Load WASM module with timeout and error handling
   */
  private async loadWasmModule<T>(
    moduleName: string,
    wasmPath: string,
    config: WasmModuleConfig
  ): Promise<T> {
    // Check if already loaded
    const cached = this.loadedModules.get(moduleName);
    if (cached) {
      return cached as T;
    }

    // Check if already loading
    const loadingPromise = this.loadingPromises.get(moduleName);
    if (loadingPromise) {
      return loadingPromise as Promise<T>;
    }

    // Start loading
    const promise = this.doLoadWasmModule<T>(moduleName, wasmPath, config);
    this.loadingPromises.set(moduleName, promise);

    try {
      const module = await promise;
      this.loadedModules.set(moduleName, module);
      this.loadingPromises.delete(moduleName);
      return module;
    } catch (error) {
      this.loadingPromises.delete(moduleName);
      throw error;
    }
  }

  private async doLoadWasmModule<T>(
    moduleName: string,
    wasmPath: string,
    config: WasmModuleConfig
  ): Promise<T> {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => {
        reject(new WasmLoadError(moduleName, {
          reason: 'timeout',
          timeoutMs: config.initTimeoutMs
        }));
      }, config.initTimeoutMs);
    });

    const loadPromise = this.loadWasmFromPath<T>(wasmPath, config);

    try {
      return await Promise.race([loadPromise, timeoutPromise]);
    } catch (error) {
      if (error instanceof WasmLoadError) {
        throw error;
      }
      throw new WasmLoadError(moduleName, {
        originalError: error,
        wasmPath
      });
    }
  }

  private async loadWasmFromPath<T>(
    wasmPath: string,
    config: WasmModuleConfig
  ): Promise<T> {
    try {
      const wasmBytes = await readFile(wasmPath);
      
      const memoryConfig: WebAssembly.MemoryDescriptor = {
        initial: config.memoryInitialPages,
        maximum: config.memoryMaximumPages
      };

      const memory = new WebAssembly.Memory(memoryConfig);
      
      const imports = {
        env: {
          memory
        },
        // Add standard WASM-bindgen imports
        wbg: {
          __wbindgen_throw: (ptr: number, len: number) => {
            throw new Error('WASM throw');
          },
          __wbindgen_rethrow: (idx: number) => {
            throw new Error('WASM rethrow');
          },
          __wbindgen_memory: () => memory,
        }
      };

      const wasmModule = await WebAssembly.instantiate(wasmBytes, imports);
      
      // Initialize the module if it has a start function
      const instance = wasmModule.instance as any;
      if (instance.exports._start) {
        instance.exports._start();
      } else if (instance.exports.main) {
        instance.exports.main();
      }

      return instance.exports as T;
    } catch (error) {
      throw new WasmLoadError('Unknown', {
        originalError: error,
        wasmPath
      });
    }
  }

  /**
   * Load Graph Reasoner WASM module
   */
  public async loadGraphReasoner(config: WasmModuleConfig): Promise<GraphReasonerWasm> {
    return this.loadWasmModule<GraphReasonerWasm>(
      'graph_reasoner',
      config.wasmPath,
      config
    );
  }

  /**
   * Load Text Extractor WASM module
   */
  public async loadTextExtractor(config: WasmModuleConfig): Promise<TextExtractorWasm> {
    return this.loadWasmModule<TextExtractorWasm>(
      'text_extractor',
      config.wasmPath,
      config
    );
  }

  /**
   * Load Planner System WASM module
   */
  public async loadPlannerSystem(config: WasmModuleConfig): Promise<PlannerSystemWasm> {
    return this.loadWasmModule<PlannerSystemWasm>(
      'planner_system',
      config.wasmPath,
      config
    );
  }

  /**
   * Check if a module is loaded
   */
  public isModuleLoaded(moduleName: string): boolean {
    return this.loadedModules.has(moduleName);
  }

  /**
   * Get a loaded module
   */
  public getModule<T>(moduleName: string): T | null {
    return this.loadedModules.get(moduleName) as T || null;
  }

  /**
   * Unload a module
   */
  public unloadModule(moduleName: string): boolean {
    return this.loadedModules.delete(moduleName);
  }

  /**
   * Unload all modules
   */
  public unloadAllModules(): void {
    this.loadedModules.clear();
    this.loadingPromises.clear();
  }

  /**
   * Get memory usage statistics
   */
  public getMemoryStats(): {
    loadedModules: number;
    loadingModules: number;
    moduleNames: string[];
  } {
    return {
      loadedModules: this.loadedModules.size,
      loadingModules: this.loadingPromises.size,
      moduleNames: Array.from(this.loadedModules.keys())
    };
  }

  /**
   * Preload all modules
   */
  public async preloadAll(configs: {
    graphReasoner: WasmModuleConfig;
    textExtractor: WasmModuleConfig;
    plannerSystem: WasmModuleConfig;
  }): Promise<void> {
    const loadPromises = [
      this.loadGraphReasoner(configs.graphReasoner),
      this.loadTextExtractor(configs.textExtractor),
      this.loadPlannerSystem(configs.plannerSystem)
    ];

    try {
      await Promise.all(loadPromises);
    } catch (error) {
      // Clean up any partially loaded modules
      this.unloadAllModules();
      throw error;
    }
  }

  /**
   * Create default WASM module configs
   */
  public static createDefaultConfigs(basePath: string): {
    graphReasoner: WasmModuleConfig;
    textExtractor: WasmModuleConfig;
    plannerSystem: WasmModuleConfig;
  } {
    const defaultConfig = {
      initTimeoutMs: 30000,
      memoryInitialPages: 256,
      memoryMaximumPages: 1024
    };

    return {
      graphReasoner: {
        ...defaultConfig,
        wasmPath: join(basePath, 'graph_reasoner.wasm')
      },
      textExtractor: {
        ...defaultConfig,
        wasmPath: join(basePath, 'extractors.wasm')
      },
      plannerSystem: {
        ...defaultConfig,
        wasmPath: join(basePath, 'planner.wasm')
      }
    };
  }
}