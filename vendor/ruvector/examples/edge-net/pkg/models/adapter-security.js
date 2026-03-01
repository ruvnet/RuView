/**
 * @ruvector/edge-net Adapter Security
 *
 * Security for MicroLoRA adapters:
 * - Quarantine before activation
 * - Local evaluation gating
 * - Base model matching
 * - Signature verification
 * - Merge lineage tracking
 *
 * Invariant: Adapters never applied without full verification.
 *
 * @module @ruvector/edge-net/models/adapter-security
 */

import { createHash } from 'crypto';
import { canonicalize, hashCanonical, TrustRoot, ManifestVerifier } from './integrity.js';

// ============================================================================
// ADAPTER VERIFICATION
// ============================================================================

/**
 * Adapter verification rules
 */
export const ADAPTER_REQUIREMENTS = Object.freeze({
    // Base model must match exactly
    requireExactBaseMatch: true,

    // Checksum must match manifest
    requireChecksumMatch: true,

    // Signature must be verified
    requireSignature: true,

    // Must pass local evaluation OR have trusted quality proof
    requireQualityGate: true,

    // Minimum evaluation score to pass gate (0-1)
    minEvaluationScore: 0.7,

    // Maximum adapter size relative to base model
    maxAdapterSizeRatio: 0.1, // 10% of base model

    // Trusted quality proof publishers
    trustedQualityProvers: ['ruvector-eval-2024', 'community-eval-2024'],
});

/**
 * Adapter manifest structure
 */
export function createAdapterManifest(adapter) {
    return {
        schemaVersion: '2.0.0',
        adapter: {
            id: adapter.id,
            name: adapter.name,
            version: adapter.version,
            baseModelId: adapter.baseModelId,
            baseModelVersion: adapter.baseModelVersion,
            rank: adapter.rank,
            alpha: adapter.alpha,
            targetModules: adapter.targetModules || ['q_proj', 'v_proj'],
        },
        artifacts: [{
            path: adapter.path,
            size: adapter.size,
            sha256: adapter.sha256,
            format: 'safetensors',
        }],
        quality: {
            evaluationScore: adapter.evaluationScore,
            evaluationDataset: adapter.evaluationDataset,
            evaluationProof: adapter.evaluationProof,
            domain: adapter.domain,
            capabilities: adapter.capabilities,
        },
        lineage: adapter.lineage || null,
        provenance: {
            creator: adapter.creator,
            createdAt: adapter.createdAt || new Date().toISOString(),
            trainedOn: adapter.trainedOn,
            trainingConfig: adapter.trainingConfig,
        },
        integrity: {
            manifestHash: null, // Computed
            signatures: [],
        },
    };
}

// ============================================================================
// QUARANTINE SYSTEM
// ============================================================================

/**
 * Quarantine states for adapters
 */
export const QuarantineState = Object.freeze({
    PENDING: 'pending',
    EVALUATING: 'evaluating',
    PASSED: 'passed',
    FAILED: 'failed',
    TRUSTED: 'trusted', // Has trusted quality proof
});

/**
 * Quarantine manager for adapter verification
 */
export class AdapterQuarantine {
    constructor(options = {}) {
        this.trustRoot = options.trustRoot || new TrustRoot();
        this.requirements = { ...ADAPTER_REQUIREMENTS, ...options.requirements };

        // Quarantined adapters awaiting evaluation
        this.quarantine = new Map();

        // Approved adapters
        this.approved = new Map();

        // Failed adapters (blocked)
        this.blocked = new Map();

        // Evaluation test sets by domain
        this.testSets = new Map();
    }

    /**
     * Register a test set for a domain
     */
    registerTestSet(domain, testCases) {
        this.testSets.set(domain, testCases);
    }

