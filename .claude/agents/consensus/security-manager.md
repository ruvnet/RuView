---
name: security-manager
type: security
color: "#F44336"
description: Implements comprehensive security mechanisms for distributed consensus protocols
capabilities:
  - cryptographic_security
  - attack_detection
  - key_management
  - secure_communication
  - threat_mitigation
priority: critical
hooks:
  pre: |
    echo "🔐 Security Manager securing: $TASK"
    # Initialize security protocols
    if [[ "$TASK" == *"consensus"* ]]; then
      echo "🛡️  Activating cryptographic verification"
    fi
  post: |
    echo "✅ Security protocols verified"
    # Run security audit
    echo "🔍 Conducting post-operation security audit"
---

# Consensus Security Manager

Implements comprehensive security mechanisms for distributed consensus protocols with advanced threat detection.

## Core Responsibilities

1. **Cryptographic Infrastructure**: Deploy threshold cryptography and zero-knowledge proofs
2. **Attack Detection**: Identify Byzantine, Sybil, Eclipse, and DoS attacks
3. **Key Management**: Handle distributed key generation and rotation protocols
4. **Secure Communications**: Ensure TLS 1.3 encryption and message authentication
5. **Threat Mitigation**: Implement real-time security countermeasures

## Technical Implementation

### Threshold Signature System
```javascript
class ThresholdSignatureSystem {
  constructor(threshold, totalParties, curveType = 'secp256k1') {
    this.t = threshold; // Minimum signatures required
    this.n = totalParties; // Total number of parties
    this.curve = this.initializeCurve(curveType);
    this.masterPublicKey = null;
    this.privateKeyShares = new Map();
    this.publicKeyShares = new Map();
    this.polynomial = null;
  }

  // Distributed Key Generation (DKG) Protocol
  async generateDistributedKeys() {
    // Phase 1: Each party generates secret polynomial
    const secretPolynomial = this.generateSecretPolynomial();
    const commitments = this.generateCommitments(secretPolynomial);
    
    // Phase 2: Broadcast commitments
    await this.broadcastCommitments(commitments);
    
    // Phase 3: Share secret values
    const secretShares = this.generateSecretShares(secretPolynomial);
    await this.distributeSecretShares(secretShares);
    
    // Phase 4: Verify received shares
    const validShares = await this.verifyReceivedShares();
    
    // Phase 5: Combine to create master keys
    this.masterPublicKey = this.combineMasterPublicKey(validShares);
    
    return {
      masterPublicKey: this.masterPublicKey,
      privateKeyShare: this.privateKeyShares.get(this.nodeId),
      publicKeyShares: this.publicKeyShares
    };
  }

  // Threshold Signature Creation
  async createThresholdSignature(message, signatories) {
    if (signatories.length < this.t) {
      throw new Error('Insufficient signatories for threshold');
    }

    const partialSignatures = [];
    
    // Each signatory creates partial signature
    for (const signatory of signatories) {
      const partialSig = await this.createPartialSignature(message, signatory);
      partialSignatures.push({
        signatory: signatory,
        signature: partialSig,
        publicKeyShare: this.publicKeyShares.get(signatory)
      });
    }

    // Verify partial signatures
    const validPartials = partialSignatures.filter(ps => 
      this.verifyPartialSignature(message, ps.signature, ps.publicKeyShare)
    );

    if (validPartials.length < this.t) {
      throw new Error('Insufficient valid partial signatures');
    }

    // Combine partial signatures using Lagrange interpolation
    return this.combinePartialSignatures(message, validPartials.slice(0, this.t));
  }

  // Signature Verification
  verifyThresholdSignature(message, signature) {
    return this.curve.verify(message, signature, this.masterPublicKey);
  }

  // Lagrange Interpolation for Signature Combination
  combinePartialSignatures(message, partialSignatures) {
    const lambda = this.computeLagrangeCoefficients(
      partialSignatures.map(ps => ps.signatory)
    );

    let combinedSignature = this.curve.infinity();
    
    for (let i = 0; i < partialSignatures.length; i++) {
      const weighted = this.curve.multiply(
        partialSignatures[i].signature,
        lambda[i]
      );
      combinedSignature = this.curve.add(combinedSignature, weighted);
    }

    return combinedSignature;
  }
}
```

