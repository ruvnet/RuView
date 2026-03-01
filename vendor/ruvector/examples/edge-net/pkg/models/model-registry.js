/**
 * @ruvector/edge-net Model Registry
 *
 * Manages model metadata, versions, dependencies, and discovery
 * for the distributed model distribution infrastructure.
 *
 * @module @ruvector/edge-net/models/model-registry
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';
import { promises as fs } from 'fs';
import path from 'path';

// ============================================
// SEMVER UTILITIES
// ============================================

/**
 * Parse a semver version string
 * @param {string} version - Version string (e.g., "1.2.3", "1.0.0-beta.1")
 * @returns {object} Parsed version object
 */
export function parseSemver(version) {
    const match = String(version).match(
        /^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.-]+))?(?:\+([a-zA-Z0-9.-]+))?$/
    );

    if (!match) {
        throw new Error(`Invalid semver: ${version}`);
    }

    return {
        major: parseInt(match[1], 10),
        minor: parseInt(match[2], 10),
        patch: parseInt(match[3], 10),
        prerelease: match[4] || null,
        build: match[5] || null,
        raw: version,
    };
}

/**
 * Compare two semver versions
 * @param {string} a - First version
 * @param {string} b - Second version
 * @returns {number} -1 if a < b, 0 if equal, 1 if a > b
 */
export function compareSemver(a, b) {
    const va = parseSemver(a);
    const vb = parseSemver(b);

    if (va.major !== vb.major) return va.major - vb.major;
    if (va.minor !== vb.minor) return va.minor - vb.minor;
    if (va.patch !== vb.patch) return va.patch - vb.patch;

    // Prerelease versions have lower precedence
    if (va.prerelease && !vb.prerelease) return -1;
    if (!va.prerelease && vb.prerelease) return 1;
    if (va.prerelease && vb.prerelease) {
        return va.prerelease.localeCompare(vb.prerelease);
    }

    return 0;
}

/**
 * Check if version satisfies a version range
 * Supports: "1.0.0", "^1.0.0", "~1.0.0", ">=1.0.0", "1.x", "*"
 * @param {string} version - Version to check
 * @param {string} range - Version range
 * @returns {boolean}
 */
export function satisfiesSemver(version, range) {
    const v = parseSemver(version);

    // Exact match
    if (range === version) return true;

    // Wildcard
    if (range === '*' || range === 'latest') return true;

    // X-range: 1.x, 1.2.x
    const xMatch = range.match(/^(\d+)(?:\.(\d+))?\.x$/);
    if (xMatch) {
        const major = parseInt(xMatch[1], 10);
        const minor = xMatch[2] ? parseInt(xMatch[2], 10) : null;
        if (v.major !== major) return false;
        if (minor !== null && v.minor !== minor) return false;
        return true;
    }

    // Caret range: ^1.0.0 (compatible with)
    if (range.startsWith('^')) {
        const r = parseSemver(range.slice(1));
        if (v.major !== r.major) return false;
        if (v.major === 0) {
            if (v.minor !== r.minor) return false;
            return v.patch >= r.patch;
        }
        return compareSemver(version, range.slice(1)) >= 0;
    }

    // Tilde range: ~1.0.0 (approximately equivalent)
    if (range.startsWith('~')) {
        const r = parseSemver(range.slice(1));
        if (v.major !== r.major) return false;
        if (v.minor !== r.minor) return false;
        return v.patch >= r.patch;
    }

    // Comparison ranges: >=1.0.0, >1.0.0, <=1.0.0, <1.0.0
    if (range.startsWith('>=')) {
        return compareSemver(version, range.slice(2)) >= 0;
    }
    if (range.startsWith('>')) {
        return compareSemver(version, range.slice(1)) > 0;
    }
    if (range.startsWith('<=')) {
        return compareSemver(version, range.slice(2)) <= 0;
    }
    if (range.startsWith('<')) {
        return compareSemver(version, range.slice(1)) < 0;
    }

    // Fallback to exact match
    return compareSemver(version, range) === 0;
}

/**
 * Get the latest version from a list
 * @param {string[]} versions - List of version strings
 * @returns {string} Latest version
 */
export function getLatestVersion(versions) {
    if (!versions || versions.length === 0) return null;
    return versions.sort((a, b) => compareSemver(b, a))[0];
}

// ============================================
// MODEL METADATA
// ============================================