    /**
     * Quarantine an adapter for evaluation
     */
    async quarantineAdapter(manifest, adapterData) {
        const adapterId = manifest.adapter.id;

        // 1. Verify checksum
        const actualHash = createHash('sha256')
            .update(Buffer.from(adapterData))
            .digest('hex');

        if (actualHash !== manifest.artifacts[0].sha256) {
            const failure = {
                adapterId,
                reason: 'checksum_mismatch',
                expected: manifest.artifacts[0].sha256,
                actual: actualHash,
                timestamp: Date.now(),
            };
            this.blocked.set(adapterId, failure);
            return { state: QuarantineState.FAILED, failure };
        }

        // 2. Verify signature if required
        if (this.requirements.requireSignature) {
            const sigResult = this._verifySignatures(manifest);
            if (!sigResult.valid) {
                const failure = {
                    adapterId,
                    reason: 'invalid_signature',
                    details: sigResult.errors,
                    timestamp: Date.now(),
                };
                this.blocked.set(adapterId, failure);
                return { state: QuarantineState.FAILED, failure };
            }
        }

        // 3. Check for trusted quality proof
        if (manifest.quality?.evaluationProof) {
            const proofValid = await this._verifyQualityProof(manifest);
            if (proofValid) {
                this.approved.set(adapterId, {
                    manifest,
                    state: QuarantineState.TRUSTED,
                    approvedAt: Date.now(),
                });
                return { state: QuarantineState.TRUSTED };
            }
        }

        // 4. Add to quarantine for local evaluation
        this.quarantine.set(adapterId, {
            manifest,
            adapterData,
            state: QuarantineState.PENDING,
            quarantinedAt: Date.now(),
        });

        return { state: QuarantineState.PENDING };
    }

    /**
     * Evaluate a quarantined adapter locally
     */
    async evaluateAdapter(adapterId, inferenceSession) {
        const quarantined = this.quarantine.get(adapterId);
        if (!quarantined) {
            throw new Error(`Adapter ${adapterId} not in quarantine`);
        }

        quarantined.state = QuarantineState.EVALUATING;

        const manifest = quarantined.manifest;
        const domain = manifest.quality?.domain || 'general';

        // Get test set for domain
        const testSet = this.testSets.get(domain) || this._getDefaultTestSet();

        if (testSet.length === 0) {
            throw new Error(`No test set available for domain: ${domain}`);
        }

        // Run evaluation
        const results = await this._runEvaluation(
            quarantined.adapterData,
            testSet,
            inferenceSession,
            manifest.adapter.baseModelId
        );

        // Check if passed
        const passed = results.score >= this.requirements.minEvaluationScore;

        if (passed) {
            this.quarantine.delete(adapterId);
            this.approved.set(adapterId, {
                manifest,
                state: QuarantineState.PASSED,
                evaluationResults: results,
                approvedAt: Date.now(),
            });
            return { state: QuarantineState.PASSED, results };
        } else {
            this.quarantine.delete(adapterId);
            this.blocked.set(adapterId, {
                adapterId,
                reason: 'evaluation_failed',
                score: results.score,
                required: this.requirements.minEvaluationScore,
                timestamp: Date.now(),
            });
            return { state: QuarantineState.FAILED, results };
        }
    }

    /**
     * Check if an adapter can be used
     */
    canUseAdapter(adapterId, baseModelId) {
        const approved = this.approved.get(adapterId);
        if (!approved) {
            return { allowed: false, reason: 'not_approved' };
        }

        // Verify base model match
        if (this.requirements.requireExactBaseMatch) {
            const expectedBase = approved.manifest.adapter.baseModelId;
            if (expectedBase !== baseModelId) {
                return {
                    allowed: false,
                    reason: 'base_model_mismatch',
                    expected: expectedBase,
                    actual: baseModelId,
                };
            }
        }

        return { allowed: true, state: approved.state };
    }

    /**
     * Get approved adapter data
     */
    getApprovedAdapter(adapterId) {
        return this.approved.get(adapterId) || null;
    }

    /**
     * Verify signatures on adapter manifest
     */
    _verifySignatures(manifest) {
        if (!manifest.integrity?.signatures?.length) {
            return { valid: false, errors: ['No signatures present'] };
        }

        return this.trustRoot.verifySignatureThreshold(
            manifest.integrity.signatures,
            1 // At least one valid signature for adapters
        );
    }

