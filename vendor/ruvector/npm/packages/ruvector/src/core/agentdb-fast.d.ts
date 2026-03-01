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
/**
 * Episode entry for trajectory storage
 */
export interface Episode {
    id: string;
    state: number[];
    action: string | number;
    reward: number;
    nextState: number[];
    done: boolean;
    metadata?: Record<string, any>;
    timestamp?: number;
}
/**
 * Trajectory (sequence of episodes)
 */
export interface Trajectory {
    id: string;
    episodes: Episode[];
    totalReward: number;
    metadata?: Record<string, any>;
}
/**
 * Search result for episode queries
 */
export interface EpisodeSearchResult {
    episode: Episode;
    similarity: number;
    trajectoryId?: string;
}
/**
 * Fast in-memory AgentDB implementation
 */
export declare class FastAgentDB {
    private episodes;
    private trajectories;
    private vectorDb;
    private dimensions;
    private maxEpisodes;
    private episodeOrder;
    /**
     * Create a new FastAgentDB instance
     *
     * @param dimensions - Vector dimensions for state embeddings
     * @param maxEpisodes - Maximum episodes to store (LRU eviction)
     */
    constructor(dimensions?: number, maxEpisodes?: number);
    /**
     * Initialize the vector database
     */
    private initVectorDb;
    /**
     * Store an episode
     *
     * @param episode - Episode to store
     * @returns Episode ID
     */
    storeEpisode(episode: Omit<Episode, 'id'> & {
        id?: string;
    }): Promise<string>;
    /**
     * Store multiple episodes in batch
     */
    storeEpisodes(episodes: (Omit<Episode, 'id'> & {
        id?: string;
    })[]): Promise<string[]>;
    /**
     * Retrieve an episode by ID
     */
    getEpisode(id: string): Promise<Episode | null>;
    /**
     * Search for similar episodes by state
     *
     * @param queryState - State vector to search for
     * @param k - Number of results to return
     * @returns Similar episodes sorted by similarity
     */
    searchByState(queryState: number[] | Float32Array, k?: number): Promise<EpisodeSearchResult[]>;
    /**
     * Fallback similarity search using brute-force cosine similarity
     */
    private fallbackSearch;
    /**
     * Compute cosine similarity between two vectors
     */
    private cosineSimilarity;
    /**
     * Store a trajectory (sequence of episodes)
     */
    storeTrajectory(episodes: (Omit<Episode, 'id'> & {
        id?: string;
    })[], metadata?: Record<string, any>): Promise<string>;
    /**
     * Get a trajectory by ID
     */
    getTrajectory(id: string): Promise<Trajectory | null>;
    /**
     * Get top trajectories by total reward
     */
    getTopTrajectories(k?: number): Promise<Trajectory[]>;
    /**
     * Sample random episodes (for experience replay)
     */
    sampleEpisodes(n: number): Promise<Episode[]>;
    /**
     * Get database statistics
     */
    getStats(): {
        episodeCount: number;
        trajectoryCount: number;
        dimensions: number;
        maxEpisodes: number;
        vectorDbAvailable: boolean;
    };
    /**
     * Clear all data
     */
    clear(): void;
    /**
     * Generate a unique ID
     */
    private generateId;
}
/**
 * Create a fast AgentDB instance
 */
export declare function createFastAgentDB(dimensions?: number, maxEpisodes?: number): FastAgentDB;
/**
 * Get the default FastAgentDB instance
 */
export declare function getDefaultAgentDB(): FastAgentDB;
declare const _default: {
    FastAgentDB: typeof FastAgentDB;
    createFastAgentDB: typeof createFastAgentDB;
    getDefaultAgentDB: typeof getDefaultAgentDB;
};
export default _default;
//# sourceMappingURL=agentdb-fast.d.ts.map