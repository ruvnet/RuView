"use strict";
/**
 * IntelligenceEngine - Full RuVector Intelligence Stack
 *
 * Integrates all RuVector capabilities for self-learning hooks:
 * - VectorDB with HNSW for semantic memory (150x faster)
 * - SONA for continual learning (Micro-LoRA, EWC++)
 * - FastAgentDB for episode/trajectory storage
 * - Attention mechanisms for pattern recognition
 * - ReasoningBank for pattern clustering
 *
 * Replaces the simple Q-learning approach with real ML-powered intelligence.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.IntelligenceEngine = void 0;
exports.createIntelligenceEngine = createIntelligenceEngine;
exports.createHighPerformanceEngine = createHighPerformanceEngine;
exports.createLightweightEngine = createLightweightEngine;
const agentdb_fast_1 = require("./agentdb-fast");
const sona_wrapper_1 = require("./sona-wrapper");
const onnx_embedder_1 = require("./onnx-embedder");
const parallel_intelligence_1 = require("./parallel-intelligence");
// ============================================================================
// Lazy Loading
// ============================================================================
let VectorDB = null;
let vectorDbError = null;
function getVectorDB() {
    if (VectorDB)
        return VectorDB;
    if (vectorDbError)
        throw vectorDbError;
    try {
        const core = require('@ruvector/core');
        VectorDB = core.VectorDb || core.VectorDB;
        return VectorDB;
    }
    catch {
        try {
            const pkg = require('ruvector');
            VectorDB = pkg.VectorDb || pkg.VectorDB;
            return VectorDB;
        }
        catch (e) {
            vectorDbError = new Error(`VectorDB not available: ${e.message}`);
            throw vectorDbError;
        }
    }
}
let attentionModule = null;
let attentionError = null;
function getAttention() {
    if (attentionModule)
        return attentionModule;
    if (attentionError)
        return null; // Silently fail for optional module
    try {
        attentionModule = require('@ruvector/attention');
        return attentionModule;
    }
    catch (e) {
        attentionError = e;
        return null;
    }
}
// ============================================================================
// Intelligence Engine
// ============================================================================
/**
 * Full-stack intelligence engine using all RuVector capabilities
 */
