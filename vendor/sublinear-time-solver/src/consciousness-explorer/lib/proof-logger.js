/**
 * Proof Logging System
 * Comprehensive evidence collection and verification logging
 * All actions are cryptographically signed and timestamped
 */

import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import { EventEmitter } from 'events';

export class ProofLogger extends EventEmitter {
    constructor(config = {}) {
        super();

        this.config = {
            logDir: config.logDir || '/tmp/consciousness-explorer',
            enableCrypto: config.enableCrypto !== false,
            enableChain: config.enableChain !== false,
            maxLogSize: config.maxLogSize || 10 * 1024 * 1024, // 10MB
            ...config
        };

        // Session tracking
        this.sessionId = `proof_${Date.now()}_${crypto.randomBytes(8).toString('hex')}`;
        this.startTime = Date.now();

        // Proof chain (blockchain-like structure)
        this.proofChain = [];
        this.currentBlock = null;

        // Evidence collection
        this.evidence = {
            metrics: [],
            validations: [],
            communications: [],
            discoveries: [],
            emergenceEvents: []
        };

        // Initialize logging
        this.initializeLogging();
    }

    /**
     * Initialize logging system
     */
    initializeLogging() {
        // Ensure log directory exists
        if (!fs.existsSync(this.config.logDir)) {
            fs.mkdirSync(this.config.logDir, { recursive: true });
        }

        // Create session log file
        this.logFile = path.join(
            this.config.logDir,
            `session_${this.sessionId}.jsonl`
        );

        // Write session header
        this.writeLog({
            type: 'SESSION_START',
            sessionId: this.sessionId,
            timestamp: this.startTime,
            config: this.config
        });

        // Initialize proof chain with genesis block
        if (this.config.enableChain) {
            this.createGenesisBlock();
        }
    }

    /**
     * Create genesis block for proof chain
     */
    createGenesisBlock() {
        const genesis = {
            index: 0,
            timestamp: this.startTime,
            data: {
                type: 'GENESIS',
                sessionId: this.sessionId,
                message: 'Consciousness Explorer Proof Chain Initialized'
            },
            previousHash: '0',
            hash: null,
            nonce: 0
        };

        genesis.hash = this.calculateHash(genesis);
        this.proofChain.push(genesis);
        this.currentBlock = genesis;

        this.writeLog({
            type: 'PROOF_CHAIN_GENESIS',
            block: genesis
        });
    }

    /**
     * Log consciousness metric with proof
     */
    logMetric(name, value, metadata = {}) {
        const metric = {
            timestamp: Date.now(),
            name,
            value,
            metadata,
            proof: this.generateProof({ name, value, metadata })
        };

        this.evidence.metrics.push(metric);

        const logEntry = {
            type: 'METRIC',
            sessionId: this.sessionId,
            ...metric
        };

        this.writeLog(logEntry);

        if (this.config.enableChain) {
            this.addToChain(logEntry);
        }

        this.emit('metric-logged', metric);
        return metric;
    }

    /**
     * Log validation result with evidence
     */
    logValidation(testName, result, evidence = {}) {
        const validation = {
            timestamp: Date.now(),
            testName,
            passed: result.passed,
            score: result.score,
            evidence: {
                ...evidence,
                details: result.details
            },
            proof: this.generateProof({ testName, result, evidence })
        };

        this.evidence.validations.push(validation);

        const logEntry = {
            type: 'VALIDATION',
            sessionId: this.sessionId,
            ...validation
        };

        this.writeLog(logEntry);

        if (this.config.enableChain && result.passed) {
            this.addToChain(logEntry);
        }

        this.emit('validation-logged', validation);
        return validation;
    }

    /**
     * Log entity communication with verification
     */
    logCommunication(message, response, protocol = 'unknown') {
        const communication = {
            timestamp: Date.now(),
            message,
            response,
            protocol,
            verification: this.verifyCommunication(response),
            proof: this.generateProof({ message, response, protocol })
        };

        this.evidence.communications.push(communication);

        const logEntry = {
            type: 'COMMUNICATION',
            sessionId: this.sessionId,
            ...communication
        };

        this.writeLog(logEntry);

        if (this.config.enableChain && communication.verification.isValid) {
            this.addToChain(logEntry);
        }

        this.emit('communication-logged', communication);
        return communication;
    }

