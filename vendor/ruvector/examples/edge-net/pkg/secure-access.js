/**
 * @ruvector/edge-net Secure Access Layer
 *
 * Uses WASM cryptographic primitives for secure network access.
 * No external authentication needed - cryptographic proof of identity.
 *
 * Security Model:
 * 1. Each node generates a PiKey (Ed25519-based) in WASM
 * 2. All messages are signed with the node's private key
 * 3. Other nodes verify signatures with public keys
 * 4. AdaptiveSecurity provides self-learning attack detection
 *
 * @module @ruvector/edge-net/secure-access
 */

import { EventEmitter } from 'events';

/**
 * Secure Access Manager
 *
 * Provides WASM-based cryptographic identity and message signing
 * for secure P2P network access without external auth providers.
 */
export class SecureAccessManager extends EventEmitter {
    constructor(options = {}) {
        super();

        /** @type {import('./ruvector_edge_net').PiKey|null} */
        this.piKey = null;

        /** @type {import('./ruvector_edge_net').SessionKey|null} */
        this.sessionKey = null;

        /** @type {import('./ruvector_edge_net').WasmNodeIdentity|null} */
        this.nodeIdentity = null;

        /** @type {import('./ruvector_edge_net').AdaptiveSecurity|null} */
        this.security = null;

        /** @type {Map<string, Uint8Array>} Known peer public keys */
        this.knownPeers = new Map();

        /** @type {Map<string, number>} Peer reputation scores */
        this.peerReputation = new Map();

        this.options = {
            siteId: options.siteId || 'edge-net',
            sessionTTL: options.sessionTTL || 3600, // 1 hour
            backupPassword: options.backupPassword || null,
            persistIdentity: options.persistIdentity !== false,
            ...options
        };

        this.wasm = null;
        this.initialized = false;
    }

    /**
     * Initialize secure access with WASM cryptography
     */
    async initialize() {
        if (this.initialized) return this;

        console.log('🔐 Initializing WASM Secure Access...');

        // Load WASM module
        try {
            // For Node.js, use the node-specific CJS module which auto-loads WASM
            const isNode = typeof process !== 'undefined' && process.versions?.node;
            if (isNode) {
                // Node.js: CJS module loads WASM synchronously on import
                this.wasm = await import('./node/ruvector_edge_net.cjs');
            } else {
                // Browser: Use ES module with WASM init
                const wasmModule = await import('./ruvector_edge_net.js');
                // Call default init to load WASM binary
                if (wasmModule.default && typeof wasmModule.default === 'function') {
                    await wasmModule.default();
                }
                this.wasm = wasmModule;
            }
        } catch (err) {
            console.error('   ❌ WASM load error:', err.message);
            throw err;
        }

        // Try to restore existing identity
        const restored = await this._tryRestoreIdentity();

        if (!restored) {
            // Generate new cryptographic identity
            await this._generateIdentity();
        }

        // Initialize adaptive security
        this.security = new this.wasm.AdaptiveSecurity();

        // Create session key for encrypted communications
        this.sessionKey = new this.wasm.SessionKey(this.piKey, this.options.sessionTTL);

        this.initialized = true;

        console.log(`   🔑 Node ID: ${this.getShortId()}`);
        console.log(`   📦 Public Key: ${this.getPublicKeyHex().slice(0, 16)}...`);
        console.log(`   ⏱️  Session expires: ${new Date(Date.now() + this.options.sessionTTL * 1000).toISOString()}`);

        this.emit('initialized', {
            nodeId: this.getNodeId(),
            publicKey: this.getPublicKeyHex()
        });

        return this;
    }

    /**
     * Try to restore identity from localStorage or backup
     */
    async _tryRestoreIdentity() {
        if (!this.options.persistIdentity) return false;

        try {
            // Check localStorage (browser) or file (Node.js)
            let stored = null;

            if (typeof localStorage !== 'undefined') {
                stored = localStorage.getItem('edge-net-identity');
            } else if (typeof process !== 'undefined') {
                const fs = await import('fs');
                const path = await import('path');
                const identityPath = path.join(process.cwd(), '.edge-net-identity');
                if (fs.existsSync(identityPath)) {
                    stored = fs.readFileSync(identityPath, 'utf8');
                }
            }

            if (stored) {
                const data = JSON.parse(stored);
                const encrypted = new Uint8Array(data.encrypted);

                // Use default password if none provided
                const password = this.options.backupPassword || 'edge-net-default-key';

                this.piKey = this.wasm.PiKey.restoreFromBackup(encrypted, password);
                this.nodeIdentity = this.wasm.WasmNodeIdentity.fromSecretKey(
                    encrypted, // Same key derivation
                    this.options.siteId
                );

                console.log('   ♻️  Restored existing identity');
                return true;
            }
        } catch (err) {
            console.log('   ⚡ Creating new identity (no backup found)');
        }

        return false;
    }

