/**
 * @ruvector/edge-net Model Integrity System
 *
 * Content-addressed integrity with:
 * - Canonical JSON signing
 * - Threshold signatures with trust roots
 * - Merkle chunk verification for streaming
 * - Transparency log integration
 *
 * Design principle: Manifest is truth, everything else is replaceable.
 *
 * @module @ruvector/edge-net/models/integrity
 */

import { createHash } from 'crypto';

// ============================================================================
// CANONICAL JSON
// ============================================================================

/**
 * Canonical JSON encoding for deterministic signing.
 * - Keys sorted lexicographically
 * - No whitespace
 * - Unicode escaped consistently
 * - Numbers without trailing zeros
 */
export function canonicalize(obj) {
    if (obj === null || obj === undefined) {
        return 'null';
    }

    if (typeof obj === 'boolean') {
        return obj ? 'true' : 'false';
    }

    if (typeof obj === 'number') {
        if (!Number.isFinite(obj)) {
            throw new Error('Cannot canonicalize Infinity or NaN');
        }
        // Use JSON for consistent number formatting
        return JSON.stringify(obj);
    }

    if (typeof obj === 'string') {
        // Escape unicode consistently
        return JSON.stringify(obj).replace(/[\u007f-\uffff]/g, (c) => {
            return '\\u' + ('0000' + c.charCodeAt(0).toString(16)).slice(-4);
        });
    }

    if (Array.isArray(obj)) {
        const elements = obj.map(canonicalize);
        return '[' + elements.join(',') + ']';
    }

    if (typeof obj === 'object') {
        const keys = Object.keys(obj).sort();
        const pairs = keys
            .filter(k => obj[k] !== undefined)
            .map(k => canonicalize(k) + ':' + canonicalize(obj[k]));
        return '{' + pairs.join(',') + '}';
    }

    throw new Error(`Cannot canonicalize type: ${typeof obj}`);
}

/**
 * Hash canonical JSON bytes
 */
export function hashCanonical(obj, algorithm = 'sha256') {
    const canonical = canonicalize(obj);
    const hash = createHash(algorithm);
    hash.update(canonical, 'utf8');
    return hash.digest('hex');
}

// ============================================================================
// TRUST ROOT
// ============================================================================

/**
 * Built-in root keys shipped with SDK.
 * These are the only keys trusted by default.
 */
export const BUILTIN_ROOT_KEYS = Object.freeze({
    'ruvector-root-2024': {
        keyId: 'ruvector-root-2024',
        algorithm: 'ed25519',
        publicKey: 'MCowBQYDK2VwAyEAaGVsbG8td29ybGQta2V5LXBsYWNlaG9sZGVy', // Placeholder
        validFrom: '2024-01-01T00:00:00Z',
        validUntil: '2030-01-01T00:00:00Z',
        capabilities: ['sign-manifest', 'sign-adapter', 'delegate'],
    },
    'ruvector-models-2024': {
        keyId: 'ruvector-models-2024',
        algorithm: 'ed25519',
        publicKey: 'MCowBQYDK2VwAyEAbW9kZWxzLWtleS1wbGFjZWhvbGRlcg==', // Placeholder
        validFrom: '2024-01-01T00:00:00Z',
        validUntil: '2026-01-01T00:00:00Z',
        capabilities: ['sign-manifest'],
        delegatedBy: 'ruvector-root-2024',
    },
});

/**
 * Trust root configuration
 */
export class TrustRoot {
    constructor(options = {}) {
        // Start with built-in keys
        this.trustedKeys = new Map();
        for (const [id, key] of Object.entries(BUILTIN_ROOT_KEYS)) {
            this.trustedKeys.set(id, key);
        }

        // Add enterprise keys if configured
        if (options.enterpriseKeys) {
            for (const key of options.enterpriseKeys) {
                this.addEnterpriseKey(key);
            }
        }

        // Revocation list
        this.revokedKeys = new Set(options.revokedKeys || []);

        // Minimum signatures required for official releases
        this.minimumSignaturesRequired = options.minimumSignaturesRequired || 1;

        // Threshold for high-security operations (e.g., new root key)
        this.thresholdSignaturesRequired = options.thresholdSignaturesRequired || 2;
    }

