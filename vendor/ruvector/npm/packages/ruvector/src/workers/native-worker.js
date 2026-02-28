"use strict";
/**
 * Native Worker Runner for RuVector
 *
 * Direct integration with:
 * - ONNX embedder (384d, SIMD-accelerated)
 * - VectorDB (HNSW indexing)
 * - Intelligence engine (Q-learning, memory)
 *
 * No delegation to external tools - pure ruvector execution.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.NativeWorker = void 0;
exports.createSecurityWorker = createSecurityWorker;
exports.createAnalysisWorker = createAnalysisWorker;
exports.createLearningWorker = createLearningWorker;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const glob_1 = require("glob");
const onnx_embedder_1 = require("../core/onnx-embedder");
const security_1 = require("../analysis/security");
const complexity_1 = require("../analysis/complexity");
const patterns_1 = require("../analysis/patterns");
// Lazy imports for optional dependencies
let VectorDb = null;
let intelligence = null;
async function loadOptionalDeps() {
    try {
        const core = await Promise.resolve().then(() => __importStar(require('@ruvector/core')));
        VectorDb = core.VectorDb;
    }
    catch {
        // VectorDB not available
    }
    try {
        const intel = await Promise.resolve().then(() => __importStar(require('../core/intelligence-engine')));
        intelligence = intel;
    }
    catch {
        // Intelligence not available
    }
}
/**
 * Native Worker Runner
 */