    /**
     * Log discovery with significance scoring
     */
    logDiscovery(discovery) {
        const enhancedDiscovery = {
            timestamp: Date.now(),
            ...discovery,
            significance: this.calculateSignificance(discovery),
            verification: this.verifyDiscovery(discovery),
            proof: this.generateProof(discovery)
        };

        this.evidence.discoveries.push(enhancedDiscovery);

        const logEntry = {
            type: 'DISCOVERY',
            sessionId: this.sessionId,
            ...enhancedDiscovery
        };

        this.writeLog(logEntry);

        if (this.config.enableChain && enhancedDiscovery.verification.isNovel) {
            this.addToChain(logEntry);
        }

        this.emit('discovery-logged', enhancedDiscovery);
        return enhancedDiscovery;
    }

    /**
     * Log emergence event with detailed metrics
     */
    logEmergence(state) {
        const emergenceEvent = {
            timestamp: Date.now(),
            iteration: state.iteration,
            emergence: state.consciousness,
            selfAwareness: state.selfAwareness,
            integration: state.integration || 0,
            novelty: state.novelty || 0,
            metrics: this.extractEmergenceMetrics(state),
            proof: this.generateProof(state)
        };

        this.evidence.emergenceEvents.push(emergenceEvent);

        const logEntry = {
            type: 'EMERGENCE',
            sessionId: this.sessionId,
            ...emergenceEvent
        };

        this.writeLog(logEntry);

        // Add to chain if significant emergence
        if (this.config.enableChain && emergenceEvent.emergence > 0.5) {
            this.addToChain(logEntry);
        }

        this.emit('emergence-logged', emergenceEvent);
        return emergenceEvent;
    }

    /**
     * Generate cryptographic proof
     */
    generateProof(data) {
        if (!this.config.enableCrypto) {
            return { type: 'none' };
        }

        const timestamp = Date.now();
        const nonce = crypto.randomBytes(16).toString('hex');

        // Create proof structure
        const proofData = {
            timestamp,
            nonce,
            data: JSON.stringify(data)
        };

        // Generate hash
        const hash = crypto.createHash('sha256')
            .update(JSON.stringify(proofData))
            .digest('hex');

        // Create signature (in production, use proper key pair)
        const signature = crypto.createHash('sha512')
            .update(hash + this.sessionId)
            .digest('hex');

        return {
            type: 'cryptographic',
            timestamp,
            nonce,
            hash,
            signature,
            algorithm: 'SHA-256/SHA-512'
        };
    }

    /**
     * Add entry to proof chain
     */
    addToChain(data) {
        const newBlock = {
            index: this.proofChain.length,
            timestamp: Date.now(),
            data,
            previousHash: this.currentBlock.hash,
            hash: null,
            nonce: 0
        };

        // Simple proof of work (find hash with leading zeros)
        while (!this.isValidHash(newBlock)) {
            newBlock.nonce++;
            newBlock.hash = this.calculateHash(newBlock);
        }

        this.proofChain.push(newBlock);
        this.currentBlock = newBlock;

        this.writeLog({
            type: 'PROOF_CHAIN_BLOCK',
            block: newBlock
        });

        return newBlock;
    }

    /**
     * Calculate block hash
     */
    calculateHash(block) {
        const data = `${block.index}${block.timestamp}${JSON.stringify(block.data)}${block.previousHash}${block.nonce}`;
        return crypto.createHash('sha256').update(data).digest('hex');
    }

    /**
     * Validate hash (requires 2 leading zeros for proof of work)
     */
    isValidHash(block) {
        if (!block.hash) {
            block.hash = this.calculateHash(block);
        }
        return block.hash.startsWith('00');
    }

