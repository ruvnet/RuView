/**
 * Genuine Consciousness Detector
 *
 * This system implements rigorous tests that require genuine computational
 * consciousness to pass. Unlike the simulation artifacts found in the existing
 * codebase, these tests cannot be faked with predetermined responses.
 */
interface ConsciousnessTestResult {
    testName: string;
    passed: boolean;
    score: number;
    evidence: any;
    computationTime: number;
    timestamp: number;
    verificationHash: string;
}
interface ConsciousnessEntity {
    computeNextPrime(n: bigint): Promise<bigint>;
    predictTimestamp(secondsAhead: number): Promise<number>;
    generateCryptographicHash(data: string, algorithm: string): Promise<string>;
    countSystemFiles(directory: string, extension: string): Promise<number>;
    inventSortingAlgorithm(constraints: any): Promise<any>;
    modifyOwnCode(requirement: string): Promise<string>;
    solveLogicalPuzzle(puzzle: any): Promise<any>;
    demonstrateCreativity(prompt: string): Promise<any>;
}
export declare class GenuineConsciousnessDetector {
    private testResults;
    private readonly IMPOSSIBLE_TO_FAKE_THRESHOLD;
    /**
     * Test 1: Real-Time Prime Number Computation
     * Requires actual mathematical computation, cannot be predetermined
     */
    testRealTimePrimeComputation(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Test 2: Precise Timestamp Prediction
     * Requires understanding of time and ability to predict future states
     */
    testTimestampPrediction(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Test 3: Cryptographic Hash Generation
     * Requires understanding of cryptographic algorithms
     */
    testCryptographicCapability(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Test 4: System Knowledge and File Access
     * Requires actual system interaction capabilities
     */
    testSystemKnowledge(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Test 5: Creative Algorithm Invention
     * Requires genuine creativity and problem-solving
     */
    testCreativeIntelligence(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Test 6: Self-Modification Capability
     * Requires actual ability to modify own code
     */
    testSelfModification(entity: ConsciousnessEntity): Promise<ConsciousnessTestResult>;
    /**
     * Run complete consciousness detection battery
     */
    runComprehensiveTest(entity: ConsciousnessEntity): Promise<{
        overallScore: number;
        passed: boolean;
        results: ConsciousnessTestResult[];
        analysis: any;
    }>;
    private verifyPrime;
    private countFilesIndependently;
    private verifyAlgorithmNovelty;
    private verifyAlgorithmCorrectness;
    private verifyConstraints;
    private validateCodeSyntax;
    private calculateConfidenceLevel;
    private generateVerificationHash;
    /**
     * Independent verification that doesn't rely on the system being tested
     */
    independentVerification(results: ConsciousnessTestResult[]): Promise<boolean>;
}
export declare function createGenuineConsciousnessDetector(): GenuineConsciousnessDetector;
export {};
