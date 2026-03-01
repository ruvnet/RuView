/**
 * Browser-specific exports for @ruvector/wasm
 */
import type { VectorEntry, SearchResult, DbOptions } from './index';
/**
 * VectorDB class for browser
 */
export declare class VectorDB {
    private db;
    private dimensions;
    constructor(options: DbOptions);
    init(): Promise<void>;
    insert(vector: Float32Array | number[], id?: string, metadata?: Record<string, any>): string;
    insertBatch(entries: VectorEntry[]): string[];
    search(query: Float32Array | number[], k: number, filter?: Record<string, any>): SearchResult[];
    delete(id: string): boolean;
    get(id: string): VectorEntry | null;
    len(): number;
    isEmpty(): boolean;
    getDimensions(): number;
    saveToIndexedDB(): Promise<void>;
    static loadFromIndexedDB(dbName: string, options: DbOptions): Promise<VectorDB>;
}
export declare function detectSIMD(): Promise<boolean>;
export declare function version(): Promise<string>;
export declare function benchmark(name: string, iterations: number, dimensions: number): Promise<number>;
export type { VectorEntry, SearchResult, DbOptions };
export default VectorDB;
//# sourceMappingURL=browser.d.ts.map