class NativeWorker {
    constructor(config) {
        this.vectorDb = null;
        this.findings = [];
        this.stats = {
            filesAnalyzed: 0,
            patternsFound: 0,
            embeddingsGenerated: 0,
            vectorsStored: 0,
        };
        this.config = config;
    }
    /**
     * Initialize worker with capabilities
     */
    async init() {
        await loadOptionalDeps();
        // Initialize ONNX embedder if needed
        if (this.config.capabilities?.onnxEmbeddings) {
            await (0, onnx_embedder_1.initOnnxEmbedder)();
        }
        // Initialize VectorDB if needed
        if (this.config.capabilities?.vectorDb && VectorDb) {
            const dbPath = path.join(process.cwd(), '.ruvector', 'workers', `${this.config.name}.db`);
            fs.mkdirSync(path.dirname(dbPath), { recursive: true });
            this.vectorDb = new VectorDb({
                dimensions: 384,
                storagePath: dbPath,
            });
        }
    }
    /**
     * Run all phases in sequence
     */
    async run(targetPath = '.') {
        const startTime = performance.now();
        const phaseResults = [];
        await this.init();
        let context = { targetPath, files: [], patterns: [], embeddings: [] };
        for (const phaseConfig of this.config.phases) {
            const phaseStart = performance.now();
            try {
                context = await this.executePhase(phaseConfig.type, context, phaseConfig.config);
                phaseResults.push({
                    phase: phaseConfig.type,
                    success: true,
                    data: this.summarizePhaseData(phaseConfig.type, context),
                    timeMs: performance.now() - phaseStart,
                });
            }
            catch (error) {
                phaseResults.push({
                    phase: phaseConfig.type,
                    success: false,
                    data: null,
                    timeMs: performance.now() - phaseStart,
                    error: error.message,
                });
                // Continue to next phase on error (fault-tolerant)
            }
        }
        const totalTimeMs = performance.now() - startTime;
        return {
            worker: this.config.name,
            success: phaseResults.every(p => p.success),
            phases: phaseResults,
            totalTimeMs,
            summary: {
                filesAnalyzed: this.stats.filesAnalyzed,
                patternsFound: this.stats.patternsFound,
                embeddingsGenerated: this.stats.embeddingsGenerated,
                vectorsStored: this.stats.vectorsStored,
                findings: this.findings,
            },
        };
    }
    /**
     * Execute a single phase
     */
    async executePhase(type, context, config) {
        switch (type) {
            case 'file-discovery':
                return this.phaseFileDiscovery(context, config);
            case 'pattern-extraction':
                return this.phasePatternExtraction(context, config);
            case 'embedding-generation':
                return this.phaseEmbeddingGeneration(context, config);
            case 'vector-storage':
                return this.phaseVectorStorage(context, config);
            case 'similarity-search':
                return this.phaseSimilaritySearch(context, config);
            case 'security-scan':
                return this.phaseSecurityScan(context, config);
            case 'complexity-analysis':
                return this.phaseComplexityAnalysis(context, config);
            case 'summarization':
                return this.phaseSummarization(context, config);
            default:
                throw new Error(`Unknown phase: ${type}`);
        }
    }
    /**
     * Phase: File Discovery
     */
    async phaseFileDiscovery(context, config) {
        const patterns = config?.patterns || ['**/*.ts', '**/*.js', '**/*.tsx', '**/*.jsx'];
        const exclude = config?.exclude || ['**/node_modules/**', '**/dist/**', '**/.git/**'];
        const files = [];
        for (const pattern of patterns) {
            const matches = await (0, glob_1.glob)(pattern, {
                cwd: context.targetPath,
                ignore: exclude,
                nodir: true,
            });
            files.push(...matches.map(f => path.join(context.targetPath, f)));
        }
        this.stats.filesAnalyzed = files.length;
        return { ...context, files };
    }
    /**
     * Phase: Pattern Extraction (uses shared analysis module)
     */
    async phasePatternExtraction(context, config) {
        const patterns = [];
        const patternTypes = config?.types || ['function', 'class', 'import', 'export', 'todo'];
        for (const file of context.files.slice(0, 100)) {
            try {
                const filePatterns = (0, patterns_1.extractAllPatterns)(file);
                const matches = (0, patterns_1.toPatternMatches)(filePatterns);
                // Filter by requested pattern types
                for (const match of matches) {
                    if (patternTypes.includes(match.type)) {
                        patterns.push(match);
                        // Add findings for TODOs
                        if (match.type === 'todo') {
                            this.findings.push({
                                type: 'info',
                                message: match.match,
                                file,
                            });
                        }
                    }
                }
            }
            catch {
                // Skip unreadable files
            }
        }
        this.stats.patternsFound = patterns.length;
        return { ...context, patterns };
    }
    /**
     * Phase: Embedding Generation (ONNX)
     */
    async phaseEmbeddingGeneration(context, config) {
        if (!(0, onnx_embedder_1.isReady)()) {
            await (0, onnx_embedder_1.initOnnxEmbedder)();
        }
        const embeddings = [];
        const batchSize = config?.batchSize || 32;
        // Collect texts to embed
        const texts = [];
        // Embed file content summaries
        for (const file of context.files.slice(0, 50)) {
            try {
                const content = fs.readFileSync(file, 'utf-8');
                const summary = content.slice(0, 512); // First 512 chars
                texts.push({ text: summary, file });
            }
            catch {
                // Skip
            }
        }
        // Embed patterns
        for (const pattern of context.patterns.slice(0, 100)) {
            texts.push({ text: pattern.match, file: pattern.file });
        }
        // Batch embed
        for (let i = 0; i < texts.length; i += batchSize) {
            const batch = texts.slice(i, i + batchSize);
            const results = await (0, onnx_embedder_1.embedBatch)(batch.map(t => t.text));
            for (let j = 0; j < results.length; j++) {
                embeddings.push({
                    text: batch[j].text,
                    embedding: results[j].embedding,
                    file: batch[j].file,
                });
            }
        }
        this.stats.embeddingsGenerated = embeddings.length;
        return { ...context, embeddings };
    }
    /**
     * Phase: Vector Storage
     */
    async phaseVectorStorage(context, config) {
        if (!this.vectorDb) {
            return context;
        }
        let stored = 0;
        for (const item of context.embeddings) {
            try {
                await this.vectorDb.insert({
                    vector: new Float32Array(item.embedding),
                    metadata: {
                        text: item.text.slice(0, 200),
                        file: item.file,
                        worker: this.config.name,
                        timestamp: Date.now(),
                    },
                });
                stored++;
            }
            catch {
                // Skip duplicates/errors
            }
        }
        this.stats.vectorsStored = stored;
        return context;
    }
    /**
     * Phase: Similarity Search
     */
    async phaseSimilaritySearch(context, config) {
        if (!this.vectorDb || context.embeddings.length === 0) {
            return context;
        }
        const query = config?.query || context.embeddings[0]?.text;
        if (!query)
            return context;
        const queryResult = await (0, onnx_embedder_1.embed)(query);
        const results = await this.vectorDb.search({
            vector: new Float32Array(queryResult.embedding),
            k: config?.k || 5,
        });
        return { ...context, searchResults: results };
    }
    /**
     * Phase: Security Scan (uses shared analysis module)
     */
    async phaseSecurityScan(context, config) {
        // Use consolidated security scanner
        const findings = (0, security_1.scanFiles)(context.files, undefined, 100);
        // Convert to worker findings format
        for (const finding of findings) {
            this.findings.push({
                type: 'security',
                message: `${finding.rule}: ${finding.message}`,
                file: finding.file,
                line: finding.line,
                severity: finding.severity === 'critical' ? 4 :
                    finding.severity === 'high' ? 3 :
                        finding.severity === 'medium' ? 2 : 1,
            });
        }
        return context;
    }
    /**
     * Phase: Complexity Analysis (uses shared analysis module)
     */
    async phaseComplexityAnalysis(context, config) {
        const complexityThreshold = config?.threshold || 10;
        const complexFiles = [];
        for (const file of context.files.slice(0, 50)) {
            // Use consolidated complexity analyzer
            const result = (0, complexity_1.analyzeFile)(file);
            if (result.cyclomaticComplexity > complexityThreshold) {
                complexFiles.push(result);
                const rating = (0, complexity_1.getComplexityRating)(result.cyclomaticComplexity);
                this.findings.push({
                    type: 'warning',
                    message: `High complexity: ${result.cyclomaticComplexity} (threshold: ${complexityThreshold})`,
                    file,
                    severity: rating === 'critical' ? 4 : rating === 'high' ? 3 : 2,
                });
            }
        }
        return { ...context, complexFiles };
    }
    /**
     * Phase: Summarization
     */
    async phaseSummarization(context, config) {
        const summary = {
            filesAnalyzed: context.files?.length || 0,
            patternsFound: context.patterns?.length || 0,
            embeddingsGenerated: context.embeddings?.length || 0,
            findingsCount: this.findings.length,
            findingsByType: {
                info: this.findings.filter(f => f.type === 'info').length,
                warning: this.findings.filter(f => f.type === 'warning').length,
                error: this.findings.filter(f => f.type === 'error').length,
                security: this.findings.filter(f => f.type === 'security').length,
            },
            topFindings: this.findings.slice(0, 10),
        };
        return { ...context, summary };
    }
    /**
     * Summarize phase data for results
     */
    summarizePhaseData(type, context) {
        switch (type) {
            case 'file-discovery':
                return { filesFound: context.files?.length || 0 };
            case 'pattern-extraction':
                return { patternsFound: context.patterns?.length || 0 };
            case 'embedding-generation':
                return { embeddingsGenerated: context.embeddings?.length || 0 };
            case 'vector-storage':
                return { vectorsStored: this.stats.vectorsStored };
            case 'similarity-search':
                return { resultsFound: context.searchResults?.length || 0 };
            case 'security-scan':
                return { securityFindings: this.findings.filter(f => f.type === 'security').length };
            case 'complexity-analysis':
                return { complexFiles: context.complexFiles?.length || 0 };
            case 'summarization':
                return context.summary;
            default:
                return {};
        }
    }
}
exports.NativeWorker = NativeWorker;
/**
 * Quick worker factory functions
 */
