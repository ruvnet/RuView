"use strict";
/**
 * AgentDB Fast - High-performance in-process alternative to AgentDB CLI
 *
 * The AgentDB CLI has ~2.3s startup overhead due to npx initialization.
 * This module provides 50-200x faster operations by using in-process calls.
 *
 * Features:
 * - In-memory episode storage with LRU eviction
 * - Vector similarity search using @ruvector/core
 * - Compatible API with AgentDB's episode/trajectory interfaces
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.FastAgentDB = void 0;
exports.createFastAgentDB = createFastAgentDB;
exports.getDefaultAgentDB = getDefaultAgentDB;
// Lazy load ruvector core
let coreModule = null;
function getCoreModule() {
    if (coreModule)
        return coreModule;
    try {
        coreModule = require('@ruvector/core');
        return coreModule;
    }
    catch {
        // Fallback to ruvector if core not available
        try {
            coreModule = require('ruvector');
            return coreModule;
        }
        catch (e) {
            throw new Error(`Neither @ruvector/core nor ruvector is available: ${e.message}`);
        }
    }
}
/**
 * Fast in-memory AgentDB implementation
 */
class FastAgentDB {
    /**
     * Create a new FastAgentDB instance
     *
     * @param dimensions - Vector dimensions for state embeddings
     * @param maxEpisodes - Maximum episodes to store (LRU eviction)
     */
    constructor(dimensions = 128, maxEpisodes = 100000) {
        this.episodes = new Map();
        this.trajectories = new Map();
        this.vectorDb = null;
        this.episodeOrder = []; // For LRU eviction
        this.dimensions = dimensions;
        this.maxEpisodes = maxEpisodes;
    }
    /**
     * Initialize the vector database
     */
    async initVectorDb() {
        if (this.vectorDb)
            return;
        try {
            const core = getCoreModule();
            this.vectorDb = new core.VectorDB({
                dimensions: this.dimensions,
                distanceMetric: 'Cosine',
            });
        }
        catch (e) {
            // Vector DB not available, use fallback similarity
            console.warn(`VectorDB not available, using fallback similarity: ${e.message}`);
        }
    }
    /**
     * Store an episode
     *
     * @param episode - Episode to store
     * @returns Episode ID
     */
    async storeEpisode(episode) {
        await this.initVectorDb();
        const id = episode.id ?? this.generateId();
        const fullEpisode = {
            ...episode,
            id,
            timestamp: episode.timestamp ?? Date.now(),
        };
        // LRU eviction if needed
        if (this.episodes.size >= this.maxEpisodes) {
            const oldestId = this.episodeOrder.shift();
            if (oldestId) {
                this.episodes.delete(oldestId);
            }
        }
        this.episodes.set(id, fullEpisode);
        this.episodeOrder.push(id);
        // Index in vector DB if available
        if (this.vectorDb && fullEpisode.state.length === this.dimensions) {
            try {
                await this.vectorDb.insert({
                    id,
                    vector: new Float32Array(fullEpisode.state),
                });
            }
            catch {
                // Ignore indexing errors
            }
        }
        return id;
    }
    /**
     * Store multiple episodes in batch
     */
    async storeEpisodes(episodes) {
        const ids = [];
        for (const episode of episodes) {
            const id = await this.storeEpisode(episode);
            ids.push(id);
        }
        return ids;
    }
    /**
     * Retrieve an episode by ID
     */
    async getEpisode(id) {
        const episode = this.episodes.get(id);
        if (episode) {
            // Update LRU order
            const idx = this.episodeOrder.indexOf(id);
            if (idx > -1) {
                this.episodeOrder.splice(idx, 1);
                this.episodeOrder.push(id);
            }
        }
        return episode ?? null;
    }
    /**
     * Search for similar episodes by state
     *
     * @param queryState - State vector to search for
     * @param k - Number of results to return
     * @returns Similar episodes sorted by similarity
     */
    async searchByState(queryState, k = 10) {
        await this.initVectorDb();
        const query = Array.isArray(queryState) ? queryState : Array.from(queryState);
        // Use vector DB if available
        if (this.vectorDb && query.length === this.dimensions) {
            try {
                const results = await this.vectorDb.search({
                    vector: new Float32Array(query),
                    k,
                });
                return results
                    .map((r) => {
                    const episode = this.episodes.get(r.id);
                    if (!episode)
                        return null;
                    return {
                        episode,
                        similarity: 1 - r.score, // Convert distance to similarity
                    };
                })
                    .filter((r) => r !== null);
            }
            catch {
                // Fall through to fallback
            }
        }
        // Fallback: brute-force cosine similarity
        return this.fallbackSearch(query, k);
    }
    /**
     * Fallback similarity search using brute-force cosine similarity
     */
    fallbackSearch(query, k) {
        const results = [];
        for (const episode of this.episodes.values()) {
            if (episode.state.length !== query.length)
                continue;
            const similarity = this.cosineSimilarity(query, episode.state);
            results.push({ episode, similarity });
        }
        return results
            .sort((a, b) => b.similarity - a.similarity)
            .slice(0, k);
    }
    /**
     * Compute cosine similarity between two vectors
     */
    cosineSimilarity(a, b) {
        let dotProduct = 0;
        let normA = 0;
        let normB = 0;
        for (let i = 0; i < a.length; i++) {
            dotProduct += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA) * Math.sqrt(normB);
        return denom === 0 ? 0 : dotProduct / denom;
    }
    /**
     * Store a trajectory (sequence of episodes)
     */
    async storeTrajectory(episodes, metadata) {
        const trajectoryId = this.generateId();
        const storedEpisodes = [];
        let totalReward = 0;
        for (const episode of episodes) {
            const id = await this.storeEpisode(episode);
            const stored = await this.getEpisode(id);
            if (stored) {
                storedEpisodes.push(stored);
                totalReward += stored.reward;
            }
        }
        const trajectory = {
            id: trajectoryId,
            episodes: storedEpisodes,
            totalReward,
            metadata,
        };
        this.trajectories.set(trajectoryId, trajectory);
        return trajectoryId;
    }
    /**
     * Get a trajectory by ID
     */
    async getTrajectory(id) {
        return this.trajectories.get(id) ?? null;
    }
    /**
     * Get top trajectories by total reward
     */
    async getTopTrajectories(k = 10) {
        return Array.from(this.trajectories.values())
            .sort((a, b) => b.totalReward - a.totalReward)
            .slice(0, k);
    }
    /**
     * Sample random episodes (for experience replay)
     */
    async sampleEpisodes(n) {
        const allEpisodes = Array.from(this.episodes.values());
        const sampled = [];
        for (let i = 0; i < Math.min(n, allEpisodes.length); i++) {
            const idx = Math.floor(Math.random() * allEpisodes.length);
            sampled.push(allEpisodes[idx]);
        }
        return sampled;
    }
    /**
     * Get database statistics
     */
    getStats() {
        return {
            episodeCount: this.episodes.size,
            trajectoryCount: this.trajectories.size,
            dimensions: this.dimensions,
            maxEpisodes: this.maxEpisodes,
            vectorDbAvailable: this.vectorDb !== null,
        };
    }
    /**
     * Clear all data
     */
    clear() {
        this.episodes.clear();
        this.trajectories.clear();
        this.episodeOrder = [];
    }
    /**
     * Generate a unique ID
     */
    generateId() {
        return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
}
exports.FastAgentDB = FastAgentDB;
/**
 * Create a fast AgentDB instance
 */
function createFastAgentDB(dimensions = 128, maxEpisodes = 100000) {
    return new FastAgentDB(dimensions, maxEpisodes);
}
// Singleton instance for convenience
let defaultInstance = null;
/**
 * Get the default FastAgentDB instance
 */
function getDefaultAgentDB() {
    if (!defaultInstance) {
        defaultInstance = new FastAgentDB();
    }
    return defaultInstance;
}
exports.default = {
    FastAgentDB,
    createFastAgentDB,
    getDefaultAgentDB,
};
//# sourceMappingURL=agentdb-fast.js.map