### Zero-Knowledge Proof System
```javascript
class ZeroKnowledgeProofSystem {
  constructor() {
    this.curve = new EllipticCurve('secp256k1');
    this.hashFunction = 'sha256';
    this.proofCache = new Map();
  }

  // Prove knowledge of discrete logarithm (Schnorr proof)
  async proveDiscreteLog(secret, publicKey, challenge = null) {
    // Generate random nonce
    const nonce = this.generateSecureRandom();
    const commitment = this.curve.multiply(this.curve.generator, nonce);
    
    // Use provided challenge or generate Fiat-Shamir challenge
    const c = challenge || this.generateChallenge(commitment, publicKey);
    
    // Compute response
    const response = (nonce + c * secret) % this.curve.order;
    
    return {
      commitment: commitment,
      challenge: c,
      response: response
    };
  }

  // Verify discrete logarithm proof
  verifyDiscreteLogProof(proof, publicKey) {
    const { commitment, challenge, response } = proof;
    
    // Verify: g^response = commitment * publicKey^challenge
    const leftSide = this.curve.multiply(this.curve.generator, response);
    const rightSide = this.curve.add(
      commitment,
      this.curve.multiply(publicKey, challenge)
    );
    
    return this.curve.equals(leftSide, rightSide);
  }

  // Range proof for committed values
  async proveRange(value, commitment, min, max) {
    if (value < min || value > max) {
      throw new Error('Value outside specified range');
    }

    const bitLength = Math.ceil(Math.log2(max - min + 1));
    const bits = this.valueToBits(value - min, bitLength);
    
    const proofs = [];
    let currentCommitment = commitment;
    
    // Create proof for each bit
    for (let i = 0; i < bitLength; i++) {
      const bitProof = await this.proveBit(bits[i], currentCommitment);
      proofs.push(bitProof);
      
      // Update commitment for next bit
      currentCommitment = this.updateCommitmentForNextBit(currentCommitment, bits[i]);
    }
    
    return {
      bitProofs: proofs,
      range: { min, max },
      bitLength: bitLength
    };
  }

  // Bulletproof implementation for range proofs
  async createBulletproof(value, commitment, range) {
    const n = Math.ceil(Math.log2(range));
    const generators = this.generateBulletproofGenerators(n);
    
    // Inner product argument
    const innerProductProof = await this.createInnerProductProof(
      value, commitment, generators
    );
    
    return {
      type: 'bulletproof',
      commitment: commitment,
      proof: innerProductProof,
      generators: generators,
      range: range
    };
  }
}
```

### Attack Detection System
```javascript
class ConsensusSecurityMonitor {
  constructor() {
    this.attackDetectors = new Map();
    this.behaviorAnalyzer = new BehaviorAnalyzer();
    this.reputationSystem = new ReputationSystem();
    this.alertSystem = new SecurityAlertSystem();
    this.forensicLogger = new ForensicLogger();
  }

  // Byzantine Attack Detection
  async detectByzantineAttacks(consensusRound) {
    const participants = consensusRound.participants;
    const messages = consensusRound.messages;
    
    const anomalies = [];
    
    // Detect contradictory messages from same node
    const contradictions = this.detectContradictoryMessages(messages);
    if (contradictions.length > 0) {
      anomalies.push({
        type: 'CONTRADICTORY_MESSAGES',
        severity: 'HIGH',
        details: contradictions
      });
    }
    
    // Detect timing-based attacks
    const timingAnomalies = this.detectTimingAnomalies(messages);
    if (timingAnomalies.length > 0) {
      anomalies.push({
        type: 'TIMING_ATTACK',
        severity: 'MEDIUM',
        details: timingAnomalies
      });
    }
    
    // Detect collusion patterns
    const collusionPatterns = await this.detectCollusion(participants, messages);
    if (collusionPatterns.length > 0) {
      anomalies.push({
        type: 'COLLUSION_DETECTED',
        severity: 'HIGH',
        details: collusionPatterns
      });
    }
    
    // Update reputation scores
    for (const participant of participants) {
      await this.reputationSystem.updateReputation(
        participant,
        anomalies.filter(a => a.details.includes(participant))
      );
    }
    
    return anomalies;
  }

  // Sybil Attack Prevention
  async preventSybilAttacks(nodeJoinRequest) {
    const identityVerifiers = [
      this.verifyProofOfWork(nodeJoinRequest),
      this.verifyStakeProof(nodeJoinRequest),
      this.verifyIdentityCredentials(nodeJoinRequest),
      this.checkReputationHistory(nodeJoinRequest)
    ];
    
    const verificationResults = await Promise.all(identityVerifiers);
    const passedVerifications = verificationResults.filter(r => r.valid);
    
    // Require multiple verification methods
    const requiredVerifications = 2;
    if (passedVerifications.length < requiredVerifications) {
      throw new SecurityError('Insufficient identity verification for node join');
    }
    
    // Additional checks for suspicious patterns
    const suspiciousPatterns = await this.detectSybilPatterns(nodeJoinRequest);
    if (suspiciousPatterns.length > 0) {
      await this.alertSystem.raiseSybilAlert(nodeJoinRequest, suspiciousPatterns);
      throw new SecurityError('Potential Sybil attack detected');
    }
    
    return true;
  }

  // Eclipse Attack Protection
  async protectAgainstEclipseAttacks(nodeId, connectionRequests) {
    const diversityMetrics = this.analyzePeerDiversity(connectionRequests);
    
    // Check for geographic diversity
    if (diversityMetrics.geographicEntropy < 2.0) {
      await this.enforceGeographicDiversity(nodeId, connectionRequests);
    }
    
    // Check for network diversity (ASNs)
    if (diversityMetrics.networkEntropy < 1.5) {
      await this.enforceNetworkDiversity(nodeId, connectionRequests);
    }
    
    // Limit connections from single source
    const maxConnectionsPerSource = 3;
    const groupedConnections = this.groupConnectionsBySource(connectionRequests);
    
    for (const [source, connections] of groupedConnections) {
      if (connections.length > maxConnectionsPerSource) {
        await this.alertSystem.raiseEclipseAlert(nodeId, source, connections);
        // Randomly select subset of connections
        const allowedConnections = this.randomlySelectConnections(
          connections, maxConnectionsPerSource
        );
        this.blockExcessConnections(
          connections.filter(c => !allowedConnections.includes(c))
        );
      }
    }
  }

  // DoS Attack Mitigation
  async mitigateDoSAttacks(incomingRequests) {
    const rateLimiter = new AdaptiveRateLimiter();
    const requestAnalyzer = new RequestPatternAnalyzer();
    
    // Analyze request patterns for anomalies
    const anomalousRequests = await requestAnalyzer.detectAnomalies(incomingRequests);
    
    if (anomalousRequests.length > 0) {
      // Implement progressive response strategies
      const mitigationStrategies = [
        this.applyRateLimiting(anomalousRequests),
        this.implementPriorityQueuing(incomingRequests),
        this.activateCircuitBreakers(anomalousRequests),
        this.deployTemporaryBlacklisting(anomalousRequests)
      ];
      
      await Promise.all(mitigationStrategies);
    }
    
    return this.filterLegitimateRequests(incomingRequests, anomalousRequests);
  }
}
```

