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
/**
 * Built-in file-based intelligence provider.
 *
 * Reads quality signals from a JSON file. This is the default provider
 * for non-Rust integrations that write signal files.
 */
export declare class FileSignalProvider implements IntelligenceProvider {
    private readonly filePath;
    constructor(filePath: string);
    name(): string;
    loadSignals(): QualitySignal[];
    qualityWeights(): ProviderQualityWeights | undefined;
}
/**
 * Aggregates quality signals from multiple registered providers.
 *
 * If no providers are registered, loadAllSignals returns empty arrays
 * with zero overhead.
 */
export declare class IntelligenceLoader {
    private providers;
    /** Register an external intelligence provider */
    registerProvider(provider: IntelligenceProvider): void;
    /** Returns the number of registered providers */
    get providerCount(): number;
    /** Returns the names of all registered providers */
    get providerNames(): string[];
    /**
     * Load signals from all registered providers.
     *
     * Non-fatal: if a provider fails, its error is captured but
     * other providers continue loading.
     */
    loadAllSignals(): {
        signals: QualitySignal[];
        errors: ProviderError[];
    };
    /** Load signals grouped by provider with weight overrides */
    loadGrouped(): ProviderResult[];
}
//# sourceMappingURL=intelligence.d.ts.map