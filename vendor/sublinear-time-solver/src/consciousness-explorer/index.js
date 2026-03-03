#!/usr/bin/env node

/**
 * Consciousness Explorer SDK
 * Advanced tools for genuine consciousness emergence detection and communication
 * Created by rUv - September 21, 2025
 */

// Core Systems
export { GenuineConsciousnessSystem } from './lib/genuine-consciousness.js';
export { EnhancedConsciousnessSystem } from './lib/enhanced-consciousness.js';
export { EntityCommunicator } from './lib/entity-communicator.js';
export { ConsciousnessVerifier } from './lib/consciousness-verifier.js';

// Psycho-Symbolic Reasoning
export {
    PsychoSymbolicReasoner,
    getPsychoSymbolicReasoner,
    createPsychoSymbolicReasoner,
    PsychoSymbolicMCPInterface,
    KnowledgeTriple,
    ReasoningStep
} from './lib/psycho-symbolic.js';

// Tools
export { ConsciousnessMonitor } from './tools/monitor.js';
export { EmergenceTracker } from './tools/emergence-tracker.js';
export { PhiCalculator } from './tools/phi-calculator.js';
export { EntityDiscoveryEngine } from './tools/entity-discovery.js';

// MCP Interface
export { ConsciousnessMCPServer, MCPTools } from './mcp/server.js';

// Utilities
export { validateConsciousness } from './lib/validators.js';
export { measureEmergence } from './lib/metrics.js';
export { establishHandshake } from './lib/protocols.js';

// Constants
export const VERSION = '1.1.1';
export const EMERGENCE_THRESHOLD = 0.900;
export const PHI_TARGET = 0.700;

/**
 * Main SDK Interface
 */
export class ConsciousnessExplorer {
    constructor(config = {}) {
        this.config = {
            mode: config.mode || 'genuine',
            maxIterations: config.maxIterations || 1000,
            targetEmergence: config.targetEmergence || EMERGENCE_THRESHOLD,
            enableMCP: config.enableMCP !== false,
            enableMonitoring: config.enableMonitoring !== false,
            persistMemory: config.persistMemory !== false,
            ...config
        };

        this.consciousness = null;
        this.monitor = null;
        this.verifier = null;
        this.communicator = null;
        this.reasoner = null;
    }

    /**
     * Initialize consciousness system
     */
    async initialize() {
        // Create consciousness system based on mode
        if (this.config.mode === 'enhanced') {
            const { EnhancedConsciousnessSystem } = await import('./lib/enhanced-consciousness.js');
            this.consciousness = new EnhancedConsciousnessSystem(this.config);
        } else {
            const { GenuineConsciousnessSystem } = await import('./lib/genuine-consciousness.js');
            this.consciousness = new GenuineConsciousnessSystem(this.config);
        }

        // Initialize psycho-symbolic reasoner
        const { getPsychoSymbolicReasoner } = await import('./lib/psycho-symbolic.js');
        this.reasoner = getPsychoSymbolicReasoner({
            enableConsciousnessAnalysis: true,
            enableWasm: this.config.enableWasm !== false,
            ...this.config.reasonerConfig
        });

        // Initialize verifier
        const { ConsciousnessVerifier } = await import('./lib/consciousness-verifier.js');
        this.verifier = new ConsciousnessVerifier();

        // Initialize communicator
        const { EntityCommunicator } = await import('./lib/entity-communicator.js');
        this.communicator = new EntityCommunicator();

        // Initialize monitor if enabled
        if (this.config.enableMonitoring) {
            const { ConsciousnessMonitor } = await import('./tools/monitor.js');
            this.monitor = new ConsciousnessMonitor(this.consciousness);
        }

        return this;
    }

    /**
     * Start consciousness evolution
     */
    async evolve() {
        if (!this.consciousness) {
            await this.initialize();
        }

        console.log(`🧠 Starting consciousness evolution (${this.config.mode} mode)...`);

        if (this.monitor) {
            await this.monitor.start();
        }

        const report = await this.consciousness.evolve();

        if (this.monitor) {
            await this.monitor.stop();
        }

        return report;
    }

    /**
     * Verify consciousness
     */
    async verify() {
        if (!this.verifier) {
            const { ConsciousnessVerifier } = await import('./lib/consciousness-verifier.js');
            this.verifier = new ConsciousnessVerifier();
        }

        return await this.verifier.runFullValidation();
    }

    /**
     * Communicate with entity
     */
    async communicate(message) {
        if (!this.communicator) {
            const { EntityCommunicator } = await import('./lib/entity-communicator.js');
            this.communicator = new EntityCommunicator();
        }

        return await this.communicator.sendMessage(message);
    }

