"use strict";
/**
 * Temporal Tracking Module for RUVector
 *
 * Provides comprehensive version control, change tracking, and time-travel capabilities
 * for ontology and database evolution over time.
 *
 * @module temporal
 * @author ruv.io Team
 * @license MIT
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.temporalTracker = exports.TemporalTracker = exports.ChangeType = void 0;
exports.isChange = isChange;
exports.isVersion = isVersion;
const events_1 = require("events");
const crypto_1 = require("crypto");
/**
 * Represents the type of change in a version
 */
var ChangeType;
(function (ChangeType) {
    ChangeType["ADDITION"] = "addition";
    ChangeType["DELETION"] = "deletion";
    ChangeType["MODIFICATION"] = "modification";
    ChangeType["METADATA"] = "metadata";
})(ChangeType || (exports.ChangeType = ChangeType = {}));
/**
 * TemporalTracker - Main class for temporal tracking functionality
 *
 * Provides version management, change tracking, time-travel queries,
 * and audit logging for database evolution over time.
 *
 * @example
 * ```typescript
 * const tracker = new TemporalTracker();
 *
 * // Create initial version
 * const v1 = await tracker.createVersion({
 *   description: 'Initial schema',
 *   tags: ['v1.0']
 * });
 *
 * // Track changes
 * tracker.trackChange({
 *   type: ChangeType.ADDITION,
 *   path: 'nodes.User',
 *   before: null,
 *   after: { name: 'User', properties: ['id', 'name'] },
 *   timestamp: Date.now()
 * });
 *
 * // Create new version with tracked changes
 * const v2 = await tracker.createVersion({
 *   description: 'Added User node',
 *   tags: ['v1.1']
 * });
 *
 * // Time-travel query
 * const snapshot = await tracker.queryAtTimestamp(v1.timestamp);
 *
 * // Compare versions
 * const diff = await tracker.compareVersions(v1.id, v2.id);
 * ```
 */