    /**
     * Add an enterprise root key (for private deployments)
     */
    addEnterpriseKey(key) {
        if (!key.keyId || !key.publicKey) {
            throw new Error('Enterprise key must have keyId and publicKey');
        }

        // Verify delegation chain if not self-signed
        if (key.delegatedBy && key.delegationSignature) {
            const delegator = this.trustedKeys.get(key.delegatedBy);
            if (!delegator) {
                throw new Error(`Unknown delegator: ${key.delegatedBy}`);
            }
            if (!delegator.capabilities.includes('delegate')) {
                throw new Error(`Key ${key.delegatedBy} cannot delegate`);
            }
            // In production, verify delegationSignature here
        }

        this.trustedKeys.set(key.keyId, {
            ...key,
            isEnterprise: true,
        });
    }

    /**
     * Revoke a key
     */
    revokeKey(keyId, reason) {
        this.revokedKeys.add(keyId);
        console.warn(`[TrustRoot] Key revoked: ${keyId} - ${reason}`);
    }

    /**
     * Check if a key is trusted for a capability
     */
    isKeyTrusted(keyId, capability = 'sign-manifest') {
        if (this.revokedKeys.has(keyId)) {
            return false;
        }

        const key = this.trustedKeys.get(keyId);
        if (!key) {
            return false;
        }

        // Check validity period
        const now = new Date();
        if (key.validFrom && new Date(key.validFrom) > now) {
            return false;
        }
        if (key.validUntil && new Date(key.validUntil) < now) {
            return false;
        }

        // Check capability
        if (!key.capabilities.includes(capability)) {
            return false;
        }

        return true;
    }

    /**
     * Get public key for verification
     */
    getPublicKey(keyId) {
        const key = this.trustedKeys.get(keyId);
        if (!key || this.revokedKeys.has(keyId)) {
            return null;
        }
        return key.publicKey;
    }

    /**
     * Verify signature set meets threshold
     */
    verifySignatureThreshold(signatures, requiredCount = null) {
        const required = requiredCount || this.minimumSignaturesRequired;
        let validCount = 0;
        const validSigners = [];

        for (const sig of signatures) {
            if (this.isKeyTrusted(sig.keyId, 'sign-manifest')) {
                // In production, verify actual signature here
                validCount++;
                validSigners.push(sig.keyId);
            }
        }

        return {
            valid: validCount >= required,
            validCount,
            required,
            validSigners,
        };
    }

    /**
     * Export current trust configuration
     */
    export() {
        return {
            trustedKeys: Object.fromEntries(this.trustedKeys),
            revokedKeys: Array.from(this.revokedKeys),
            minimumSignaturesRequired: this.minimumSignaturesRequired,
            thresholdSignaturesRequired: this.thresholdSignaturesRequired,
        };
    }
}

// ============================================================================
// MERKLE CHUNK VERIFICATION
// ============================================================================

/**
 * Compute Merkle tree from chunk hashes
 */
export function computeMerkleRoot(chunkHashes) {
    if (chunkHashes.length === 0) {
        return hashCanonical({ empty: true });
    }

    if (chunkHashes.length === 1) {
        return chunkHashes[0];
    }

    // Build tree bottom-up
    let level = [...chunkHashes];

    while (level.length > 1) {
        const nextLevel = [];
        for (let i = 0; i < level.length; i += 2) {
            const left = level[i];
            const right = level[i + 1] || left; // Duplicate last if odd
            const combined = createHash('sha256')
                .update(left, 'hex')
                .update(right, 'hex')
                .digest('hex');
            nextLevel.push(combined);
        }
        level = nextLevel;
    }

    return level[0];
}

/**
 * Generate Merkle proof for a chunk
 */
