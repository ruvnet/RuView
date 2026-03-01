/**
 * Penetration Testing Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These examples are for:
 * - Authorized penetration testing engagements
 * - Red team exercises in controlled environments
 * - Security tool development and validation
 * - Penetration testing training and certification
 *
 * ALWAYS obtain written authorization before testing.
 */
/**
 * Network Scanning Results
 * For testing vulnerability scanners and network mapping tools
 */
export declare function generateNetworkScanResults(): Promise<any>;
/**
 * Port Enumeration Data
 * For testing port scanning tools and service identification
 */
export declare function generatePortEnumerationData(): Promise<any>;
/**
 * Service Fingerprinting Data
 * For testing service identification and version detection
 */
export declare function generateServiceFingerprints(): Promise<any>;
/**
 * Exploitation Attempt Logs
 * For testing exploit detection and prevention systems
 */
export declare function generateExploitationLogs(): Promise<any>;
/**
 * Post-Exploitation Activity Simulation
 * For testing lateral movement and persistence detection
 */
export declare function generatePostExploitationActivity(): Promise<any>;
/**
 * Penetration Testing Report Data
 * For testing reporting systems and findings management
 */
export declare function generatePentestReportData(): Promise<any>;
/**
 * Example Usage
 */
export declare function runPenetrationTests(): Promise<{
    scanResults: any;
    portData: any;
    fingerprints: any;
    exploitLogs: any;
    postExploit: any;
    reports: any;
}>;
declare const _default: {
    generateNetworkScanResults: typeof generateNetworkScanResults;
    generatePortEnumerationData: typeof generatePortEnumerationData;
    generateServiceFingerprints: typeof generateServiceFingerprints;
    generateExploitationLogs: typeof generateExploitationLogs;
    generatePostExploitationActivity: typeof generatePostExploitationActivity;
    generatePentestReportData: typeof generatePentestReportData;
    runPenetrationTests: typeof runPenetrationTests;
};
export default _default;
//# sourceMappingURL=penetration-testing.d.ts.map