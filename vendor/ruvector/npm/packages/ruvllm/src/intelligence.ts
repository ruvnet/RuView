/**
 * External Intelligence Providers for SONA Learning (ADR-043)
 *
 * TypeScript bindings for the IntelligenceProvider trait, enabling
 * external systems to feed quality signals into RuvLLM's learning loops.
 *
 * @example
 * ```typescript
 * import { IntelligenceLoader, FileSignalProvider, QualitySignal } from '@ruvector/ruvllm';
 *
 * const loader = new IntelligenceLoader();
 * loader.registerProvider(new FileSignalProvider('./signals.json'));
 *
 * const { signals, errors } = loader.loadAllSignals();
 * console.log(`Loaded ${signals.length} signals`);
 * ```
 */

import * as fs from 'fs';
import * as path from 'path';

/** Maximum signal file size (10 MiB) */
const MAX_SIGNAL_FILE_SIZE = 10 * 1024 * 1024;

/** Maximum number of signals per file */
const MAX_SIGNALS_PER_FILE = 10_000;

/** Valid outcome values */
const VALID_OUTCOMES = new Set(['success', 'partial_success', 'failure']);

/** Valid human verdict values */
const VALID_VERDICTS = new Set(['approved', 'rejected']);

/**
 * A quality signal from an external system.
 *
 * Represents one completed task with quality assessment data
 * that can feed into SONA trajectories, the embedding classifier,
 * and model router calibration.
 */
export interface QualitySignal {
  /** Unique identifier for this signal */
  id: string;
  /** Human-readable task description (used for embedding generation) */
  taskDescription: string;
  /** Execution outcome */
  outcome: 'success' | 'partial_success' | 'failure';
  /** Composite quality score (0.0 - 1.0) */
  qualityScore: number;
  /** Optional human verdict */
  humanVerdict?: 'approved' | 'rejected';
  /** Optional structured quality factors for detailed analysis */
  qualityFactors?: QualityFactors;
  /** ISO 8601 timestamp of task completion */
  completedAt: string;
}

/**
 * Granular quality factor breakdown.
 *
 * Not all providers will have all factors. Undefined fields mean
 * "not assessed" (distinct from 0.0, which means "assessed as zero").
 */
export interface QualityFactors {
  acceptanceCriteriaMet?: number;
  testsPassing?: number;
  noRegressions?: number;
  lintClean?: number;
  typeCheckClean?: number;
  followsPatterns?: number;
  contextRelevance?: number;
  reasoningCoherence?: number;
  executionEfficiency?: number;
}

/**
 * Quality weight overrides from a provider.
 *
 * Weights should sum to approximately 1.0.
 */
export interface ProviderQualityWeights {
  taskCompletion: number;
  codeQuality: number;
  process: number;
}

/**
 * Error from a single provider during batch loading.
 */
export interface ProviderError {
  providerName: string;
  message: string;
}

/**
 * Result from a single provider during grouped loading.
 */
export interface ProviderResult {
  providerName: string;
  signals: QualitySignal[];
  weights?: ProviderQualityWeights;
}

/**
 * Interface for external systems that supply quality signals to RuvLLM.
 *
 * Implement this interface and register with IntelligenceLoader.
 */
export interface IntelligenceProvider {
  /** Human-readable name for this provider */
  name(): string;
  /** Load quality signals from this provider's data source */
  loadSignals(): QualitySignal[];
  /** Optional quality weight overrides */
  qualityWeights?(): ProviderQualityWeights | undefined;
}

function asOptionalNumber(val: unknown): number | undefined {
  if (val === undefined || val === null) return undefined;
  const n = Number(val);
  return Number.isFinite(n) && n >= 0 && n <= 1 ? n : undefined;
}

function validateOutcome(val: unknown): QualitySignal['outcome'] {
  const s = String(val ?? 'failure');
  return VALID_OUTCOMES.has(s) ? s as QualitySignal['outcome'] : 'failure';
}

function validateVerdict(val: unknown): QualitySignal['humanVerdict'] | undefined {
  if (val === undefined || val === null) return undefined;
  const s = String(val);
  return VALID_VERDICTS.has(s) ? s as QualitySignal['humanVerdict'] : undefined;
}

function validateScore(val: unknown): number {
  const n = Number(val ?? 0);
  if (!Number.isFinite(n) || n < 0 || n > 1) return 0;
  return n;
}