function createSecurityWorker(name = 'security-scanner') {
    return new NativeWorker({
        name,
        description: 'Security vulnerability scanner',
        phases: [
            { type: 'file-discovery', config: { patterns: ['**/*.ts', '**/*.js', '**/*.tsx', '**/*.jsx'] } },
            { type: 'security-scan' },
            { type: 'summarization' },
        ],
        capabilities: { onnxEmbeddings: false, vectorDb: false },
    });
}
function createAnalysisWorker(name = 'code-analyzer') {
    return new NativeWorker({
        name,
        description: 'Code analysis with embeddings',
        phases: [
            { type: 'file-discovery' },
            { type: 'pattern-extraction' },
            { type: 'embedding-generation' },
            { type: 'vector-storage' },
            { type: 'complexity-analysis' },
            { type: 'summarization' },
        ],
        capabilities: { onnxEmbeddings: true, vectorDb: true },
    });
}
function createLearningWorker(name = 'pattern-learner') {
    return new NativeWorker({
        name,
        description: 'Pattern learning with vector storage',
        phases: [
            { type: 'file-discovery' },
            { type: 'pattern-extraction' },
            { type: 'embedding-generation' },
            { type: 'vector-storage' },
            { type: 'summarization' },
        ],
        capabilities: { onnxEmbeddings: true, vectorDb: true, intelligenceMemory: true },
    });
}
//# sourceMappingURL=native-worker.js.map