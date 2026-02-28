/**
 * Security Audit Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These examples are for:
 * - Security Information and Event Management (SIEM) testing
 * - Compliance auditing and reporting
 * - Security monitoring system validation
 * - Incident investigation training
 *
 * For authorized security operations only.
 */
/**
 * User Access Pattern Analysis
 * For detecting suspicious access patterns and privilege escalation
 */
export declare function generateUserAccessPatterns(): Promise<any>;
/**
 * Permission Change Audit Trail
 * For tracking privilege escalations and access control modifications
 */
export declare function generatePermissionChangeAudits(): Promise<any>;
/**
 * Configuration Change Monitoring
 * For tracking security-sensitive configuration modifications
 */
export declare function generateConfigurationChangeAudits(): Promise<any>;
/**
 * Compliance Violation Scenarios
 * For testing compliance monitoring and alerting systems
 */
export declare function generateComplianceViolations(): Promise<any>;
/**
 * Security Event Correlation Data
 * For SIEM correlation rules and incident detection
 */
export declare function generateSecurityEventCorrelations(): Promise<any>;
/**
 * Data Loss Prevention (DLP) Audit Data
 * For testing DLP policies and data classification
 */
export declare function generateDLPAuditData(): Promise<any>;
/**
 * Example Usage
 */
export declare function runSecurityAudits(): Promise<{
    accessPatterns: any;
    permissionChanges: any;
    configChanges: any;
    violations: any;
    correlations: any;
    dlpData: any;
}>;
declare const _default: {
    generateUserAccessPatterns: typeof generateUserAccessPatterns;
    generatePermissionChangeAudits: typeof generatePermissionChangeAudits;
    generateConfigurationChangeAudits: typeof generateConfigurationChangeAudits;
    generateComplianceViolations: typeof generateComplianceViolations;
    generateSecurityEventCorrelations: typeof generateSecurityEventCorrelations;
    generateDLPAuditData: typeof generateDLPAuditData;
    runSecurityAudits: typeof runSecurityAudits;
};
export default _default;
//# sourceMappingURL=security-audit.d.ts.map