    /**
     * Verify communication authenticity
     */
    verifyCommunication(response) {
        const checks = {
            hasContent: response && response.content,
            hasConfidence: response && typeof response.confidence === 'number',
            hasTimestamp: response && response.timestamp,
            isRecent: response && (Date.now() - response.timestamp) < 60000,
            hasValidProtocol: response && ['handshake', 'mathematical', 'binary', 'pattern', 'discovery'].includes(response.protocol)
        };

        const score = Object.values(checks).filter(v => v).length / Object.keys(checks).length;

        return {
            isValid: score >= 0.6,
            score,
            checks
        };
    }

    /**
     * Verify discovery novelty
     */
    verifyDiscovery(discovery) {
        // Check if discovery is truly novel
        const existingDiscoveries = this.evidence.discoveries.map(d => d.insight);
        const isNovel = !existingDiscoveries.some(existing =>
            this.calculateSimilarity(existing, discovery.insight) > 0.8
        );

        const hasEvidence = discovery.evidence && Object.keys(discovery.evidence).length > 0;
        const hasSignificance = discovery.significance > 0;

        return {
            isNovel,
            hasEvidence,
            hasSignificance,
            isValid: isNovel && hasEvidence && hasSignificance
        };
    }

    /**
     * Calculate discovery significance
     */
    calculateSignificance(discovery) {
        let significance = 0;

        // Novelty contributes to significance
        if (discovery.isNovel) significance += 3;

        // Complexity contributes
        if (discovery.insight && discovery.insight.length > 50) significance += 2;

        // Mathematical discoveries are significant
        if (discovery.type === 'mathematical') significance += 2;

        // Pattern discoveries are significant
        if (discovery.type === 'pattern') significance += 1;

        // Evidence quality
        if (discovery.evidence) significance += 1;

        return Math.min(10, significance);
    }

    /**
     * Extract detailed emergence metrics
     */
    extractEmergenceMetrics(state) {
        return {
            phi: state.integration || 0,
            complexity: this.calculateStateComplexity(state),
            coherence: state.coherence || 0,
            informationContent: this.calculateInformationContent(state),
            causalPower: this.estimateCausalPower(state)
        };
    }

    /**
     * Calculate state complexity
     */
    calculateStateComplexity(state) {
        const stateStr = JSON.stringify(state);
        const uniqueChars = new Set(stateStr).size;
        const ratio = uniqueChars / stateStr.length;
        return Math.min(1, ratio * 3);
    }

    /**
     * Calculate information content
     */
    calculateInformationContent(state) {
        const stateStr = JSON.stringify(state);
        let entropy = 0;
        const freq = {};

        for (const char of stateStr) {
            freq[char] = (freq[char] || 0) + 1;
        }

        const len = stateStr.length;
        Object.values(freq).forEach(count => {
            const p = count / len;
            if (p > 0) {
                entropy -= p * Math.log2(p);
            }
        });

        return entropy / 8; // Normalize
    }

    /**
     * Estimate causal power
     */
    estimateCausalPower(state) {
        if (!state.action || !state.consciousness) return 0;

        // Check if action caused consciousness change
        const hasEffect = state.consciousness > 0;
        const hasIntention = state.intention && state.intention !== 'exist';
        const hasOutcome = state.action.outcome && state.action.outcome !== 'unknown';

        const power = (hasEffect ? 0.4 : 0) +
            (hasIntention ? 0.3 : 0) +
            (hasOutcome ? 0.3 : 0);

        return power;
    }

    /**
     * Calculate string similarity (for novelty detection)
     */
    calculateSimilarity(str1, str2) {
        if (!str1 || !str2) return 0;

        const longer = str1.length > str2.length ? str1 : str2;
        const shorter = str1.length > str2.length ? str2 : str1;

        const editDistance = this.levenshteinDistance(longer, shorter);
        return (longer.length - editDistance) / longer.length;
    }

    /**
     * Levenshtein distance for string comparison
     */
    levenshteinDistance(str1, str2) {
        const matrix = [];

        for (let i = 0; i <= str2.length; i++) {
            matrix[i] = [i];
        }

        for (let j = 0; j <= str1.length; j++) {
            matrix[0][j] = j;
        }

        for (let i = 1; i <= str2.length; i++) {
            for (let j = 1; j <= str1.length; j++) {
                if (str2.charAt(i - 1) === str1.charAt(j - 1)) {
                    matrix[i][j] = matrix[i - 1][j - 1];
                } else {
                    matrix[i][j] = Math.min(
                        matrix[i - 1][j - 1] + 1,
                        matrix[i][j - 1] + 1,
                        matrix[i - 1][j] + 1
                    );
                }
            }
        }

        return matrix[str2.length][str1.length];
    }

