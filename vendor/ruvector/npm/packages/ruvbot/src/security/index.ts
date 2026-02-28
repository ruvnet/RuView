/**
 * Security Context - AI Defense and Protection
 *
 * Provides production-ready adversarial defense through aidefence integration.
 * Features: prompt injection detection, jailbreak prevention, PII masking.
 */

export {
  AIDefenceGuard,
  createAIDefenceGuard,
  createAIDefenceMiddleware,
  createStrictConfig,
  createPermissiveConfig,
  DEFAULT_AIDEFENCE_CONFIG,
  AIDefenceConfigSchema,
  type AIDefenceConfig,
  type DetectionResult,
  type ThreatInfo,
  type ThreatLevel,
  type ThreatType,
  type AnalysisContext,
  type AuditEntry,
} from './AIDefenceGuard.js';
