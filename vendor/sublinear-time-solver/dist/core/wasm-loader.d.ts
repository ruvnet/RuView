/**
 * WASM Module Loader
 * Loads and initializes WebAssembly modules for high-performance computing
 */
export interface WasmModule {
    instance: any;
    exports: any;
    memory?: any;
}
export declare class WasmLoader {
    private static modules;
    private static initialized;
    /**
     * Initialize all WASM modules
     */
    static initialize(): Promise<void>;
    /**
     * Load a specific WASM module
     */
    static loadModule(name: string, filename: string): Promise<WasmModule>;
    /**
     * Get a loaded WASM module
     */
    static getModule(name: string): WasmModule | undefined;
    /**
     * Check if a module is available
     */
    static hasModule(name: string): boolean;
    /**
     * Get all loaded module names
     */
    static getLoadedModules(): string[];
    /**
     * Get memory usage statistics
     */
    static getMemoryStats(): {
        [key: string]: number;
    };
    /**
     * Check if WASM is available and return feature flags
     */
    static getFeatureFlags(): {
        hasWasm: boolean;
        hasGraphReasoner: boolean;
        hasPlanner: boolean;
        hasExtractors: boolean;
        hasTemporalNeural: boolean;
        hasStrangeLoop: boolean;
        hasNanoConsciousness: boolean;
    };
}
