/**
 * Browser-specific entry point with IndexedDB support
 */

export * from './index';

// Re-export with browser-specific defaults
import { RuDag, DagStorage } from './index';

/**
 * Create a browser-optimized DAG with IndexedDB persistence
 */
export async function createBrowserDag(name?: string): Promise<RuDag> {
  const storage = new DagStorage();
  const dag = new RuDag({ name, storage });
  await dag.init();
  return dag;
}

/**
 * Browser storage manager for DAGs
 */
export class BrowserDagManager {
  private storage: DagStorage;
  private initialized = false;

  constructor() {
    this.storage = new DagStorage();
  }

  async init(): Promise<void> {
    if (this.initialized) return;
    await this.storage.init();
    this.initialized = true;
  }

  async createDag(name?: string): Promise<RuDag> {
    await this.init();
    const dag = new RuDag({ name, storage: this.storage });
    await dag.init();
    return dag;
  }

  async loadDag(id: string): Promise<RuDag | null> {
    await this.init();
    return RuDag.load(id, this.storage);
  }

  async listDags() {
    await this.init();
    return this.storage.list();
  }

  async deleteDag(id: string): Promise<boolean> {
    await this.init();
    return this.storage.delete(id);
  }

  async clearAll(): Promise<void> {
    await this.init();
    return this.storage.clear();
  }

  async getStats() {
    await this.init();
    return this.storage.stats();
  }

  close(): void {
    this.storage.close();
    this.initialized = false;
  }
}
