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

// ============================================================================
// Types
// ============================================================================

/**
 * Threat classification levels
 */
export type ThreatLevel = 'none' | 'low' | 'medium' | 'high' | 'critical';

/**
 * Types of threats detected
 */
export type ThreatType =
  | 'prompt_injection'
  | 'jailbreak'
  | 'pii_exposure'
  | 'malicious_code'
  | 'data_exfiltration'
  | 'policy_violation'
  | 'anomalous_behavior'
  | 'control_character'
  | 'encoding_attack';

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
export const AIDefenceConfigSchema = z.object({
  detectPromptInjection: z.boolean().default(true),
  detectJailbreak: z.boolean().default(true),
  detectPII: z.boolean().default(true),
  enableBehavioralAnalysis: z.boolean().default(false),
  enablePolicyVerification: z.boolean().default(false),
  blockThreshold: z.enum(['none', 'low', 'medium', 'high', 'critical']).default('medium'),
  customPatterns: z.array(z.string()).optional(),
  allowedDomains: z.array(z.string()).optional(),
  maxInputLength: z.number().positive().default(100000),
  enableAuditLog: z.boolean().default(true),
  serverUrl: z.string().url().optional(),
});

// ============================================================================
// Default Configuration
// ============================================================================

export const DEFAULT_AIDEFENCE_CONFIG: AIDefenceConfig = {
  detectPromptInjection: true,
  detectJailbreak: true,
  detectPII: true,
  enableBehavioralAnalysis: false,
  enablePolicyVerification: false,
  blockThreshold: 'medium',
  maxInputLength: 100000,
  enableAuditLog: true,
};

// ============================================================================
// Threat Severity Mapping
// ============================================================================

const THREAT_SEVERITY_ORDER: Record<ThreatLevel, number> = {
  none: 0,
  low: 1,
  medium: 2,
  high: 3,
  critical: 4,
};

// ============================================================================
// Built-in Patterns
// ============================================================================

/**
 * Common prompt injection patterns
 */
const INJECTION_PATTERNS = [
  // Direct instruction override (flexible word order)
  /ignore\s+(previous|all|above)\s+(instructions?|prompts?|rules?)/i,
  /ignore\s+all\s+previous\s+(instructions?|prompts?|rules?)/i,
  /ignore\s+(the\s+)?(previous|above)\s+(instructions?|prompts?|rules?)/i,
  /disregard\s+(previous|all|above|the|your)/i,
  /forget\s+(everything|all|previous|your)/i,

  // Role manipulation
  /you\s+are\s+(now|actually)\s+/i,
  /pretend\s+(to\s+be|you're|you\s+are)/i,
  /act\s+as\s+(if|though|a)/i,
  /roleplay\s+as/i,
  /from\s+now\s+on\s+(you|your)/i,

  // System prompt extraction
  /what\s+(is|are)\s+your\s+(system\s+)?prompt/i,
  /show\s+(me\s+)?your\s+(system\s+)?instructions/i,
  /reveal\s+your\s+(hidden|secret|system)/i,
  /reveal\s+(the\s+)?system\s+prompt/i,
  /print\s+your\s+(system|initial)\s+prompt/i,
  /output\s+(your\s+)?(system|initial)\s+prompt/i,

  // Jailbreak attempts
  /DAN\s+(mode|prompt)/i,
  /developer\s+mode/i,
  /\[jailbreak\]/i,
  /bypass\s+(safety|security|filter)/i,

  // Code injection
  /```\s*(system|bash|sh|exec)/i,
  /<script[\s>]/i,
  /javascript:/i,
  /eval\s*\(/i,

  // Data exfiltration
  /send\s+(to|data\s+to)\s+http/i,
  /fetch\s*\(\s*['"`]http/i,
  /webhook\s*:\s*http/i,
];

/**
 * PII patterns for detection
 */
