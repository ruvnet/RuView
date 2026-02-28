import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { edgeNetService, type PiKeyInstance } from '../services/edgeNet';

export interface PeerIdentity {
  id: string;
  publicKey: string;
  publicKeyBytes?: Uint8Array;
  displayName: string;
  avatar?: string;
  createdAt: Date;
  shortId: string;
  identityHex: string;
  hasPiMagic: boolean;
}

export interface NetworkRegistration {
  networkId: string;
  networkName: string;
  status: 'pending' | 'active' | 'suspended' | 'expired';
  joinedAt: Date;
  capabilities: string[];
  reputation: number;
  creditsEarned: number;
}

export interface IdentityState {
  identity: PeerIdentity | null;
  registrations: NetworkRegistration[];
  isGenerating: boolean;
  isRegistering: boolean;
  error: string | null;
  piKeyBackup: string | null; // Encrypted backup (hex encoded)
  hasRealPiKey: boolean;

  // Actions
  generateIdentity: (displayName: string) => Promise<void>;
  importIdentity: (privateKeyOrBackup: string, password?: string) => Promise<void>;
  exportIdentity: (password: string) => Promise<string | null>;
  clearIdentity: () => void;
  registerNetwork: (networkId: string, capabilities: string[]) => Promise<void>;
  leaveNetwork: (networkId: string) => void;
  updateCapabilities: (networkId: string, capabilities: string[]) => void;
  signData: (data: Uint8Array) => Uint8Array | null;
  verifySignature: (data: Uint8Array, signature: Uint8Array, publicKey: Uint8Array) => boolean;
}

// Helper: Convert bytes to hex
function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

// Helper: Convert hex to bytes
function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return bytes;
}

// Real Web Crypto API fallback for Ed25519 (when WASM unavailable)
async function generateWebCryptoKeys(): Promise<{
  publicKey: Uint8Array;
  privateKey: Uint8Array;
  sign: (data: Uint8Array) => Promise<Uint8Array>;
  verify: (data: Uint8Array, sig: Uint8Array, pk: Uint8Array) => Promise<boolean>;
}> {
  // Use Web Crypto API for real Ed25519 keys
  // Note: Ed25519 support varies by browser, fall back to ECDSA P-256 if needed
  let keyPair: CryptoKeyPair;

  try {
    // Try Ed25519 first (supported in newer browsers)
    keyPair = await crypto.subtle.generateKey(
      { name: 'Ed25519' },
      true,
      ['sign', 'verify']
    );
  } catch {
    // Fall back to ECDSA P-256
    console.log('[Identity] Ed25519 not supported, using ECDSA P-256');
    keyPair = await crypto.subtle.generateKey(
      { name: 'ECDSA', namedCurve: 'P-256' },
      true,
      ['sign', 'verify']
    );
  }

  // Export keys
  const publicKeyBuffer = await crypto.subtle.exportKey('raw', keyPair.publicKey);
  const privateKeyBuffer = await crypto.subtle.exportKey('pkcs8', keyPair.privateKey);

  const publicKey = new Uint8Array(publicKeyBuffer);
  const privateKey = new Uint8Array(privateKeyBuffer);

  return {
    publicKey,
    privateKey,
    sign: async (data: Uint8Array) => {
      const algorithm = keyPair.privateKey.algorithm.name === 'Ed25519'
        ? { name: 'Ed25519' }
        : { name: 'ECDSA', hash: 'SHA-256' };
      const signature = await crypto.subtle.sign(algorithm, keyPair.privateKey, data.buffer as ArrayBuffer);
      return new Uint8Array(signature);
    },
    verify: async (data: Uint8Array, sig: Uint8Array, _pk: Uint8Array) => {
      const algorithm = keyPair.publicKey.algorithm.name === 'Ed25519'
        ? { name: 'Ed25519' }
        : { name: 'ECDSA', hash: 'SHA-256' };
      return crypto.subtle.verify(algorithm, keyPair.publicKey, sig.buffer as ArrayBuffer, data.buffer as ArrayBuffer);
    },
  };
}

// Generate unique peer ID from public key
function generatePeerId(publicKey: Uint8Array): string {
  // Use first 44 chars of base64 encoded public key for libp2p-style ID
  const base64 = btoa(String.fromCharCode(...publicKey));
  return `12D3KooW${base64.replace(/[+/=]/g, '').substring(0, 44)}`;
}