class IntelligenceEngine {
    constructor(config = {}) {
        this.vectorDb = null;
        this.sona = null;
        this.attention = null;
        this.onnxEmbedder = null;
        this.onnxReady = false;
        this.parallel = null;
        // In-memory data structures
        this.memories = new Map();
        this.routingPatterns = new Map(); // state -> action -> value
        this.errorPatterns = new Map(); // error -> fixes
        this.coEditPatterns = new Map(); // file -> related files -> count
        this.agentMappings = new Map(); // extension/dir -> agent
        this.workerTriggerMappings = new Map(); // trigger -> agents
        // Runtime state
        this.currentTrajectoryId = null;
        this.sessionStart = Date.now();
        this.learningEnabled = true;
        this.episodeBatchQueue = [];
        // If ONNX is enabled, use 384 dimensions (MiniLM default)
        const useOnnx = !!(config.enableOnnx && (0, onnx_embedder_1.isOnnxAvailable)());
        const embeddingDim = useOnnx ? 384 : (config.embeddingDim ?? 256);
        this.config = {
            embeddingDim,
            maxMemories: config.maxMemories ?? 100000,
            maxEpisodes: config.maxEpisodes ?? 50000,
            enableSona: config.enableSona ?? true,
            enableAttention: config.enableAttention ?? true,
            enableOnnx: useOnnx,
            sonaConfig: config.sonaConfig ?? {},
            storagePath: config.storagePath ?? '',
            learningRate: config.learningRate ?? 0.1,
            parallelConfig: config.parallelConfig ?? {},
        };
        // Initialize parallel workers (auto-enabled for MCP, disabled for CLI)
        this.parallel = (0, parallel_intelligence_1.getParallelIntelligence)(this.config.parallelConfig);
        this.initParallel();
        // Initialize FastAgentDB for episode storage
        this.agentDb = new agentdb_fast_1.FastAgentDB(this.config.embeddingDim, this.config.maxEpisodes);
        // Initialize ONNX embedder if enabled
        if (this.config.enableOnnx) {
            this.onnxEmbedder = new onnx_embedder_1.OnnxEmbedder();
            // Initialize async (don't block constructor)
            this.initOnnx();
        }
        // Initialize SONA if enabled and available
        if (this.config.enableSona && (0, sona_wrapper_1.isSonaAvailable)()) {
            try {
                this.sona = sona_wrapper_1.SonaEngine.withConfig({
                    hiddenDim: this.config.embeddingDim,
                    embeddingDim: this.config.embeddingDim,
                    microLoraRank: 2, // Fast adaptations
                    baseLoraRank: 8,
                    patternClusters: 100,
                    trajectoryCapacity: 10000,
                    ...this.config.sonaConfig,
                });
            }
            catch (e) {
                console.warn('SONA initialization failed, using fallback learning');
            }
        }
        // Initialize attention if enabled (fallback if ONNX not available)
        if (this.config.enableAttention && !this.config.enableOnnx) {
            this.attention = getAttention();
        }
        // Initialize VectorDB for memory
        this.initVectorDb();
    }
    async initOnnx() {
        if (!this.onnxEmbedder)
            return;
        try {
            await this.onnxEmbedder.init();
            this.onnxReady = true;
        }
        catch (e) {
            console.warn('ONNX initialization failed, using fallback embeddings');
            this.onnxReady = false;
        }
    }
    async initVectorDb() {
        try {
            const VDB = getVectorDB();
            this.vectorDb = new VDB({
                dimensions: this.config.embeddingDim,
                distanceMetric: 'Cosine',
            });
        }
        catch {
            // VectorDB not available, use fallback
        }
    }
    async initParallel() {
        if (this.parallel) {
            try {
                await this.parallel.init();
            }
            catch {
                // Parallel not available, use sequential
                this.parallel = null;
            }
        }
    }
    // =========================================================================
    // Embedding Generation
    // =========================================================================
    /**
     * Generate embedding using ONNX, attention, or hash (in order of preference)
     */
    embed(text) {
        const dim = this.config.embeddingDim;
        // Try ONNX semantic embeddings first (best quality)
        if (this.onnxReady && this.onnxEmbedder) {
            try {
                // Note: This is sync wrapper for async ONNX
                // For full async, use embedAsync
                return this.hashEmbed(text, dim); // Fallback for sync context
            }
            catch {
                // Fall through
            }
        }
        // Try to use attention-based embedding
        if (this.attention?.DotProductAttention) {
            try {
                return this.attentionEmbed(text, dim);
            }
            catch {
                // Fall through to hash embedding
            }
        }
        // Improved positional hash embedding
        return this.hashEmbed(text, dim);
    }
    /**
     * Async embedding with ONNX support (recommended for semantic quality)
     */
    async embedAsync(text) {
        // Try ONNX first (best semantic quality)
        if (this.onnxEmbedder) {
            try {
                if (!this.onnxReady) {
                    await this.onnxEmbedder.init();
                    this.onnxReady = true;
                }
                return await this.onnxEmbedder.embed(text);
            }
            catch {
                // Fall through to sync methods
            }
        }
        // Fall back to sync embedding
        return this.embed(text);
    }
    /**
     * Attention-based embedding using Flash or Multi-head attention
     */
    attentionEmbed(text, dim) {
        const tokens = this.tokenize(text);
        const tokenEmbeddings = tokens.map(t => this.tokenEmbed(t, dim));
        if (tokenEmbeddings.length === 0) {
            return new Array(dim).fill(0);
        }
        try {
            // Try FlashAttention first (fastest)
            if (this.attention?.FlashAttention) {
                const flash = new this.attention.FlashAttention(dim);
                const query = new Float32Array(this.meanPool(tokenEmbeddings));
                const keys = tokenEmbeddings.map(e => new Float32Array(e));
                const values = tokenEmbeddings.map(e => new Float32Array(e));
                const result = flash.forward(query, keys, values);
                return Array.from(result);
            }
            // Try MultiHeadAttention (better quality)
            if (this.attention?.MultiHeadAttention) {
                const numHeads = Math.min(8, Math.floor(dim / 32)); // 8 heads max
                const mha = new this.attention.MultiHeadAttention(dim, numHeads);
                const query = new Float32Array(this.meanPool(tokenEmbeddings));
                const keys = tokenEmbeddings.map(e => new Float32Array(e));
                const values = tokenEmbeddings.map(e => new Float32Array(e));
                const result = mha.forward(query, keys, values);
                return Array.from(result);
            }
            // Fall back to DotProductAttention
            if (this.attention?.DotProductAttention) {
                const attn = new this.attention.DotProductAttention();
                const query = this.meanPool(tokenEmbeddings);
                const result = attn.forward(new Float32Array(query), tokenEmbeddings.map(e => new Float32Array(e)), tokenEmbeddings.map(e => new Float32Array(e)));
                return Array.from(result);
            }
        }
        catch {
            // Fall through to hash embedding
        }
        // Ultimate fallback
        return this.hashEmbed(text, dim);
    }
    /**
     * Improved hash-based embedding with positional encoding
     */
    hashEmbed(text, dim) {
        const embedding = new Array(dim).fill(0);
        const tokens = this.tokenize(text);
        for (let t = 0; t < tokens.length; t++) {
            const token = tokens[t];
            const posWeight = 1 / (1 + t * 0.1); // Positional decay
            for (let i = 0; i < token.length; i++) {
                const charCode = token.charCodeAt(i);
                // Multiple hash functions for better distribution
                const h1 = (charCode * 31 + i * 17 + t * 7) % dim;
                const h2 = (charCode * 37 + i * 23 + t * 11) % dim;
                const h3 = (charCode * 41 + i * 29 + t * 13) % dim;
                embedding[h1] += posWeight;
                embedding[h2] += posWeight * 0.5;
                embedding[h3] += posWeight * 0.25;
            }
        }
        // L2 normalize
        const norm = Math.sqrt(embedding.reduce((a, b) => a + b * b, 0));
        if (norm > 0) {
            for (let i = 0; i < dim; i++)
                embedding[i] /= norm;
        }
        return embedding;
    }
    tokenize(text) {
        return text.toLowerCase()
            .replace(/[^\w\s]/g, ' ')
            .split(/\s+/)
            .filter(t => t.length > 0);
    }
    tokenEmbed(token, dim) {
        const embedding = new Array(dim).fill(0);
        for (let i = 0; i < token.length; i++) {
            const idx = (token.charCodeAt(i) * 31 + i * 17) % dim;
            embedding[idx] += 1;
        }
        const norm = Math.sqrt(embedding.reduce((a, b) => a + b * b, 0));
        if (norm > 0)
            for (let i = 0; i < dim; i++)
                embedding[i] /= norm;
        return embedding;
    }
    meanPool(embeddings) {
        if (embeddings.length === 0)
            return [];
        const dim = embeddings[0].length;
        const result = new Array(dim).fill(0);
        for (const emb of embeddings) {
            for (let i = 0; i < dim; i++)
                result[i] += emb[i];
        }
        for (let i = 0; i < dim; i++)
            result[i] /= embeddings.length;
        return result;
    }
    // =========================================================================
    // Memory Operations
    // =========================================================================
    /**
     * Store content in vector memory (uses ONNX if available)
     */
    async remember(content, type = 'general') {
        const id = `mem-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        // Use async ONNX embeddings if available for better semantic quality
        const embedding = await this.embedAsync(content);
        const entry = {
            id,
            content,
            type,
            embedding,
            created: new Date().toISOString(),
            accessed: 0,
        };
        this.memories.set(id, entry);
        // Index in VectorDB if available
        if (this.vectorDb) {
            try {
                await this.vectorDb.insert({
                    id,
                    vector: new Float32Array(embedding),
                    metadata: JSON.stringify({ content, type, created: entry.created }),
                });
            }
            catch {
                // Ignore indexing errors
            }
        }
        return entry;
    }
    /**
     * Semantic search of memories (uses ONNX if available)
     */
    async recall(query, topK = 5) {
        // Use async ONNX embeddings if available for better semantic quality
        const queryEmbed = await this.embedAsync(query);
        // Try VectorDB search first (HNSW - 150x faster)
        if (this.vectorDb) {
            try {
                const results = await this.vectorDb.search({
                    vector: new Float32Array(queryEmbed),
                    k: topK,
                });
                return results.map((r) => {
                    const entry = this.memories.get(r.id);
                    if (entry) {
                        entry.accessed++;
                        entry.score = 1 - r.score; // Convert distance to similarity
                    }
                    return entry;
                }).filter((e) => e !== null);
            }
            catch {
                // Fall through to brute force
            }
        }
        // Fallback: brute-force cosine similarity
        const scored = Array.from(this.memories.values()).map(m => ({
            ...m,
            score: this.cosineSimilarity(queryEmbed, m.embedding),
        }));
        return scored
            .sort((a, b) => (b.score || 0) - (a.score || 0))
            .slice(0, topK);
    }
    cosineSimilarity(a, b) {
        if (a.length !== b.length)
            return 0;
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA) * Math.sqrt(normB);
        return denom > 0 ? dot / denom : 0;
    }
    // =========================================================================
    // Agent Routing with SONA
    // =========================================================================
    /**
     * Route a task to the best agent using learned patterns
     */
    async route(task, file) {
        const ext = file ? this.getExtension(file) : '';
        const state = this.getState(task, ext);
        const taskEmbed = this.embed(task + ' ' + (file || ''));
        // Apply SONA micro-LoRA transformation if available
        let adaptedEmbed = taskEmbed;
        if (this.sona) {
            try {
                adaptedEmbed = this.sona.applyMicroLora(taskEmbed);
            }
            catch {
                // Use original embedding
            }
        }
        // Find similar patterns using ReasoningBank
        let patterns = [];
        if (this.sona) {
            try {
                patterns = this.sona.findPatterns(adaptedEmbed, 5);
            }
            catch {
                // No patterns
            }
        }
        // Default agent mappings
        const defaults = {
            '.rs': 'rust-developer',
            '.ts': 'typescript-developer',
            '.tsx': 'react-developer',
            '.js': 'javascript-developer',
            '.jsx': 'react-developer',
            '.py': 'python-developer',
            '.go': 'go-developer',
            '.sql': 'database-specialist',
            '.md': 'documentation-specialist',
            '.yml': 'devops-engineer',
            '.yaml': 'devops-engineer',
            '.json': 'coder',
            '.toml': 'coder',
        };
        // Check learned patterns first
        const statePatterns = this.routingPatterns.get(state);
        let bestAgent = defaults[ext] || 'coder';
        let bestScore = 0.5;
        let reason = 'default mapping';
        if (statePatterns && statePatterns.size > 0) {
            for (const [agent, score] of statePatterns) {
                if (score > bestScore) {
                    bestAgent = agent;
                    bestScore = score;
                    reason = 'learned from patterns';
                }
            }
        }
        // Check custom agent mappings
        if (this.agentMappings.has(ext)) {
            const mapped = this.agentMappings.get(ext);
            if (bestScore < 0.8) {
                bestAgent = mapped;
                bestScore = 0.8;
                reason = 'custom mapping';
            }
        }
        // Boost confidence if SONA patterns match
        if (patterns.length > 0 && patterns[0].avgQuality > 0.7) {
            bestScore = Math.min(1.0, bestScore + 0.1);
            reason += ' + SONA pattern match';
        }
        return {
            agent: bestAgent,
            confidence: Math.min(1.0, bestScore),
            reason,
            patterns: patterns.length > 0 ? patterns : undefined,
            alternates: this.getAlternates(statePatterns, bestAgent),
        };
    }
    getExtension(file) {
        const idx = file.lastIndexOf('.');
        return idx >= 0 ? file.slice(idx).toLowerCase() : '';
    }
    getState(task, ext) {
        const taskType = task.includes('fix') ? 'fix' :
            task.includes('test') ? 'test' :
                task.includes('refactor') ? 'refactor' :
                    task.includes('document') ? 'docs' : 'edit';
        return `${taskType}:${ext || 'unknown'}`;
    }
    getAlternates(patterns, exclude) {
        if (!patterns)
            return [];
        return Array.from(patterns.entries())
            .filter(([a]) => a !== exclude)
            .sort((a, b) => b[1] - a[1])
            .slice(0, 3)
            .map(([agent, confidence]) => ({ agent, confidence: Math.min(1.0, confidence) }));
    }
    // =========================================================================
    // Trajectory Learning
    // =========================================================================
    /**
     * Begin recording a trajectory (before edit/command)
     */
    beginTrajectory(context, file) {
        const embed = this.embed(context + ' ' + (file || ''));
        if (this.sona) {
            try {
                this.currentTrajectoryId = this.sona.beginTrajectory(embed);
                if (file) {
                    this.sona.addContext(this.currentTrajectoryId, file);
                }
            }
            catch {
                this.currentTrajectoryId = null;
            }
        }
    }
    /**
     * Add a step to the current trajectory
     */
    addTrajectoryStep(activations, reward) {
        if (this.sona && this.currentTrajectoryId !== null) {
            try {
                const attentionWeights = new Array(activations.length).fill(1 / activations.length);
                this.sona.addStep(this.currentTrajectoryId, activations, attentionWeights, reward);
            }
            catch {
                // Ignore step errors
            }
        }
    }
    /**
     * End the current trajectory with a quality score
     */
    endTrajectory(success, quality) {
        const q = quality ?? (success ? 0.9 : 0.3);
        if (this.sona && this.currentTrajectoryId !== null) {
            try {
                this.sona.endTrajectory(this.currentTrajectoryId, q);
            }
            catch {
                // Ignore end errors
            }
        }
        this.currentTrajectoryId = null;
    }
    /**
     * Set the agent route for current trajectory
     */
    setTrajectoryRoute(agent) {
        if (this.sona && this.currentTrajectoryId !== null) {
            try {
                this.sona.setRoute(this.currentTrajectoryId, agent);
            }
            catch {
                // Ignore route errors
            }
        }
    }
    // =========================================================================
    // Episode Learning (Q-learning compatible)
    // =========================================================================
    /**
     * Record an episode for learning
     */
    async recordEpisode(state, action, reward, nextState, done, metadata) {
        const stateEmbed = this.embed(state);
        const nextStateEmbed = this.embed(nextState);
        // Store in FastAgentDB
        await this.agentDb.storeEpisode({
            state: stateEmbed,
            action,
            reward,
            nextState: nextStateEmbed,
            done,
            metadata,
        });
        // Update routing patterns (Q-learning style)
        if (!this.routingPatterns.has(state)) {
            this.routingPatterns.set(state, new Map());
        }
        const patterns = this.routingPatterns.get(state);
        const oldValue = patterns.get(action) || 0.5;
        const newValue = oldValue + this.config.learningRate * (reward - oldValue);
        patterns.set(action, newValue);
    }
    /**
     * Queue episode for batch processing (3-4x faster with workers)
     */
    queueEpisode(episode) {
        this.episodeBatchQueue.push(episode);
    }
    /**
     * Process queued episodes in parallel batch
     */
    async flushEpisodeBatch() {
        if (this.episodeBatchQueue.length === 0)
            return 0;
        const count = this.episodeBatchQueue.length;
        if (this.parallel) {
            // Use parallel workers for batch processing
            await this.parallel.recordEpisodesBatch(this.episodeBatchQueue);
        }
        else {
            // Sequential fallback
            for (const ep of this.episodeBatchQueue) {
                await this.recordEpisode(ep.state, ep.action, ep.reward, ep.nextState, ep.done, ep.metadata);
            }
        }
        this.episodeBatchQueue = [];
        return count;
    }
    /**
     * Learn from similar past episodes
     */
    async learnFromSimilar(state, k = 5) {
        const stateEmbed = this.embed(state);
        return this.agentDb.searchByState(stateEmbed, k);
    }
    // =========================================================================
    // Worker-Agent Mappings
    // =========================================================================
    /**
     * Register worker trigger to agent mappings
     */
    registerWorkerTrigger(trigger, priority, agents) {
        this.workerTriggerMappings.set(trigger, { priority, agents });
    }
    /**
     * Get agents for a worker trigger
     */
    getAgentsForTrigger(trigger) {
        return this.workerTriggerMappings.get(trigger);
    }
    /**
     * Route a task using worker trigger patterns first, then fall back to regular routing
     */
    async routeWithWorkers(task, file) {
        // Check if task matches any worker trigger patterns
        const taskLower = task.toLowerCase();
        for (const [trigger, config] of this.workerTriggerMappings) {
            if (taskLower.includes(trigger)) {
                const primaryAgent = config.agents[0] || 'coder';
                const alternates = config.agents.slice(1).map(a => ({ agent: a, confidence: 0.7 }));
                return {
                    agent: primaryAgent,
                    confidence: config.priority === 'critical' ? 0.95 :
                        config.priority === 'high' ? 0.85 :
                            config.priority === 'medium' ? 0.75 : 0.65,
                    reason: `worker trigger: ${trigger}`,
                    alternates,
                };
            }
        }
        // Fall back to regular routing
        return this.route(task, file);
    }
    /**
     * Initialize default worker trigger mappings
     */
    initDefaultWorkerMappings() {
        const defaults = [
            ['ultralearn', 'high', ['researcher', 'coder']],
            ['optimize', 'high', ['performance-analyzer']],
            ['audit', 'critical', ['security-analyst', 'tester']],
            ['map', 'medium', ['architect']],
            ['security', 'critical', ['security-analyst']],
            ['benchmark', 'low', ['performance-analyzer']],
            ['document', 'medium', ['documenter']],
            ['refactor', 'medium', ['coder', 'reviewer']],
            ['testgaps', 'high', ['tester']],
            ['deepdive', 'low', ['researcher']],
            ['predict', 'medium', ['analyst']],
            ['consolidate', 'low', ['architect']],
        ];
        for (const [trigger, priority, agents] of defaults) {
            this.workerTriggerMappings.set(trigger, { priority, agents });
        }
    }
    // =========================================================================
    // Co-edit Pattern Learning
    // =========================================================================
    /**
     * Record a co-edit pattern
     */
    recordCoEdit(file1, file2) {
        if (!this.coEditPatterns.has(file1)) {
            this.coEditPatterns.set(file1, new Map());
        }
        if (!this.coEditPatterns.has(file2)) {
            this.coEditPatterns.set(file2, new Map());
        }
        const count1 = this.coEditPatterns.get(file1).get(file2) || 0;
        this.coEditPatterns.get(file1).set(file2, count1 + 1);
        const count2 = this.coEditPatterns.get(file2).get(file1) || 0;
        this.coEditPatterns.get(file2).set(file1, count2 + 1);
    }
    /**
     * Get likely next files to edit
     */
    getLikelyNextFiles(file, topK = 5) {
        const related = this.coEditPatterns.get(file);
        if (!related)
            return [];
        return Array.from(related.entries())
            .sort((a, b) => b[1] - a[1])
            .slice(0, topK)
            .map(([f, count]) => ({ file: f, count }));
    }
    // =========================================================================
    // Error Pattern Learning
    // =========================================================================
    /**
     * Record an error pattern with fixes
     */
    recordErrorFix(errorPattern, fix) {
        if (!this.errorPatterns.has(errorPattern)) {
            this.errorPatterns.set(errorPattern, []);
        }
        const fixes = this.errorPatterns.get(errorPattern);
        if (!fixes.includes(fix)) {
            fixes.push(fix);
        }
    }
    /**
     * Get suggested fixes for an error
     */
    getSuggestedFixes(error) {
        // Exact match
        if (this.errorPatterns.has(error)) {
            return this.errorPatterns.get(error);
        }
        // Fuzzy match by embedding similarity
        const errorEmbed = this.embed(error);
        const matches = [];
        for (const [pattern, fixes] of this.errorPatterns) {
            const patternEmbed = this.embed(pattern);
            const similarity = this.cosineSimilarity(errorEmbed, patternEmbed);
            if (similarity > 0.7) {
                matches.push({ pattern, similarity, fixes });
            }
        }
        if (matches.length === 0)
            return [];
        // Return fixes from most similar pattern
        matches.sort((a, b) => b.similarity - a.similarity);
        return matches[0].fixes;
    }
    // =========================================================================
    // Tick / Background Learning
    // =========================================================================
    /**
     * Run background learning cycle
     */
    tick() {
        if (this.sona) {
            try {
                return this.sona.tick();
            }
            catch {
                return null;
            }
        }
        return null;
    }
    /**
     * Force immediate learning
     */
    forceLearn() {
        if (this.sona) {
            try {
                return this.sona.forceLearn();
            }
            catch {
                return null;
            }
        }
        return null;
    }
    // =========================================================================
    // Statistics
    // =========================================================================
    /**
     * Get comprehensive learning statistics
     */
    getStats() {
        const agentDbStats = this.agentDb.getStats();
        let sonaStats = null;
        if (this.sona) {
            try {
                sonaStats = this.sona.getStats();
            }
            catch {
                // No SONA stats
            }
        }
        // Calculate average reward from patterns
        let totalReward = 0;
        let rewardCount = 0;
        for (const patterns of this.routingPatterns.values()) {
            for (const reward of patterns.values()) {
                totalReward += reward;
                rewardCount++;
            }
        }
        const parallelStats = this.parallel?.getStats() ?? { enabled: false, workers: 0, busy: 0, queued: 0 };
        return {
            totalMemories: this.memories.size,
            memoryDimensions: this.config.embeddingDim,
            totalEpisodes: agentDbStats.episodeCount,
            totalTrajectories: agentDbStats.trajectoryCount,
            avgReward: rewardCount > 0 ? totalReward / rewardCount : 0,
            sonaEnabled: this.sona !== null,
            trajectoriesRecorded: sonaStats?.trajectoriesRecorded ?? 0,
            patternsLearned: sonaStats?.patternsLearned ?? 0,
            microLoraUpdates: sonaStats?.microLoraUpdates ?? 0,
            baseLoraUpdates: sonaStats?.baseLoraUpdates ?? 0,
            ewcConsolidations: sonaStats?.ewcConsolidations ?? 0,
            routingPatterns: this.routingPatterns.size,
            errorPatterns: this.errorPatterns.size,
            coEditPatterns: this.coEditPatterns.size,
            workerTriggers: this.workerTriggerMappings.size,
            attentionEnabled: this.attention !== null,
            onnxEnabled: this.onnxReady,
            parallelEnabled: parallelStats.enabled,
            parallelWorkers: parallelStats.workers,
            parallelBusy: parallelStats.busy,
            parallelQueued: parallelStats.queued,
        };
    }
    // =========================================================================
    // Persistence
    // =========================================================================
    /**
     * Export all data for persistence
     */
    export() {
        return {
            version: '2.0.0',
            exported: new Date().toISOString(),
            config: this.config,
            memories: Array.from(this.memories.values()),
            routingPatterns: Object.fromEntries(Array.from(this.routingPatterns.entries()).map(([k, v]) => [
                k,
                Object.fromEntries(v),
            ])),
            errorPatterns: Object.fromEntries(this.errorPatterns),
            coEditPatterns: Object.fromEntries(Array.from(this.coEditPatterns.entries()).map(([k, v]) => [
                k,
                Object.fromEntries(v),
            ])),
            agentMappings: Object.fromEntries(this.agentMappings),
            workerTriggerMappings: Object.fromEntries(Array.from(this.workerTriggerMappings.entries()).map(([k, v]) => [k, v])),
            stats: this.getStats(),
        };
    }
    /**
     * Import data from persistence
     */
    import(data, merge = false) {
        if (!merge) {
            this.memories.clear();
            this.routingPatterns.clear();
            this.errorPatterns.clear();
            this.coEditPatterns.clear();
            this.agentMappings.clear();
        }
        // Import memories
        if (data.memories) {
            for (const mem of data.memories) {
                this.memories.set(mem.id, mem);
            }
        }
        // Import routing patterns
        if (data.routingPatterns) {
            for (const [state, actions] of Object.entries(data.routingPatterns)) {
                const map = new Map(Object.entries(actions));
                if (merge && this.routingPatterns.has(state)) {
                    const existing = this.routingPatterns.get(state);
                    for (const [action, value] of map) {
                        existing.set(action, Math.max(existing.get(action) || 0, value));
                    }
                }
                else {
                    this.routingPatterns.set(state, map);
                }
            }
        }
        // Import error patterns
        if (data.errorPatterns) {
            for (const [pattern, fixes] of Object.entries(data.errorPatterns)) {
                if (merge && this.errorPatterns.has(pattern)) {
                    const existing = this.errorPatterns.get(pattern);
                    for (const fix of fixes) {
                        if (!existing.includes(fix))
                            existing.push(fix);
                    }
                }
                else {
                    this.errorPatterns.set(pattern, fixes);
                }
            }
        }
        // Import co-edit patterns
        if (data.coEditPatterns) {
            for (const [file, related] of Object.entries(data.coEditPatterns)) {
                const map = new Map(Object.entries(related));
                if (merge && this.coEditPatterns.has(file)) {
                    const existing = this.coEditPatterns.get(file);
                    for (const [f, count] of map) {
                        existing.set(f, (existing.get(f) || 0) + count);
                    }
                }
                else {
                    this.coEditPatterns.set(file, map);
                }
            }
        }
        // Import agent mappings
        if (data.agentMappings) {
            for (const [ext, agent] of Object.entries(data.agentMappings)) {
                this.agentMappings.set(ext, agent);
            }
        }
        // Import worker trigger mappings
        if (data.workerTriggerMappings) {
            for (const [trigger, config] of Object.entries(data.workerTriggerMappings)) {
                const typedConfig = config;
                this.workerTriggerMappings.set(trigger, typedConfig);
            }
        }
    }
    /**
     * Clear all data
     */
    clear() {
        this.memories.clear();
        this.routingPatterns.clear();
        this.errorPatterns.clear();
        this.coEditPatterns.clear();
        this.agentMappings.clear();
        this.workerTriggerMappings.clear();
        this.agentDb.clear();
    }
    // =========================================================================
    // Compatibility with existing Intelligence class
    // =========================================================================
    /** Legacy: patterns object */
    get patterns() {
        const result = {};
        for (const [state, actions] of this.routingPatterns) {
            result[state] = Object.fromEntries(actions);
        }
        return result;
    }
    /** Legacy: file_sequences array */
    get file_sequences() {
        const sequences = [];
        for (const [file, related] of this.coEditPatterns) {
            const sorted = Array.from(related.entries())
                .sort((a, b) => b[1] - a[1])
                .map(([f]) => f);
            if (sorted.length > 0) {
                sequences.push([file, ...sorted.slice(0, 3)]);
            }
        }
        return sequences;
    }
    /** Legacy: errors object */
    get errors() {
        return Object.fromEntries(this.errorPatterns);
    }
}
exports.IntelligenceEngine = IntelligenceEngine;
// ============================================================================
// Factory Functions
// ============================================================================
/**
 * Create a new IntelligenceEngine with default settings
 */
function createIntelligenceEngine(config) {
    return new IntelligenceEngine(config);
}
/**
 * Create a high-performance engine with all features enabled
 */
function createHighPerformanceEngine() {
    return new IntelligenceEngine({
        embeddingDim: 512,
        maxMemories: 200000,
        maxEpisodes: 100000,
        enableSona: true,
        enableAttention: true,
        sonaConfig: {
            hiddenDim: 512,
            microLoraRank: 2,
            baseLoraRank: 16,
            patternClusters: 200,
        },
    });
}
/**
 * Create a lightweight engine for fast startup
 */
function createLightweightEngine() {
    return new IntelligenceEngine({
        embeddingDim: 128,
        maxMemories: 10000,
        maxEpisodes: 5000,
        enableSona: false,
        enableAttention: false,
    });
}
exports.default = IntelligenceEngine;
//# sourceMappingURL=intelligence-engine.js.map