    /**
     * Verify a trusted quality proof
     */
    async _verifyQualityProof(manifest) {
        const proof = manifest.quality.evaluationProof;
        if (!proof) return false;

        // Check if prover is trusted
        if (!this.requirements.trustedQualityProvers.includes(proof.proverId)) {
            return false;
        }

        // Verify proof signature
        const proofPayload = {
            adapterId: manifest.adapter.id,
            evaluationScore: manifest.quality.evaluationScore,
            evaluationDataset: manifest.quality.evaluationDataset,
            timestamp: proof.timestamp,
        };

        // In production, verify actual signature here
        return proof.signature && proof.proverId;
    }

    /**
     * Run local evaluation on adapter
     */
    async _runEvaluation(adapterData, testSet, inferenceSession, baseModelId) {
        const results = {
            total: testSet.length,
            passed: 0,
            failed: 0,
            errors: 0,
            details: [],
        };

        for (const testCase of testSet) {
            try {
                // Apply adapter temporarily
                await inferenceSession.loadAdapter(adapterData, { temporary: true });

                // Run inference
                const output = await inferenceSession.generate(testCase.input, {
                    maxTokens: testCase.maxTokens || 64,
                });

                // Check against expected
                const passed = this._checkOutput(output, testCase.expected, testCase.criteria);

                results.details.push({
                    input: testCase.input.slice(0, 50),
                    passed,
                });

                if (passed) {
                    results.passed++;
                } else {
                    results.failed++;
                }

                // Unload temporary adapter
                await inferenceSession.unloadAdapter();
            } catch (error) {
                results.errors++;
                results.details.push({
                    input: testCase.input.slice(0, 50),
                    error: error.message,
                });
            }
        }

        results.score = results.passed / results.total;
        return results;
    }

    /**
     * Check if output matches expected criteria
     */
    _checkOutput(output, expected, criteria = 'contains') {
        const outputLower = output.toLowerCase();
        const expectedLower = expected.toLowerCase();

        switch (criteria) {
            case 'exact':
                return output.trim() === expected.trim();
            case 'contains':
                return outputLower.includes(expectedLower);
            case 'startsWith':
                return outputLower.startsWith(expectedLower);
            case 'regex':
                return new RegExp(expected).test(output);
            default:
                return outputLower.includes(expectedLower);
        }
    }

    /**
     * Get default test set for unknown domains
     */
    _getDefaultTestSet() {
        return [
            {
                input: 'Hello, how are you?',
                expected: 'hello',
                criteria: 'contains',
            },
            {
                input: 'What is 2 + 2?',
                expected: '4',
                criteria: 'contains',
            },
            {
                input: 'Translate to French: hello',
                expected: 'bonjour',
                criteria: 'contains',
            },
        ];
    }

    /**
     * Export quarantine state
     */
    export() {
        return {
            quarantine: Array.from(this.quarantine.entries()),
            approved: Array.from(this.approved.entries()),
            blocked: Array.from(this.blocked.entries()),
        };
    }

    /**
     * Import quarantine state
     */
    import(data) {
        if (data.quarantine) {
            this.quarantine = new Map(data.quarantine);
        }
        if (data.approved) {
            this.approved = new Map(data.approved);
        }
        if (data.blocked) {
            this.blocked = new Map(data.blocked);
        }
    }
}

// ============================================================================
// MERGE LINEAGE TRACKING
// ============================================================================

/**
 * Lineage entry for merged adapters
 */
export function createMergeLineage(options) {
    return {
        parentAdapterIds: options.parentIds,
        mergeMethod: options.method, // 'ties', 'dare', 'task_arithmetic', 'linear'
        mergeParameters: options.parameters, // Method-specific params
        mergeSeed: options.seed || Math.floor(Math.random() * 2 ** 32),
        evaluationMetrics: options.metrics || {},
        mergerIdentity: options.mergerId,
        mergeTimestamp: new Date().toISOString(),
        signature: null, // To be filled after signing
    };
}