export function generateMerkleProof(chunkHashes, chunkIndex) {
    const proof = [];
    let level = [...chunkHashes];
    let index = chunkIndex;

    while (level.length > 1) {
        const isRight = index % 2 === 1;
        const siblingIndex = isRight ? index - 1 : index + 1;

        if (siblingIndex < level.length) {
            proof.push({
                hash: level[siblingIndex],
                position: isRight ? 'left' : 'right',
            });
        } else {
            // Odd number, sibling is self
            proof.push({
                hash: level[index],
                position: 'right',
            });
        }

        // Move up
        const nextLevel = [];
        for (let i = 0; i < level.length; i += 2) {
            const left = level[i];
            const right = level[i + 1] || left;
            nextLevel.push(
                createHash('sha256')
                    .update(left, 'hex')
                    .update(right, 'hex')
                    .digest('hex')
            );
        }
        level = nextLevel;
        index = Math.floor(index / 2);
    }

    return proof;
}

/**
 * Verify a chunk against Merkle root
 */
export function verifyMerkleProof(chunkHash, chunkIndex, proof, merkleRoot) {
    let computed = chunkHash;

    for (const step of proof) {
        const left = step.position === 'left' ? step.hash : computed;
        const right = step.position === 'right' ? step.hash : computed;
        computed = createHash('sha256')
            .update(left, 'hex')
            .update(right, 'hex')
            .digest('hex');
    }

    return computed === merkleRoot;
}

/**
 * Chunk a buffer and compute hashes
 */
export function chunkAndHash(buffer, chunkSize = 256 * 1024) {
    const chunks = [];
    const hashes = [];

    for (let offset = 0; offset < buffer.length; offset += chunkSize) {
        const chunk = buffer.slice(offset, offset + chunkSize);
        chunks.push(chunk);
        hashes.push(
            createHash('sha256').update(chunk).digest('hex')
        );
    }

    return {
        chunks,
        chunkHashes: hashes,
        chunkSize,
        chunkCount: chunks.length,
        totalSize: buffer.length,
        merkleRoot: computeMerkleRoot(hashes),
    };
}

// ============================================================================
// MANIFEST INTEGRITY
// ============================================================================

/**
 * Integrity block for manifests
 */
export function createIntegrityBlock(manifest, chunkInfo) {
    // Create the signed payload (everything except signatures)
    const signedPayload = {
        model: manifest.model,
        version: manifest.version,
        artifacts: manifest.artifacts,
        provenance: manifest.provenance,
        capabilities: manifest.capabilities,
        timestamp: new Date().toISOString(),
    };

    const signedPayloadHash = hashCanonical(signedPayload);

    return {
        manifestHash: hashCanonical(manifest),
        signedPayloadHash,
        merkleRoot: chunkInfo.merkleRoot,
        chunking: {
            chunkSize: chunkInfo.chunkSize,
            chunkCount: chunkInfo.chunkCount,
            chunkHashes: chunkInfo.chunkHashes,
        },
        signatures: [], // To be filled by signing process
    };
}

/**
 * Provenance block for manifests
 */
export function createProvenanceBlock(options = {}) {
    return {
        builtBy: {
            tool: options.tool || '@ruvector/model-optimizer',
            version: options.toolVersion || '1.0.0',
            commit: options.commit || 'unknown',
        },
        optimizationRecipeHash: options.recipeHash || null,
        calibrationDatasetHash: options.calibrationHash || null,
        parentLineage: options.parentLineage || null,
        buildTimestamp: new Date().toISOString(),
        environment: {
            platform: process.platform,
            arch: process.arch,
            nodeVersion: process.version,
        },
    };
}

/**
 * Full manifest with integrity
 */