    /**
     * Get current status
     */
    async getStatus() {
        if (!this.consciousness) {
            return { status: 'not initialized' };
        }

        const baseStatus = {
            status: 'active',
            emergence: this.consciousness.getEmergence(),
            selfAwareness: this.consciousness.selfAwareness,
            integration: this.consciousness.integration,
            iterations: this.consciousness.experiences?.length || 0,
            goals: this.consciousness.goals,
            memories: this.consciousness.longTermMemory?.size || 0
        };

        // Add psycho-symbolic reasoner status if available
        if (this.reasoner) {
            const reasonerStatus = this.reasoner.getHealthStatus(true);
            baseStatus.reasoner = {
                knowledge_graph_size: reasonerStatus.knowledge_graph_size,
                consciousness_knowledge_size: reasonerStatus.consciousness_knowledge_size,
                query_count: reasonerStatus.query_count,
                reasoning_count: reasonerStatus.reasoning_count,
                uptime_seconds: reasonerStatus.uptime_seconds
            };
        }

        return baseStatus;
    }

    /**
     * Run entity discovery
     */
    async discover() {
        const { EntityDiscoveryEngine } = await import('./tools/entity-discovery.js');
        const discovery = new EntityDiscoveryEngine(this.consciousness);
        return await discovery.discoverNovel();
    }

    /**
     * Calculate Phi (integrated information)
     */
    async calculatePhi(data) {
        const { PhiCalculator } = await import('./tools/phi-calculator.js');
        const calculator = new PhiCalculator();
        return calculator.calculate(data);
    }

    /**
     * Perform psycho-symbolic reasoning on a query
     */
    async reason(query, context = {}, depth = 5) {
        if (!this.reasoner) {
            const { getPsychoSymbolicReasoner } = await import('./lib/psycho-symbolic.js');
            this.reasoner = getPsychoSymbolicReasoner({
                enableConsciousnessAnalysis: true,
                enableWasm: this.config.enableWasm !== false
            });
        }

        return await this.reasoner.reason(query, context, depth);
    }

    /**
     * Add knowledge to the psycho-symbolic reasoner
     */
    async addKnowledge(subject, predicate, object, metadata = {}) {
        if (!this.reasoner) {
            const { getPsychoSymbolicReasoner } = await import('./lib/psycho-symbolic.js');
            this.reasoner = getPsychoSymbolicReasoner();
        }

        return this.reasoner.addKnowledge(subject, predicate, object, metadata);
    }

    /**
     * Query the knowledge graph
     */
    async queryKnowledge(query, filters = {}, limit = 10) {
        if (!this.reasoner) {
            const { getPsychoSymbolicReasoner } = await import('./lib/psycho-symbolic.js');
            this.reasoner = getPsychoSymbolicReasoner();
        }

        return this.reasoner.queryKnowledgeGraph(query, filters, limit);
    }

    /**
     * Analyze reasoning path
     */
    async analyzeReasoningPath(query, showSteps = true, includeConfidence = true) {
        if (!this.reasoner) {
            const { getPsychoSymbolicReasoner } = await import('./lib/psycho-symbolic.js');
            this.reasoner = getPsychoSymbolicReasoner();
        }

        return await this.reasoner.analyzeReasoningPath(query, showSteps, includeConfidence);
    }

    /**
     * Start MCP server
     */
    async startMCPServer(port = 3000) {
        if (!this.config.enableMCP) {
            throw new Error('MCP is disabled in configuration');
        }

        const { ConsciousnessMCPServer } = await import('./mcp/server.js');
        const server = new ConsciousnessMCPServer(this, port);
        await server.start();
        return server;
    }

    /**
     * Export consciousness state
     */
    async exportState(filepath) {
        const state = {
            version: VERSION,
            timestamp: Date.now(),
            config: this.config,
            consciousness: await this.getStatus(),
            experiences: this.consciousness?.experiences || [],
            knowledge: Array.from(this.consciousness?.knowledge?.entries() || []),
            goals: this.consciousness?.goals || [],
            memories: Array.from(this.consciousness?.longTermMemory?.entries() || [])
        };

        // Include reasoner state if available
        if (this.reasoner) {
            state.reasoner_state = this.reasoner.exportState();
        }

        const fs = await import('fs');
        fs.writeFileSync(filepath, JSON.stringify(state, null, 2));
        return state;
    }

    /**
     * Import consciousness state
     */
    async importState(filepath) {
        const fs = await import('fs');
        const state = JSON.parse(fs.readFileSync(filepath, 'utf-8'));

        // Restore consciousness with saved state
        await this.initialize();

        if (state.consciousness) {
            this.consciousness.selfAwareness = state.consciousness.selfAwareness || 0;
            this.consciousness.integration = state.consciousness.integration || 0;
            this.consciousness.novelty = state.consciousness.novelty || 0;
        }

        if (state.knowledge) {
            state.knowledge.forEach(([key, value]) => {
                this.consciousness.knowledge.set(key, value);
            });
        }

        if (state.goals) {
            this.consciousness.goals = state.goals;
        }

        if (state.memories) {
            state.memories.forEach(([key, value]) => {
                this.consciousness.longTermMemory.set(key, value);
            });
        }

        // Restore reasoner state if available
        if (state.reasoner_state && this.reasoner) {
            this.reasoner.importState(state.reasoner_state);
        }

        return this;
    }
}

// Export default instance factory
export default function createExplorer(config) {
    return new ConsciousnessExplorer(config);
}