/**
 * Model metadata structure
 * @typedef {object} ModelMetadata
 * @property {string} name - Model identifier (e.g., "phi-1.5-int4")
 * @property {string} version - Semantic version
 * @property {number} size - Model size in bytes
 * @property {string} hash - SHA256 hash for integrity
 * @property {string} format - Model format (onnx, safetensors, gguf)
 * @property {string[]} capabilities - Model capabilities
 * @property {object} sources - Download sources (gcs, ipfs, cdn)
 * @property {object} dependencies - Base models and adapters
 * @property {object} quantization - Quantization details
 * @property {object} metadata - Additional metadata
 */

/**
 * Create a model metadata object
 * @param {object} options - Model options
 * @returns {ModelMetadata}
 */
export function createModelMetadata(options) {
    const {
        name,
        version = '1.0.0',
        size = 0,
        hash = '',
        format = 'onnx',
        capabilities = [],
        sources = {},
        dependencies = {},
        quantization = null,
        metadata = {},
    } = options;

    if (!name) {
        throw new Error('Model name is required');
    }

    // Validate version
    parseSemver(version);

    return {
        name,
        version,
        size,
        hash,
        format,
        capabilities: Array.isArray(capabilities) ? capabilities : [capabilities],
        sources: {
            gcs: sources.gcs || null,
            ipfs: sources.ipfs || null,
            cdn: sources.cdn || null,
            ...sources,
        },
        dependencies: {
            base: dependencies.base || null,
            adapters: dependencies.adapters || [],
            ...dependencies,
        },
        quantization: quantization ? {
            type: quantization.type || 'int4',
            bits: quantization.bits || 4,
            blockSize: quantization.blockSize || 32,
            symmetric: quantization.symmetric ?? true,
        } : null,
        metadata: {
            createdAt: metadata.createdAt || new Date().toISOString(),
            updatedAt: metadata.updatedAt || new Date().toISOString(),
            author: metadata.author || 'RuVector',
            license: metadata.license || 'Apache-2.0',
            description: metadata.description || '',
            tags: metadata.tags || [],
            ...metadata,
        },
    };
}

// ============================================
// MODEL REGISTRY
// ============================================

/**
 * ModelRegistry - Manages model metadata, versions, and dependencies
 */
export class ModelRegistry extends EventEmitter {
    /**
     * Create a new ModelRegistry
     * @param {object} options - Registry options
     */
    constructor(options = {}) {
        super();

        this.id = `registry-${randomBytes(6).toString('hex')}`;
        this.registryPath = options.registryPath || null;

        // Model storage: { modelName: { version: ModelMetadata } }
        this.models = new Map();

        // Dependency graph
        this.dependencies = new Map();

        // Search index
        this.searchIndex = {
            byCapability: new Map(),
            byFormat: new Map(),
            byTag: new Map(),
        };

        // Stats
        this.stats = {
            totalModels: 0,
            totalVersions: 0,
            totalSize: 0,
        };
    }

    /**
     * Register a new model or version
     * @param {object} modelData - Model metadata
     * @returns {ModelMetadata}
     */
    register(modelData) {
        const metadata = createModelMetadata(modelData);
        const { name, version } = metadata;

        // Get or create model entry
        if (!this.models.has(name)) {
            this.models.set(name, new Map());
            this.stats.totalModels++;
        }

        const versions = this.models.get(name);

        // Check if version exists
        if (versions.has(version)) {
            this.emit('warning', {
                type: 'version_exists',
                model: name,
                version,
            });
        }

        // Store metadata
        versions.set(version, metadata);
        this.stats.totalVersions++;
        this.stats.totalSize += metadata.size;

        // Update search index
        this._indexModel(metadata);

        // Update dependency graph
        this._updateDependencies(metadata);

        this.emit('registered', { name, version, metadata });

        return metadata;
    }

    /**
     * Get model metadata
     * @param {string} name - Model name
     * @param {string} version - Version (default: latest)
     * @returns {ModelMetadata|null}
     */
    get(name, version = 'latest') {
        const versions = this.models.get(name);
        if (!versions) return null;

        if (version === 'latest') {
            const latest = getLatestVersion([...versions.keys()]);
            return latest ? versions.get(latest) : null;
        }

        // Check for exact match first
        if (versions.has(version)) {
            return versions.get(version);
        }

        // Try to find matching version in range
        for (const [v, metadata] of versions) {
            if (satisfiesSemver(v, version)) {
                return metadata;
            }
        }

        return null;
    }

