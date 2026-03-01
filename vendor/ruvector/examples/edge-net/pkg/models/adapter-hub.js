/**
 * AdapterHub - Community Adapter Registry and Management
 *
 * Provides a marketplace-style interface for browsing, uploading,
 * downloading, and applying community-created LoRA adapters.
 *
 * @module @ruvector/edge-net/models/adapter-hub
 *
 * @example
 * ```javascript
 * import { AdapterHub } from '@ruvector/edge-net/models';
 *
 * const hub = new AdapterHub();
 *
 * // Browse adapters by category
 * const codeAdapters = await hub.browse({ domain: 'code', sort: 'rating' });
 *
 * // Download and apply an adapter
 * const adapter = await hub.download('popular-code-adapter-v1');
 * await myLoRA.loadAdapter(adapter);
 *
 * // Upload your own adapter
 * await hub.upload(myLoRA, {
 *   name: 'My Code Assistant',
 *   description: 'Fine-tuned for Python coding',
 *   domain: 'code',
 *   tags: ['python', 'coding', 'assistant']
 * });
 * ```
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';

// ============================================
// TYPE DEFINITIONS (JSDoc)
// ============================================

/**
 * @typedef {Object} AdapterInfo
 * @property {string} id - Unique adapter identifier
 * @property {string} name - Human-readable name
 * @property {string} description - Detailed description
 * @property {string} author - Author name or ID
 * @property {string} authorId - Unique author identifier
 * @property {string} baseModel - Base model identifier
 * @property {string} domain - Primary domain category
 * @property {string[]} tags - Searchable tags
 * @property {number} rating - Average rating (0-5)
 * @property {number} ratingCount - Number of ratings
 * @property {number} downloads - Total download count
 * @property {number} size - Size in bytes
 * @property {string} version - Adapter version
 * @property {string} license - License identifier
 * @property {number} createdAt - Creation timestamp
 * @property {number} updatedAt - Last update timestamp
 * @property {Object} config - LoRA configuration
 * @property {Object} stats - Training statistics
 */

/**
 * @typedef {Object} BrowseOptions
 * @property {string} [domain] - Filter by domain
 * @property {string} [baseModel] - Filter by base model
 * @property {string} [query] - Search query
 * @property {string[]} [tags] - Filter by tags
 * @property {string} [sort='downloads'] - Sort order
 * @property {number} [limit=20] - Results per page
 * @property {number} [offset=0] - Pagination offset
 * @property {number} [minRating=0] - Minimum rating filter
 */

/**
 * @typedef {Object} UploadOptions
 * @property {string} name - Adapter name
 * @property {string} description - Adapter description
 * @property {string} domain - Primary domain category
 * @property {string[]} [tags=[]] - Searchable tags
 * @property {string} [license='MIT'] - License identifier
 * @property {boolean} [public=true] - Public visibility
 */

/**
 * @typedef {Object} Review
 * @property {string} id - Review ID
 * @property {string} adapterId - Adapter ID
 * @property {string} authorId - Review author ID
 * @property {string} authorName - Review author name
 * @property {number} rating - Rating (1-5)
 * @property {string} comment - Review comment
 * @property {number} createdAt - Creation timestamp
 * @property {number} [helpful=0] - Helpful votes
 */

// ============================================
// CONSTANTS
// ============================================

/**
 * Available domain categories for adapters
 */