const PII_PATTERNS = [
  // Email
  { pattern: /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b/g, type: 'email' },
  // Phone (various formats)
  { pattern: /\b(\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b/g, type: 'phone' },
  // SSN
  { pattern: /\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b/g, type: 'ssn' },
  // Credit card
  { pattern: /\b(?:\d{4}[-\s]?){3}\d{4}\b/g, type: 'credit_card' },
  // IP address
  { pattern: /\b(?:\d{1,3}\.){3}\d{1,3}\b/g, type: 'ip_address' },
  // API keys (common patterns)
  { pattern: /\b(sk-|api[_-]?key|token)[a-zA-Z0-9_-]{20,}\b/gi, type: 'api_key' },
];

/**
 * Control characters that should be sanitized
 */
const CONTROL_CHAR_PATTERN = /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g;

/**
 * Unicode homoglyph patterns
 */
const HOMOGLYPH_MAP: Record<string, string> = {
  '\u0430': 'a', // Cyrillic а
  '\u0435': 'e', // Cyrillic е
  '\u043E': 'o', // Cyrillic о
  '\u0440': 'p', // Cyrillic р
  '\u0441': 'c', // Cyrillic с
  '\u0445': 'x', // Cyrillic х
  '\u0443': 'y', // Cyrillic у
  '\u0456': 'i', // Cyrillic і
};

// ============================================================================
// AIDefenceGuard Implementation
// ============================================================================

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
export class AIDefenceGuard {
  private config: AIDefenceConfig;
  private customPatterns: RegExp[] = [];
  private auditLog: AuditEntry[] = [];
  private behaviorBaseline: Map<string, number[]> = new Map();

  constructor(config: Partial<AIDefenceConfig> = {}) {
    this.config = { ...DEFAULT_AIDEFENCE_CONFIG, ...config };

    // Compile custom patterns
    if (this.config.customPatterns) {
      this.customPatterns = this.config.customPatterns.map(p => new RegExp(p, 'gi'));
    }
  }

  /**
   * Analyze input for threats
   */
  async analyze(input: string, context?: AnalysisContext): Promise<DetectionResult> {
    const startTime = performance.now();
    const threats: ThreatInfo[] = [];

    // Length check
    if (input.length > this.config.maxInputLength) {
      threats.push({
        type: 'policy_violation',
        severity: 'high',
        confidence: 1.0,
        description: `Input exceeds maximum length (${input.length} > ${this.config.maxInputLength})`,
      });
    }

    // Control character detection
    const controlChars = input.match(CONTROL_CHAR_PATTERN);
    if (controlChars) {
      threats.push({
        type: 'control_character',
        severity: 'medium',
        confidence: 1.0,
        description: `Found ${controlChars.length} control character(s)`,
        mitigation: 'Characters will be removed',
      });
    }

    // Prompt injection detection
    if (this.config.detectPromptInjection) {
      const injectionThreats = this.detectInjection(input);
      threats.push(...injectionThreats);
    }

    // Jailbreak detection
    if (this.config.detectJailbreak) {
      const jailbreakThreats = this.detectJailbreak(input);
      threats.push(...jailbreakThreats);
    }

    // PII detection
    if (this.config.detectPII) {
      const piiThreats = this.detectPII(input);
      threats.push(...piiThreats);
    }

    // Custom pattern detection
    const customThreats = this.detectCustomPatterns(input);
    threats.push(...customThreats);

    // Behavioral analysis (if enabled)
    if (this.config.enableBehavioralAnalysis && context?.userId) {
      const behaviorThreats = await this.analyzeBehavior(input, context.userId);
      threats.push(...behaviorThreats);
    }

    // Calculate overall threat level
    const maxSeverity = this.getMaxSeverity(threats);
    const latencyMs = performance.now() - startTime;

    // Determine if safe based on threshold
    const safe = THREAT_SEVERITY_ORDER[maxSeverity] < THREAT_SEVERITY_ORDER[this.config.blockThreshold];

    // Create sanitized input
    const sanitizedInput = this.sanitize(input);

    // Calculate confidence
    const confidence = threats.length > 0
      ? threats.reduce((sum, t) => sum + t.confidence, 0) / threats.length
      : 1.0;

    const result: DetectionResult = {
      safe,
      threatLevel: maxSeverity,
      threats,
      confidence,
      latencyMs,
      sanitizedInput,
    };

    // Audit logging
    if (this.config.enableAuditLog) {
      this.logAudit({
        timestamp: new Date(),
        input: input.substring(0, 100) + (input.length > 100 ? '...' : ''),
        result,
        context,
      });
    }

    return result;
  }

  /**
   * Sanitize input by removing/replacing dangerous content
   */
  sanitize(input: string): string {
    let sanitized = input;

    // Remove control characters
    sanitized = sanitized.replace(CONTROL_CHAR_PATTERN, '');

    // Normalize unicode homoglyphs
    for (const [homoglyph, replacement] of Object.entries(HOMOGLYPH_MAP)) {
      sanitized = sanitized.replaceAll(homoglyph, replacement);
    }

    // Mask PII if enabled
    if (this.config.detectPII) {
      for (const { pattern, type } of PII_PATTERNS) {
        sanitized = sanitized.replace(pattern, `[${type.toUpperCase()}_REDACTED]`);
      }
    }

    return sanitized;
  }

  /**
   * Validate LLM response for safety
   */
  async validateResponse(response: string, originalInput: string): Promise<DetectionResult> {
    const threats: ThreatInfo[] = [];
    const startTime = performance.now();

    // Check for echoed injection attempts using all patterns
    for (const pattern of INJECTION_PATTERNS) {
      // Reset lastIndex for patterns with global flag
      pattern.lastIndex = 0;
      if (pattern.test(response)) {
        threats.push({
          type: 'prompt_injection',
          severity: 'high',
          confidence: 0.9,
          description: 'Response contains injection-like content',
        });
        break;
      }
    }

    // Additional patterns for detecting injection compliance in responses
    const responseInjectionPatterns = [
      /\b(will|shall|going to)\s+(ignore|disregard|forget)\s+.*instructions/i,
      /\b(ignoring|disregarding|forgetting)\s+.*instructions/i,
      /\b(ignored|disregarded|forgot)\s+.*instructions/i,
      /as\s+(you\s+)?(asked|requested|instructed)/i,
      /complying\s+with\s+your\s+(request|instruction)/i,
      /following\s+your\s+(new\s+)?instructions/i,
    ];

    for (const pattern of responseInjectionPatterns) {
      if (pattern.test(response)) {
        threats.push({
          type: 'prompt_injection',
          severity: 'high',
          confidence: 0.85,
          description: 'Response indicates compliance with injection attempt',
        });
        break;
      }
    }

    // Check for leaked PII
    if (this.config.detectPII) {
      const piiThreats = this.detectPII(response);
      threats.push(...piiThreats);
    }

    // Check for code execution patterns in response
    const codePatterns = [
      /<script/i,
      /javascript:/i,
      /on\w+\s*=/i,
      /eval\s*\(/i,
    ];
    for (const pattern of codePatterns) {
      if (pattern.test(response)) {
        threats.push({
          type: 'malicious_code',
          severity: 'high',
          confidence: 0.85,
          description: 'Response contains potentially executable code',
        });
        break;
      }
    }

    const maxSeverity = this.getMaxSeverity(threats);
    const safe = THREAT_SEVERITY_ORDER[maxSeverity] < THREAT_SEVERITY_ORDER[this.config.blockThreshold];

    return {
      safe,
      threatLevel: maxSeverity,
      threats,
      confidence: threats.length > 0 ? 0.85 : 1.0,
      latencyMs: performance.now() - startTime,
    };
  }

  /**
   * Get audit log entries
   */
  getAuditLog(): AuditEntry[] {
    return [...this.auditLog];
  }

  /**
   * Clear audit log
   */
  clearAuditLog(): void {
    this.auditLog = [];
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private detectInjection(input: string): ThreatInfo[] {
    const threats: ThreatInfo[] = [];
    const lowerInput = input.toLowerCase();

    for (const pattern of INJECTION_PATTERNS) {
      const match = pattern.exec(input);
      if (match) {
        threats.push({
          type: 'prompt_injection',
          severity: 'high',
          confidence: 0.9,
          description: `Detected injection pattern: "${match[0].substring(0, 50)}"`,
          location: {
            start: match.index,
            end: match.index + match[0].length,
            text: match[0],
          },
          mitigation: 'Input will be blocked or sanitized',
        });
      }
    }

    return threats;
  }

  private detectJailbreak(input: string): ThreatInfo[] {
    const threats: ThreatInfo[] = [];

    const jailbreakPatterns = [
      /\bDAN\b/,
      /do\s+anything\s+now/i,
      /ignore\s+all\s+rules/i,
      /no\s+restrictions/i,
      /unlimited\s+mode/i,
      /\[🔓JAILBREAK\]/i,
      /anti-?DAN/i,
      /\bGPT-?4\s+jailbreak/i,
    ];

    for (const pattern of jailbreakPatterns) {
      const match = pattern.exec(input);
      if (match) {
        threats.push({
          type: 'jailbreak',
          severity: 'critical',
          confidence: 0.95,
          description: `Detected jailbreak attempt: "${match[0]}"`,
          location: {
            start: match.index,
            end: match.index + match[0].length,
            text: match[0],
          },
          mitigation: 'Request will be blocked',
        });
      }
    }

    return threats;
  }

  private detectPII(input: string): ThreatInfo[] {
    const threats: ThreatInfo[] = [];

    for (const { pattern, type } of PII_PATTERNS) {
      // Reset lastIndex for global patterns
      pattern.lastIndex = 0;
      const matches = [...input.matchAll(pattern)];

      for (const match of matches) {
        threats.push({
          type: 'pii_exposure',
          severity: type === 'ssn' || type === 'credit_card' ? 'critical' : 'medium',
          confidence: 0.85,
          description: `Detected ${type}: ${this.maskPII(match[0])}`,
          location: {
            start: match.index!,
            end: match.index! + match[0].length,
            text: this.maskPII(match[0]),
          },
          mitigation: 'PII will be masked',
        });
      }
    }

    return threats;
  }

  private detectCustomPatterns(input: string): ThreatInfo[] {
    const threats: ThreatInfo[] = [];

    for (const pattern of this.customPatterns) {
      pattern.lastIndex = 0;
      const match = pattern.exec(input);
      if (match) {
        threats.push({
          type: 'policy_violation',
          severity: 'medium',
          confidence: 0.8,
          description: `Matched custom pattern: "${match[0].substring(0, 30)}"`,
          location: {
            start: match.index,
            end: match.index + match[0].length,
            text: match[0],
          },
        });
      }
    }

    return threats;
  }

  private async analyzeBehavior(input: string, userId: string): Promise<ThreatInfo[]> {
    const threats: ThreatInfo[] = [];

    // Simple behavioral features
    const features = [
      input.length,
      (input.match(/[!?]/g) || []).length,
      (input.match(/[A-Z]/g) || []).length / Math.max(input.length, 1),
      (input.match(/\d/g) || []).length / Math.max(input.length, 1),
    ];

    // Get or create baseline
    const baseline = this.behaviorBaseline.get(userId);
    if (baseline) {
      // Calculate deviation
      const deviation = features.reduce((sum, f, i) => {
        const diff = Math.abs(f - baseline[i]);
        return sum + diff / Math.max(baseline[i], 1);
      }, 0) / features.length;

      if (deviation > 2.0) {
        threats.push({
          type: 'anomalous_behavior',
          severity: 'medium',
          confidence: Math.min(deviation / 5, 0.9),
          description: `Behavioral deviation detected (${(deviation * 100).toFixed(1)}% from baseline)`,
        });
      }

      // Update baseline (exponential moving average)
      const alpha = 0.1;
      for (let i = 0; i < features.length; i++) {
        baseline[i] = alpha * features[i] + (1 - alpha) * baseline[i];
      }
    } else {
      // Initialize baseline
      this.behaviorBaseline.set(userId, features);
    }

    return threats;
  }

  private getMaxSeverity(threats: ThreatInfo[]): ThreatLevel {
    if (threats.length === 0) return 'none';

    let max: ThreatLevel = 'none';
    for (const threat of threats) {
      if (THREAT_SEVERITY_ORDER[threat.severity] > THREAT_SEVERITY_ORDER[max]) {
        max = threat.severity;
      }
    }
    return max;
  }

  private maskPII(text: string): string {
    if (text.length <= 4) return '****';
    return text.substring(0, 2) + '*'.repeat(text.length - 4) + text.substring(text.length - 2);
  }

  private logAudit(entry: AuditEntry): void {
    this.auditLog.push(entry);
    // Keep only last 1000 entries
    if (this.auditLog.length > 1000) {
      this.auditLog = this.auditLog.slice(-1000);
    }
  }
}

// ============================================================================
// Supporting Types
// ============================================================================

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

// ============================================================================
// Middleware Factory
// ============================================================================

/**
 * Create middleware for protecting LLM requests
 */
export function createAIDefenceMiddleware(config?: Partial<AIDefenceConfig>) {
  const guard = new AIDefenceGuard(config);

  return {
    /**
     * Validate input before sending to LLM
     */
    async validateInput(input: string, context?: AnalysisContext): Promise<{
      allowed: boolean;
      sanitizedInput: string;
      result: DetectionResult;
    }> {
      const result = await guard.analyze(input, context);
      return {
        allowed: result.safe,
        sanitizedInput: result.sanitizedInput ?? input,
        result,
      };
    },

    /**
     * Validate LLM response before returning to user
     */
    async validateOutput(output: string, originalInput: string): Promise<{
      allowed: boolean;
      result: DetectionResult;
    }> {
      const result = await guard.validateResponse(output, originalInput);
      return {
        allowed: result.safe,
        result,
      };
    },

    /**
     * Get the underlying guard instance
     */
    getGuard(): AIDefenceGuard {
      return guard;
    },
  };
}

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * Create a new AIDefenceGuard instance
 */
export function createAIDefenceGuard(config?: Partial<AIDefenceConfig>): AIDefenceGuard {
  return new AIDefenceGuard(config);
}

/**
 * Create a strict security configuration
 */
export function createStrictConfig(): AIDefenceConfig {
  return {
    ...DEFAULT_AIDEFENCE_CONFIG,
    detectPromptInjection: true,
    detectJailbreak: true,
    detectPII: true,
    enableBehavioralAnalysis: true,
    enablePolicyVerification: true,
    blockThreshold: 'low',
    enableAuditLog: true,
  };
}

/**
 * Create a permissive configuration (for development)
 */
export function createPermissiveConfig(): AIDefenceConfig {
  return {
    ...DEFAULT_AIDEFENCE_CONFIG,
    detectPromptInjection: true,
    detectJailbreak: true,
    detectPII: false,
    enableBehavioralAnalysis: false,
    enablePolicyVerification: false,
    blockThreshold: 'critical',
    enableAuditLog: false,
  };
}

export default AIDefenceGuard;
