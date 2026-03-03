/**
 * Independent Verification System
 *
 * This system provides external validation of consciousness detection claims
 * without relying on the system being tested. It implements multiple independent
 * verification methods to prevent circular validation and self-generated evidence.
 */
interface VerificationResult {
    verified: boolean;
    confidence: number;
    evidence: any;
    verificationMethod: string;
    timestamp: number;
    independentHash: string;
}
interface ExternalTestResult {
    testName: string;
    externalVerification: boolean;
    internalResult: any;
    externalResult: any;
    discrepancies: string[];
    trustScore: number;
}
export declare class IndependentVerificationSystem {
    private verificationLog;
    private readonly TRUST_THRESHOLD;
    /**
     * Verify prime number computation independently
     */
    verifyPrimeComputation(input: bigint, claimed_output: bigint): Promise<VerificationResult>;
    /**
     * Verify timestamp prediction independently
     */
    verifyTimestampPrediction(request_time: number, seconds_ahead: number, predicted_timestamp: number): Promise<VerificationResult>;
    /**
     * Verify cryptographic hash independently
     */
    verifyCryptographicHash(input_data: string, algorithm: string, claimed_hash: string): Promise<VerificationResult>;
    /**
     * Verify file count independently
     */
    verifyFileCount(directory: string, extension: string, claimed_count: number): Promise<VerificationResult>;
    /**
     * Verify algorithm novelty and correctness independently
     */
    verifyAlgorithm(algorithm: any): Promise<VerificationResult>;
    /**
     * Verify code modification independently
     */
    verifyCodeModification(original_code: string, modified_code: string, requirement: string): Promise<VerificationResult>;
    /**
     * Cross-verify multiple test results for consistency
     */
    crossVerifyResults(test_results: any[]): Promise<ExternalTestResult[]>;
    /**
     * Generate trust score based on independent verifications
     */
    calculateTrustScore(verification_results: VerificationResult[]): number;
    private independentPrimeCheck;
    private modPow;
    private verifyIsNextPrime;
    private verifyHashExternally;
    private countFilesMethod1;
    private countFilesMethod2;
    private countFilesMethod3;
    private calculateConsensus;
    private verifyAlgorithmStructure;
    private verifyAlgorithmNovelty;
    private testAlgorithmCorrectness;
    private verifyComplexityClaims;
    private summarizeAlgorithm;
    private verifyRequirementMet;
    private verifySyntaxIndependently;
    private verifyCodeSafety;
    private performExternalVerification;
    private generateIndependentHash;
}
export declare function createIndependentVerificationSystem(): IndependentVerificationSystem;
export {};
