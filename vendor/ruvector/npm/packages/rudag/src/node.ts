/**
 * Node.js-specific entry point with filesystem support
 *
 * @security Path traversal prevention via ID validation
 */

export * from './index';

import { RuDag, MemoryStorage } from './index';
import { promises as fs } from 'fs';
import { join, normalize, resolve } from 'path';

/**
 * Validate storage ID to prevent path traversal attacks
 * @security Only allows alphanumeric, dash, underscore characters
 */
function isValidStorageId(id: string): boolean {
  if (typeof id !== 'string' || id.length === 0 || id.length > 256) return false;
  // Strictly alphanumeric with dash/underscore - no dots, slashes, etc.
  return /^[a-zA-Z0-9_-]+$/.test(id);
}

/**
 * Ensure path is within base directory
 * @security Prevents path traversal via realpath comparison
 */
async function ensureWithinBase(basePath: string, targetPath: string): Promise<string> {
  const resolvedBase = resolve(basePath);
  const resolvedTarget = resolve(targetPath);

  if (!resolvedTarget.startsWith(resolvedBase + '/') && resolvedTarget !== resolvedBase) {
    throw new Error('Path traversal detected: target path outside base directory');
  }

  return resolvedTarget;
}

/**
 * Create a Node.js DAG with memory storage
 */
export async function createNodeDag(name?: string): Promise<RuDag> {
  const storage = new MemoryStorage();
  const dag = new RuDag({ name, storage });
  await dag.init();
  return dag;
}

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
export class FileDagStorage {
  private basePath: string;
  private initialized = false;

  constructor(basePath: string = '.rudag') {
    // Normalize and resolve base path
    this.basePath = resolve(normalize(basePath));
  }

  async init(): Promise<void> {
    if (this.initialized) return;

    try {
      await fs.mkdir(this.basePath, { recursive: true });
      this.initialized = true;
    } catch (error) {
      throw new Error(`Failed to create storage directory: ${error}`);
    }
  }

  private async getFilePath(id: string): Promise<string> {
    if (!isValidStorageId(id)) {
      throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
    }
    const targetPath = join(this.basePath, `${id}.dag`);
    return ensureWithinBase(this.basePath, targetPath);
  }

  private async getMetaPath(id: string): Promise<string> {
    if (!isValidStorageId(id)) {
      throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
    }
    const targetPath = join(this.basePath, `${id}.meta.json`);
    return ensureWithinBase(this.basePath, targetPath);
  }

  async save(id: string, data: Uint8Array, options: { name?: string; metadata?: Record<string, unknown> } = {}): Promise<void> {
    await this.init();

    const filePath = await this.getFilePath(id);
    const metaPath = await this.getMetaPath(id);

    // Load existing metadata for createdAt preservation
    let existingMeta: StoredMeta | null = null;
    try {
      const metaContent = await fs.readFile(metaPath, 'utf-8');
      existingMeta = JSON.parse(metaContent) as StoredMeta;
    } catch {
      // File doesn't exist or invalid - will create new
    }

    const now = Date.now();
    const meta: StoredMeta = {
      id,
      name: options.name,
      metadata: options.metadata,
      createdAt: existingMeta?.createdAt || now,
      updatedAt: now,
    };

    // Write both files atomically (as much as possible)
    await Promise.all([
      fs.writeFile(filePath, Buffer.from(data)),
      fs.writeFile(metaPath, JSON.stringify(meta, null, 2)),
    ]);
  }

  async load(id: string): Promise<Uint8Array | null> {
    await this.init();

    const filePath = await this.getFilePath(id);

    try {
      const data = await fs.readFile(filePath);
      return new Uint8Array(data);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return null;
      }
      throw error;
    }
  }

  async loadMeta(id: string): Promise<StoredMeta | null> {
    await this.init();

    const metaPath = await this.getMetaPath(id);

    try {
      const content = await fs.readFile(metaPath, 'utf-8');
      return JSON.parse(content) as StoredMeta;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return null;
      }
      throw error;
    }
  }

  async delete(id: string): Promise<boolean> {
    await this.init();

    const filePath = await this.getFilePath(id);
    const metaPath = await this.getMetaPath(id);

    const results = await Promise.allSettled([
      fs.unlink(filePath),
      fs.unlink(metaPath),
    ]);

    // Return true if at least one file was deleted
    return results.some(r => r.status === 'fulfilled');
  }

  async list(): Promise<string[]> {
    await this.init();

    try {
      const files = await fs.readdir(this.basePath);
      return files
        .filter(f => f.endsWith('.dag'))
        .map(f => f.slice(0, -4)) // Remove .dag extension
        .filter(id => isValidStorageId(id)); // Extra safety filter
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return [];
      }
      throw error;
    }
  }

  async clear(): Promise<void> {
    await this.init();

    const ids = await this.list();
    await Promise.all(ids.map(id => this.delete(id)));
  }

  async stats(): Promise<{ count: number; totalSize: number }> {
    await this.init();

    const ids = await this.list();
    let totalSize = 0;

    for (const id of ids) {
      try {
        const filePath = await this.getFilePath(id);
        const stat = await fs.stat(filePath);
        totalSize += stat.size;
      } catch {
        // Skip files that can't be accessed
      }
    }

    return { count: ids.length, totalSize };
  }
}

/**
 * Node.js DAG manager with file persistence
 */
export class NodeDagManager {
  private storage: FileDagStorage;

  constructor(basePath?: string) {
    this.storage = new FileDagStorage(basePath);
  }

  async init(): Promise<void> {
    await this.storage.init();
  }

  async createDag(name?: string): Promise<RuDag> {
    const dag = new RuDag({ name, storage: null, autoSave: false });
    await dag.init();
    return dag;
  }

  async saveDag(dag: RuDag): Promise<void> {
    const data = dag.toBytes();
    await this.storage.save(dag.getId(), data, { name: dag.getName() });
  }

  async loadDag(id: string): Promise<RuDag | null> {
    const data = await this.storage.load(id);
    if (!data) return null;

    const meta = await this.storage.loadMeta(id);
    return RuDag.fromBytes(data, { id, name: meta?.name });
  }

  async deleteDag(id: string): Promise<boolean> {
    return this.storage.delete(id);
  }

  async listDags(): Promise<string[]> {
    return this.storage.list();
  }

  async clearAll(): Promise<void> {
    return this.storage.clear();
  }

  async getStats(): Promise<{ count: number; totalSize: number }> {
    return this.storage.stats();
  }
}