function mapQualityFactors(raw: Record<string, unknown>): QualityFactors {
  return {
    acceptanceCriteriaMet: asOptionalNumber(raw.acceptance_criteria_met),
    testsPassing: asOptionalNumber(raw.tests_passing),
    noRegressions: asOptionalNumber(raw.no_regressions),
    lintClean: asOptionalNumber(raw.lint_clean),
    typeCheckClean: asOptionalNumber(raw.type_check_clean),
    followsPatterns: asOptionalNumber(raw.follows_patterns),
    contextRelevance: asOptionalNumber(raw.context_relevance),
    reasoningCoherence: asOptionalNumber(raw.reasoning_coherence),
    executionEfficiency: asOptionalNumber(raw.execution_efficiency),
  };
}

/**
 * Built-in file-based intelligence provider.
 *
 * Reads quality signals from a JSON file. This is the default provider
 * for non-Rust integrations that write signal files.
 */
export class FileSignalProvider implements IntelligenceProvider {
  private readonly filePath: string;

  constructor(filePath: string) {
    this.filePath = path.resolve(filePath);
  }

  name(): string {
    return 'file-signals';
  }

  loadSignals(): QualitySignal[] {
    if (!fs.existsSync(this.filePath)) {
      return [];
    }

    // Check file size before reading (prevent OOM)
    const stat = fs.statSync(this.filePath);
    if (stat.size > MAX_SIGNAL_FILE_SIZE) {
      throw new Error(
        `Signal file exceeds max size (${stat.size} bytes, limit ${MAX_SIGNAL_FILE_SIZE})`
      );
    }

    const raw = fs.readFileSync(this.filePath, 'utf-8');
    const data: unknown = JSON.parse(raw);
    if (!Array.isArray(data)) {
      return [];
    }

    // Check signal count
    if (data.length > MAX_SIGNALS_PER_FILE) {
      throw new Error(
        `Signal file contains ${data.length} signals, max is ${MAX_SIGNALS_PER_FILE}`
      );
    }

    return data.map((item: Record<string, unknown>) => {
      const qfRaw = (item.quality_factors ?? item.qualityFactors) as Record<string, unknown> | undefined;
      return {
        id: String(item.id ?? ''),
        taskDescription: String(item.task_description ?? item.taskDescription ?? ''),
        outcome: validateOutcome(item.outcome),
        qualityScore: validateScore(item.quality_score ?? item.qualityScore),
        humanVerdict: validateVerdict(item.human_verdict ?? item.humanVerdict),
        qualityFactors: qfRaw ? mapQualityFactors(qfRaw) : undefined,
        completedAt: String(item.completed_at ?? item.completedAt ?? new Date().toISOString()),
      };
    });
  }

  qualityWeights(): ProviderQualityWeights | undefined {
    try {
      const weightsPath = path.join(path.dirname(this.filePath), 'quality-weights.json');
      if (!fs.existsSync(weightsPath)) return undefined;
      const raw = fs.readFileSync(weightsPath, 'utf-8');
      const data = JSON.parse(raw) as Record<string, unknown>;
      return {
        taskCompletion: Number(data.task_completion ?? data.taskCompletion ?? 0.5),
        codeQuality: Number(data.code_quality ?? data.codeQuality ?? 0.3),
        process: Number(data.process ?? 0.2),
      };
    } catch {
      return undefined;
    }
  }
}

/**
 * Aggregates quality signals from multiple registered providers.
 *
 * If no providers are registered, loadAllSignals returns empty arrays
 * with zero overhead.
 */
export class IntelligenceLoader {
  private providers: IntelligenceProvider[] = [];

  /** Register an external intelligence provider */
  registerProvider(provider: IntelligenceProvider): void {
    this.providers.push(provider);
  }

  /** Returns the number of registered providers */
  get providerCount(): number {
    return this.providers.length;
  }

  /** Returns the names of all registered providers */
  get providerNames(): string[] {
    return this.providers.map(p => p.name());
  }

  /**
   * Load signals from all registered providers.
   *
   * Non-fatal: if a provider fails, its error is captured but
   * other providers continue loading.
   */
  loadAllSignals(): { signals: QualitySignal[]; errors: ProviderError[] } {
    const signals: QualitySignal[] = [];
    const errors: ProviderError[] = [];

    for (const provider of this.providers) {
      try {
        const providerSignals = provider.loadSignals();
        signals.push(...providerSignals);
      } catch (e) {
        errors.push({
          providerName: provider.name(),
          message: e instanceof Error ? e.message : String(e),
        });
      }
    }

    return { signals, errors };
  }

  /** Load signals grouped by provider with weight overrides */
  loadGrouped(): ProviderResult[] {
    return this.providers.map(provider => {
      let providerSignals: QualitySignal[] = [];
      try {
        providerSignals = provider.loadSignals();
      } catch {
        // Non-fatal
      }
      return {
        providerName: provider.name(),
        signals: providerSignals,
        weights: provider.qualityWeights?.(),
      };
    });
  }
}