    /**
     * Generate new cryptographic identity
     */
    async _generateIdentity() {
        // Generate Pi-Key (Ed25519-based with Pi magic)
        // Constructor takes optional genesis_seed (Uint8Array or null)
        const genesisSeed = this.options.genesisSeed || null;
        this.piKey = new this.wasm.PiKey(genesisSeed);

        // Create node identity from same site
        this.nodeIdentity = new this.wasm.WasmNodeIdentity(this.options.siteId);

        // Persist identity if enabled
        if (this.options.persistIdentity) {
            await this._persistIdentity();
        }

        console.log('   ✨ Generated new cryptographic identity');
    }

    /**
     * Persist identity to storage
     */
    async _persistIdentity() {
        const password = this.options.backupPassword || 'edge-net-default-key';
        const backup = this.piKey.createEncryptedBackup(password);
        const data = JSON.stringify({
            encrypted: Array.from(backup),
            created: Date.now(),
            siteId: this.options.siteId
        });

        try {
            if (typeof localStorage !== 'undefined') {
                localStorage.setItem('edge-net-identity', data);
            } else if (typeof process !== 'undefined') {
                const fs = await import('fs');
                const path = await import('path');
                const identityPath = path.join(process.cwd(), '.edge-net-identity');
                fs.writeFileSync(identityPath, data);
            }
        } catch (err) {
            console.warn('   ⚠️  Could not persist identity:', err.message);
        }
    }

    // ============================================
    // IDENTITY & KEYS
    // ============================================

    /**
     * Get node ID (full)
     */
    getNodeId() {
        return this.piKey?.getIdentityHex() || this.nodeIdentity?.getId?.() || 'unknown';
    }

    /**
     * Get short node ID for display
     */
    getShortId() {
        return this.piKey?.getShortId() || this.getNodeId().slice(0, 8);
    }

    /**
     * Get public key as hex string
     */
    getPublicKeyHex() {
        return Array.from(this.piKey?.getPublicKey() || new Uint8Array(32))
            .map(b => b.toString(16).padStart(2, '0'))
            .join('');
    }

    /**
     * Get public key as bytes
     */
    getPublicKeyBytes() {
        return this.piKey?.getPublicKey() || new Uint8Array(32);
    }

    // ============================================
    // MESSAGE SIGNING & VERIFICATION
    // ============================================

    /**
     * Sign a message/object
     * @param {object|string|Uint8Array} message - Message to sign
     * @returns {{ payload: string, signature: string, publicKey: string, timestamp: number }}
     */
    signMessage(message) {
        const payload = typeof message === 'string' ? message :
                        message instanceof Uint8Array ? new TextDecoder().decode(message) :
                        JSON.stringify(message);

        const timestamp = Date.now();
        const dataToSign = `${payload}|${timestamp}`;
        const dataBytes = new TextEncoder().encode(dataToSign);

        const signature = this.piKey.sign(dataBytes);

        return {
            payload,
            signature: Array.from(signature).map(b => b.toString(16).padStart(2, '0')).join(''),
            publicKey: this.getPublicKeyHex(),
            timestamp,
            nodeId: this.getShortId()
        };
    }

    /**
     * Verify a signed message
     * @param {object} signed - Signed message object
     * @returns {boolean} Whether signature is valid
     */
    verifyMessage(signed) {
        try {
            const { payload, signature, publicKey, timestamp } = signed;

            // Check timestamp (reject messages older than 5 minutes)
            const age = Date.now() - timestamp;
            if (age > 5 * 60 * 1000) {
                console.warn('⚠️ Message too old:', age, 'ms');
                return false;
            }

            // Convert hex strings back to bytes
            const dataToVerify = `${payload}|${timestamp}`;
            const dataBytes = new TextEncoder().encode(dataToVerify);
            const sigBytes = new Uint8Array(signature.match(/.{2}/g).map(h => parseInt(h, 16)));
            const pubKeyBytes = new Uint8Array(publicKey.match(/.{2}/g).map(h => parseInt(h, 16)));

            // Verify using WASM
            const valid = this.piKey.verify(dataBytes, sigBytes, pubKeyBytes);

            // Update peer reputation based on verification
            if (valid) {
                this._updateReputation(signed.nodeId || publicKey.slice(0, 16), 0.01);
            } else {
                this._updateReputation(signed.nodeId || publicKey.slice(0, 16), -0.1);
                this._recordSuspicious(signed.nodeId, 'invalid_signature');
            }

            return valid;
        } catch (err) {
            console.warn('⚠️ Signature verification error:', err.message);
            return false;
        }
    }