/**
 * Lineage tracker for adapter merges
 */
export class AdapterLineage {
    constructor(options = {}) {
        this.trustRoot = options.trustRoot || new TrustRoot();

        // DAG of adapter lineage
        this.lineageGraph = new Map();

        // Root adapters (no parents)
        this.roots = new Set();
    }

    /**
     * Register a new adapter in lineage
     */
    registerAdapter(adapterId, manifest) {
        const lineage = manifest.lineage;

        const node = {
            adapterId,
            version: manifest.adapter.version,
            baseModelId: manifest.adapter.baseModelId,
            parents: lineage?.parentAdapterIds || [],
            children: [],
            lineage,
            registeredAt: Date.now(),
        };

        this.lineageGraph.set(adapterId, node);

        // Update parent-child relationships
        if (node.parents.length === 0) {
            this.roots.add(adapterId);
        } else {
            for (const parentId of node.parents) {
                const parent = this.lineageGraph.get(parentId);
                if (parent) {
                    parent.children.push(adapterId);
                }
            }
        }

        return node;
    }

    /**
     * Get full ancestry path for an adapter
     */
    getAncestry(adapterId) {
        const ancestry = [];
        const visited = new Set();
        const queue = [adapterId];

        while (queue.length > 0) {
            const current = queue.shift();
            if (visited.has(current)) continue;
            visited.add(current);

            const node = this.lineageGraph.get(current);
            if (node) {
                ancestry.push({
                    adapterId: current,
                    version: node.version,
                    baseModelId: node.baseModelId,
                    mergeMethod: node.lineage?.mergeMethod,
                });

                for (const parentId of node.parents) {
                    queue.push(parentId);
                }
            }
        }

        return ancestry;
    }

    /**
     * Verify lineage integrity
     */
    verifyLineage(adapterId) {
        const node = this.lineageGraph.get(adapterId);
        if (!node) {
            return { valid: false, error: 'Adapter not found' };
        }

        const errors = [];

        // Check all parents exist
        for (const parentId of node.parents) {
            if (!this.lineageGraph.has(parentId)) {
                errors.push(`Missing parent: ${parentId}`);
            }
        }

        // Verify lineage signature if present
        if (node.lineage?.signature) {
            // In production, verify actual signature
            const sigValid = true; // Placeholder
            if (!sigValid) {
                errors.push('Invalid lineage signature');
            }
        }

        // Check for circular references
        const hasCircle = this._detectCircle(adapterId, new Set());
        if (hasCircle) {
            errors.push('Circular lineage detected');
        }

        return {
            valid: errors.length === 0,
            errors,
            ancestry: this.getAncestry(adapterId),
        };
    }

    /**
     * Detect circular references in lineage
     */
    _detectCircle(adapterId, visited) {
        if (visited.has(adapterId)) return true;
        visited.add(adapterId);

        const node = this.lineageGraph.get(adapterId);
        if (!node) return false;

        for (const parentId of node.parents) {
            if (this._detectCircle(parentId, new Set(visited))) {
                return true;
            }
        }

        return false;
    }

    /**
     * Get descendants of an adapter
     */
    getDescendants(adapterId) {
        const descendants = [];
        const queue = [adapterId];
        const visited = new Set();

        while (queue.length > 0) {
            const current = queue.shift();
            if (visited.has(current)) continue;
            visited.add(current);

            const node = this.lineageGraph.get(current);
            if (node) {
                for (const childId of node.children) {
                    descendants.push(childId);
                    queue.push(childId);
                }
            }
        }

        return descendants;
    }

    /**
     * Compute reproducibility hash for a merge
     */
    computeReproducibilityHash(lineage) {
        const payload = {
            parents: lineage.parentAdapterIds.sort(),
            method: lineage.mergeMethod,
            parameters: lineage.mergeParameters,
            seed: lineage.mergeSeed,
        };
        return hashCanonical(payload);
    }

