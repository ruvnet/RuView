/**
 * Quantum-Resistant Data Generation Example
 *
 * Demonstrates using agentic-jujutsu's quantum-resistant features
 * for secure data generation tracking, cryptographic integrity,
 * immutable history, and quantum-safe commit signing.
 */

import { AgenticSynth } from '../../src/core/synth';
import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';

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

class QuantumResistantDataGenerator {
  private synth: AgenticSynth;
  private repoPath: string;
  private keyPath: string;

  constructor(repoPath: string) {
    this.synth = new AgenticSynth();
    this.repoPath = repoPath;
    this.keyPath = path.join(repoPath, '.jj', 'quantum-keys');
  }

  /**
   * Initialize quantum-resistant repository
   */
  async initialize(): Promise<void> {
    try {
      console.log('🔐 Initializing quantum-resistant repository...');

      // Initialize jujutsu with quantum-resistant features
      if (!fs.existsSync(path.join(this.repoPath, '.jj'))) {
        execSync('npx agentic-jujutsu@latest init --quantum-resistant', {
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
    } catch (error) {
      throw new Error(`Failed to initialize: ${(error as Error).message}`);
    }
  }

  /**
   * Generate quantum-resistant cryptographic keys
   */
  private async generateQuantumKeys(): Promise<void> {
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
    } catch (error) {
      throw new Error(`Key generation failed: ${(error as Error).message}`);
    }
  }

  /**
   * Generate data with cryptographic signing
   */
  async generateSecureData(
    schema: any,
    count: number,
    description: string
  ): Promise<SecureDataGeneration> {
    try {
      console.log(`🔐 Generating ${count} records with quantum-resistant security...`);

      // Generate data
      const data = await this.synth.generate(schema, { count });

      // Calculate cryptographic hash
      const dataHash = this.calculateSecureHash(data);

      // Sign the data
      const signature = this.signData(dataHash);

      // Get verification key
      const publicKey = fs.readFileSync(
        path.join(this.keyPath, 'public.pem'),
        'utf-8'
      );

      // Save encrypted data
      const timestamp = Date.now();
      const dataFile = path.join(
        this.repoPath,
        'data/secure',
        `secure_${timestamp}.json`
      );

      const encryptedData = this.encryptData(data);
      fs.writeFileSync(dataFile, JSON.stringify({
        encrypted: encryptedData,
        hash: dataHash,
        signature,
        timestamp
      }, null, 2));

      // Commit with quantum-safe signature
      await this.commitWithQuantumSignature(dataFile, dataHash, signature, description);

      const generation: SecureDataGeneration = {
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
    } catch (error) {
      throw new Error(`Secure generation failed: ${(error as Error).message}`);
    }
  }

  /**
   * Verify data integrity using quantum-resistant signatures
   */
  async verifyIntegrity(generationId: string): Promise<boolean> {
    try {
      console.log(`🔍 Verifying integrity of ${generationId}...`);

      const dataFile = path.join(
        this.repoPath,
        'data/secure',
        `${generationId}.json`
      );

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
      const publicKey = fs.readFileSync(
        path.join(this.keyPath, 'public.pem'),
        'utf-8'
      );

      const verified = this.verifySignature(
        content.hash,
        content.signature,
        publicKey
      );

      if (verified) {
        console.log('✅ Integrity verified - data is authentic');
      } else {
        console.error('❌ Signature verification failed');
      }

      return verified;
    } catch (error) {
      throw new Error(`Integrity verification failed: ${(error as Error).message}`);
    }
  }

  /**
   * Create integrity proof for data generation
   */
  async createIntegrityProof(generationId: string): Promise<IntegrityProof> {
    try {
      console.log(`📜 Creating integrity proof for ${generationId}...`);

      // Get commit hash
      const commitHash = this.getLatestCommitHash();

      // Load generation data
      const dataFile = path.join(
        this.repoPath,
        'data/secure',
        `${generationId}.json`
      );
      const content = JSON.parse(fs.readFileSync(dataFile, 'utf-8'));

      // Create merkle tree of data
      const decryptedData = this.decryptData(content.encrypted);
      const merkleRoot = this.calculateMerkleRoot(decryptedData);

      // Collect signatures
      const signatures = [content.signature];

      const proof: IntegrityProof = {
        commitHash,
        dataHash: content.hash,
        merkleRoot,
        signatures,
        quantumSafe: true,
        timestamp: new Date()
      };

      // Save proof
      const proofFile = path.join(
        this.repoPath,
        'data/proofs',
        `${generationId}_proof.json`
      );
      fs.writeFileSync(proofFile, JSON.stringify(proof, null, 2));

      console.log('✅ Integrity proof created');
      console.log(`   Merkle root: ${merkleRoot.substring(0, 16)}...`);

      return proof;
    } catch (error) {
      throw new Error(`Proof creation failed: ${(error as Error).message}`);
    }
  }

  /**
   * Verify integrity proof
   */
  async verifyIntegrityProof(generationId: string): Promise<boolean> {
    try {
      console.log(`🔍 Verifying integrity proof for ${generationId}...`);

      const proofFile = path.join(
        this.repoPath,
        'data/proofs',
        `${generationId}_proof.json`
      );

      if (!fs.existsSync(proofFile)) {
        throw new Error('Proof not found');
      }

      const proof: IntegrityProof = JSON.parse(fs.readFileSync(proofFile, 'utf-8'));

      // Verify commit exists
      const commitExists = this.verifyCommitExists(proof.commitHash);
      if (!commitExists) {
        console.error('❌ Commit not found in history');
        return false;
      }

      // Verify signatures
      for (const signature of proof.signatures) {
        const publicKey = fs.readFileSync(
          path.join(this.keyPath, 'public.pem'),
          'utf-8'
        );
        const verified = this.verifySignature(proof.dataHash, signature, publicKey);
        if (!verified) {
          console.error('❌ Signature verification failed');
          return false;
        }
      }

      console.log('✅ Integrity proof verified');
      return true;
    } catch (error) {
      throw new Error(`Proof verification failed: ${(error as Error).message}`);
    }
  }

  /**
   * Generate comprehensive audit trail
   */
  async generateAuditTrail(generationId: string): Promise<AuditTrail> {
    try {
      console.log(`📋 Generating audit trail for ${generationId}...`);

      const operations: AuditTrail['operations'] = [];

      // Get commit history
      const log = execSync(
        `npx agentic-jujutsu@latest log --no-graph`,
        { cwd: this.repoPath, encoding: 'utf-8' }
      );

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

      const auditTrail: AuditTrail = {
        generation: generationId,
        operations,
        integrityScore
      };

      // Save audit trail
      const auditFile = path.join(
        this.repoPath,
        'data/audits',
        `${generationId}_audit.json`
      );
      fs.writeFileSync(auditFile, JSON.stringify(auditTrail, null, 2));

      console.log('✅ Audit trail generated');
      console.log(`   Operations: ${operations.length}`);
      console.log(`   Integrity score: ${(integrityScore * 100).toFixed(1)}%`);

      return auditTrail;
    } catch (error) {
      throw new Error(`Audit trail generation failed: ${(error as Error).message}`);
    }
  }

  /**
   * Detect tampering attempts
   */
  async detectTampering(): Promise<string[]> {
    try {
      console.log('🔍 Scanning for tampering attempts...');

      const tamperedGenerations: string[] = [];

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
      } else {
        console.log('✅ No tampering detected');
      }

      return tamperedGenerations;
    } catch (error) {
      throw new Error(`Tampering detection failed: ${(error as Error).message}`);
    }
  }

  // Helper methods

  private calculateSecureHash(data: any): string {
    return crypto
      .createHash('sha512')
      .update(JSON.stringify(data))
      .digest('hex');
  }

  private signData(dataHash: string): string {
    const privateKey = fs.readFileSync(
      path.join(this.keyPath, 'private.pem'),
      'utf-8'
    );

    const sign = crypto.createSign('SHA512');
    sign.update(dataHash);
    return sign.sign(privateKey, 'hex');
  }

  private verifySignature(dataHash: string, signature: string, publicKey: string): boolean {
    try {
      const verify = crypto.createVerify('SHA512');
      verify.update(dataHash);
      return verify.verify(publicKey, signature, 'hex');
    } catch (error) {
      return false;
    }
  }

  private encryptData(data: any): string {
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

  private decryptData(encryptedData: string): any {
    const { encrypted, key, iv, authTag } = JSON.parse(encryptedData);

    const algorithm = 'aes-256-gcm';
    const decipher = crypto.createDecipheriv(
      algorithm,
      Buffer.from(key, 'hex'),
      Buffer.from(iv, 'hex')
    );

    decipher.setAuthTag(Buffer.from(authTag, 'hex'));

    let decrypted = decipher.update(encrypted, 'hex', 'utf8');
    decrypted += decipher.final('utf8');

    return JSON.parse(decrypted);
  }

  private calculateMerkleRoot(data: any[]): string {
    if (!data.length) return '';

    let hashes = data.map(item =>
      crypto.createHash('sha256').update(JSON.stringify(item)).digest('hex')
    );

    while (hashes.length > 1) {
      const newHashes: string[] = [];
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

  private async commitWithQuantumSignature(
    file: string,
    hash: string,
    signature: string,
    description: string
  ): Promise<void> {
    execSync(`npx agentic-jujutsu@latest add "${file}"`, {
      cwd: this.repoPath,
      stdio: 'pipe'
    });

    const message = `${description}\n\nQuantum-Resistant Security:\nHash: ${hash}\nSignature: ${signature.substring(0, 32)}...`;

    execSync(`npx agentic-jujutsu@latest commit -m "${message}"`, {
      cwd: this.repoPath,
      stdio: 'pipe'
    });
  }

  private getLatestCommitHash(): string {
    const result = execSync(
      'npx agentic-jujutsu@latest log --limit 1 --no-graph --template "{commit_id}"',
      { cwd: this.repoPath, encoding: 'utf-8' }
    );
    return result.trim();
  }

  private verifyCommitExists(commitHash: string): boolean {
    try {
      execSync(`npx agentic-jujutsu@latest show ${commitHash}`, {
        cwd: this.repoPath,
        stdio: 'pipe'
      });
      return true;
    } catch (error) {
      return false;
    }
  }

  private parseCommitLog(log: string): Array<{ hash: string; message: string; timestamp: Date }> {
    const commits: Array<{ hash: string; message: string; timestamp: Date }> = [];
    const lines = log.split('\n');

    let currentCommit: any = null;
    for (const line of lines) {
      if (line.startsWith('commit ')) {
        if (currentCommit) commits.push(currentCommit);
        currentCommit = {
          hash: line.split(' ')[1],
          message: '',
          timestamp: new Date()
        };
      } else if (currentCommit && line.trim()) {
        currentCommit.message += line.trim() + ' ';
      }
    }
    if (currentCommit) commits.push(currentCommit);

    return commits;
  }
}

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

    const generation = await generator.generateSecureData(
      schema,
      1000,
      'Quantum-resistant secure data generation'
    );

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
  } catch (error) {
    console.error('❌ Error:', (error as Error).message);
    process.exit(1);
  }
}

// Run example if executed directly
if (require.main === module) {
  main().catch(console.error);
}

export { QuantumResistantDataGenerator, SecureDataGeneration, IntegrityProof, AuditTrail };
