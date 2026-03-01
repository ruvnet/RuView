/**
 * Node.js-specific entry point with filesystem support
 *
 * @security Path traversal prevention via ID validation
 */
export * from './index';
import { RuDag } from './index';
/**
 * Create a Node.js DAG with memory storage
 */
export declare function createNodeDag(name?: string): Promise<RuDag>;
/**
 * Stored DAG metadata
 */
interface StoredMeta {
    id: string;
    name?: string;
    metadata?: Record<string, unknown>;
    createdAt: number;
    updatedAt: number;
}
/**
 * File-based storage for Node.js environments
 * @security All file operations validate paths to prevent traversal attacks
 */
export declare class FileDagStorage {
    private basePath;
    private initialized;
    constructor(basePath?: string);
    init(): Promise<void>;
    private getFilePath;
    private getMetaPath;
    save(id: string, data: Uint8Array, options?: {
        name?: string;
        metadata?: Record<string, unknown>;
    }): Promise<void>;
    load(id: string): Promise<Uint8Array | null>;
    loadMeta(id: string): Promise<StoredMeta | null>;
    delete(id: string): Promise<boolean>;
    list(): Promise<string[]>;
    clear(): Promise<void>;
    stats(): Promise<{
        count: number;
        totalSize: number;
    }>;
}
/**
 * Node.js DAG manager with file persistence
 */
export declare class NodeDagManager {
    private storage;
    constructor(basePath?: string);
    init(): Promise<void>;
    createDag(name?: string): Promise<RuDag>;
    saveDag(dag: RuDag): Promise<void>;
    loadDag(id: string): Promise<RuDag | null>;
    deleteDag(id: string): Promise<boolean>;
    listDags(): Promise<string[]>;
    clearAll(): Promise<void>;
    getStats(): Promise<{
        count: number;
        totalSize: number;
    }>;
}
//# sourceMappingURL=node.d.ts.map