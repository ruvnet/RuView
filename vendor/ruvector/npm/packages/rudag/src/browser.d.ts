/**
 * Browser-specific entry point with IndexedDB support
 */
export * from './index';
import { RuDag } from './index';
/**
 * Create a browser-optimized DAG with IndexedDB persistence
 */
export declare function createBrowserDag(name?: string): Promise<RuDag>;
/**
 * Browser storage manager for DAGs
 */
export declare class BrowserDagManager {
    private storage;
    private initialized;
    constructor();
    init(): Promise<void>;
    createDag(name?: string): Promise<RuDag>;
    loadDag(id: string): Promise<RuDag | null>;
    listDags(): Promise<import("./storage").StoredDag[]>;
    deleteDag(id: string): Promise<boolean>;
    clearAll(): Promise<void>;
    getStats(): Promise<{
        count: number;
        totalSize: number;
    }>;
    close(): void;
}
//# sourceMappingURL=browser.d.ts.map