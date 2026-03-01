/**
 * Security Testing Generator - Penetration testing and vulnerability data
 *
 * Generates realistic security testing scenarios, vulnerability data, attack patterns,
 * and log analytics for testing security systems, training ML models, and conducting
 * security research.
 *
 * @packageDocumentation
 */

import { EventEmitter } from 'events';
import { AgenticSynth, SynthConfig, GenerationResult, EventOptions } from '@ruvector/agentic-synth';

/**
 * Vulnerability severity levels
 */
export type VulnerabilitySeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';

/**
 * Common vulnerability types
 */
export type VulnerabilityType =
  | 'sql-injection'
  | 'xss'
  | 'csrf'
  | 'rce'
  | 'path-traversal'
  | 'authentication-bypass'
  | 'privilege-escalation'
  | 'dos'
  | 'information-disclosure'
  | 'misconfiguration';

/**
 * Vulnerability test case
 */
export interface VulnerabilityTestCase {
  id: string;
  type: VulnerabilityType;
  severity: VulnerabilitySeverity;
  description: string;
  target: string;
  payload: string;
  expectedResult: string;
  cwe?: string; // Common Weakness Enumeration ID
  cvss?: number; // CVSS score (0-10)
}

/**
 * Security log entry
 */
export interface SecurityLogEntry {
  timestamp: Date;
  level: 'debug' | 'info' | 'warning' | 'error' | 'critical';
  source: string;
  eventType: string;
  message: string;
  ip?: string;
  user?: string;
  details?: Record<string, unknown>;
}

/**
 * Anomaly detection pattern
 */
export interface AnomalyPattern {
  id: string;
  type: 'brute-force' | 'port-scan' | 'data-exfiltration' | 'privilege-abuse' | 'suspicious-traffic';
  confidence: number; // 0-1
  indicators: string[];
  affectedResources: string[];
  timeline: Date[];
}

/**
 * Penetration testing scenario
 */
export interface PenetrationTestScenario {
  id: string;
  name: string;
  objective: string;
  targetSystem: string;
  attackVector: string;
  steps: Array<{
    step: number;
    action: string;
    tool?: string;
    command?: string;
    expectedOutcome: string;
  }>;
  successCriteria: string[];
  mitigations: string[];
}

/**
 * Security testing configuration
 */
export interface SecurityTestingConfig extends Partial<SynthConfig> {
  targetTypes?: string[]; // Types of systems to target
  includePayloads?: boolean; // Include actual exploit payloads
  severityFilter?: VulnerabilitySeverity[]; // Filter by severity
  logFormat?: 'json' | 'syslog' | 'custom';
}

/**
 * Security Testing Generator for penetration testing and vulnerability research
 *
 * Features:
 * - Vulnerability test case generation
 * - Penetration testing scenarios
 * - Security log analytics data
 * - Anomaly detection patterns
 * - Attack simulation data
 * - CVSS scoring and CWE mapping
 *
 * @example
 * ```typescript
 * const generator = new SecurityTestingGenerator({
 *   provider: 'gemini',
 *   apiKey: process.env.GEMINI_API_KEY,
 *   includePayloads: true,
 *   severityFilter: ['critical', 'high']
 * });
 *
 * // Generate vulnerability test cases
 * const vulns = await generator.generateVulnerabilities({
 *   count: 20,
 *   types: ['sql-injection', 'xss', 'rce']
 * });
 *
 * // Generate security logs
 * const logs = await generator.generateSecurityLogs({
 *   count: 1000,
 *   startDate: new Date('2024-01-01'),
 *   includeAnomalies: true
 * });
 *
 * // Create penetration test scenario
 * const scenario = await generator.generatePentestScenario({
 *   target: 'web-application',
 *   complexity: 'advanced'
 * });
 * ```
 */
export class SecurityTestingGenerator extends EventEmitter {
  private synth: AgenticSynth;
  private config: SecurityTestingConfig;
  private generatedVulnerabilities: VulnerabilityTestCase[] = [];
  private generatedLogs: SecurityLogEntry[] = [];
  private detectedAnomalies: AnomalyPattern[] = [];

  constructor(config: SecurityTestingConfig = {}) {
    super();

    this.config = {
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY || '',
      ...(config.model && { model: config.model }),
      cacheStrategy: config.cacheStrategy || 'memory',
      cacheTTL: config.cacheTTL || 3600,
      maxRetries: config.maxRetries || 3,
      timeout: config.timeout || 30000,
      streaming: config.streaming || false,
      automation: config.automation || false,
      vectorDB: config.vectorDB || false,
      targetTypes: config.targetTypes || ['web', 'api', 'network', 'system'],
      includePayloads: config.includePayloads ?? true,
      severityFilter: config.severityFilter || ['critical', 'high', 'medium', 'low', 'info'],
      logFormat: config.logFormat || 'json'
    };

    this.synth = new AgenticSynth(this.config);
  }

