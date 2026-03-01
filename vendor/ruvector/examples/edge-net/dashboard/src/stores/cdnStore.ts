import { create } from 'zustand';
import type { CDNScript, CDNConfig } from '../types';

interface CDNState extends CDNConfig {
  isLoading: boolean;
  error: string | null;

  // Actions
  setScripts: (scripts: CDNScript[]) => void;
  toggleScript: (scriptId: string) => void;
  loadScript: (scriptId: string) => Promise<void>;
  unloadScript: (scriptId: string) => void;
  setAutoLoad: (autoLoad: boolean) => void;
  setCacheEnabled: (cacheEnabled: boolean) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

const defaultScripts: CDNScript[] = [
  // WASM Modules
  {
    id: 'edge-net-wasm',
    name: '@ruvector/edge-net',
    description: 'Core Edge-Net WASM module with Time Crystal and P2P capabilities',
    url: 'https://unpkg.com/@ruvector/edge-net@0.1.1/ruvector_edge_net_bg.wasm',
    size: '3.2 MB',
    category: 'wasm',
    enabled: true,
    loaded: false,
  },
  {
    id: 'attention-wasm',
    name: '@ruvector/attention-unified-wasm',
    description: 'DAG Attention mechanisms for task orchestration',
    url: 'https://unpkg.com/@ruvector/attention-unified-wasm@0.1.0/attention_unified_bg.wasm',
    size: '850 KB',
    category: 'wasm',
    enabled: false,
    loaded: false,
  },
  {
    id: 'economy-wasm',
    name: '@ruvector/economy-wasm',
    description: 'Credit economy and marketplace functionality',
    url: 'https://unpkg.com/@ruvector/economy-wasm@0.1.0/economy_bg.wasm',
    size: '620 KB',
    category: 'wasm',
    enabled: false,
    loaded: false,
  },
  // AI Libraries
  {
    id: 'tensorflow',
    name: 'TensorFlow.js',
    description: 'Machine learning library for browser-based AI',
    url: 'https://cdnjs.cloudflare.com/ajax/libs/tensorflow/4.15.0/tf.min.js',
    size: '1.8 MB',
    category: 'ai',
    enabled: false,
    loaded: false,
  },
  {
    id: 'onnx-runtime',
    name: 'ONNX Runtime Web',
    description: 'Run ONNX models in the browser with WebAssembly',
    url: 'https://cdnjs.cloudflare.com/ajax/libs/onnxruntime-web/1.17.0/ort.min.js',
    size: '2.1 MB',
    category: 'ai',
    enabled: false,
    loaded: false,
  },
  // Crypto Libraries
  {
    id: 'noble-curves',
    name: 'Noble Curves',
    description: 'Elliptic curve cryptography (Ed25519, secp256k1)',
    url: 'https://unpkg.com/@noble/curves@1.3.0/index.js',
    size: '45 KB',
    category: 'crypto',
    enabled: false,
    loaded: false,
  },
  {
    id: 'tweetnacl',
    name: 'TweetNaCl.js',
    description: 'Port of TweetNaCl cryptographic library',
    url: 'https://cdnjs.cloudflare.com/ajax/libs/tweetnacl/1.0.3/nacl-fast.min.js',
    size: '32 KB',
    category: 'crypto',
    enabled: false,
    loaded: false,
  },
  // Network Libraries
  {
    id: 'libp2p',
    name: 'libp2p',
    description: 'Modular peer-to-peer networking stack',
    url: 'https://unpkg.com/libp2p@1.2.0/dist/index.min.js',
    size: '680 KB',
    category: 'network',
    enabled: false,
    loaded: false,
  },
  {
    id: 'simple-peer',
    name: 'Simple Peer',
    description: 'Simple WebRTC video, voice, and data channels',
    url: 'https://cdnjs.cloudflare.com/ajax/libs/simple-peer/9.11.1/simplepeer.min.js',
    size: '95 KB',
    category: 'network',
    enabled: false,
    loaded: false,
  },
  // Utility Libraries
  {
    id: 'comlink',
    name: 'Comlink',
    description: 'Make Web Workers enjoyable with RPC-style API',
    url: 'https://unpkg.com/comlink@4.4.1/dist/umd/comlink.js',
    size: '4 KB',
    category: 'utility',
    enabled: false,
    loaded: false,
  },
  {
    id: 'idb-keyval',
    name: 'idb-keyval',
    description: 'Super simple IndexedDB key-value store',
    url: 'https://unpkg.com/idb-keyval@6.2.1/dist/umd.js',
    size: '3 KB',
    category: 'utility',
    enabled: false,
    loaded: false,
  },
];

export const useCDNStore = create<CDNState>((set, get) => ({
  scripts: defaultScripts,
  autoLoad: false,
  cacheEnabled: true,
  isLoading: false,
  error: null,

  setScripts: (scripts) => set({ scripts }),

  toggleScript: (scriptId) =>
    set((state) => ({
      scripts: state.scripts.map((s) =>
        s.id === scriptId ? { ...s, enabled: !s.enabled } : s
      ),
    })),

  loadScript: async (scriptId) => {
    const { scripts } = get();
    const script = scripts.find((s) => s.id === scriptId);

    if (!script || script.loaded) return;

    set({ isLoading: true, error: null });

    try {
      // Create script element
      const scriptEl = document.createElement('script');
      scriptEl.src = script.url;
      scriptEl.async = true;
      scriptEl.id = `cdn-${scriptId}`;

      await new Promise<void>((resolve, reject) => {
        scriptEl.onload = () => resolve();
        scriptEl.onerror = () => reject(new Error(`Failed to load ${script.name}`));
        document.head.appendChild(scriptEl);
      });

      set((state) => ({
        scripts: state.scripts.map((s) =>
          s.id === scriptId ? { ...s, loaded: true } : s
        ),
        isLoading: false,
      }));

      console.log(`[CDN] Loaded: ${script.name}`);
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load script',
        isLoading: false,
      });
    }
  },

  unloadScript: (scriptId) => {
    const scriptEl = document.getElementById(`cdn-${scriptId}`);
    if (scriptEl) {
      scriptEl.remove();
    }

    set((state) => ({
      scripts: state.scripts.map((s) =>
        s.id === scriptId ? { ...s, loaded: false } : s
      ),
    }));

    console.log(`[CDN] Unloaded: ${scriptId}`);
  },

  setAutoLoad: (autoLoad) => set({ autoLoad }),
  setCacheEnabled: (cacheEnabled) => set({ cacheEnabled }),
  setLoading: (loading) => set({ isLoading: loading }),
  setError: (error) => set({ error }),
}));
