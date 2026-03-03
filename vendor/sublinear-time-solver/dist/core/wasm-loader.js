/**
 * WASM Module Loader
 * Loads and initializes WebAssembly modules for high-performance computing
 */
import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
export class WasmLoader {
    static modules = new Map();
    static initialized = false;
    /**
     * Initialize all WASM modules
     */
    static async initialize() {
        if (this.initialized)
            return;
        console.log('🚀 Initializing WASM modules...');
        // Load all available WASM modules
        const modules = [
            { name: 'graph_reasoner', file: 'graph_reasoner_bg.wasm' },
            { name: 'planner', file: 'planner_bg.wasm' },
            { name: 'extractors', file: 'extractors_bg.wasm' },
            { name: 'temporal_neural', file: 'temporal_neural_solver_bg.wasm' },
            { name: 'strange_loop', file: 'strange_loop_bg.wasm' },
            { name: 'nano_consciousness', file: 'nano_consciousness_bg.wasm' }
        ];
        const loadPromises = modules.map(async (mod) => {
            try {
                await this.loadModule(mod.name, mod.file);
                console.log(`✅ Loaded ${mod.name}`);
            }
            catch (err) {
                console.log(`⚠️  ${mod.name} not available (optional)`);
            }
        });
        await Promise.all(loadPromises);
        this.initialized = true;
        console.log(`✨ WASM initialization complete (${this.modules.size} modules loaded)`);
    }
    /**
     * Load a specific WASM module
     */
    static async loadModule(name, filename) {
        // Check if already loaded
        if (this.modules.has(name)) {
            return this.modules.get(name);
        }
        try {
            // Try to load from dist/wasm first
            const wasmPath = join(__dirname, '..', 'wasm', filename);
            const wasmBuffer = await readFile(wasmPath);
            // Compile and instantiate the WASM module
            const wasmModule = await globalThis.WebAssembly.compile(wasmBuffer);
            // Create imports object with common requirements
            const imports = {
                env: {
                    memory: new globalThis.WebAssembly.Memory({ initial: 256, maximum: 65536 }),
                    __wbindgen_throw: (ptr, len) => {
                        throw new Error(`WASM error at ${ptr} (len: ${len})`);
                    }
                },
                wbg: {
                    __wbg_random: () => Math.random(),
                    __wbg_now: () => Date.now(),
                    __wbindgen_object_drop_ref: () => { },
                    __wbindgen_string_new: (ptr, len) => {
                        // Simplified string handling
                        return `string_${ptr}_${len}`;
                    }
                }
            };
            const instance = await globalThis.WebAssembly.instantiate(wasmModule, imports);
            const module = {
                instance,
                exports: instance.exports,
                memory: imports.env.memory
            };
            this.modules.set(name, module);
            return module;
        }
        catch (error) {
            throw new Error(`Failed to load WASM module ${name}: ${error}`);
        }
    }
    /**
     * Get a loaded WASM module
     */
    static getModule(name) {
        return this.modules.get(name);
    }
    /**
     * Check if a module is available
     */
    static hasModule(name) {
        return this.modules.has(name);
    }
    /**
     * Get all loaded module names
     */
    static getLoadedModules() {
        return Array.from(this.modules.keys());
    }
    /**
     * Get memory usage statistics
     */
    static getMemoryStats() {
        const stats = {};
        for (const [name, module] of this.modules) {
            if (module.memory) {
                stats[name] = module.memory.buffer.byteLength;
            }
        }
        return stats;
    }
    /**
     * Check if WASM is available and return feature flags
     */
    static getFeatureFlags() {
        return {
            hasWasm: this.initialized && this.modules.size > 0,
            hasGraphReasoner: this.hasModule('graph_reasoner'),
            hasPlanner: this.hasModule('planner'),
            hasExtractors: this.hasModule('extractors'),
            hasTemporalNeural: this.hasModule('temporal_neural'),
            hasStrangeLoop: this.hasModule('strange_loop'),
            hasNanoConsciousness: this.hasModule('nano_consciousness')
        };
    }
}
// Auto-initialize on import (optional)
if (typeof process !== 'undefined' && process.env.AUTO_INIT_WASM === 'true') {
    WasmLoader.initialize().catch(console.error);
}