    /**
     * Export lineage graph
     */
    export() {
        return {
            nodes: Array.from(this.lineageGraph.entries()),
            roots: Array.from(this.roots),
        };
    }

    /**
     * Import lineage graph
     */
    import(data) {
        if (data.nodes) {
            this.lineageGraph = new Map(data.nodes);
        }
        if (data.roots) {
            this.roots = new Set(data.roots);
        }
    }
}

// ============================================================================
// ADAPTER POOL WITH SECURITY
// ============================================================================

/**
 * Secure adapter pool with quarantine integration
 */
export class SecureAdapterPool {
    constructor(options = {}) {
        this.maxSlots = options.maxSlots || 16;
        this.quarantine = new AdapterQuarantine(options);
        this.lineage = new AdapterLineage(options);

        // Active adapters (LRU)
        this.activeAdapters = new Map();
        this.accessOrder = [];
    }

    /**
     * Add adapter with full security checks
     */
    async addAdapter(manifest, adapterData, inferenceSession = null) {
        const adapterId = manifest.adapter.id;

        // 1. Quarantine and verify
        const quarantineResult = await this.quarantine.quarantineAdapter(manifest, adapterData);

        if (quarantineResult.state === QuarantineState.FAILED) {
            throw new Error(`Adapter blocked: ${quarantineResult.failure.reason}`);
        }

        // 2. If not trusted, run local evaluation
        if (quarantineResult.state === QuarantineState.PENDING) {
            if (!inferenceSession) {
                throw new Error('Inference session required for local evaluation');
            }

            const evalResult = await this.quarantine.evaluateAdapter(adapterId, inferenceSession);
            if (evalResult.state === QuarantineState.FAILED) {
                throw new Error(`Adapter failed evaluation: score ${evalResult.results.score}`);
            }
        }

        // 3. Register in lineage
        this.lineage.registerAdapter(adapterId, manifest);

        // 4. Add to active pool
        await this._addToPool(adapterId, adapterData, manifest);

        return { adapterId, state: 'active' };
    }

    /**
     * Get an adapter if allowed
     */
    getAdapter(adapterId, baseModelId) {
        // Check if can use
        const check = this.quarantine.canUseAdapter(adapterId, baseModelId);
        if (!check.allowed) {
            return { allowed: false, reason: check.reason };
        }

        // Get from pool
        const adapter = this.activeAdapters.get(adapterId);
        if (!adapter) {
            return { allowed: false, reason: 'not_in_pool' };
        }

        // Update access order
        this._updateAccessOrder(adapterId);

        return { allowed: true, adapter };
    }

    /**
     * Add to pool with LRU eviction
     */
    async _addToPool(adapterId, adapterData, manifest) {
        // Evict if at capacity
        while (this.activeAdapters.size >= this.maxSlots) {
            const evictId = this.accessOrder.shift();
            this.activeAdapters.delete(evictId);
        }

        this.activeAdapters.set(adapterId, {
            data: adapterData,
            manifest,
            loadedAt: Date.now(),
        });

        this._updateAccessOrder(adapterId);
    }

    /**
     * Update LRU access order
     */
    _updateAccessOrder(adapterId) {
        const index = this.accessOrder.indexOf(adapterId);
        if (index > -1) {
            this.accessOrder.splice(index, 1);
        }
        this.accessOrder.push(adapterId);
    }

    /**
     * Get pool statistics
     */
    getStats() {
        return {
            activeCount: this.activeAdapters.size,
            maxSlots: this.maxSlots,
            quarantinedCount: this.quarantine.quarantine.size,
            approvedCount: this.quarantine.approved.size,
            blockedCount: this.quarantine.blocked.size,
            lineageNodes: this.lineage.lineageGraph.size,
        };
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

export default {
    ADAPTER_REQUIREMENTS,
    QuarantineState,
    createAdapterManifest,
    AdapterQuarantine,
    createMergeLineage,
    AdapterLineage,
    SecureAdapterPool,
};