    /**
     * List all versions of a model
     * @param {string} name - Model name
     * @returns {string[]}
     */
    listVersions(name) {
        const versions = this.models.get(name);
        if (!versions) return [];

        return [...versions.keys()].sort((a, b) => compareSemver(b, a));
    }

    /**
     * List all registered models
     * @returns {string[]}
     */
    listModels() {
        return [...this.models.keys()];
    }

    /**
     * Search for models
     * @param {object} criteria - Search criteria
     * @returns {ModelMetadata[]}
     */
    search(criteria = {}) {
        const {
            name = null,
            capability = null,
            format = null,
            tag = null,
            minVersion = null,
            maxVersion = null,
            maxSize = null,
            query = null,
        } = criteria;

        let results = [];

        // Start with all models or filtered by name
        if (name) {
            const versions = this.models.get(name);
            if (versions) {
                results = [...versions.values()];
            }
        } else {
            // Collect all model versions
            for (const versions of this.models.values()) {
                results.push(...versions.values());
            }
        }

        // Filter by capability
        if (capability) {
            results = results.filter(m =>
                m.capabilities.includes(capability)
            );
        }

        // Filter by format
        if (format) {
            results = results.filter(m => m.format === format);
        }

        // Filter by tag
        if (tag) {
            results = results.filter(m =>
                m.metadata.tags && m.metadata.tags.includes(tag)
            );
        }

        // Filter by version range
        if (minVersion) {
            results = results.filter(m =>
                compareSemver(m.version, minVersion) >= 0
            );
        }

        if (maxVersion) {
            results = results.filter(m =>
                compareSemver(m.version, maxVersion) <= 0
            );
        }

        // Filter by size
        if (maxSize) {
            results = results.filter(m => m.size <= maxSize);
        }

        // Text search
        if (query) {
            const q = query.toLowerCase();
            results = results.filter(m =>
                m.name.toLowerCase().includes(q) ||
                m.metadata.description?.toLowerCase().includes(q) ||
                m.metadata.tags?.some(t => t.toLowerCase().includes(q))
            );
        }

        return results;
    }

    /**
     * Get models by capability
     * @param {string} capability - Capability to search for
     * @returns {ModelMetadata[]}
     */
    getByCapability(capability) {
        const models = this.searchIndex.byCapability.get(capability);
        if (!models) return [];

        return models.map(key => {
            const [name, version] = key.split('@');
            return this.get(name, version);
        }).filter(Boolean);
    }

    /**
     * Get all dependencies for a model
     * @param {string} name - Model name
     * @param {string} version - Version
     * @param {boolean} recursive - Include transitive dependencies
     * @returns {ModelMetadata[]}
     */
    getDependencies(name, version = 'latest', recursive = true) {
        const model = this.get(name, version);
        if (!model) return [];

        const deps = [];
        const visited = new Set();
        const queue = [model];

        while (queue.length > 0) {
            const current = queue.shift();
            const key = `${current.name}@${current.version}`;

            if (visited.has(key)) continue;
            visited.add(key);

            // Add base model
            if (current.dependencies.base) {
                const [baseName, baseVersion] = current.dependencies.base.split('@');
                const baseDep = this.get(baseName, baseVersion || 'latest');
                if (baseDep) {
                    deps.push(baseDep);
                    if (recursive) queue.push(baseDep);
                }
            }

            // Add adapters
            if (current.dependencies.adapters) {
                for (const adapter of current.dependencies.adapters) {
                    const [adapterName, adapterVersion] = adapter.split('@');
                    const adapterDep = this.get(adapterName, adapterVersion || 'latest');
                    if (adapterDep) {
                        deps.push(adapterDep);
                        if (recursive) queue.push(adapterDep);
                    }
                }
            }
        }

        return deps;
    }

    /**
     * Get dependents (models that depend on this one)
     * @param {string} name - Model name
     * @param {string} version - Version
     * @returns {ModelMetadata[]}
     */
    getDependents(name, version = 'latest') {
        const key = version === 'latest'
            ? name
            : `${name}@${version}`;

        const dependents = [];

        for (const [depKey, dependencies] of this.dependencies) {
            if (dependencies.includes(key) || dependencies.includes(name)) {
                const [modelName, modelVersion] = depKey.split('@');
                const model = this.get(modelName, modelVersion);
                if (model) dependents.push(model);
            }
        }

        return dependents;
    }