  /**
   * Generate vulnerability test cases
   */
  async generateVulnerabilities(options: {
    count?: number;
    types?: VulnerabilityType[];
    severity?: VulnerabilitySeverity;
  } = {}): Promise<GenerationResult<VulnerabilityTestCase>> {
    this.emit('vulnerabilities:generating', { options });

    try {
      const result = await this.synth.generateStructured<{
        type: string;
        severity: string;
        description: string;
        target: string;
        payload: string;
        expectedResult: string;
        cwe: string;
        cvss: number;
      }>({
        count: options.count || 10,
        schema: {
          type: { type: 'string', enum: options.types || ['sql-injection', 'xss', 'csrf'] },
          severity: { type: 'string', enum: this.config.severityFilter },
          description: { type: 'string' },
          target: { type: 'string' },
          payload: { type: 'string' },
          expectedResult: { type: 'string' },
          cwe: { type: 'string' },
          cvss: { type: 'number', minimum: 0, maximum: 10 }
        }
      });

      const vulnerabilities: VulnerabilityTestCase[] = result.data.map(v => ({
        id: this.generateId('vuln'),
        type: v.type as VulnerabilityType,
        severity: v.severity as VulnerabilitySeverity,
        description: v.description,
        target: v.target,
        payload: this.config.includePayloads ? v.payload : '[REDACTED]',
        expectedResult: v.expectedResult,
        cwe: v.cwe,
        cvss: v.cvss
      }));

      // Filter by severity if specified
      const filtered = options.severity
        ? vulnerabilities.filter(v => v.severity === options.severity)
        : vulnerabilities;

      this.generatedVulnerabilities.push(...filtered);

      this.emit('vulnerabilities:generated', { count: filtered.length });

      return {
        data: filtered,
        metadata: result.metadata
      };
    } catch (error) {
      this.emit('vulnerabilities:error', { error });
      throw error;
    }
  }

  /**
   * Generate security log entries
   */
  async generateSecurityLogs(options: {
    count?: number;
    startDate?: Date;
    endDate?: Date;
    includeAnomalies?: boolean;
    sources?: string[];
  } = {}): Promise<GenerationResult<SecurityLogEntry>> {
    this.emit('logs:generating', { options });

    try {
      const eventOptions: Partial<EventOptions> = {
        count: options.count || 100,
        eventTypes: ['login', 'logout', 'access', 'error', 'warning', 'attack'],
        distribution: 'poisson',
        timeRange: {
          start: options.startDate || new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
          end: options.endDate || new Date()
        }
      };

      const result = await this.synth.generateEvents<{
        level: string;
        source: string;
        eventType: string;
        message: string;
        ip: string;
        user: string;
      }>(eventOptions);

      const logs: SecurityLogEntry[] = result.data.map(event => ({
        timestamp: new Date(),
        level: this.parseLogLevel(event.level),
        source: event.source || 'system',
        eventType: event.eventType,
        message: event.message,
        ip: event.ip,
        user: event.user,
        details: {}
      }));

      // Inject anomalies if requested
      if (options.includeAnomalies) {
        await this.injectAnomalies(logs);
      }

      this.generatedLogs.push(...logs);

      this.emit('logs:generated', { count: logs.length });

      return {
        data: logs,
        metadata: result.metadata
      };
    } catch (error) {
      this.emit('logs:error', { error });
      throw error;
    }
  }

  /**
   * Generate penetration testing scenario
   */
  async generatePentestScenario(options: {
    target?: string;
    complexity?: 'basic' | 'intermediate' | 'advanced';
    objective?: string;
  } = {}): Promise<PenetrationTestScenario> {
    this.emit('pentest:generating', { options });

    try {
      const result = await this.synth.generateStructured<{
        name: string;
        objective: string;
        targetSystem: string;
        attackVector: string;
        steps: Array<{
          step: number;
          action: string;
          tool: string;
          command: string;
          expectedOutcome: string;
        }>;
        successCriteria: string[];
        mitigations: string[];
      }>({
        count: 1,
        schema: {
          name: { type: 'string' },
          objective: { type: 'string' },
          targetSystem: { type: 'string' },
          attackVector: { type: 'string' },
          steps: { type: 'array', items: { type: 'object' } },
          successCriteria: { type: 'array', items: { type: 'string' } },
          mitigations: { type: 'array', items: { type: 'string' } }
        }
      });

      const scenario: PenetrationTestScenario = {
        id: this.generateId('pentest'),
        ...result.data[0]
      };

      this.emit('pentest:generated', { scenarioId: scenario.id });

      return scenario;
    } catch (error) {
      this.emit('pentest:error', { error });
      throw error;
    }
  }