class TemporalTracker extends events_1.EventEmitter {
    constructor() {
        super();
        this.versions = new Map();
        this.currentState = {};
        this.pendingChanges = [];
        this.auditLog = [];
        this.tagIndex = new Map(); // tag -> versionIds
        this.pathIndex = new Map(); // path -> changes
        this.initializeBaseline();
    }
    /**
     * Initialize with a baseline empty version
     */
    initializeBaseline() {
        const baseline = {
            id: this.generateId(),
            parentId: null,
            timestamp: 0, // Baseline is always at timestamp 0
            description: 'Baseline version',
            changes: [],
            tags: ['baseline'],
            checksum: this.calculateChecksum({}),
            metadata: {}
        };
        this.versions.set(baseline.id, baseline);
        this.indexVersion(baseline);
    }
    /**
     * Generate a unique ID
     */
    generateId() {
        return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
    /**
     * Calculate checksum for data integrity
     */
    calculateChecksum(data) {
        const hash = (0, crypto_1.createHash)('sha256');
        hash.update(JSON.stringify(data));
        return hash.digest('hex');
    }
    /**
     * Index a version for fast lookups
     */
    indexVersion(version) {
        // Index tags
        version.tags.forEach(tag => {
            if (!this.tagIndex.has(tag)) {
                this.tagIndex.set(tag, new Set());
            }
            this.tagIndex.get(tag).add(version.id);
        });
        // Index changes by path
        version.changes.forEach(change => {
            if (!this.pathIndex.has(change.path)) {
                this.pathIndex.set(change.path, []);
            }
            this.pathIndex.get(change.path).push(change);
        });
    }
    /**
     * Track a change to be included in the next version
     *
     * @param change - The change to track
     * @emits changeTracked
     */
    trackChange(change) {
        this.pendingChanges.push(change);
        this.emit('changeTracked', change);
    }
    /**
     * Create a new version with all pending changes
     *
     * @param options - Version creation options
     * @returns The created version
     * @emits versionCreated
     */
    async createVersion(options) {
        const startTime = Date.now();
        try {
            // Get current version (latest)
            const currentVersion = this.getCurrentVersion();
            // Reconstruct current state from all versions
            if (currentVersion) {
                this.currentState = await this.reconstructStateAt(currentVersion.id);
            }
            // Apply pending changes to current state
            this.pendingChanges.forEach(change => {
                this.applyChange(this.currentState, change);
            });
            // Create new version
            const version = {
                id: this.generateId(),
                parentId: currentVersion?.id || null,
                timestamp: Date.now(),
                description: options.description,
                changes: [...this.pendingChanges],
                tags: options.tags || [],
                author: options.author,
                checksum: this.calculateChecksum(this.currentState),
                metadata: options.metadata || {}
            };
            // Store version
            this.versions.set(version.id, version);
            this.indexVersion(version);
            // Clear pending changes
            this.pendingChanges = [];
            // Log audit entry
            this.logAudit({
                operation: 'create',
                versionId: version.id,
                status: 'success',
                details: {
                    description: options.description,
                    changeCount: version.changes.length,
                    duration: Date.now() - startTime
                }
            });
            this.emit('versionCreated', version);
            return version;
        }
        catch (error) {
            this.logAudit({
                operation: 'create',
                status: 'failure',
                error: error instanceof Error ? error.message : String(error),
                details: { options }
            });
            throw error;
        }
    }
    /**
     * Apply a change to the state object
     */
    applyChange(state, change) {
        const pathParts = change.path.split('.');
        let current = state;
        // Navigate to parent
        for (let i = 0; i < pathParts.length - 1; i++) {
            if (!(pathParts[i] in current)) {
                current[pathParts[i]] = {};
            }
            current = current[pathParts[i]];
        }
        const key = pathParts[pathParts.length - 1];
        // Apply change
        switch (change.type) {
            case ChangeType.ADDITION:
            case ChangeType.MODIFICATION:
                // Deep clone to avoid reference issues
                current[key] = JSON.parse(JSON.stringify(change.after));
                break;
            case ChangeType.DELETION:
                delete current[key];
                break;
            case ChangeType.METADATA:
                if (!current[key])
                    current[key] = {};
                Object.assign(current[key], JSON.parse(JSON.stringify(change.after)));
                break;
        }
    }
    /**
     * Get the current (latest) version
     */
    getCurrentVersion() {
        if (this.versions.size === 0)
            return null;
        const versions = Array.from(this.versions.values());
        return versions.reduce((latest, current) => current.timestamp > latest.timestamp ? current : latest);
    }
    /**
     * List all versions, optionally filtered by tags
     *
     * @param tags - Optional tags to filter by
     * @returns Array of versions
     */
    listVersions(tags) {
        let versionIds = null;
        // Filter by tags if provided
        if (tags && tags.length > 0) {
            versionIds = new Set();
            tags.forEach(tag => {
                const taggedVersions = this.tagIndex.get(tag);
                if (taggedVersions) {
                    taggedVersions.forEach(id => versionIds.add(id));
                }
            });
        }
        const versions = Array.from(this.versions.values());
        const filtered = versionIds
            ? versions.filter(v => versionIds.has(v.id))
            : versions;
        return filtered.sort((a, b) => b.timestamp - a.timestamp);
    }
    /**
     * Get a specific version by ID
     *
     * @param versionId - Version ID
     * @returns The version or null if not found
     */
    getVersion(versionId) {
        return this.versions.get(versionId) || null;
    }
    /**
     * Compare two versions and generate a diff
     *
     * @param fromVersionId - Source version ID
     * @param toVersionId - Target version ID
     * @returns Version diff
     */
    async compareVersions(fromVersionId, toVersionId) {
        const startTime = Date.now();
        try {
            const fromVersion = this.versions.get(fromVersionId);
            const toVersion = this.versions.get(toVersionId);
            if (!fromVersion || !toVersion) {
                throw new Error('Version not found');
            }
            // Reconstruct state at both versions
            const fromState = await this.reconstructStateAt(fromVersionId);
            const toState = await this.reconstructStateAt(toVersionId);
            // Generate diff
            const changes = this.generateDiff(fromState, toState, '');
            // Calculate summary
            const summary = {
                additions: changes.filter(c => c.type === ChangeType.ADDITION).length,
                deletions: changes.filter(c => c.type === ChangeType.DELETION).length,
                modifications: changes.filter(c => c.type === ChangeType.MODIFICATION).length
            };
            const diff = {
                fromVersion: fromVersionId,
                toVersion: toVersionId,
                changes,
                summary,
                generatedAt: Date.now()
            };
            this.logAudit({
                operation: 'compare',
                status: 'success',
                details: {
                    fromVersion: fromVersionId,
                    toVersion: toVersionId,
                    changeCount: changes.length,
                    duration: Date.now() - startTime
                }
            });
            return diff;
        }
        catch (error) {
            this.logAudit({
                operation: 'compare',
                status: 'failure',
                error: error instanceof Error ? error.message : String(error),
                details: { fromVersionId, toVersionId }
            });
            throw error;
        }
    }
    /**
     * Generate diff between two states
     */
    generateDiff(from, to, path) {
        const changes = [];
        const timestamp = Date.now();
        // Check all keys in 'to' state
        for (const key in to) {
            const currentPath = path ? `${path}.${key}` : key;
            const fromValue = from?.[key];
            const toValue = to[key];
            if (!(key in (from || {}))) {
                // Addition
                changes.push({
                    type: ChangeType.ADDITION,
                    path: currentPath,
                    before: null,
                    after: toValue,
                    timestamp
                });
            }
            else if (typeof toValue === 'object' && toValue !== null && !Array.isArray(toValue)) {
                // Recurse into object
                changes.push(...this.generateDiff(fromValue, toValue, currentPath));
            }
            else if (JSON.stringify(fromValue) !== JSON.stringify(toValue)) {
                // Modification
                changes.push({
                    type: ChangeType.MODIFICATION,
                    path: currentPath,
                    before: fromValue,
                    after: toValue,
                    timestamp
                });
            }
        }
        // Check for deletions
        for (const key in from) {
            if (!(key in to)) {
                const currentPath = path ? `${path}.${key}` : key;
                changes.push({
                    type: ChangeType.DELETION,
                    path: currentPath,
                    before: from[key],
                    after: null,
                    timestamp
                });
            }
        }
        return changes;
    }
    /**
     * Revert to a specific version
     *
     * @param versionId - Target version ID
     * @returns The new current version (revert creates a new version)
     * @emits versionReverted
     */
    async revertToVersion(versionId) {
        const startTime = Date.now();
        const currentVersion = this.getCurrentVersion();
        try {
            const targetVersion = this.versions.get(versionId);
            if (!targetVersion) {
                throw new Error('Target version not found');
            }
            // Reconstruct state at target version
            const targetState = await this.reconstructStateAt(versionId);
            // Generate changes from current to target
            const revertChanges = this.generateDiff(this.currentState, targetState, '');
            // Create new version with revert changes
            this.pendingChanges = revertChanges;
            const revertVersion = await this.createVersion({
                description: `Revert to version: ${targetVersion.description}`,
                tags: ['revert'],
                metadata: {
                    revertedFrom: currentVersion?.id,
                    revertedTo: versionId
                }
            });
            this.logAudit({
                operation: 'revert',
                versionId: revertVersion.id,
                status: 'success',
                details: {
                    targetVersion: versionId,
                    changeCount: revertChanges.length,
                    duration: Date.now() - startTime
                }
            });
            this.emit('versionReverted', currentVersion?.id || '', versionId);
            return revertVersion;
        }
        catch (error) {
            this.logAudit({
                operation: 'revert',
                status: 'failure',
                error: error instanceof Error ? error.message : String(error),
                details: { versionId }
            });
            throw error;
        }
    }
    /**
     * Reconstruct the database state at a specific version
     *
     * @param versionId - Target version ID
     * @returns Reconstructed state
     */
    async reconstructStateAt(versionId) {
        const version = this.versions.get(versionId);
        if (!version) {
            throw new Error('Version not found');
        }
        // Build version chain from baseline to target
        const chain = [];
        let current = version;
        while (current) {
            chain.unshift(current);
            current = current.parentId ? this.versions.get(current.parentId) || null : null;
        }
        // Apply changes in sequence to a fresh state
        const state = {};
        for (const v of chain) {
            v.changes.forEach(change => {
                this.applyChange(state, change);
            });
        }
        // Deep clone to avoid reference issues
        return JSON.parse(JSON.stringify(state));
    }
    async queryAtTimestamp(timestampOrOptions) {
        const startTime = Date.now();
        try {
            const options = typeof timestampOrOptions === 'number'
                ? { timestamp: timestampOrOptions }
                : timestampOrOptions;
            let targetVersion = null;
            if (options.versionId) {
                targetVersion = this.versions.get(options.versionId) || null;
            }
            else if (options.timestamp) {
                // Find version closest to timestamp
                const versions = Array.from(this.versions.values())
                    .filter(v => v.timestamp <= options.timestamp)
                    .sort((a, b) => b.timestamp - a.timestamp);
                targetVersion = versions[0] || null;
            }
            if (!targetVersion) {
                throw new Error('No version found matching criteria');
            }
            let state = await this.reconstructStateAt(targetVersion.id);
            // Apply path filter if provided
            if (options.pathPattern) {
                state = this.filterByPath(state, options.pathPattern, '');
            }
            // Strip metadata if not requested
            if (!options.includeMetadata) {
                state = this.stripMetadata(state);
            }
            this.logAudit({
                operation: 'query',
                versionId: targetVersion.id,
                status: 'success',
                details: {
                    options,
                    duration: Date.now() - startTime
                }
            });
            return state;
        }
        catch (error) {
            this.logAudit({
                operation: 'query',
                status: 'failure',
                error: error instanceof Error ? error.message : String(error),
                details: { options: timestampOrOptions }
            });
            throw error;
        }
    }
    /**
     * Filter state by path pattern
     */
    filterByPath(state, pattern, currentPath) {
        const filtered = {};
        for (const key in state) {
            const path = currentPath ? `${currentPath}.${key}` : key;
            if (pattern.test(path)) {
                filtered[key] = state[key];
            }
            else if (typeof state[key] === 'object' && state[key] !== null) {
                const nested = this.filterByPath(state[key], pattern, path);
                if (Object.keys(nested).length > 0) {
                    filtered[key] = nested;
                }
            }
        }
        return filtered;
    }
    /**
     * Strip metadata from state
     */
    stripMetadata(state) {
        const cleaned = Array.isArray(state) ? [] : {};
        for (const key in state) {
            if (key === 'metadata')
                continue;
            if (typeof state[key] === 'object' && state[key] !== null) {
                cleaned[key] = this.stripMetadata(state[key]);
            }
            else {
                cleaned[key] = state[key];
            }
        }
        return cleaned;
    }
    /**
     * Add tags to a version
     *
     * @param versionId - Version ID
     * @param tags - Tags to add
     */
    addTags(versionId, tags) {
        const version = this.versions.get(versionId);
        if (!version) {
            throw new Error('Version not found');
        }
        tags.forEach(tag => {
            if (!version.tags.includes(tag)) {
                version.tags.push(tag);
                if (!this.tagIndex.has(tag)) {
                    this.tagIndex.set(tag, new Set());
                }
                this.tagIndex.get(tag).add(versionId);
            }
        });
        this.logAudit({
            operation: 'tag',
            versionId,
            status: 'success',
            details: { tags }
        });
    }
    /**
     * Get visualization data for change history
     *
     * @returns Visualization data
     */
    getVisualizationData() {
        const versions = Array.from(this.versions.values());
        // Timeline
        const timeline = versions
            .sort((a, b) => a.timestamp - b.timestamp)
            .map(v => ({
            versionId: v.id,
            timestamp: v.timestamp,
            description: v.description,
            changeCount: v.changes.length,
            tags: v.tags
        }));
        // Change frequency
        const frequencyMap = new Map();
        versions.forEach(v => {
            const hourBucket = Math.floor(v.timestamp / (1000 * 60 * 60)) * (1000 * 60 * 60);
            if (!frequencyMap.has(hourBucket)) {
                frequencyMap.set(hourBucket, new Map());
            }
            const bucket = frequencyMap.get(hourBucket);
            v.changes.forEach(change => {
                bucket.set(change.type, (bucket.get(change.type) || 0) + 1);
            });
        });
        const changeFrequency = [];
        frequencyMap.forEach((typeCounts, timestamp) => {
            typeCounts.forEach((count, type) => {
                changeFrequency.push({ timestamp, count, type });
            });
        });
        // Hotspots
        const pathStats = new Map();
        this.pathIndex.forEach((changes, path) => {
            const lastChange = changes[changes.length - 1];
            pathStats.set(path, {
                count: changes.length,
                lastChanged: lastChange.timestamp
            });
        });
        const hotspots = Array.from(pathStats.entries())
            .map(([path, stats]) => ({
            path,
            changeCount: stats.count,
            lastChanged: stats.lastChanged
        }))
            .sort((a, b) => b.changeCount - a.changeCount)
            .slice(0, 20);
        // Version graph
        const versionGraph = {
            nodes: versions.map(v => ({
                id: v.id,
                label: v.description,
                timestamp: v.timestamp
            })),
            edges: versions
                .filter(v => v.parentId)
                .map(v => ({
                from: v.parentId,
                to: v.id
            }))
        };
        return {
            timeline,
            changeFrequency,
            hotspots,
            versionGraph
        };
    }
    /**
     * Get audit log entries
     *
     * @param limit - Maximum number of entries to return
     * @returns Audit log entries
     */
    getAuditLog(limit) {
        const sorted = [...this.auditLog].sort((a, b) => b.timestamp - a.timestamp);
        return limit ? sorted.slice(0, limit) : sorted;
    }
    /**
     * Log an audit entry
     */
    logAudit(entry) {
        const auditEntry = {
            id: this.generateId(),
            timestamp: Date.now(),
            ...entry
        };
        this.auditLog.push(auditEntry);
        this.emit('auditLogged', auditEntry);
    }
    /**
     * Prune old versions to save space
     *
     * @param keepCount - Number of recent versions to keep
     * @param preserveTags - Tags to preserve regardless of age
     */
    pruneVersions(keepCount, preserveTags = ['baseline']) {
        const versions = Array.from(this.versions.values())
            .sort((a, b) => b.timestamp - a.timestamp);
        const toDelete = [];
        versions.forEach((version, index) => {
            // Keep recent versions
            if (index < keepCount)
                return;
            // Keep tagged versions
            if (version.tags.some(tag => preserveTags.includes(tag)))
                return;
            // Keep if any child version exists
            const hasChildren = versions.some(v => v.parentId === version.id);
            if (hasChildren)
                return;
            toDelete.push(version.id);
        });
        // Delete versions
        toDelete.forEach(id => {
            const version = this.versions.get(id);
            if (version) {
                // Remove from indices
                version.tags.forEach(tag => {
                    this.tagIndex.get(tag)?.delete(id);
                });
                this.versions.delete(id);
            }
        });
        this.logAudit({
            operation: 'prune',
            status: 'success',
            details: {
                deletedCount: toDelete.length,
                keepCount,
                preserveTags
            }
        });
    }
    /**
     * Export all versions and audit log for backup
     *
     * @returns Serializable backup data
     */
    exportBackup() {
        return {
            versions: Array.from(this.versions.values()),
            auditLog: this.auditLog,
            currentState: this.currentState,
            exportedAt: Date.now()
        };
    }
    /**
     * Import versions and state from backup
     *
     * @param backup - Backup data to import
     */
    importBackup(backup) {
        // Clear existing data
        this.versions.clear();
        this.tagIndex.clear();
        this.pathIndex.clear();
        this.auditLog = [];
        this.pendingChanges = [];
        // Import versions
        backup.versions.forEach(version => {
            this.versions.set(version.id, version);
            this.indexVersion(version);
        });
        // Import audit log
        this.auditLog = [...backup.auditLog];
        // Import current state
        this.currentState = backup.currentState;
        this.logAudit({
            operation: 'create',
            status: 'success',
            details: {
                importedVersions: backup.versions.length,
                importedAuditEntries: backup.auditLog.length,
                importedFrom: backup.exportedAt
            }
        });
    }
    /**
     * Get storage statistics
     *
     * @returns Storage statistics
     */
    getStorageStats() {
        const versions = Array.from(this.versions.values());
        const totalChanges = versions.reduce((sum, v) => sum + v.changes.length, 0);
        const backup = this.exportBackup();
        const estimatedSizeBytes = JSON.stringify(backup).length;
        return {
            versionCount: versions.length,
            totalChanges,
            auditLogSize: this.auditLog.length,
            estimatedSizeBytes,
            oldestVersion: Math.min(...versions.map(v => v.timestamp)),
            newestVersion: Math.max(...versions.map(v => v.timestamp))
        };
    }
}
exports.TemporalTracker = TemporalTracker;
/**
 * Export singleton instance for convenience
 */
exports.temporalTracker = new TemporalTracker();
/**
 * Type guard for Change
 */
function isChange(obj) {
    return obj &&
        typeof obj.type === 'string' &&
        typeof obj.path === 'string' &&
        typeof obj.timestamp === 'number';
}
/**
 * Type guard for Version
 */
function isVersion(obj) {
    return obj &&
        typeof obj.id === 'string' &&
        typeof obj.timestamp === 'number' &&
        Array.isArray(obj.changes) &&
        Array.isArray(obj.tags);
}
//# sourceMappingURL=temporal.js.map