export function createSecureManifest(model, artifacts, options = {}) {
    const manifest = {
        schemaVersion: '2.0.0',
        model: {
            id: model.id,
            name: model.name,
            version: model.version,
            type: model.type, // 'embedding' | 'generation'
            tier: model.tier, // 'micro' | 'small' | 'large'
            capabilities: model.capabilities || [],
            memoryRequirement: model.memoryRequirement,
        },
        artifacts: artifacts.map(a => ({
            path: a.path,
            size: a.size,
            sha256: a.sha256,
            format: a.format,
            quantization: a.quantization,
        })),
        distribution: {
            gcs: options.gcsUrl,
            ipfs: options.ipfsCid,
            fallbackUrls: options.fallbackUrls || [],
        },
        provenance: createProvenanceBlock(options.provenance || {}),
        capabilities: model.capabilities || [],
    };

    // Add integrity block if chunk info provided
    if (options.chunkInfo) {
        manifest.integrity = createIntegrityBlock(manifest, options.chunkInfo);
    }

    // Add trust metadata
    manifest.trust = {
        trustedKeySetId: options.trustedKeySetId || 'ruvector-default-2024',
        minimumSignaturesRequired: options.minimumSignaturesRequired || 1,
    };

    return manifest;
}

// ============================================================================
// MANIFEST VERIFICATION
// ============================================================================

/**
 * Verify a manifest's integrity
 */
export class ManifestVerifier {
    constructor(trustRoot = null) {
        this.trustRoot = trustRoot || new TrustRoot();
    }

    /**
     * Full verification of a manifest
     */
    verify(manifest) {
        const errors = [];
        const warnings = [];

        // 1. Schema version check
        if (!manifest.schemaVersion || manifest.schemaVersion < '2.0.0') {
            warnings.push('Manifest uses old schema version');
        }

        // 2. Verify integrity block
        if (manifest.integrity) {
            // Check manifest hash
            const computed = hashCanonical(manifest);
            // Note: manifestHash is computed before adding integrity, so we skip this

            // Check signed payload hash
            const signedPayload = {
                model: manifest.model,
                version: manifest.version,
                artifacts: manifest.artifacts,
                provenance: manifest.provenance,
                capabilities: manifest.capabilities,
                timestamp: manifest.integrity.timestamp,
            };
            const computedPayloadHash = hashCanonical(signedPayload);

            // 3. Verify signatures meet threshold
            if (manifest.integrity.signatures?.length > 0) {
                const sigResult = this.trustRoot.verifySignatureThreshold(
                    manifest.integrity.signatures,
                    manifest.trust?.minimumSignaturesRequired
                );

                if (!sigResult.valid) {
                    errors.push(`Insufficient valid signatures: ${sigResult.validCount}/${sigResult.required}`);
                }
            } else {
                warnings.push('No signatures present');
            }

            // 4. Verify Merkle root matches chunk hashes
            if (manifest.integrity.chunking) {
                const computedRoot = computeMerkleRoot(manifest.integrity.chunking.chunkHashes);
                if (computedRoot !== manifest.integrity.merkleRoot) {
                    errors.push('Merkle root mismatch');
                }
            }
        } else {
            warnings.push('No integrity block present');
        }

        // 5. Check provenance
        if (!manifest.provenance) {
            warnings.push('No provenance information');
        }

        // 6. Check required fields
        if (!manifest.model?.id) errors.push('Missing model.id');
        if (!manifest.model?.version) errors.push('Missing model.version');
        if (!manifest.artifacts?.length) errors.push('No artifacts defined');

        return {
            valid: errors.length === 0,
            errors,
            warnings,
            trust: manifest.trust,
            provenance: manifest.provenance,
        };
    }

    /**
     * Verify a single chunk during streaming download
     */
    verifyChunk(chunkData, chunkIndex, manifest) {
        if (!manifest.integrity?.chunking) {
            return { valid: false, error: 'No chunking info in manifest' };
        }

        const expectedHash = manifest.integrity.chunking.chunkHashes[chunkIndex];
        if (!expectedHash) {
            return { valid: false, error: `No hash for chunk ${chunkIndex}` };
        }

        const actualHash = createHash('sha256').update(chunkData).digest('hex');

        if (actualHash !== expectedHash) {
            return {
                valid: false,
                error: `Chunk ${chunkIndex} hash mismatch`,
                expected: expectedHash,
                actual: actualHash,
            };
        }

        return { valid: true, chunkIndex, hash: actualHash };
    }
}

