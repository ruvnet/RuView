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

import * as fs from 'fs';
import * as path from 'path';
import { glob } from 'glob';
import {
  WorkerConfig,
  WorkerResult,
  PhaseResult,
  PhaseType,
  WorkerSummary,
  Finding,
} from './types';
import { embed, embedBatch, initOnnxEmbedder, isReady, getStats } from '../core/onnx-embedder';
import { scanFiles, SecurityFinding } from '../analysis/security';
import { analyzeFile, ComplexityResult, getComplexityRating } from '../analysis/complexity';
import { extractAllPatterns, toPatternMatches, PatternMatch } from '../analysis/patterns';

// Lazy imports for optional dependencies
let VectorDb: any = null;
let intelligence: any = null;

async function loadOptionalDeps() {
  try {
    const core = await import('@ruvector/core');
    VectorDb = core.VectorDb;
  } catch {
    // VectorDB not available
  }

  try {
    const intel = await import('../core/intelligence-engine');
    intelligence = intel;
  } catch {
    // Intelligence not available
  }
}

/**
 * Native Worker Runner
 */
export class NativeWorker {
  private config: WorkerConfig;
  private vectorDb: any = null;
  private findings: Finding[] = [];
  private stats = {
    filesAnalyzed: 0,
    patternsFound: 0,
    embeddingsGenerated: 0,
    vectorsStored: 0,
  };

  constructor(config: WorkerConfig) {
    this.config = config;
  }

  /**
   * Initialize worker with capabilities
   */
  async init(): Promise<void> {
    await loadOptionalDeps();

    // Initialize ONNX embedder if needed
    if (this.config.capabilities?.onnxEmbeddings) {
      await initOnnxEmbedder();
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
  async run(targetPath: string = '.'): Promise<WorkerResult> {
    const startTime = performance.now();
    const phaseResults: PhaseResult[] = [];

    await this.init();

    let context: any = { targetPath, files: [], patterns: [], embeddings: [] };

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
      } catch (error: any) {
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
  private async executePhase(
    type: PhaseType,
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
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
  private async phaseFileDiscovery(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    const patterns = config?.patterns || ['**/*.ts', '**/*.js', '**/*.tsx', '**/*.jsx'];
    const exclude = config?.exclude || ['**/node_modules/**', '**/dist/**', '**/.git/**'];

    const files: string[] = [];
    for (const pattern of patterns) {
      const matches = await glob(pattern, {
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
  private async phasePatternExtraction(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    const patterns: PatternMatch[] = [];
    const patternTypes = config?.types || ['function', 'class', 'import', 'export', 'todo'];

    for (const file of context.files.slice(0, 100)) {
      try {
        const filePatterns = extractAllPatterns(file);
        const matches = toPatternMatches(filePatterns);

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
      } catch {
        // Skip unreadable files
      }
    }

    this.stats.patternsFound = patterns.length;
    return { ...context, patterns };
  }

  /**
   * Phase: Embedding Generation (ONNX)
   */
  private async phaseEmbeddingGeneration(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    if (!isReady()) {
      await initOnnxEmbedder();
    }

    const embeddings: Array<{ text: string; embedding: number[]; file?: string }> = [];
    const batchSize = config?.batchSize || 32;

    // Collect texts to embed
    const texts: Array<{ text: string; file?: string }> = [];

    // Embed file content summaries
    for (const file of context.files.slice(0, 50)) {
      try {
        const content = fs.readFileSync(file, 'utf-8');
        const summary = content.slice(0, 512); // First 512 chars
        texts.push({ text: summary, file });
      } catch {
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
      const results = await embedBatch(batch.map(t => t.text));

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
  private async phaseVectorStorage(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
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
      } catch {
        // Skip duplicates/errors
      }
    }

    this.stats.vectorsStored = stored;
    return context;
  }

  /**
   * Phase: Similarity Search
   */
  private async phaseSimilaritySearch(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    if (!this.vectorDb || context.embeddings.length === 0) {
      return context;
    }

    const query = config?.query || context.embeddings[0]?.text;
    if (!query) return context;

    const queryResult = await embed(query);
    const results = await this.vectorDb.search({
      vector: new Float32Array(queryResult.embedding),
      k: config?.k || 5,
    });

    return { ...context, searchResults: results };
  }

  /**
   * Phase: Security Scan (uses shared analysis module)
   */
  private async phaseSecurityScan(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    // Use consolidated security scanner
    const findings = scanFiles(context.files, undefined, 100);

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
  private async phaseComplexityAnalysis(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
    const complexityThreshold = config?.threshold || 10;
    const complexFiles: ComplexityResult[] = [];

    for (const file of context.files.slice(0, 50)) {
      // Use consolidated complexity analyzer
      const result = analyzeFile(file);

      if (result.cyclomaticComplexity > complexityThreshold) {
        complexFiles.push(result);
        const rating = getComplexityRating(result.cyclomaticComplexity);
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
  private async phaseSummarization(
    context: any,
    config?: Record<string, any>
  ): Promise<any> {
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
  private summarizePhaseData(type: PhaseType, context: any): any {
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

/**
 * Quick worker factory functions
 */
export function createSecurityWorker(name = 'security-scanner'): NativeWorker {
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

export function createAnalysisWorker(name = 'code-analyzer'): NativeWorker {
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

export function createLearningWorker(name = 'pattern-learner'): NativeWorker {
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
