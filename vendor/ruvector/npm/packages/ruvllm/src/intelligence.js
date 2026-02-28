"use strict";
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
exports.IntelligenceLoader = exports.FileSignalProvider = void 0;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
/** Maximum signal file size (10 MiB) */
const MAX_SIGNAL_FILE_SIZE = 10 * 1024 * 1024;
/** Maximum number of signals per file */
const MAX_SIGNALS_PER_FILE = 10000;
/** Valid outcome values */
const VALID_OUTCOMES = new Set(['success', 'partial_success', 'failure']);
/** Valid human verdict values */
const VALID_VERDICTS = new Set(['approved', 'rejected']);
function asOptionalNumber(val) {
    if (val === undefined || val === null)
        return undefined;
    const n = Number(val);
    return Number.isFinite(n) && n >= 0 && n <= 1 ? n : undefined;
}
function validateOutcome(val) {
    const s = String(val ?? 'failure');
    return VALID_OUTCOMES.has(s) ? s : 'failure';
}
function validateVerdict(val) {
    if (val === undefined || val === null)
        return undefined;
    const s = String(val);
    return VALID_VERDICTS.has(s) ? s : undefined;
}
function validateScore(val) {
    const n = Number(val ?? 0);
    if (!Number.isFinite(n) || n < 0 || n > 1)
        return 0;
    return n;
}
function mapQualityFactors(raw) {
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
class FileSignalProvider {
    constructor(filePath) {
        this.filePath = path.resolve(filePath);
    }
    name() {
        return 'file-signals';
    }
    loadSignals() {
        if (!fs.existsSync(this.filePath)) {
            return [];
        }
        // Check file size before reading (prevent OOM)
        const stat = fs.statSync(this.filePath);
        if (stat.size > MAX_SIGNAL_FILE_SIZE) {
            throw new Error(`Signal file exceeds max size (${stat.size} bytes, limit ${MAX_SIGNAL_FILE_SIZE})`);
        }
        const raw = fs.readFileSync(this.filePath, 'utf-8');
        const data = JSON.parse(raw);
        if (!Array.isArray(data)) {
            return [];
        }
        // Check signal count
        if (data.length > MAX_SIGNALS_PER_FILE) {
            throw new Error(`Signal file contains ${data.length} signals, max is ${MAX_SIGNALS_PER_FILE}`);
        }
        return data.map((item) => {
            const qfRaw = (item.quality_factors ?? item.qualityFactors);
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
    qualityWeights() {
        try {
            const weightsPath = path.join(path.dirname(this.filePath), 'quality-weights.json');
            if (!fs.existsSync(weightsPath))
                return undefined;
            const raw = fs.readFileSync(weightsPath, 'utf-8');
            const data = JSON.parse(raw);
            return {
                taskCompletion: Number(data.task_completion ?? data.taskCompletion ?? 0.5),
                codeQuality: Number(data.code_quality ?? data.codeQuality ?? 0.3),
                process: Number(data.process ?? 0.2),
            };
        }
        catch {
            return undefined;
        }
    }
}
exports.FileSignalProvider = FileSignalProvider;
/**
 * Aggregates quality signals from multiple registered providers.
 *
 * If no providers are registered, loadAllSignals returns empty arrays
 * with zero overhead.
 */
class IntelligenceLoader {
    constructor() {
        this.providers = [];
    }
    /** Register an external intelligence provider */
    registerProvider(provider) {
        this.providers.push(provider);
    }
    /** Returns the number of registered providers */
    get providerCount() {
        return this.providers.length;
    }
    /** Returns the names of all registered providers */
    get providerNames() {
        return this.providers.map(p => p.name());
    }
    /**
     * Load signals from all registered providers.
     *
     * Non-fatal: if a provider fails, its error is captured but
     * other providers continue loading.
     */
    loadAllSignals() {
        const signals = [];
        const errors = [];
        for (const provider of this.providers) {
            try {
                const providerSignals = provider.loadSignals();
                signals.push(...providerSignals);
            }
            catch (e) {
                errors.push({
                    providerName: provider.name(),
                    message: e instanceof Error ? e.message : String(e),
                });
            }
        }
        return { signals, errors };
    }
    /** Load signals grouped by provider with weight overrides */
    loadGrouped() {
        return this.providers.map(provider => {
            let providerSignals = [];
            try {
                providerSignals = provider.loadSignals();
            }
            catch {
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
exports.IntelligenceLoader = IntelligenceLoader;
//# sourceMappingURL=intelligence.js.map