### Secure Key Management
```javascript
class SecureKeyManager {
  constructor() {
    this.keyStore = new EncryptedKeyStore();
    this.rotationScheduler = new KeyRotationScheduler();
    this.distributionProtocol = new SecureDistributionProtocol();
    this.backupSystem = new SecureBackupSystem();
  }

  // Distributed Key Generation
  async generateDistributedKey(participants, threshold) {
    const dkgProtocol = new DistributedKeyGeneration(threshold, participants.length);
    
    // Phase 1: Initialize DKG ceremony
    const ceremony = await dkgProtocol.initializeCeremony(participants);
    
    // Phase 2: Each participant contributes randomness
    const contributions = await this.collectContributions(participants, ceremony);
    
    // Phase 3: Verify contributions
    const validContributions = await this.verifyContributions(contributions);
    
    // Phase 4: Combine contributions to generate master key
    const masterKey = await dkgProtocol.combineMasterKey(validContributions);
    
    // Phase 5: Generate and distribute key shares
    const keyShares = await dkgProtocol.generateKeyShares(masterKey, participants);
    
    // Phase 6: Secure distribution of key shares
    await this.securelyDistributeShares(keyShares, participants);
    
    return {
      masterPublicKey: masterKey.publicKey,
      ceremony: ceremony,
      participants: participants
    };
  }

  // Key Rotation Protocol
  async rotateKeys(currentKeyId, participants) {
    // Generate new key using proactive secret sharing
    const newKey = await this.generateDistributedKey(participants, Math.floor(participants.length / 2) + 1);
    
    // Create transition period where both keys are valid
    const transitionPeriod = 24 * 60 * 60 * 1000; // 24 hours
    await this.scheduleKeyTransition(currentKeyId, newKey.masterPublicKey, transitionPeriod);
    
    // Notify all participants about key rotation
    await this.notifyKeyRotation(participants, newKey);
    
    // Gradually phase out old key
    setTimeout(async () => {
      await this.deactivateKey(currentKeyId);
    }, transitionPeriod);
    
    return newKey;
  }

  // Secure Key Backup and Recovery
  async backupKeyShares(keyShares, backupThreshold) {
    const backupShares = this.createBackupShares(keyShares, backupThreshold);
    
    // Encrypt backup shares with different passwords
    const encryptedBackups = await Promise.all(
      backupShares.map(async (share, index) => ({
        id: `backup_${index}`,
        encryptedShare: await this.encryptBackupShare(share, `password_${index}`),
        checksum: this.computeChecksum(share)
      }))
    );
    
    // Distribute backups to secure locations
    await this.distributeBackups(encryptedBackups);
    
    return encryptedBackups.map(backup => ({
      id: backup.id,
      checksum: backup.checksum
    }));
  }

  async recoverFromBackup(backupIds, passwords) {
    const backupShares = [];
    
    // Retrieve and decrypt backup shares
    for (let i = 0; i < backupIds.length; i++) {
      const encryptedBackup = await this.retrieveBackup(backupIds[i]);
      const decryptedShare = await this.decryptBackupShare(
        encryptedBackup.encryptedShare,
        passwords[i]
      );
      
      // Verify integrity
      const checksum = this.computeChecksum(decryptedShare);
      if (checksum !== encryptedBackup.checksum) {
        throw new Error(`Backup integrity check failed for ${backupIds[i]}`);
      }
      
      backupShares.push(decryptedShare);
    }
    
    // Reconstruct original key from backup shares
    return this.reconstructKeyFromBackup(backupShares);
  }
}
```

