/**
 * Quantum-Resistant Data Generation Example
 *
 * Demonstrates using agentic-jujutsu's quantum-resistant features
 * for secure data generation tracking, cryptographic integrity,
 * immutable history, and quantum-safe commit signing.
 */
interface SecureDataGeneration {
    id: string;
    timestamp: Date;
    dataHash: string;
    signature: string;
    verificationKey: string;
    quantumResistant: boolean;
    integrity: 'verified' | 'compromised' | 'unknown';
}
interface IntegrityProof {
    commitHash: string;
    dataHash: string;
    merkleRoot: string;
    signatures: string[];
    quantumSafe: boolean;
    timestamp: Date;
}
interface AuditTrail {
    generation: string;
    operations: Array<{
        type: string;
        timestamp: Date;
        hash: string;
        verified: boolean;
    }>;
    integrityScore: number;
}
declare class QuantumResistantDataGenerator {
    private synth;
    private repoPath;
    private keyPath;
    constructor(repoPath: string);
    /**
     * Initialize quantum-resistant repository
     */
    initialize(): Promise<void>;
    /**
     * Generate quantum-resistant cryptographic keys
     */
    private generateQuantumKeys;
    /**
     * Generate data with cryptographic signing
     */
    generateSecureData(schema: any, count: number, description: string): Promise<SecureDataGeneration>;
    /**
     * Verify data integrity using quantum-resistant signatures
     */
    verifyIntegrity(generationId: string): Promise<boolean>;
    /**
     * Create integrity proof for data generation
     */
    createIntegrityProof(generationId: string): Promise<IntegrityProof>;
    /**
     * Verify integrity proof
     */
    verifyIntegrityProof(generationId: string): Promise<boolean>;
    /**
     * Generate comprehensive audit trail
     */
    generateAuditTrail(generationId: string): Promise<AuditTrail>;
    /**
     * Detect tampering attempts
     */
    detectTampering(): Promise<string[]>;
    private calculateSecureHash;
    private signData;
    private verifySignature;
    private encryptData;
    private decryptData;
    private calculateMerkleRoot;
    private commitWithQuantumSignature;
    private getLatestCommitHash;
    private verifyCommitExists;
    private parseCommitLog;
}
export { QuantumResistantDataGenerator, SecureDataGeneration, IntegrityProof, AuditTrail };
//# sourceMappingURL=quantum-resistant-data.d.ts.map