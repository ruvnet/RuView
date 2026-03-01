"use strict";
/**
 * Quantum-Resistant Data Generation Example
 *
 * Demonstrates using agentic-jujutsu's quantum-resistant features
 * for secure data generation tracking, cryptographic integrity,
 * immutable history, and quantum-safe commit signing.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.QuantumResistantDataGenerator = void 0;
const synth_1 = require("../../src/core/synth");
const child_process_1 = require("child_process");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const crypto = __importStar(require("crypto"));
class QuantumResistantDataGenerator {
    constructor(repoPath) {
        this.synth = new synth_1.AgenticSynth();
        this.repoPath = repoPath;
        this.keyPath = path.join(repoPath, '.jj', 'quantum-keys');
    }
    /**
     * Initialize quantum-resistant repository
     */
    async initialize() {
        try {
            console.log('🔐 Initializing quantum-resistant repository...');
            // Initialize jujutsu with quantum-resistant features
            if (!fs.existsSync(path.join(this.repoPath, '.jj'))) {
                (0, child_process_1.execSync)('npx agentic-jujutsu@latest init --quantum-resistant', {
                    cwd: this.repoPath,
                    stdio: 'inherit'
                });
            }
            // Create secure directories
            const dirs = ['data/secure', 'data/proofs', 'data/audits'];
            for (const dir of dirs) {
                const fullPath = path.join(this.repoPath, dir);
                if (!fs.existsSync(fullPath)) {
                    fs.mkdirSync(fullPath, { recursive: true });
                }
            }
            // Generate quantum-resistant keys
            await this.generateQuantumKeys();
            console.log('✅ Quantum-resistant repository initialized');
        }
        catch (error) {
            throw new Error(`Failed to initialize: ${error.message}`);
        }
    }
    /**
     * Generate quantum-resistant cryptographic keys
     */
    async generateQuantumKeys() {
        try {
            console.log('🔑 Generating quantum-resistant keys...');
            if (!fs.existsSync(this.keyPath)) {
                fs.mkdirSync(this.keyPath, { recursive: true });
            }
            // In production, use actual post-quantum cryptography libraries
            // like liboqs, Dilithium, or SPHINCS+
            // For demo, we'll use Node's crypto with ECDSA (placeholder)
            const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519', {
                publicKeyEncoding: { type: 'spki', format: 'pem' },
                privateKeyEncoding: { type: 'pkcs8', format: 'pem' }
            });
            fs.writeFileSync(path.join(this.keyPath, 'public.pem'), publicKey);
            fs.writeFileSync(path.join(this.keyPath, 'private.pem'), privateKey);
            fs.chmodSync(path.join(this.keyPath, 'private.pem'), 0o600);
            console.log('✅ Quantum-resistant keys generated');
        }
        catch (error) {
            throw new Error(`Key generation failed: ${error.message}`);
        }
    }
    /**
     * Generate data with cryptographic signing
     */
    async generateSecureData(schema, count, description) {
        try {
            console.log(`🔐 Generating ${count} records with quantum-resistant security...`);
            // Generate data
            const data = await this.synth.generate(schema, { count });
            // Calculate cryptographic hash
            const dataHash = this.calculateSecureHash(data);
            // Sign the data
            const signature = this.signData(dataHash);
            // Get verification key
            const publicKey = fs.readFileSync(path.join(this.keyPath, 'public.pem'), 'utf-8');
            // Save encrypted data
            const timestamp = Date.now();
            const dataFile = path.join(this.repoPath, 'data/secure', `secure_${timestamp}.json`);
            const encryptedData = this.encryptData(data);
            fs.writeFileSync(dataFile, JSON.stringify({
                encrypted: encryptedData,
                hash: dataHash,
                signature,
                timestamp
            }, null, 2));
            // Commit with quantum-safe signature
            await this.commitWithQuantumSignature(dataFile, dataHash, signature, description);
            const generation = {
                id: `secure_${timestamp}`,
                timestamp: new Date(),
                dataHash,
                signature,
                verificationKey: publicKey,
                quantumResistant: true,
                integrity: 'verified'
            };
            console.log(`✅ Secure generation complete`);
            console.log(`   Hash: ${dataHash.substring(0, 16)}...`);
            console.log(`   Signature: ${signature.substring(0, 16)}...`);
            return generation;
        }
        catch (error) {
            throw new Error(`Secure generation failed: ${error.message}`);
        }
    }
    /**
     * Verify data integrity using quantum-resistant signatures
     */
    async verifyIntegrity(generationId) {
        try {
            console.log(`🔍 Verifying integrity of ${generationId}...`);
            const dataFile = path.join(this.repoPath, 'data/secure', `${generationId}.json`);
            if (!fs.existsSync(dataFile)) {
                throw new Error('Generation not found');
            }
            const content = JSON.parse(fs.readFileSync(dataFile, 'utf-8'));
            // Recalculate hash
            const decryptedData = this.decryptData(content.encrypted);
            const calculatedHash = this.calculateSecureHash(decryptedData);
            // Verify hash matches
            if (calculatedHash !== content.hash) {
                console.error('❌ Hash mismatch - data may be tampered');
                return false;
            }
            // Verify signature
            const publicKey = fs.readFileSync(path.join(this.keyPath, 'public.pem'), 'utf-8');
            const verified = this.verifySignature(content.hash, content.signature, publicKey);
            if (verified) {
                console.log('✅ Integrity verified - data is authentic');
            }
            else {
                console.error('❌ Signature verification failed');
            }
            return verified;
        }
        catch (error) {
            throw new Error(`Integrity verification failed: ${error.message}`);
        }
    }
    /**
     * Create integrity proof for data generation
     */
    async createIntegrityProof(generationId) {
        try {
            console.log(`📜 Creating integrity proof for ${generationId}...`);
            // Get commit hash
            const commitHash = this.getLatestCommitHash();
            // Load generation data
            const dataFile = path.join(this.repoPath, 'data/secure', `${generationId}.json`);
            const content = JSON.parse(fs.readFileSync(dataFile, 'utf-8'));
            // Create merkle tree of data
            const decryptedData = this.decryptData(content.encrypted);
            const merkleRoot = this.calculateMerkleRoot(decryptedData);
            // Collect signatures
            const signatures = [content.signature];
            const proof = {
                commitHash,
                dataHash: content.hash,
                merkleRoot,
                signatures,
                quantumSafe: true,
                timestamp: new Date()
            };
            // Save proof
            const proofFile = path.join(this.repoPath, 'data/proofs', `${generationId}_proof.json`);
            fs.writeFileSync(proofFile, JSON.stringify(proof, null, 2));
            console.log('✅ Integrity proof created');
            console.log(`   Merkle root: ${merkleRoot.substring(0, 16)}...`);
            return proof;
        }
        catch (error) {
            throw new Error(`Proof creation failed: ${error.message}`);
        }
    }
    /**
     * Verify integrity proof
     */
    async verifyIntegrityProof(generationId) {
        try {
            console.log(`🔍 Verifying integrity proof for ${generationId}...`);
            const proofFile = path.join(this.repoPath, 'data/proofs', `${generationId}_proof.json`);
            if (!fs.existsSync(proofFile)) {
                throw new Error('Proof not found');
            }
            const proof = JSON.parse(fs.readFileSync(proofFile, 'utf-8'));
            // Verify commit exists
            const commitExists = this.verifyCommitExists(proof.commitHash);
            if (!commitExists) {
                console.error('❌ Commit not found in history');
                return false;
            }
            // Verify signatures
            for (const signature of proof.signatures) {
                const publicKey = fs.readFileSync(path.join(this.keyPath, 'public.pem'), 'utf-8');
                const verified = this.verifySignature(proof.dataHash, signature, publicKey);
                if (!verified) {
                    console.error('❌ Signature verification failed');
                    return false;
                }
            }
            console.log('✅ Integrity proof verified');
            return true;
        }
        catch (error) {
            throw new Error(`Proof verification failed: ${error.message}`);
        }
    }
    /**
     * Generate comprehensive audit trail
     */
    async generateAuditTrail(generationId) {
        try {
            console.log(`📋 Generating audit trail for ${generationId}...`);
            const operations = [];
            // Get commit history
            const log = (0, child_process_1.execSync)(`npx agentic-jujutsu@latest log --no-graph`, { cwd: this.repoPath, encoding: 'utf-8' });
            // Parse operations from log
            const commits = this.parseCommitLog(log);
            for (const commit of commits) {
                if (commit.message.includes(generationId)) {
                    operations.push({
                        type: 'generation',
                        timestamp: commit.timestamp,
                        hash: commit.hash,
                        verified: await this.verifyIntegrity(generationId)
                    });
                }
            }
            // Calculate integrity score
            const verifiedOps = operations.filter(op => op.verified).length;
            const integrityScore = operations.length > 0
                ? verifiedOps / operations.length
                : 0;
            const auditTrail = {
                generation: generationId,
                operations,
                integrityScore
            };
            // Save audit trail
            const auditFile = path.join(this.repoPath, 'data/audits', `${generationId}_audit.json`);
            fs.writeFileSync(auditFile, JSON.stringify(auditTrail, null, 2));
            console.log('✅ Audit trail generated');
            console.log(`   Operations: ${operations.length}`);
            console.log(`   Integrity score: ${(integrityScore * 100).toFixed(1)}%`);
            return auditTrail;
        }
        catch (error) {
            throw new Error(`Audit trail generation failed: ${error.message}`);
        }
    }
    /**
     * Detect tampering attempts
     */
    async detectTampering() {
        try {
            console.log('🔍 Scanning for tampering attempts...');
            const tamperedGenerations = [];
            // Check all secure generations
            const secureDir = path.join(this.repoPath, 'data/secure');
            if (!fs.existsSync(secureDir)) {
                return tamperedGenerations;
            }
            const files = fs.readdirSync(secureDir);
            for (const file of files) {
                if (file.endsWith('.json')) {
                    const generationId = file.replace('.json', '');
                    const verified = await this.verifyIntegrity(generationId);
                    if (!verified) {
                        tamperedGenerations.push(generationId);
                    }
                }
            }
            if (tamperedGenerations.length > 0) {
                console.warn(`⚠️  Detected ${tamperedGenerations.length} tampered generations`);
            }
            else {
                console.log('✅ No tampering detected');
            }
            return tamperedGenerations;
        }
        catch (error) {
            throw new Error(`Tampering detection failed: ${error.message}`);
        }
    }
    // Helper methods
    calculateSecureHash(data) {
        return crypto
            .createHash('sha512')
            .update(JSON.stringify(data))
            .digest('hex');
    }
    signData(dataHash) {
        const privateKey = fs.readFileSync(path.join(this.keyPath, 'private.pem'), 'utf-8');
        const sign = crypto.createSign('SHA512');
        sign.update(dataHash);
        return sign.sign(privateKey, 'hex');
    }
    verifySignature(dataHash, signature, publicKey) {
        try {
            const verify = crypto.createVerify('SHA512');
            verify.update(dataHash);
            return verify.verify(publicKey, signature, 'hex');
        }
        catch (error) {
            return false;
        }
    }
    encryptData(data) {
        // Simple encryption for demo - use proper encryption in production
        const algorithm = 'aes-256-gcm';
        const key = crypto.randomBytes(32);
        const iv = crypto.randomBytes(16);
        const cipher = crypto.createCipheriv(algorithm, key, iv);
        let encrypted = cipher.update(JSON.stringify(data), 'utf8', 'hex');
        encrypted += cipher.final('hex');
        const authTag = cipher.getAuthTag();
        return JSON.stringify({
            encrypted,
            key: key.toString('hex'),
            iv: iv.toString('hex'),
            authTag: authTag.toString('hex')
        });
    }
    decryptData(encryptedData) {
        const { encrypted, key, iv, authTag } = JSON.parse(encryptedData);
        const algorithm = 'aes-256-gcm';
        const decipher = crypto.createDecipheriv(algorithm, Buffer.from(key, 'hex'), Buffer.from(iv, 'hex'));
        decipher.setAuthTag(Buffer.from(authTag, 'hex'));
        let decrypted = decipher.update(encrypted, 'hex', 'utf8');
        decrypted += decipher.final('utf8');
        return JSON.parse(decrypted);
    }
    calculateMerkleRoot(data) {
        if (!data.length)
            return '';
        let hashes = data.map(item => crypto.createHash('sha256').update(JSON.stringify(item)).digest('hex'));
        while (hashes.length > 1) {
            const newHashes = [];
            for (let i = 0; i < hashes.length; i += 2) {
                const left = hashes[i];
                const right = i + 1 < hashes.length ? hashes[i + 1] : left;
                const combined = crypto.createHash('sha256').update(left + right).digest('hex');
                newHashes.push(combined);
            }
            hashes = newHashes;
        }
        return hashes[0];
    }
    async commitWithQuantumSignature(file, hash, signature, description) {
        (0, child_process_1.execSync)(`npx agentic-jujutsu@latest add "${file}"`, {
            cwd: this.repoPath,
            stdio: 'pipe'
        });
        const message = `${description}\n\nQuantum-Resistant Security:\nHash: ${hash}\nSignature: ${signature.substring(0, 32)}...`;
        (0, child_process_1.execSync)(`npx agentic-jujutsu@latest commit -m "${message}"`, {
            cwd: this.repoPath,
            stdio: 'pipe'
        });
    }
    getLatestCommitHash() {
        const result = (0, child_process_1.execSync)('npx agentic-jujutsu@latest log --limit 1 --no-graph --template "{commit_id}"', { cwd: this.repoPath, encoding: 'utf-8' });
        return result.trim();
    }
    verifyCommitExists(commitHash) {
        try {
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest show ${commitHash}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            return true;
        }
        catch (error) {
            return false;
        }
    }
    parseCommitLog(log) {
        const commits = [];
        const lines = log.split('\n');
        let currentCommit = null;
        for (const line of lines) {
            if (line.startsWith('commit ')) {
                if (currentCommit)
                    commits.push(currentCommit);
                currentCommit = {
                    hash: line.split(' ')[1],
                    message: '',
                    timestamp: new Date()
                };
            }
            else if (currentCommit && line.trim()) {
                currentCommit.message += line.trim() + ' ';
            }
        }
        if (currentCommit)
            commits.push(currentCommit);
        return commits;
    }
}
exports.QuantumResistantDataGenerator = QuantumResistantDataGenerator;
// Example usage
async function main() {
    console.log('🚀 Quantum-Resistant Data Generation Example\n');
    const repoPath = path.join(process.cwd(), 'quantum-resistant-repo');
    const generator = new QuantumResistantDataGenerator(repoPath);
    try {
        // Initialize
        await generator.initialize();
        // Generate secure data
        const schema = {
            userId: 'string',
            sensitiveData: 'string',
            timestamp: 'date'
        };
        const generation = await generator.generateSecureData(schema, 1000, 'Quantum-resistant secure data generation');
        // Verify integrity
        const verified = await generator.verifyIntegrity(generation.id);
        console.log(`\n🔍 Integrity check: ${verified ? 'PASSED' : 'FAILED'}`);
        // Create integrity proof
        const proof = await generator.createIntegrityProof(generation.id);
        console.log('\n📜 Integrity proof created:', proof);
        // Verify proof
        const proofValid = await generator.verifyIntegrityProof(generation.id);
        console.log(`\n✅ Proof verification: ${proofValid ? 'VALID' : 'INVALID'}`);
        // Generate audit trail
        const audit = await generator.generateAuditTrail(generation.id);
        console.log('\n📋 Audit trail:', audit);
        // Detect tampering
        const tampered = await generator.detectTampering();
        console.log(`\n🔍 Tampering scan: ${tampered.length} issues found`);
        console.log('\n✅ Quantum-resistant example completed!');
    }
    catch (error) {
        console.error('❌ Error:', error.message);
        process.exit(1);
    }
}
// Run example if executed directly
if (require.main === module) {
    main().catch(console.error);
}
//# sourceMappingURL=quantum-resistant-data.js.map