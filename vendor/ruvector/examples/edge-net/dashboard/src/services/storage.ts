/**
 * IndexedDB Storage Service
 * Persistent storage for Edge-Net node state
 */

const DB_NAME = 'edge-net-db';
const DB_VERSION = 1;
const STORE_NAME = 'node-state';

interface NodeState {
  id: string;
  nodeId: string | null;
  creditsEarned: number;
  creditsSpent: number;
  tasksCompleted: number;
  tasksSubmitted: number;
  totalUptime: number;
  lastActiveTimestamp: number;
  consentGiven: boolean;
  consentTimestamp: number | null;
  cpuLimit: number;
  gpuEnabled: boolean;
  gpuLimit: number;
  respectBattery: boolean;
  onlyWhenIdle: boolean;
}

class StorageService {
  private db: IDBDatabase | null = null;
  private initPromise: Promise<void> | null = null;

  async init(): Promise<void> {
    if (this.db) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, DB_VERSION);

      request.onerror = () => {
        console.error('[Storage] Failed to open IndexedDB:', request.error);
        reject(request.error);
      };

      request.onsuccess = () => {
        this.db = request.result;
        console.log('[Storage] IndexedDB opened successfully');
        resolve();
      };

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;

        if (!db.objectStoreNames.contains(STORE_NAME)) {
          db.createObjectStore(STORE_NAME, { keyPath: 'id' });
          console.log('[Storage] Created object store:', STORE_NAME);
        }
      };
    });

    return this.initPromise;
  }

  async saveState(state: NodeState): Promise<void> {
    await this.init();
    if (!this.db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction([STORE_NAME], 'readwrite');
      const store = transaction.objectStore(STORE_NAME);
      const request = store.put(state);

      request.onsuccess = () => {
        console.log('[Storage] State saved:', state.creditsEarned, 'rUv');
        resolve();
      };

      request.onerror = () => {
        console.error('[Storage] Failed to save state:', request.error);
        reject(request.error);
      };
    });
  }

  async loadState(): Promise<NodeState | null> {
    await this.init();
    if (!this.db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction([STORE_NAME], 'readonly');
      const store = transaction.objectStore(STORE_NAME);
      const request = store.get('primary');

      request.onsuccess = () => {
        const state = request.result as NodeState | undefined;
        if (state) {
          console.log('[Storage] Loaded state:', state.creditsEarned, 'rUv earned');
        } else {
          console.log('[Storage] No saved state found');
        }
        resolve(state || null);
      };

      request.onerror = () => {
        console.error('[Storage] Failed to load state:', request.error);
        reject(request.error);
      };
    });
  }

  async getDefaultState(): Promise<NodeState> {
    return {
      id: 'primary',
      nodeId: null,
      creditsEarned: 0,
      creditsSpent: 0,
      tasksCompleted: 0,
      tasksSubmitted: 0,
      totalUptime: 0,
      lastActiveTimestamp: Date.now(),
      consentGiven: false,
      consentTimestamp: null,
      cpuLimit: 50,
      gpuEnabled: false,
      gpuLimit: 30,
      respectBattery: true,
      onlyWhenIdle: true,
    };
  }

  async clear(): Promise<void> {
    await this.init();
    if (!this.db) return;

    return new Promise((resolve, reject) => {
      const transaction = this.db!.transaction([STORE_NAME], 'readwrite');
      const store = transaction.objectStore(STORE_NAME);
      const request = store.clear();

      request.onsuccess = () => {
        console.log('[Storage] State cleared');
        resolve();
      };

      request.onerror = () => {
        reject(request.error);
      };
    });
  }
}

export const storageService = new StorageService();
export type { NodeState };