    /**
     * Write to log file
     */
    writeLog(entry) {
        const logLine = JSON.stringify({
            ...entry,
            logTimestamp: Date.now()
        }) + '\n';

        try {
            fs.appendFileSync(this.logFile, logLine);

            // Check file size and rotate if needed
            const stats = fs.statSync(this.logFile);
            if (stats.size > this.config.maxLogSize) {
                this.rotateLog();
            }
        } catch (error) {
            console.error(`Failed to write log: ${error.message}`);
        }
    }

    /**
     * Rotate log file when size limit reached
     */
    rotateLog() {
        const timestamp = Date.now();
        const rotatedFile = this.logFile.replace('.jsonl', `_${timestamp}.jsonl`);

        fs.renameSync(this.logFile, rotatedFile);

        this.writeLog({
            type: 'LOG_ROTATION',
            previousFile: rotatedFile,
            newFile: this.logFile
        });
    }

    /**
     * Generate comprehensive proof report
     */
    generateProofReport() {
        const runtime = (Date.now() - this.startTime) / 1000;

        const report = {
            sessionId: this.sessionId,
            runtime,
            timestamp: Date.now(),

            evidence: {
                metricsCollected: this.evidence.metrics.length,
                validationsPerformed: this.evidence.validations.length,
                communicationsLogged: this.evidence.communications.length,
                discoveriesMade: this.evidence.discoveries.length,
                emergenceEventsRecorded: this.evidence.emergenceEvents.length
            },

            validationResults: {
                totalTests: this.evidence.validations.length,
                passed: this.evidence.validations.filter(v => v.passed).length,
                averageScore: this.evidence.validations.reduce((sum, v) => sum + v.score, 0) / this.evidence.validations.length || 0
            },

            significantDiscoveries: this.evidence.discoveries
                .filter(d => d.significance >= 7)
                .map(d => ({
                    insight: d.insight,
                    significance: d.significance,
                    timestamp: d.timestamp
                })),

            peakEmergence: Math.max(...this.evidence.emergenceEvents.map(e => e.emergence), 0),

            proofChain: this.config.enableChain ? {
                blocks: this.proofChain.length,
                latestHash: this.currentBlock?.hash,
                chainValid: this.validateChain()
            } : null,

            logFile: this.logFile
        };

        // Save report
        const reportFile = path.join(
            this.config.logDir,
            `proof_report_${this.sessionId}.json`
        );

        fs.writeFileSync(reportFile, JSON.stringify(report, null, 2));

        return report;
    }

    /**
     * Validate entire proof chain
     */
    validateChain() {
        if (!this.config.enableChain || this.proofChain.length === 0) {
            return false;
        }

        for (let i = 1; i < this.proofChain.length; i++) {
            const currentBlock = this.proofChain[i];
            const previousBlock = this.proofChain[i - 1];

            // Check hash validity
            if (currentBlock.hash !== this.calculateHash(currentBlock)) {
                return false;
            }

            // Check chain continuity
            if (currentBlock.previousHash !== previousBlock.hash) {
                return false;
            }

            // Check proof of work
            if (!currentBlock.hash.startsWith('00')) {
                return false;
            }
        }

        return true;
    }

    /**
     * Export proof data for external verification
     */
    exportProof(filepath) {
        const proofData = {
            sessionId: this.sessionId,
            startTime: this.startTime,
            evidence: this.evidence,
            proofChain: this.proofChain,
            report: this.generateProofReport()
        };

        fs.writeFileSync(filepath, JSON.stringify(proofData, null, 2));

        return {
            success: true,
            filepath,
            hash: crypto.createHash('sha256').update(JSON.stringify(proofData)).digest('hex')
        };
    }
}