/**
 * AIDefence Integration - Production-ready adversarial defense for RuvBot
 *
 * Integrates the aidefence package to provide:
 * - Prompt injection detection (<10ms)
 * - Jailbreak detection
 * - PII detection and sanitization
 * - Behavioral analysis
 * - Formal policy verification
 *
 * @see https://www.npmjs.com/package/aidefence
 */
import { z } from 'zod';
/**
 * Threat classification levels
 */
export type ThreatLevel = 'none' | 'low' | 'medium' | 'high' | 'critical';
/**
 * Types of threats detected
 */
export type ThreatType = 'prompt_injection' | 'jailbreak' | 'pii_exposure' | 'malicious_code' | 'data_exfiltration' | 'policy_violation' | 'anomalous_behavior' | 'control_character' | 'encoding_attack';
/**
 * Detection result from AIDefence
 */
export interface DetectionResult {
    safe: boolean;
    threatLevel: ThreatLevel;
    threats: ThreatInfo[];
    confidence: number;
    latencyMs: number;
    sanitizedInput?: string;
    metadata?: Record<string, unknown>;
}
/**
 * Individual threat information
 */
export interface ThreatInfo {
    type: ThreatType;
    severity: ThreatLevel;
    confidence: number;
    description: string;
    location?: {
        start: number;
        end: number;
        text: string;
    };
    mitigation?: string;
}
/**
 * AIDefence configuration
 */
export interface AIDefenceConfig {
    /** Enable prompt injection detection */
    detectPromptInjection: boolean;
    /** Enable jailbreak detection */
    detectJailbreak: boolean;
    /** Enable PII detection and masking */
    detectPII: boolean;
    /** Enable behavioral analysis */
    enableBehavioralAnalysis: boolean;
    /** Enable formal policy verification */
    enablePolicyVerification: boolean;
    /** Minimum threat level to block (default: 'medium') */
    blockThreshold: ThreatLevel;
    /** Custom detection patterns */
    customPatterns?: string[];
    /** Allowed domains for URLs */
    allowedDomains?: string[];
    /** Maximum input length (default: 100000) */
    maxInputLength: number;
    /** Enable audit logging */
    enableAuditLog: boolean;
    /** AIDefence server URL (if using external service) */
    serverUrl?: string;
}
/**
 * Validation schema for config
 */
export declare const AIDefenceConfigSchema: z.ZodObject<{
    detectPromptInjection: z.ZodDefault<z.ZodBoolean>;
    detectJailbreak: z.ZodDefault<z.ZodBoolean>;
    detectPII: z.ZodDefault<z.ZodBoolean>;
    enableBehavioralAnalysis: z.ZodDefault<z.ZodBoolean>;
    enablePolicyVerification: z.ZodDefault<z.ZodBoolean>;
    blockThreshold: z.ZodDefault<z.ZodEnum<["none", "low", "medium", "high", "critical"]>>;
    customPatterns: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    allowedDomains: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    maxInputLength: z.ZodDefault<z.ZodNumber>;
    enableAuditLog: z.ZodDefault<z.ZodBoolean>;
    serverUrl: z.ZodOptional<z.ZodString>;
}, "strip", z.ZodTypeAny, {
    detectPromptInjection: boolean;
    detectJailbreak: boolean;
    detectPII: boolean;
    enableBehavioralAnalysis: boolean;
    enablePolicyVerification: boolean;
    blockThreshold: "none" | "low" | "medium" | "high" | "critical";
    maxInputLength: number;
    enableAuditLog: boolean;
    customPatterns?: string[] | undefined;
    allowedDomains?: string[] | undefined;
    serverUrl?: string | undefined;
}, {
    detectPromptInjection?: boolean | undefined;
    detectJailbreak?: boolean | undefined;
    detectPII?: boolean | undefined;
    enableBehavioralAnalysis?: boolean | undefined;
    enablePolicyVerification?: boolean | undefined;
    blockThreshold?: "none" | "low" | "medium" | "high" | "critical" | undefined;
    customPatterns?: string[] | undefined;
    allowedDomains?: string[] | undefined;
    maxInputLength?: number | undefined;
    enableAuditLog?: boolean | undefined;
    serverUrl?: string | undefined;
}>;
export declare const DEFAULT_AIDEFENCE_CONFIG: AIDefenceConfig;
/**
 * AIDefenceGuard - Main class for AI security protection
 *
 * Provides multi-layered defense:
 * 1. Pattern-based detection (<5ms)
 * 2. PII detection and masking (<5ms)
 * 3. Control character sanitization (<1ms)
 * 4. Behavioral analysis (optional, <100ms)
 * 5. Policy verification (optional, <500ms)
 */
export declare class AIDefenceGuard {
    private config;
    private customPatterns;
    private auditLog;
    private behaviorBaseline;
    constructor(config?: Partial<AIDefenceConfig>);
    /**
     * Analyze input for threats
     */
    analyze(input: string, context?: AnalysisContext): Promise<DetectionResult>;
    /**
     * Sanitize input by removing/replacing dangerous content
     */
    sanitize(input: string): string;
    /**
     * Validate LLM response for safety
     */
    validateResponse(response: string, originalInput: string): Promise<DetectionResult>;
    /**
     * Get audit log entries
     */
    getAuditLog(): AuditEntry[];
    /**
     * Clear audit log
     */
    clearAuditLog(): void;
    private detectInjection;
    private detectJailbreak;
    private detectPII;
    private detectCustomPatterns;
    private analyzeBehavior;
    private getMaxSeverity;
    private maskPII;
    private logAudit;
}
export interface AnalysisContext {
    userId?: string;
    sessionId?: string;
    tenantId?: string;
    channel?: string;
    metadata?: Record<string, unknown>;
}
export interface AuditEntry {
    timestamp: Date;
    input: string;
    result: DetectionResult;
    context?: AnalysisContext;
}
/**
 * Create middleware for protecting LLM requests
 */
export declare function createAIDefenceMiddleware(config?: Partial<AIDefenceConfig>): {
    /**
     * Validate input before sending to LLM
     */
    validateInput(input: string, context?: AnalysisContext): Promise<{
        allowed: boolean;
        sanitizedInput: string;
        result: DetectionResult;
    }>;
    /**
     * Validate LLM response before returning to user
     */
    validateOutput(output: string, originalInput: string): Promise<{
        allowed: boolean;
        result: DetectionResult;
    }>;
    /**
     * Get the underlying guard instance
     */
    getGuard(): AIDefenceGuard;
};
/**
 * Create a new AIDefenceGuard instance
 */
export declare function createAIDefenceGuard(config?: Partial<AIDefenceConfig>): AIDefenceGuard;
/**
 * Create a strict security configuration
 */
export declare function createStrictConfig(): AIDefenceConfig;
/**
 * Create a permissive configuration (for development)
 */
export declare function createPermissiveConfig(): AIDefenceConfig;
export default AIDefenceGuard;
//# sourceMappingURL=AIDefenceGuard.d.ts.map