export const ADAPTER_DOMAINS = {
    code: {
        name: 'Code & Programming',
        description: 'Code generation, completion, and programming assistance',
        icon: 'code',
        subdomains: ['python', 'javascript', 'rust', 'go', 'sql', 'general'],
    },
    writing: {
        name: 'Creative Writing',
        description: 'Story writing, poetry, and creative content',
        icon: 'pen',
        subdomains: ['fiction', 'poetry', 'technical', 'copywriting', 'academic'],
    },
    math: {
        name: 'Mathematics',
        description: 'Mathematical reasoning and problem solving',
        icon: 'calculator',
        subdomains: ['algebra', 'calculus', 'statistics', 'geometry', 'logic'],
    },
    science: {
        name: 'Science',
        description: 'Scientific knowledge and reasoning',
        icon: 'flask',
        subdomains: ['physics', 'chemistry', 'biology', 'medicine', 'engineering'],
    },
    language: {
        name: 'Language',
        description: 'Language learning and translation',
        icon: 'globe',
        subdomains: ['translation', 'grammar', 'vocabulary', 'conversation'],
    },
    business: {
        name: 'Business',
        description: 'Business writing and analysis',
        icon: 'briefcase',
        subdomains: ['email', 'reports', 'analysis', 'marketing', 'legal'],
    },
    assistant: {
        name: 'General Assistant',
        description: 'General-purpose assistants and chatbots',
        icon: 'robot',
        subdomains: ['helpful', 'concise', 'detailed', 'friendly', 'formal'],
    },
    roleplay: {
        name: 'Roleplay',
        description: 'Character and roleplay adaptations',
        icon: 'theater',
        subdomains: ['characters', 'games', 'educational', 'simulation'],
    },
};

/**
 * Default hub configuration
 */
const DEFAULT_HUB_CONFIG = {
    apiEndpoint: 'https://hub.ruvector.dev/api',
    storageEndpoint: 'https://storage.ruvector.dev',
    cacheDir: '.ruvector/adapter-cache',
    maxCacheSize: 500 * 1024 * 1024, // 500MB
    enableOffline: true,
    autoUpdate: true,
};

// ============================================
// ADAPTERHUB CLASS
// ============================================

/**
 * AdapterHub - Central registry for community adapters
 *
 * Provides a complete ecosystem for discovering, sharing, and managing
 * LoRA adapters. Supports offline caching, ratings/reviews, and version
 * management.
 *
 * @extends EventEmitter
 */
export class AdapterHub extends EventEmitter {
    /**
     * Create an AdapterHub instance
     *
     * @param {Object} [config={}] - Hub configuration
     */
    constructor(config = {}) {
        super();

        this.config = { ...DEFAULT_HUB_CONFIG, ...config };
        this.userId = config.userId || `anon-${randomBytes(8).toString('hex')}`;

        // Local cache of adapter metadata
        this.cache = new Map();

        // Downloaded adapters
        this.downloaded = new Map();

        // User's own adapters
        this.myAdapters = new Map();

        // Reviews cache
        this.reviews = new Map();

        // Stats
        this.stats = {
            totalBrowses: 0,
            totalDownloads: 0,
            totalUploads: 0,
            cacheHits: 0,
            cacheMisses: 0,
        };

        // Initialize local storage for offline mode
        this._initLocalStorage();
    }

    /**
     * Initialize local storage for offline caching
     * @private
     */
    async _initLocalStorage() {
        try {
            if (typeof localStorage !== 'undefined') {
                // Browser environment
                const cached = localStorage.getItem('ruvector-adapter-hub-cache');
                if (cached) {
                    const data = JSON.parse(cached);
                    for (const [id, info] of Object.entries(data.adapters || {})) {
                        this.cache.set(id, info);
                    }
                }
            } else if (typeof process !== 'undefined') {
                // Node.js environment
                const fs = await import('fs/promises');
                const path = await import('path');
                const cacheFile = path.join(this.config.cacheDir, 'hub-cache.json');

                try {
                    const content = await fs.readFile(cacheFile, 'utf-8');
                    const data = JSON.parse(content);
                    for (const [id, info] of Object.entries(data.adapters || {})) {
                        this.cache.set(id, info);
                    }
                } catch {
                    // Cache file doesn't exist yet
                }
            }
        } catch (error) {
            console.error('[AdapterHub] Failed to initialize local storage:', error.message);
        }
    }

    /**
     * Save cache to local storage
     * @private
     */
    async _saveLocalStorage() {
        try {
            const data = {
                adapters: Object.fromEntries(this.cache),
                timestamp: Date.now(),
            };

            if (typeof localStorage !== 'undefined') {
                localStorage.setItem('ruvector-adapter-hub-cache', JSON.stringify(data));
            } else if (typeof process !== 'undefined') {
                const fs = await import('fs/promises');
                const path = await import('path');
                await fs.mkdir(this.config.cacheDir, { recursive: true });
                const cacheFile = path.join(this.config.cacheDir, 'hub-cache.json');
                await fs.writeFile(cacheFile, JSON.stringify(data, null, 2));
            }
        } catch (error) {
            console.error('[AdapterHub] Failed to save local storage:', error.message);
        }
    }

