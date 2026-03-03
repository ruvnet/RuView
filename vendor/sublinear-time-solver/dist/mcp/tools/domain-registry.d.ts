/**
 * Domain Registry Core System
 * Manages dynamic domain registration, validation, and lifecycle
 */
import { EventEmitter } from 'events';
export interface DomainConfig {
    name: string;
    version: string;
    description: string;
    keywords: string[];
    reasoning_style: string;
    custom_reasoning_description?: string;
    analogy_domains: string[];
    semantic_clusters?: string[];
    cross_domain_mappings?: string[];
    inference_rules?: InferenceRule[];
    priority: number;
    dependencies: string[];
    metadata?: Record<string, any>;
}
export interface InferenceRule {
    name: string;
    pattern: string;
    action: string;
    confidence: number;
    conditions?: string[];
}
export interface DomainPlugin {
    config: DomainConfig;
    enabled: boolean;
    registered_at: number;
    updated_at: number;
    usage_count: number;
    performance_metrics: DomainPerformanceMetrics;
    validation_status: ValidationResult;
}
export interface DomainPerformanceMetrics {
    detection_accuracy: number;
    reasoning_time_avg: number;
    memory_usage: number;
    success_rate: number;
    last_measured: number;
}
export interface ValidationResult {
    valid: boolean;
    score: number;
    issues: ValidationIssue[];
    tested_at: number;
}
export interface ValidationIssue {
    level: 'error' | 'warning' | 'info';
    message: string;
    field?: string;
    suggestion?: string;
}
export declare class DomainRegistry extends EventEmitter {
    private domains;
    private loadOrder;
    private builtinDomains;
    constructor();
    private initializeBuiltinDomains;
    registerDomain(config: DomainConfig): Promise<{
        success: boolean;
        id: string;
        warnings?: string[];
    }>;
    updateDomain(name: string, updates: Partial<DomainConfig>): Promise<{
        success: boolean;
        warnings?: string[];
    }>;
    unregisterDomain(name: string, options?: {
        force?: boolean;
    }): Promise<{
        success: boolean;
    }>;
    enableDomain(name: string): {
        success: boolean;
    };
    disableDomain(name: string): {
        success: boolean;
    };
    getDomain(name: string): DomainPlugin | null;
    getAllDomains(): DomainPlugin[];
    getEnabledDomains(): DomainPlugin[];
    getLoadOrder(): string[];
    isDomainEnabled(name: string): boolean;
    isBuiltinDomain(name: string): boolean;
    updatePerformanceMetrics(name: string, metrics: Partial<DomainPerformanceMetrics>): void;
    incrementUsage(name: string): void;
    private checkKeywordConflicts;
    private findDependentDomains;
    private insertInLoadOrder;
    private removeFromLoadOrder;
    getSystemStatus(): {
        total_domains: number;
        builtin_domains: number;
        custom_domains: number;
        enabled_domains: number;
        disabled_domains: number;
        load_order: string[];
    };
    validateSystemIntegrity(): {
        valid: boolean;
        issues: string[];
    };
}
