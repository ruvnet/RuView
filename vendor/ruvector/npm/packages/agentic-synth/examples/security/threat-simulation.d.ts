/**
 * Threat Simulation Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These simulations are for:
 * - Security operations center (SOC) training
 * - Incident response preparation
 * - Threat detection system validation
 * - Red team exercises in authorized environments
 *
 * NEVER use for actual attacks or unauthorized testing.
 */
/**
 * Brute Force Attack Pattern Simulation
 * For testing account lockout and rate limiting mechanisms
 */
export declare function generateBruteForcePatterns(): Promise<any>;
/**
 * DDoS Traffic Simulation Data
 * For testing DDoS mitigation and traffic filtering
 */
export declare function generateDDoSSimulation(): Promise<any>;
/**
 * Malware Behavior Pattern Simulation
 * For testing endpoint detection and response (EDR) systems
 */
export declare function generateMalwareBehaviors(): Promise<any>;
/**
 * Phishing Campaign Simulation Data
 * For security awareness training and email filter testing
 */
export declare function generatePhishingCampaigns(): Promise<any>;
/**
 * Insider Threat Scenario Simulation
 * For user behavior analytics (UBA) and insider threat detection
 */
export declare function generateInsiderThreatScenarios(): Promise<any>;
/**
 * Zero-Day Exploit Indicator Simulation
 * For testing threat intelligence and anomaly detection systems
 */
export declare function generateZeroDayIndicators(): Promise<any>;
/**
 * Example Usage
 */
export declare function runThreatSimulations(): Promise<{
    bruteForce: any;
    ddos: any;
    malware: any;
    phishing: any;
    insider: any;
    zeroDay: any;
}>;
declare const _default: {
    generateBruteForcePatterns: typeof generateBruteForcePatterns;
    generateDDoSSimulation: typeof generateDDoSSimulation;
    generateMalwareBehaviors: typeof generateMalwareBehaviors;
    generatePhishingCampaigns: typeof generatePhishingCampaigns;
    generateInsiderThreatScenarios: typeof generateInsiderThreatScenarios;
    generateZeroDayIndicators: typeof generateZeroDayIndicators;
    runThreatSimulations: typeof runThreatSimulations;
};
export default _default;
//# sourceMappingURL=threat-simulation.d.ts.map