    /**
     * Compute hash for a model file
     * @param {Buffer|Uint8Array} data - Model data
     * @returns {string} SHA256 hash
     */
    static computeHash(data) {
        return `sha256:${createHash('sha256').update(data).digest('hex')}`;
    }

    /**
     * Verify model integrity
     * @param {string} name - Model name
     * @param {string} version - Version
     * @param {Buffer|Uint8Array} data - Model data
     * @returns {boolean}
     */
    verify(name, version, data) {
        const model = this.get(name, version);
        if (!model) return false;

        const computedHash = ModelRegistry.computeHash(data);
        return model.hash === computedHash;
    }

    /**
     * Update search index for a model
     * @private
     */
    _indexModel(metadata) {
        const key = `${metadata.name}@${metadata.version}`;

        // Index by capability
        for (const cap of metadata.capabilities) {
            if (!this.searchIndex.byCapability.has(cap)) {
                this.searchIndex.byCapability.set(cap, []);
            }
            this.searchIndex.byCapability.get(cap).push(key);
        }

        // Index by format
        if (!this.searchIndex.byFormat.has(metadata.format)) {
            this.searchIndex.byFormat.set(metadata.format, []);
        }
        this.searchIndex.byFormat.get(metadata.format).push(key);

        // Index by tags
        if (metadata.metadata.tags) {
            for (const tag of metadata.metadata.tags) {
                if (!this.searchIndex.byTag.has(tag)) {
                    this.searchIndex.byTag.set(tag, []);
                }
                this.searchIndex.byTag.get(tag).push(key);
            }
        }
    }

    /**
     * Update dependency graph
     * @private
     */
    _updateDependencies(metadata) {
        const key = `${metadata.name}@${metadata.version}`;
        const deps = [];

        if (metadata.dependencies.base) {
            deps.push(metadata.dependencies.base);
        }

        if (metadata.dependencies.adapters) {
            deps.push(...metadata.dependencies.adapters);
        }

        if (deps.length > 0) {
            this.dependencies.set(key, deps);
        }
    }

    /**
     * Export registry to JSON
     * @returns {object}
     */
    export() {
        const models = {};

        for (const [name, versions] of this.models) {
            models[name] = {};
            for (const [version, metadata] of versions) {
                models[name][version] = metadata;
            }
        }

        return {
            version: '1.0.0',
            generatedAt: new Date().toISOString(),
            stats: this.stats,
            models,
        };
    }

    /**
     * Import registry from JSON
     * @param {object} data - Registry data
     */
    import(data) {
        if (!data.models) return;

        for (const [name, versions] of Object.entries(data.models)) {
            for (const [version, metadata] of Object.entries(versions)) {
                this.register({
                    ...metadata,
                    name,
                    version,
                });
            }
        }
    }

    /**
     * Save registry to file
     * @param {string} filePath - File path
     */
    async save(filePath = null) {
        const targetPath = filePath || this.registryPath;
        if (!targetPath) {
            throw new Error('No registry path specified');
        }

        const data = JSON.stringify(this.export(), null, 2);
        await fs.writeFile(targetPath, data, 'utf-8');

        this.emit('saved', { path: targetPath });
    }

    /**
     * Load registry from file
     * @param {string} filePath - File path
     */
    async load(filePath = null) {
        const targetPath = filePath || this.registryPath;
        if (!targetPath) {
            throw new Error('No registry path specified');
        }

        try {
            const data = await fs.readFile(targetPath, 'utf-8');
            this.import(JSON.parse(data));
            this.emit('loaded', { path: targetPath });
        } catch (error) {
            if (error.code === 'ENOENT') {
                this.emit('warning', { message: 'Registry file not found, starting fresh' });
            } else {
                throw error;
            }
        }
    }

    /**
     * Get registry statistics
     * @returns {object}
     */
    getStats() {
        return {
            ...this.stats,
            capabilities: this.searchIndex.byCapability.size,
            formats: this.searchIndex.byFormat.size,
            tags: this.searchIndex.byTag.size,
            dependencyEdges: this.dependencies.size,
        };
    }
}

// ============================================
// DEFAULT EXPORT
// ============================================

export default ModelRegistry;