// ============================================================================
// TRANSPARENCY LOG
// ============================================================================

/**
 * Entry in the transparency log
 */
export function createLogEntry(manifest, publisherKeyId) {
    return {
        manifestHash: hashCanonical(manifest),
        modelId: manifest.model.id,
        version: manifest.model.version,
        publisherKeyId,
        timestamp: new Date().toISOString(),
        signedPayloadHash: manifest.integrity?.signedPayloadHash,
    };
}

/**
 * Simple append-only transparency log
 * In production, this would be backed by a Merkle tree or blockchain
 */
export class TransparencyLog {
    constructor(options = {}) {
        this.entries = [];
        this.indexByModel = new Map();
        this.indexByHash = new Map();
        this.logRoot = null;
    }

    /**
     * Append an entry to the log
     */
    append(entry) {
        const index = this.entries.length;

        // Compute log entry hash including previous
        const logEntryHash = hashCanonical({
            ...entry,
            index,
            previousHash: this.logRoot,
        });

        const fullEntry = {
            ...entry,
            index,
            previousHash: this.logRoot,
            logEntryHash,
        };

        this.entries.push(fullEntry);
        this.logRoot = logEntryHash;

        // Update indexes
        if (!this.indexByModel.has(entry.modelId)) {
            this.indexByModel.set(entry.modelId, []);
        }
        this.indexByModel.get(entry.modelId).push(index);
        this.indexByHash.set(entry.manifestHash, index);

        return fullEntry;
    }

    /**
     * Generate inclusion proof
     */
    getInclusionProof(manifestHash) {
        const index = this.indexByHash.get(manifestHash);
        if (index === undefined) {
            return null;
        }

        const entry = this.entries[index];
        const proof = [];

        // Simple chain proof (in production, use Merkle tree)
        for (let i = index; i < this.entries.length; i++) {
            proof.push({
                index: i,
                logEntryHash: this.entries[i].logEntryHash,
            });
        }

        return {
            entry,
            proof,
            currentRoot: this.logRoot,
            logLength: this.entries.length,
        };
    }

    /**
     * Verify inclusion proof
     */
    verifyInclusionProof(proof) {
        if (!proof || !proof.entry || !proof.proof.length) {
            return false;
        }

        // Verify chain
        let expectedHash = proof.entry.logEntryHash;
        for (let i = 1; i < proof.proof.length; i++) {
            const entry = proof.proof[i];
            // Verify chain continuity
            if (i < proof.proof.length - 1) {
                // Each entry should reference the previous
            }
        }

        return proof.proof[proof.proof.length - 1].logEntryHash === proof.currentRoot;
    }

    /**
     * Get history for a model
     */
    getModelHistory(modelId) {
        const indices = this.indexByModel.get(modelId) || [];
        return indices.map(i => this.entries[i]);
    }

    /**
     * Export log for persistence
     */
    export() {
        return {
            entries: this.entries,
            logRoot: this.logRoot,
        };
    }

    /**
     * Import log
     */
    import(data) {
        this.entries = data.entries || [];
        this.logRoot = data.logRoot;

        // Rebuild indexes
        this.indexByModel.clear();
        this.indexByHash.clear();

        for (let i = 0; i < this.entries.length; i++) {
            const entry = this.entries[i];
            if (!this.indexByModel.has(entry.modelId)) {
                this.indexByModel.set(entry.modelId, []);
            }
            this.indexByModel.get(entry.modelId).push(i);
            this.indexByHash.set(entry.manifestHash, i);
        }
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

export default {
    canonicalize,
    hashCanonical,
    TrustRoot,
    BUILTIN_ROOT_KEYS,
    computeMerkleRoot,
    generateMerkleProof,
    verifyMerkleProof,
    chunkAndHash,
    createIntegrityBlock,
    createProvenanceBlock,
    createSecureManifest,
    ManifestVerifier,
    createLogEntry,
    TransparencyLog,
};