  /**
   * Detect anomaly patterns in logs
   */
  async detectAnomalies(logs?: SecurityLogEntry[]): Promise<AnomalyPattern[]> {
    const targetLogs = logs || this.generatedLogs;

    if (targetLogs.length === 0) {
      return [];
    }

    this.emit('anomaly:detecting', { logCount: targetLogs.length });

    // Simple pattern detection (in real scenario, use ML models)
    const patterns: AnomalyPattern[] = [];

    // Detect brute force attempts
    const loginAttempts = targetLogs.filter(log =>
      log.eventType === 'login' && log.level === 'error'
    );

    if (loginAttempts.length > 10) {
      patterns.push({
        id: this.generateId('anomaly'),
        type: 'brute-force',
        confidence: Math.min(loginAttempts.length / 50, 1),
        indicators: ['multiple-failed-logins', 'same-source-ip'],
        affectedResources: [...new Set(loginAttempts.map(l => l.user || 'unknown'))],
        timeline: loginAttempts.map(l => l.timestamp)
      });
    }

    this.detectedAnomalies.push(...patterns);

    this.emit('anomaly:detected', { count: patterns.length });

    return patterns;
  }

  /**
   * Get security statistics
   */
  getStatistics(): {
    totalVulnerabilities: number;
    criticalCount: number;
    totalLogs: number;
    anomalyCount: number;
    severityDistribution: Record<VulnerabilitySeverity, number>;
  } {
    const severityDistribution: Record<VulnerabilitySeverity, number> = {
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
      info: 0
    };

    this.generatedVulnerabilities.forEach(v => {
      severityDistribution[v.severity]++;
    });

    return {
      totalVulnerabilities: this.generatedVulnerabilities.length,
      criticalCount: severityDistribution.critical,
      totalLogs: this.generatedLogs.length,
      anomalyCount: this.detectedAnomalies.length,
      severityDistribution
    };
  }

  /**
   * Export logs to specified format
   */
  exportLogs(format: 'json' | 'csv' = 'json'): string {
    if (format === 'json') {
      return JSON.stringify(this.generatedLogs, null, 2);
    }

    // CSV format
    const headers = ['timestamp', 'level', 'source', 'eventType', 'message', 'ip', 'user'];
    const rows = this.generatedLogs.map(log => [
      log.timestamp.toISOString(),
      log.level,
      log.source,
      log.eventType,
      log.message,
      log.ip || '',
      log.user || ''
    ].join(','));

    return [headers.join(','), ...rows].join('\n');
  }

  /**
   * Reset generator state
   */
  reset(): void {
    this.generatedVulnerabilities = [];
    this.generatedLogs = [];
    this.detectedAnomalies = [];

    this.emit('reset', { timestamp: new Date() });
  }

  /**
   * Inject anomalies into log data
   */
  private async injectAnomalies(logs: SecurityLogEntry[]): Promise<void> {
    // Inject brute force pattern
    const bruteForceCount = Math.floor(logs.length * 0.05);
    for (let i = 0; i < bruteForceCount; i++) {
      logs.push({
        timestamp: new Date(Date.now() - Math.random() * 24 * 60 * 60 * 1000),
        level: 'error',
        source: 'auth',
        eventType: 'login',
        message: 'Failed login attempt',
        ip: '192.168.1.' + Math.floor(Math.random() * 255),
        user: 'admin'
      });
    }
  }

  /**
   * Parse log level string
   */
  private parseLogLevel(level: string): 'debug' | 'info' | 'warning' | 'error' | 'critical' {
    const lower = level.toLowerCase();
    if (lower.includes('crit')) return 'critical';
    if (lower.includes('err')) return 'error';
    if (lower.includes('warn')) return 'warning';
    if (lower.includes('debug')) return 'debug';
    return 'info';
  }

  /**
   * Generate unique ID
   */
  private generateId(prefix: string): string {
    return `${prefix}_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
  }
}

/**
 * Create a new security testing generator instance
 */
export function createSecurityTestingGenerator(config?: SecurityTestingConfig): SecurityTestingGenerator {
  return new SecurityTestingGenerator(config);
}