const availableNetworks = [
  {
    id: 'mainnet',
    name: 'Edge-Net Mainnet',
    description: 'Primary production network',
    requiredCapabilities: ['compute'],
  },
  {
    id: 'testnet',
    name: 'Edge-Net Testnet',
    description: 'Testing and development network',
    requiredCapabilities: [],
  },
  {
    id: 'research',
    name: 'Research Network',
    description: 'Academic and research collaboration',
    requiredCapabilities: ['compute', 'storage'],
  },
];

export { availableNetworks };

// Store the current Web Crypto instance for signing (used as fallback when WASM unavailable)
// Assigned in generateIdentity, cleared in clearIdentity, accessed in signData/verifySignature
interface WebCryptoState {
  sign: (data: Uint8Array) => Promise<Uint8Array>;
  verify: (data: Uint8Array, sig: Uint8Array, pk: Uint8Array) => Promise<boolean>;
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}
let webCryptoInstance: WebCryptoState | null = null;

// Export for external async signing when WASM unavailable
export function getWebCryptoInstance(): WebCryptoState | null {
  return webCryptoInstance;
}

let currentPiKey: PiKeyInstance | null = null;

export const useIdentityStore = create<IdentityState>()(
  persist(
    (set, get) => ({
      identity: null,
      registrations: [],
      isGenerating: false,
      isRegistering: false,
      error: null,
      piKeyBackup: null,
      hasRealPiKey: false,

      generateIdentity: async (displayName: string) => {
        set({ isGenerating: true, error: null });

        try {
          // Try real PiKey from WASM first
          const piKey = await edgeNetService.generateIdentity();

          if (piKey) {
            currentPiKey = piKey;

            const identity: PeerIdentity = {
              id: piKey.getShortId(),
              publicKey: bytesToHex(piKey.getPublicKey()),
              publicKeyBytes: piKey.getPublicKey(),
              displayName,
              createdAt: new Date(),
              shortId: piKey.getShortId(),
              identityHex: piKey.getIdentityHex(),
              hasPiMagic: piKey.verifyPiMagic(),
            };

            set({
              identity,
              hasRealPiKey: true,
              isGenerating: false,
            });

            console.log('[Identity] Generated real PiKey:', identity.shortId);
            console.log('[Identity] Has Pi magic:', identity.hasPiMagic);
            console.log('[Identity] Stats:', piKey.getStats());
            return;
          }

          // Fallback to Web Crypto API
          console.log('[Identity] Using Web Crypto API fallback');
          const cryptoKeys = await generateWebCryptoKeys();
          webCryptoInstance = cryptoKeys;

          const peerId = generatePeerId(cryptoKeys.publicKey);

          const identity: PeerIdentity = {
            id: peerId,
            publicKey: bytesToHex(cryptoKeys.publicKey),
            publicKeyBytes: cryptoKeys.publicKey,
            displayName,
            createdAt: new Date(),
            shortId: peerId.substring(0, 16),
            identityHex: bytesToHex(cryptoKeys.publicKey),
            hasPiMagic: false,
          };

          set({
            identity,
            hasRealPiKey: false,
            isGenerating: false,
          });

          console.log('[Identity] Generated Web Crypto identity:', identity.shortId);
        } catch (error) {
          console.error('[Identity] Generation failed:', error);
          set({
            error: error instanceof Error ? error.message : 'Failed to generate identity',
            isGenerating: false,
          });
        }
      },

      importIdentity: async (privateKeyOrBackup: string, password?: string) => {
        set({ isGenerating: true, error: null });

        try {
          // If password provided, treat as encrypted backup
          if (password) {
            // TODO: Implement PiKey.restoreFromBackup when available
            throw new Error('Encrypted backup import not yet implemented');
          }

          // Otherwise, validate hex private key
          if (privateKeyOrBackup.length < 32) {
            throw new Error('Invalid private key format');
          }

          // Generate new identity from seed
          const seed = hexToBytes(privateKeyOrBackup.substring(0, 64));
          const piKey = await edgeNetService.generateIdentity(seed);

          if (piKey) {
            currentPiKey = piKey;

            const identity: PeerIdentity = {
              id: piKey.getShortId(),
              publicKey: bytesToHex(piKey.getPublicKey()),
              publicKeyBytes: piKey.getPublicKey(),
              displayName: 'Imported Identity',
              createdAt: new Date(),
              shortId: piKey.getShortId(),
              identityHex: piKey.getIdentityHex(),
              hasPiMagic: piKey.verifyPiMagic(),
            };

            set({
              identity,
              hasRealPiKey: true,
              isGenerating: false,
            });

            console.log('[Identity] Imported PiKey:', identity.shortId);
            return;
          }

          throw new Error('Failed to import identity');
        } catch (error) {
          console.error('[Identity] Import failed:', error);
          set({
            error: error instanceof Error ? error.message : 'Failed to import identity',
            isGenerating: false,
          });
        }
      },

      exportIdentity: async (password: string) => {
        const { hasRealPiKey } = get();

        if (hasRealPiKey && currentPiKey) {
          try {
            // Create encrypted backup with Argon2id
            const backup = currentPiKey.createEncryptedBackup(password);
            const backupHex = bytesToHex(backup);

            set({ piKeyBackup: backupHex });

            console.log('[Identity] Created encrypted backup');
            return backupHex;
          } catch (error) {
            console.error('[Identity] Export failed:', error);
            return null;
          }
        }

        // For Web Crypto keys, export as JSON (less secure)
        const { identity } = get();
        if (!identity) return null;

        return JSON.stringify({
          publicKey: identity.publicKey,
          displayName: identity.displayName,
          note: 'Web Crypto fallback - private key not exportable',
        });
      },

      clearIdentity: () => {
        if (currentPiKey) {
          currentPiKey.free();
          currentPiKey = null;
        }
        webCryptoInstance = null;

        set({
          identity: null,
          registrations: [],
          piKeyBackup: null,
          hasRealPiKey: false,
        });

        console.log('[Identity] Cleared identity');
      },

      registerNetwork: async (networkId: string, capabilities: string[]) => {
        const { identity, registrations } = get();

        if (!identity) {
          set({ error: 'No identity found. Generate or import an identity first.' });
          return;
        }

        if (registrations.some(r => r.networkId === networkId)) {
          set({ error: 'Already registered to this network' });
          return;
        }

        set({ isRegistering: true, error: null });

        try {
          // Create real EdgeNet node for this network
          const node = await edgeNetService.createNode(`${networkId}-${identity.shortId}`);

          if (node) {
            // Enable Time Crystal for synchronization
            edgeNetService.enableTimeCrystal(8);
            edgeNetService.startNode();

            console.log('[Identity] Connected to real EdgeNet node');
          }

          const network = availableNetworks.find(n => n.id === networkId);

          const registration: NetworkRegistration = {
            networkId,
            networkName: network?.name || networkId,
            status: 'active',
            joinedAt: new Date(),
            capabilities,
            reputation: 100,
            creditsEarned: 0,
          };

          set({
            registrations: [...registrations, registration],
            isRegistering: false,
          });

          console.log('[Identity] Registered to network:', networkId);
        } catch (error) {
          console.error('[Identity] Registration failed:', error);
          set({
            error: error instanceof Error ? error.message : 'Failed to register',
            isRegistering: false,
          });
        }
      },

      leaveNetwork: (networkId: string) => {
        edgeNetService.pauseNode();

        set((state) => ({
          registrations: state.registrations.filter(r => r.networkId !== networkId),
        }));

        console.log('[Identity] Left network:', networkId);
      },

      updateCapabilities: (networkId: string, capabilities: string[]) => {
        set((state) => ({
          registrations: state.registrations.map(r =>
            r.networkId === networkId ? { ...r, capabilities } : r
          ),
        }));
      },

      signData: (data: Uint8Array): Uint8Array | null => {
        if (currentPiKey) {
          return currentPiKey.sign(data);
        }
        // Web Crypto signing is async, but we need sync here
        // Return null and use async version externally
        return null;
      },

      verifySignature: (data: Uint8Array, signature: Uint8Array, publicKey: Uint8Array): boolean => {
        if (currentPiKey) {
          return currentPiKey.verify(data, signature, publicKey);
        }
        return false;
      },
    }),
    {
      name: 'edge-net-identity',
      partialize: (state) => ({
        identity: state.identity ? {
          ...state.identity,
          publicKeyBytes: undefined, // Don't persist Uint8Array
        } : null,
        registrations: state.registrations,
        piKeyBackup: state.piKeyBackup,
        hasRealPiKey: state.hasRealPiKey,
      }),
    }
  )
);