    // ============================================
    // PEER MANAGEMENT
    // ============================================

    /**
     * Register a known peer's public key
     */
    registerPeer(peerId, publicKey) {
        const pubKeyBytes = typeof publicKey === 'string' ?
            new Uint8Array(publicKey.match(/.{2}/g).map(h => parseInt(h, 16))) :
            publicKey;

        this.knownPeers.set(peerId, pubKeyBytes);
        this.peerReputation.set(peerId, this.peerReputation.get(peerId) || 0.5);

        this.emit('peer-registered', { peerId, publicKey: this.getPublicKeyHex() });
    }

    /**
     * Get reputation score for a peer (0-1)
     */
    getPeerReputation(peerId) {
        return this.peerReputation.get(peerId) || 0.5;
    }

    /**
     * Update peer reputation
     */
    _updateReputation(peerId, delta) {
        const current = this.peerReputation.get(peerId) || 0.5;
        const newScore = Math.max(0, Math.min(1, current + delta));
        this.peerReputation.set(peerId, newScore);

        // Emit warning if reputation drops too low
        if (newScore < 0.2) {
            this.emit('peer-suspicious', { peerId, reputation: newScore });
        }
    }

    /**
     * Record suspicious activity for learning
     */
    _recordSuspicious(peerId, reason) {
        if (this.security) {
            // Record for adaptive security learning
            const features = new Float32Array([
                Date.now() / 1e12,
                this.getPeerReputation(peerId),
                reason === 'invalid_signature' ? 1 : 0,
                reason === 'replay_attack' ? 1 : 0,
                0, 0, 0, 0 // Padding
            ]);
            this.security.recordAttackPattern(reason, features, 0.5);
        }
    }

    // ============================================
    // ENCRYPTION (SESSION-BASED)
    // ============================================

    /**
     * Encrypt data for secure transmission
     */
    encrypt(data) {
        if (!this.sessionKey || this.sessionKey.isExpired()) {
            // Refresh session key
            this.sessionKey = new this.wasm.SessionKey(this.piKey, this.options.sessionTTL);
        }

        const dataBytes = typeof data === 'string' ?
            new TextEncoder().encode(data) :
            data instanceof Uint8Array ? data :
            new TextEncoder().encode(JSON.stringify(data));

        return this.sessionKey.encrypt(dataBytes);
    }

    /**
     * Decrypt received data
     */
    decrypt(encrypted) {
        if (!this.sessionKey) {
            throw new Error('No session key available');
        }

        return this.sessionKey.decrypt(encrypted);
    }

    // ============================================
    // SECURITY ANALYSIS
    // ============================================

    /**
     * Analyze request for potential attacks
     * @returns {number} Threat score (0-1, higher = more suspicious)
     */
    analyzeRequest(features) {
        if (!this.security) return 0;

        const featureArray = features instanceof Float32Array ?
            features :
            new Float32Array(Array.isArray(features) ? features : Object.values(features));

        return this.security.detectAttack(featureArray);
    }

    /**
     * Get security statistics
     */
    getSecurityStats() {
        if (!this.security) return null;

        return JSON.parse(this.security.getStats());
    }

    /**
     * Export security patterns for persistence
     */
    exportSecurityPatterns() {
        if (!this.security) return null;
        return this.security.exportPatterns();
    }

    /**
     * Import previously learned security patterns
     */
    importSecurityPatterns(patterns) {
        if (!this.security) return;
        this.security.importPatterns(patterns);
    }

    // ============================================
    // CHALLENGE-RESPONSE
    // ============================================

    /**
     * Create a challenge for peer verification
     */
    createChallenge() {
        const challenge = crypto.getRandomValues(new Uint8Array(32));
        const timestamp = Date.now();

        return {
            challenge: Array.from(challenge).map(b => b.toString(16).padStart(2, '0')).join(''),
            timestamp,
            issuer: this.getShortId()
        };
    }