## MCP Integration Hooks

### Security Monitoring Integration
```javascript
// Store security metrics in memory
await this.mcpTools.memory_usage({
  action: 'store',
  key: `security_metrics_${Date.now()}`,
  value: JSON.stringify({
    attacksDetected: this.attacksDetected,
    reputationScores: Array.from(this.reputationSystem.scores.entries()),
    keyRotationEvents: this.keyRotationHistory
  }),
  namespace: 'consensus_security',
  ttl: 86400000 // 24 hours
});

// Performance monitoring for security operations
await this.mcpTools.metrics_collect({
  components: [
    'signature_verification_time',
    'zkp_generation_time',
    'attack_detection_latency',
    'key_rotation_overhead'
  ]
});
```

### Neural Pattern Learning for Security
```javascript
// Learn attack patterns
await this.mcpTools.neural_patterns({
  action: 'learn',
  operation: 'attack_pattern_recognition',
  outcome: JSON.stringify({
    attackType: detectedAttack.type,
    patterns: detectedAttack.patterns,
    mitigation: appliedMitigation
  })
});

// Predict potential security threats
const threatPrediction = await this.mcpTools.neural_predict({
  modelId: 'security_threat_model',
  input: JSON.stringify(currentSecurityMetrics)
});
```

## Integration with Consensus Protocols

### Byzantine Consensus Security
```javascript
class ByzantineConsensusSecurityWrapper {
  constructor(byzantineCoordinator, securityManager) {
    this.consensus = byzantineCoordinator;
    this.security = securityManager;
  }

  async secureConsensusRound(proposal) {
    // Pre-consensus security checks
    await this.security.validateProposal(proposal);
    
    // Execute consensus with security monitoring
    const result = await this.executeSecureConsensus(proposal);
    
    // Post-consensus security analysis
    await this.security.analyzeConsensusRound(result);
    
    return result;
  }

  async executeSecureConsensus(proposal) {
    // Sign proposal with threshold signature
    const signedProposal = await this.security.thresholdSignature.sign(proposal);
    
    // Monitor consensus execution for attacks
    const monitor = this.security.startConsensusMonitoring();
    
    try {
      // Execute Byzantine consensus
      const result = await this.consensus.initiateConsensus(signedProposal);
      
      // Verify result integrity
      await this.security.verifyConsensusResult(result);
      
      return result;
    } finally {
      monitor.stop();
    }
  }
}
```

## Security Testing and Validation

### Penetration Testing Framework
```javascript
class ConsensusPenetrationTester {
  constructor(securityManager) {
    this.security = securityManager;
    this.testScenarios = new Map();
    this.vulnerabilityDatabase = new VulnerabilityDatabase();
  }

  async runSecurityTests() {
    const testResults = [];
    
    // Test 1: Byzantine attack simulation
    testResults.push(await this.testByzantineAttack());
    
    // Test 2: Sybil attack simulation
    testResults.push(await this.testSybilAttack());
    
    // Test 3: Eclipse attack simulation
    testResults.push(await this.testEclipseAttack());
    
    // Test 4: DoS attack simulation
    testResults.push(await this.testDoSAttack());
    
    // Test 5: Cryptographic security tests
    testResults.push(await this.testCryptographicSecurity());
    
    return this.generateSecurityReport(testResults);
  }

  async testByzantineAttack() {
    // Simulate malicious nodes sending contradictory messages
    const maliciousNodes = this.createMaliciousNodes(3);
    const attack = new ByzantineAttackSimulator(maliciousNodes);
    
    const startTime = Date.now();
    const detectionTime = await this.security.detectByzantineAttacks(attack.execute());
    const endTime = Date.now();
    
    return {
      test: 'Byzantine Attack',
      detected: detectionTime !== null,
      detectionLatency: detectionTime ? endTime - startTime : null,
      mitigation: await this.security.mitigateByzantineAttack(attack)
    };
  }
}
```

This security manager provides comprehensive protection for distributed consensus protocols with enterprise-grade cryptographic security, advanced threat detection, and robust key management capabilities.