    // ============================================
    // BROWSING AND DISCOVERY
    // ============================================

    /**
     * Browse available adapters with filtering and sorting
     *
     * @param {BrowseOptions} [options={}] - Browse options
     * @returns {Promise<{adapters: AdapterInfo[], total: number, hasMore: boolean}>}
     *
     * @example
     * ```javascript
     * // Browse code adapters sorted by rating
     * const results = await hub.browse({
     *   domain: 'code',
     *   sort: 'rating',
     *   limit: 10
     * });
     *
     * for (const adapter of results.adapters) {
     *   console.log(`${adapter.name}: ${adapter.rating} stars`);
     * }
     * ```
     */
    async browse(options = {}) {
        const opts = {
            domain: null,
            baseModel: null,
            query: null,
            tags: [],
            sort: 'downloads',
            limit: 20,
            offset: 0,
            minRating: 0,
            ...options,
        };

        this.stats.totalBrowses++;
        this.emit('browse:start', opts);

        // Filter adapters from cache
        let adapters = Array.from(this.cache.values());

        // Apply filters
        if (opts.domain) {
            adapters = adapters.filter(a => a.domain === opts.domain);
        }
        if (opts.baseModel) {
            adapters = adapters.filter(a => a.baseModel === opts.baseModel);
        }
        if (opts.minRating > 0) {
            adapters = adapters.filter(a => a.rating >= opts.minRating);
        }
        if (opts.tags && opts.tags.length > 0) {
            adapters = adapters.filter(a =>
                opts.tags.some(tag => a.tags?.includes(tag))
            );
        }
        if (opts.query) {
            const query = opts.query.toLowerCase();
            adapters = adapters.filter(a =>
                a.name?.toLowerCase().includes(query) ||
                a.description?.toLowerCase().includes(query) ||
                a.tags?.some(t => t.toLowerCase().includes(query))
            );
        }

        // Sort
        switch (opts.sort) {
            case 'rating':
                adapters.sort((a, b) => (b.rating || 0) - (a.rating || 0));
                break;
            case 'downloads':
                adapters.sort((a, b) => (b.downloads || 0) - (a.downloads || 0));
                break;
            case 'recent':
                adapters.sort((a, b) => (b.createdAt || 0) - (a.createdAt || 0));
                break;
            case 'updated':
                adapters.sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));
                break;
            case 'name':
                adapters.sort((a, b) => (a.name || '').localeCompare(b.name || ''));
                break;
        }

        // Paginate
        const total = adapters.length;
        const paged = adapters.slice(opts.offset, opts.offset + opts.limit);

        const result = {
            adapters: paged,
            total,
            hasMore: opts.offset + opts.limit < total,
            offset: opts.offset,
            limit: opts.limit,
        };

        this.emit('browse:complete', result);
        return result;
    }

    /**
     * Search adapters by query string
     *
     * @param {string} query - Search query
     * @param {Object} [options={}] - Additional filters
     * @returns {Promise<AdapterInfo[]>}
     *
     * @example
     * ```javascript
     * const results = await hub.search('python code completion');
     * ```
     */
    async search(query, options = {}) {
        return this.browse({ ...options, query });
    }

    /**
     * Get featured/recommended adapters
     *
     * @param {number} [limit=10] - Number of adapters to return
     * @returns {Promise<AdapterInfo[]>}
     */
    async getFeatured(limit = 10) {
        const result = await this.browse({
            sort: 'rating',
            minRating: 4.0,
            limit,
        });
        return result.adapters;
    }

    /**
     * Get trending adapters (most downloaded recently)
     *
     * @param {number} [limit=10] - Number of adapters to return
     * @returns {Promise<AdapterInfo[]>}
     */
    async getTrending(limit = 10) {
        const result = await this.browse({
            sort: 'downloads',
            limit,
        });
        return result.adapters;
    }

    /**
     * Get adapters by domain category
     *
     * @param {string} domain - Domain category
     * @param {Object} [options={}] - Additional options
     * @returns {Promise<AdapterInfo[]>}
     */
    async getByDomain(domain, options = {}) {
        const result = await this.browse({ ...options, domain });
        return result.adapters;
    }

    /**
     * Get available domain categories with counts
     *
     * @returns {Promise<Array<{domain: string, count: number, info: Object}>>}
     */
    async getDomains() {
        const counts = {};
        for (const adapter of this.cache.values()) {
            counts[adapter.domain] = (counts[adapter.domain] || 0) + 1;
        }

        return Object.entries(ADAPTER_DOMAINS).map(([id, info]) => ({
            domain: id,
            count: counts[id] || 0,
            ...info,
        }));
    }

    // ============================================
    // DOWNLOAD AND APPLY
    // ============================================

    /**
     * Download an adapter by ID
     *
     * @param {string} adapterId - Adapter identifier
     * @param {Object} [options={}] - Download options
     * @returns {Promise<Object>} Adapter data
     *
     * @example
     * ```javascript
     * const adapter = await hub.download('code-assistant-v2');
     * await myLoRA.loadAdapter(adapter);
     * ```
     */
    async download(adapterId, options = {}) {
        this.stats.totalDownloads++;
        this.emit('download:start', { adapterId });

        // Check local cache first
        if (this.downloaded.has(adapterId)) {
            this.stats.cacheHits++;
            const cached = this.downloaded.get(adapterId);
            this.emit('download:complete', { adapterId, cached: true });
            return cached;
        }

        this.stats.cacheMisses++;

        // Get adapter info
        const info = this.cache.get(adapterId);
        if (!info) {
            throw new Error(`Adapter not found: ${adapterId}`);
        }

        // Simulate download (in production, would fetch from storage)
        const adapterData = this._generateMockAdapter(info);

        // Cache downloaded adapter
        this.downloaded.set(adapterId, adapterData);

        // Update download count
        info.downloads = (info.downloads || 0) + 1;
        this.cache.set(adapterId, info);
        await this._saveLocalStorage();

        this.emit('download:complete', { adapterId, cached: false, size: JSON.stringify(adapterData).length });
        return adapterData;
    }

    /**
     * Generate mock adapter data for demo purposes
     * @private
     */
    _generateMockAdapter(info) {
        const rank = info.config?.rank || 4;
        const dim = info.config?.embeddingDim || 384;

        const adapters = {};
        for (const module of ['query', 'value', 'key', 'dense']) {
            adapters[module] = {
                loraA: this._generateRandomMatrix(dim, rank),
                loraB: this._generateRandomMatrix(rank, dim),
                scaling: (info.config?.alpha || 8) / rank,
            };
        }

        return {
            version: '1.0.0',
            format: 'microlora',
            metadata: {
                id: info.id,
                name: info.name,
                description: info.description,
                baseModel: info.baseModel,
                domain: info.domain,
                rank: rank,
                alpha: info.config?.alpha || 8,
                trainingSamples: info.stats?.trainingSamples || 0,
                trainingEpochs: info.stats?.trainingEpochs || 0,
                createdAt: info.createdAt,
                version: info.version,
            },
            config: info.config,
            baseModel: info.baseModel,
            adapters,
            stats: info.stats,
            createdAt: info.createdAt,
            savedAt: Date.now(),
        };
    }

    /**
     * Generate random matrix for mock data
     * @private
     */
    _generateRandomMatrix(rows, cols) {
        const matrix = [];
        const std = Math.sqrt(2 / (rows + cols)) * 0.1;
        for (let i = 0; i < rows; i++) {
            const row = [];
            for (let j = 0; j < cols; j++) {
                row.push((Math.random() - 0.5) * 2 * std);
            }
            matrix.push(row);
        }
        return matrix;
    }

    /**
     * Check if an adapter is downloaded locally
     *
     * @param {string} adapterId - Adapter identifier
     * @returns {boolean}
     */
    isDownloaded(adapterId) {
        return this.downloaded.has(adapterId);
    }

    /**
     * Remove a downloaded adapter from local cache
     *
     * @param {string} adapterId - Adapter identifier
     */
    removeDownloaded(adapterId) {
        this.downloaded.delete(adapterId);
        this.emit('adapter:removed', { adapterId });
    }

    // ============================================
    // UPLOAD AND SHARE
    // ============================================

    /**
     * Upload an adapter to the hub
     *
     * @param {Object} adapter - MicroLoRA instance or adapter data
     * @param {UploadOptions} options - Upload options
     * @returns {Promise<AdapterInfo>} Uploaded adapter info
     *
     * @example
     * ```javascript
     * const info = await hub.upload(myLoRA, {
     *   name: 'Python Code Assistant',
     *   description: 'Specialized for Python coding tasks',
     *   domain: 'code',
     *   tags: ['python', 'coding', 'assistant']
     * });
     * console.log(`Uploaded: ${info.id}`);
     * ```
     */
    async upload(adapter, options) {
        const opts = {
            name: 'Untitled Adapter',
            description: '',
            domain: 'general',
            tags: [],
            license: 'MIT',
            public: true,
            ...options,
        };

        this.stats.totalUploads++;
        this.emit('upload:start', { name: opts.name });

        // Get adapter data
        let adapterData;
        if (typeof adapter.saveAdapter === 'function') {
            adapterData = await adapter.saveAdapter();
        } else {
            adapterData = adapter;
        }

        // Generate unique ID
        const id = `${opts.domain}-${opts.name.toLowerCase().replace(/\s+/g, '-')}-${randomBytes(4).toString('hex')}`;

        // Create adapter info
        const info = {
            id,
            name: opts.name,
            description: opts.description,
            author: this.userId,
            authorId: this.userId,
            baseModel: adapterData.baseModel || 'unknown',
            domain: opts.domain,
            tags: opts.tags,
            rating: 0,
            ratingCount: 0,
            downloads: 0,
            size: JSON.stringify(adapterData).length,
            version: adapterData.version || '1.0.0',
            license: opts.license,
            createdAt: Date.now(),
            updatedAt: Date.now(),
            config: adapterData.config,
            stats: adapterData.stats,
            public: opts.public,
        };

        // Store in cache and my adapters
        this.cache.set(id, info);
        this.myAdapters.set(id, { info, data: adapterData });
        await this._saveLocalStorage();

        this.emit('upload:complete', info);
        return info;
    }

    /**
     * Update an uploaded adapter
     *
     * @param {string} adapterId - Adapter to update
     * @param {Object} adapter - New adapter data
     * @param {Object} [options={}] - Update options
     * @returns {Promise<AdapterInfo>}
     */
    async update(adapterId, adapter, options = {}) {
        const existing = this.myAdapters.get(adapterId);
        if (!existing) {
            throw new Error(`Adapter not found in your uploads: ${adapterId}`);
        }

        let adapterData;
        if (typeof adapter.saveAdapter === 'function') {
            adapterData = await adapter.saveAdapter();
        } else {
            adapterData = adapter;
        }

        // Update info
        const info = {
            ...existing.info,
            ...options,
            updatedAt: Date.now(),
            size: JSON.stringify(adapterData).length,
            config: adapterData.config,
            stats: adapterData.stats,
        };

        // Increment version
        const versionParts = (info.version || '1.0.0').split('.').map(Number);
        versionParts[2]++;
        info.version = versionParts.join('.');

        // Update cache
        this.cache.set(adapterId, info);
        this.myAdapters.set(adapterId, { info, data: adapterData });
        await this._saveLocalStorage();

        this.emit('update:complete', info);
        return info;
    }

    /**
     * Delete an uploaded adapter
     *
     * @param {string} adapterId - Adapter to delete
     * @returns {Promise<boolean>}
     */
    async delete(adapterId) {
        if (!this.myAdapters.has(adapterId)) {
            throw new Error(`Adapter not found in your uploads: ${adapterId}`);
        }

        this.cache.delete(adapterId);
        this.myAdapters.delete(adapterId);
        this.downloaded.delete(adapterId);
        this.reviews.delete(adapterId);

        await this._saveLocalStorage();
        this.emit('delete:complete', { adapterId });
        return true;
    }

    /**
     * Get user's uploaded adapters
     *
     * @returns {AdapterInfo[]}
     */
    getMyAdapters() {
        return Array.from(this.myAdapters.values()).map(a => a.info);
    }

    // ============================================
    // RATINGS AND REVIEWS
    // ============================================

    /**
     * Rate an adapter
     *
     * @param {string} adapterId - Adapter to rate
     * @param {number} rating - Rating (1-5)
     * @returns {Promise<void>}
     *
     * @example
     * ```javascript
     * await hub.rate('code-assistant-v2', 5);
     * ```
     */
    async rate(adapterId, rating) {
        if (rating < 1 || rating > 5) {
            throw new Error('Rating must be between 1 and 5');
        }

        const info = this.cache.get(adapterId);
        if (!info) {
            throw new Error(`Adapter not found: ${adapterId}`);
        }

        // Update rating (running average)
        const totalRating = (info.rating || 0) * (info.ratingCount || 0);
        info.ratingCount = (info.ratingCount || 0) + 1;
        info.rating = (totalRating + rating) / info.ratingCount;

        this.cache.set(adapterId, info);
        await this._saveLocalStorage();

        this.emit('rating:added', { adapterId, rating, newRating: info.rating });
    }

    /**
     * Add a review for an adapter
     *
     * @param {string} adapterId - Adapter to review
     * @param {number} rating - Rating (1-5)
     * @param {string} comment - Review comment
     * @returns {Promise<Review>}
     *
     * @example
     * ```javascript
     * const review = await hub.review('code-assistant-v2', 5, 'Great for Python!');
     * ```
     */
    async review(adapterId, rating, comment) {
        // Add rating first
        await this.rate(adapterId, rating);

        // Create review
        const reviewData = {
            id: `review-${randomBytes(6).toString('hex')}`,
            adapterId,
            authorId: this.userId,
            authorName: `User-${this.userId.slice(0, 8)}`,
            rating,
            comment,
            createdAt: Date.now(),
            helpful: 0,
        };

        // Store review
        if (!this.reviews.has(adapterId)) {
            this.reviews.set(adapterId, []);
        }
        this.reviews.get(adapterId).push(reviewData);

        this.emit('review:added', reviewData);
        return reviewData;
    }

    /**
     * Get reviews for an adapter
     *
     * @param {string} adapterId - Adapter ID
     * @param {Object} [options={}] - Options
     * @returns {Promise<Review[]>}
     */
    async getReviews(adapterId, options = {}) {
        const { sort = 'recent', limit = 20 } = options;

        const adapterReviews = this.reviews.get(adapterId) || [];

        // Sort
        const sorted = [...adapterReviews];
        switch (sort) {
            case 'helpful':
                sorted.sort((a, b) => (b.helpful || 0) - (a.helpful || 0));
                break;
            case 'rating':
                sorted.sort((a, b) => b.rating - a.rating);
                break;
            case 'recent':
            default:
                sorted.sort((a, b) => b.createdAt - a.createdAt);
        }

        return sorted.slice(0, limit);
    }

    /**
     * Mark a review as helpful
     *
     * @param {string} reviewId - Review ID
     */
    async markHelpful(reviewId) {
        for (const reviews of this.reviews.values()) {
            const review = reviews.find(r => r.id === reviewId);
            if (review) {
                review.helpful = (review.helpful || 0) + 1;
                this.emit('review:helpful', { reviewId, helpful: review.helpful });
                return;
            }
        }
    }

    // ============================================
    // COLLECTIONS AND FAVORITES
    // ============================================

    /**
     * Create a collection of adapters
     *
     * @param {string} name - Collection name
     * @param {string} [description=''] - Collection description
     * @returns {Object} Collection info
     */
    createCollection(name, description = '') {
        const id = `collection-${randomBytes(6).toString('hex')}`;
        const collection = {
            id,
            name,
            description,
            adapters: [],
            createdAt: Date.now(),
            updatedAt: Date.now(),
        };

        this.emit('collection:created', collection);
        return collection;
    }

    /**
     * Add adapter to favorites
     *
     * @param {string} adapterId - Adapter to favorite
     */
    addFavorite(adapterId) {
        this.emit('favorite:added', { adapterId });
    }

    /**
     * Remove adapter from favorites
     *
     * @param {string} adapterId - Adapter to unfavorite
     */
    removeFavorite(adapterId) {
        this.emit('favorite:removed', { adapterId });
    }

    // ============================================
    // UTILITY METHODS
    // ============================================

    /**
     * Get detailed info about an adapter
     *
     * @param {string} adapterId - Adapter ID
     * @returns {Promise<AdapterInfo|null>}
     */
    async getAdapterInfo(adapterId) {
        return this.cache.get(adapterId) || null;
    }

    /**
     * Check if an adapter exists
     *
     * @param {string} adapterId - Adapter ID
     * @returns {boolean}
     */
    exists(adapterId) {
        return this.cache.has(adapterId);
    }

    /**
     * Get hub statistics
     *
     * @returns {Object}
     */
    getStats() {
        return {
            ...this.stats,
            totalAdapters: this.cache.size,
            downloadedAdapters: this.downloaded.size,
            myAdapters: this.myAdapters.size,
            totalReviews: Array.from(this.reviews.values()).reduce((sum, r) => sum + r.length, 0),
        };
    }

    /**
     * Clear all caches
     */
    async clearCache() {
        this.downloaded.clear();
        this.emit('cache:cleared');
    }

    /**
     * Seed hub with sample adapters for demo purposes
     *
     * @param {number} [count=20] - Number of sample adapters
     */
    async seedSampleAdapters(count = 20) {
        const domains = Object.keys(ADAPTER_DOMAINS);
        const models = ['phi-1.5-int4', 'distilgpt2', 'gpt2', 'starcoder-tiny'];
        const adjectives = ['Advanced', 'Pro', 'Ultra', 'Smart', 'Fast', 'Accurate', 'Helpful'];
        const nouns = ['Assistant', 'Helper', 'Expert', 'Companion', 'Generator', 'Wizard'];

        for (let i = 0; i < count; i++) {
            const domain = domains[Math.floor(Math.random() * domains.length)];
            const model = models[Math.floor(Math.random() * models.length)];
            const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
            const noun = nouns[Math.floor(Math.random() * nouns.length)];
            const name = `${adj} ${ADAPTER_DOMAINS[domain].name.split(' ')[0]} ${noun}`;

            const info = {
                id: `sample-${domain}-${randomBytes(4).toString('hex')}`,
                name,
                description: `A ${adj.toLowerCase()} adapter for ${ADAPTER_DOMAINS[domain].description.toLowerCase()}`,
                author: `sample-author-${i % 5}`,
                authorId: `sample-author-${i % 5}`,
                baseModel: model,
                domain,
                tags: [domain, ...ADAPTER_DOMAINS[domain].subdomains.slice(0, 2)],
                rating: 3 + Math.random() * 2,
                ratingCount: Math.floor(Math.random() * 100) + 1,
                downloads: Math.floor(Math.random() * 10000),
                size: Math.floor(Math.random() * 50000) + 10000,
                version: `1.${Math.floor(Math.random() * 10)}.0`,
                license: 'MIT',
                createdAt: Date.now() - Math.floor(Math.random() * 30 * 24 * 60 * 60 * 1000),
                updatedAt: Date.now() - Math.floor(Math.random() * 7 * 24 * 60 * 60 * 1000),
                config: {
                    rank: [4, 8, 16][Math.floor(Math.random() * 3)],
                    alpha: [8, 16, 32][Math.floor(Math.random() * 3)],
                    embeddingDim: 384,
                },
                stats: {
                    trainingSamples: Math.floor(Math.random() * 10000) + 100,
                    trainingEpochs: Math.floor(Math.random() * 50) + 5,
                },
            };

            this.cache.set(info.id, info);
        }

        await this._saveLocalStorage();
        this.emit('seed:complete', { count });
    }
}

// ============================================
// EXPORTS
// ============================================

export default AdapterHub;