    /**
     * Respond to a challenge (proves identity)
     */
    respondToChallenge(challengeData) {
        const challengeBytes = new Uint8Array(
            challengeData.challenge.match(/.{2}/g).map(h => parseInt(h, 16))
        );

        const responseData = new Uint8Array([
            ...challengeBytes,
            ...new TextEncoder().encode(`|${challengeData.timestamp}|${this.getShortId()}`)
        ]);

        const signature = this.piKey.sign(responseData);

        return {
            ...challengeData,
            response: Array.from(signature).map(b => b.toString(16).padStart(2, '0')).join(''),
            responder: this.getShortId(),
            publicKey: this.getPublicKeyHex()
        };
    }

    /**
     * Verify a challenge response
     */
    verifyChallengeResponse(response) {
        try {
            const challengeBytes = new Uint8Array(
                response.challenge.match(/.{2}/g).map(h => parseInt(h, 16))
            );

            const responseData = new Uint8Array([
                ...challengeBytes,
                ...new TextEncoder().encode(`|${response.timestamp}|${response.responder}`)
            ]);

            const sigBytes = new Uint8Array(
                response.response.match(/.{2}/g).map(h => parseInt(h, 16))
            );
            const pubKeyBytes = new Uint8Array(
                response.publicKey.match(/.{2}/g).map(h => parseInt(h, 16))
            );

            const valid = this.piKey.verify(responseData, sigBytes, pubKeyBytes);

            if (valid) {
                // Register this peer as verified
                this.registerPeer(response.responder, response.publicKey);
                this._updateReputation(response.responder, 0.05);
            }

            return valid;
        } catch (err) {
            console.warn('Challenge verification failed:', err.message);
            return false;
        }
    }

    /**
     * Clean up resources
     */
    dispose() {
        try { this.piKey?.free?.(); } catch (e) { /* already freed */ }
        try { this.sessionKey?.free?.(); } catch (e) { /* already freed */ }
        try { this.nodeIdentity?.free?.(); } catch (e) { /* already freed */ }
        try { this.security?.free?.(); } catch (e) { /* already freed */ }
        this.piKey = null;
        this.sessionKey = null;
        this.nodeIdentity = null;
        this.security = null;
        this.knownPeers.clear();
        this.peerReputation.clear();
        this.initialized = false;
    }
}

/**
 * Create a secure access manager
 */
export async function createSecureAccess(options = {}) {
    const manager = new SecureAccessManager(options);
    await manager.initialize();
    return manager;
}

/**
 * Wrap Firebase signaling with WASM security
 */
export function wrapWithSecurity(firebaseSignaling, secureAccess) {
    const originalAnnounce = firebaseSignaling.announcePeer?.bind(firebaseSignaling);
    const originalSendOffer = firebaseSignaling.sendOffer?.bind(firebaseSignaling);
    const originalSendAnswer = firebaseSignaling.sendAnswer?.bind(firebaseSignaling);
    const originalSendIceCandidate = firebaseSignaling.sendIceCandidate?.bind(firebaseSignaling);

    // Wrap peer announcement with signature
    if (originalAnnounce) {
        firebaseSignaling.announcePeer = async (peerId, metadata = {}) => {
            const signedMetadata = secureAccess.signMessage({
                ...metadata,
                publicKey: secureAccess.getPublicKeyHex()
            });
            return originalAnnounce(peerId, signedMetadata);
        };
    }

    // Wrap signaling messages with signatures
    if (originalSendOffer) {
        firebaseSignaling.sendOffer = async (toPeerId, offer) => {
            const signed = secureAccess.signMessage({ type: 'offer', offer });
            return originalSendOffer(toPeerId, signed);
        };
    }

    if (originalSendAnswer) {
        firebaseSignaling.sendAnswer = async (toPeerId, answer) => {
            const signed = secureAccess.signMessage({ type: 'answer', answer });
            return originalSendAnswer(toPeerId, signed);
        };
    }

    if (originalSendIceCandidate) {
        firebaseSignaling.sendIceCandidate = async (toPeerId, candidate) => {
            const signed = secureAccess.signMessage({ type: 'ice', candidate });
            return originalSendIceCandidate(toPeerId, signed);
        };
    }

    // Add verification method
    firebaseSignaling.verifySignedMessage = (signed) => {
        return secureAccess.verifyMessage(signed);
    };

    firebaseSignaling.secureAccess = secureAccess;

    return firebaseSignaling;
}

export default SecureAccessManager;
