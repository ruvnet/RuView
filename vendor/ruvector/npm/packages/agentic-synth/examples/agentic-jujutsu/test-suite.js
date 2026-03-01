"use strict";
/**
 * Comprehensive Test Suite for Agentic-Jujutsu Integration
 *
 * Tests all features of agentic-jujutsu integration with agentic-synth:
 * - Version control
 * - Multi-agent coordination
 * - ReasoningBank learning
 * - Quantum-resistant features
 * - Collaborative workflows
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
const vitest_1 = require("vitest");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const version_control_integration_1 = require("./version-control-integration");
const multi_agent_data_generation_1 = require("./multi-agent-data-generation");
const reasoning_bank_learning_1 = require("./reasoning-bank-learning");
const quantum_resistant_data_1 = require("./quantum-resistant-data");
const collaborative_workflows_1 = require("./collaborative-workflows");
const TEST_ROOT = path.join(process.cwd(), 'test-repos');
// Test utilities
function cleanupTestRepos() {
    if (fs.existsSync(TEST_ROOT)) {
        fs.rmSync(TEST_ROOT, { recursive: true, force: true });
    }
}
function createTestRepo(name) {
    const repoPath = path.join(TEST_ROOT, name);
    fs.mkdirSync(repoPath, { recursive: true });
    return repoPath;
}
(0, vitest_1.describe)('Version Control Integration', () => {
    let repoPath;
    let generator;
    (0, vitest_1.beforeAll)(() => {
        cleanupTestRepos();
        repoPath = createTestRepo('version-control-test');
        generator = new version_control_integration_1.VersionControlledDataGenerator(repoPath);
    });
    (0, vitest_1.afterAll)(() => {
        cleanupTestRepos();
    });
    (0, vitest_1.it)('should initialize jujutsu repository', async () => {
        await generator.initializeRepository();
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, '.jj'))).toBe(true);
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data'))).toBe(true);
    });
    (0, vitest_1.it)('should generate and commit data with metadata', async () => {
        const schema = {
            name: 'string',
            email: 'email',
            age: 'number'
        };
        const commit = await generator.generateAndCommit(schema, 100, 'Test data generation');
        (0, vitest_1.expect)(commit).toBeDefined();
        (0, vitest_1.expect)(commit.hash).toBeTruthy();
        (0, vitest_1.expect)(commit.metadata.recordCount).toBe(100);
        (0, vitest_1.expect)(commit.metadata.quality).toBeGreaterThan(0);
    });
    (0, vitest_1.it)('should create and manage branches', async () => {
        await generator.createGenerationBranch('experiment-1', 'Testing branch creation');
        const branchFile = path.join(repoPath, '.jj', 'branches', 'experiment-1.desc');
        (0, vitest_1.expect)(fs.existsSync(branchFile)).toBe(true);
    });
    (0, vitest_1.it)('should compare datasets between commits', async () => {
        const schema = { name: 'string', value: 'number' };
        const commit1 = await generator.generateAndCommit(schema, 50, 'Dataset 1');
        const commit2 = await generator.generateAndCommit(schema, 75, 'Dataset 2');
        const comparison = await generator.compareDatasets(commit1.hash, commit2.hash);
        (0, vitest_1.expect)(comparison).toBeDefined();
        (0, vitest_1.expect)(comparison.ref1).toBe(commit1.hash);
        (0, vitest_1.expect)(comparison.ref2).toBe(commit2.hash);
    });
    (0, vitest_1.it)('should tag versions', async () => {
        await generator.tagVersion('v1.0.0', 'First stable version');
        // Tag creation is tested by not throwing
        (0, vitest_1.expect)(true).toBe(true);
    });
    (0, vitest_1.it)('should retrieve generation history', async () => {
        const history = await generator.getHistory(5);
        (0, vitest_1.expect)(Array.isArray(history)).toBe(true);
        (0, vitest_1.expect)(history.length).toBeGreaterThan(0);
    });
});
(0, vitest_1.describe)('Multi-Agent Data Generation', () => {
    let repoPath;
    let coordinator;
    (0, vitest_1.beforeAll)(() => {
        repoPath = createTestRepo('multi-agent-test');
        coordinator = new multi_agent_data_generation_1.MultiAgentDataCoordinator(repoPath);
    });
    (0, vitest_1.it)('should initialize multi-agent environment', async () => {
        await coordinator.initialize();
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, '.jj'))).toBe(true);
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data', 'users'))).toBe(true);
    });
    (0, vitest_1.it)('should register agents', async () => {
        const agent = await coordinator.registerAgent('test-agent-1', 'Test Agent', 'users', { name: 'string', email: 'email' });
        (0, vitest_1.expect)(agent.id).toBe('test-agent-1');
        (0, vitest_1.expect)(agent.branch).toContain('agent/test-agent-1');
    });
    (0, vitest_1.it)('should generate data for specific agent', async () => {
        await coordinator.registerAgent('test-agent-2', 'Agent 2', 'products', { name: 'string', price: 'number' });
        const contribution = await coordinator.agentGenerate('test-agent-2', 50, 'Test generation');
        (0, vitest_1.expect)(contribution.agentId).toBe('test-agent-2');
        (0, vitest_1.expect)(contribution.recordCount).toBe(50);
        (0, vitest_1.expect)(contribution.quality).toBeGreaterThan(0);
    });
    (0, vitest_1.it)('should coordinate parallel generation', async () => {
        await coordinator.registerAgent('agent-a', 'Agent A', 'typeA', { id: 'string' });
        await coordinator.registerAgent('agent-b', 'Agent B', 'typeB', { id: 'string' });
        const contributions = await coordinator.coordinateParallelGeneration([
            { agentId: 'agent-a', count: 25, description: 'Task A' },
            { agentId: 'agent-b', count: 30, description: 'Task B' }
        ]);
        (0, vitest_1.expect)(contributions.length).toBe(2);
        (0, vitest_1.expect)(contributions[0].recordCount).toBe(25);
        (0, vitest_1.expect)(contributions[1].recordCount).toBe(30);
    });
    (0, vitest_1.it)('should get agent activity', async () => {
        const activity = await coordinator.getAgentActivity('agent-a');
        (0, vitest_1.expect)(activity).toBeDefined();
        (0, vitest_1.expect)(activity.agent).toBe('Agent A');
    });
});
(0, vitest_1.describe)('ReasoningBank Learning', () => {
    let repoPath;
    let generator;
    (0, vitest_1.beforeAll)(() => {
        repoPath = createTestRepo('reasoning-bank-test');
        generator = new reasoning_bank_learning_1.ReasoningBankDataGenerator(repoPath);
    });
    (0, vitest_1.it)('should initialize ReasoningBank system', async () => {
        await generator.initialize();
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data', 'trajectories'))).toBe(true);
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data', 'patterns'))).toBe(true);
    });
    (0, vitest_1.it)('should generate with learning enabled', async () => {
        const schema = { name: 'string', value: 'number' };
        const result = await generator.generateWithLearning(schema, { count: 100 }, 'Learning test');
        (0, vitest_1.expect)(result.data.length).toBe(100);
        (0, vitest_1.expect)(result.trajectory).toBeDefined();
        (0, vitest_1.expect)(result.trajectory.quality).toBeGreaterThan(0);
        (0, vitest_1.expect)(result.trajectory.verdict).toBeTruthy();
    });
    (0, vitest_1.it)('should recognize patterns from trajectories', async () => {
        // Generate multiple trajectories
        const schema = { id: 'string', score: 'number' };
        await generator.generateWithLearning(schema, { count: 50 }, 'Pattern test 1');
        await generator.generateWithLearning(schema, { count: 50 }, 'Pattern test 2');
        const patterns = await generator.recognizePatterns();
        (0, vitest_1.expect)(Array.isArray(patterns)).toBe(true);
    });
    (0, vitest_1.it)('should perform continuous improvement', async () => {
        const improvement = await generator.continuousImprovement(2);
        (0, vitest_1.expect)(improvement).toBeDefined();
        (0, vitest_1.expect)(improvement.iterations.length).toBe(2);
        (0, vitest_1.expect)(improvement.qualityTrend.length).toBe(2);
        (0, vitest_1.expect)(improvement.bestQuality).toBeGreaterThan(0);
    });
});
(0, vitest_1.describe)('Quantum-Resistant Features', () => {
    let repoPath;
    let generator;
    (0, vitest_1.beforeAll)(() => {
        repoPath = createTestRepo('quantum-resistant-test');
        generator = new quantum_resistant_data_1.QuantumResistantDataGenerator(repoPath);
    });
    (0, vitest_1.it)('should initialize quantum-resistant repository', async () => {
        await generator.initialize();
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, '.jj', 'quantum-keys'))).toBe(true);
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data', 'secure'))).toBe(true);
    });
    (0, vitest_1.it)('should generate secure data with signatures', async () => {
        const schema = { userId: 'string', data: 'string' };
        const generation = await generator.generateSecureData(schema, 50, 'Secure generation test');
        (0, vitest_1.expect)(generation.id).toBeTruthy();
        (0, vitest_1.expect)(generation.dataHash).toBeTruthy();
        (0, vitest_1.expect)(generation.signature).toBeTruthy();
        (0, vitest_1.expect)(generation.quantumResistant).toBe(true);
    });
    (0, vitest_1.it)('should verify data integrity', async () => {
        const schema = { id: 'string' };
        const generation = await generator.generateSecureData(schema, 25, 'Test');
        const verified = await generator.verifyIntegrity(generation.id);
        (0, vitest_1.expect)(verified).toBe(true);
    });
    (0, vitest_1.it)('should create integrity proofs', async () => {
        const schema = { value: 'number' };
        const generation = await generator.generateSecureData(schema, 30, 'Proof test');
        const proof = await generator.createIntegrityProof(generation.id);
        (0, vitest_1.expect)(proof).toBeDefined();
        (0, vitest_1.expect)(proof.dataHash).toBeTruthy();
        (0, vitest_1.expect)(proof.merkleRoot).toBeTruthy();
        (0, vitest_1.expect)(proof.quantumSafe).toBe(true);
    });
    (0, vitest_1.it)('should verify integrity proofs', async () => {
        const schema = { name: 'string' };
        const generation = await generator.generateSecureData(schema, 20, 'Verify test');
        await generator.createIntegrityProof(generation.id);
        const verified = await generator.verifyIntegrityProof(generation.id);
        (0, vitest_1.expect)(verified).toBe(true);
    });
    (0, vitest_1.it)('should generate audit trails', async () => {
        const schema = { id: 'string' };
        const generation = await generator.generateSecureData(schema, 15, 'Audit test');
        const audit = await generator.generateAuditTrail(generation.id);
        (0, vitest_1.expect)(audit).toBeDefined();
        (0, vitest_1.expect)(audit.generation).toBe(generation.id);
        (0, vitest_1.expect)(audit.integrityScore).toBeGreaterThanOrEqual(0);
    });
    (0, vitest_1.it)('should detect tampering', async () => {
        const tampered = await generator.detectTampering();
        (0, vitest_1.expect)(Array.isArray(tampered)).toBe(true);
        // Should be empty if no tampering
        (0, vitest_1.expect)(tampered.length).toBe(0);
    });
});
(0, vitest_1.describe)('Collaborative Workflows', () => {
    let repoPath;
    let workflow;
    (0, vitest_1.beforeAll)(() => {
        repoPath = createTestRepo('collaborative-test');
        workflow = new collaborative_workflows_1.CollaborativeDataWorkflow(repoPath);
    });
    (0, vitest_1.it)('should initialize collaborative workspace', async () => {
        await workflow.initialize();
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'data', 'shared'))).toBe(true);
        (0, vitest_1.expect)(fs.existsSync(path.join(repoPath, 'reviews'))).toBe(true);
    });
    (0, vitest_1.it)('should create teams', async () => {
        const team = await workflow.createTeam('test-team', 'Test Team', ['alice', 'bob']);
        (0, vitest_1.expect)(team.id).toBe('test-team');
        (0, vitest_1.expect)(team.name).toBe('Test Team');
        (0, vitest_1.expect)(team.members.length).toBe(2);
    });
    (0, vitest_1.it)('should allow team to generate data', async () => {
        await workflow.createTeam('gen-team', 'Generation Team', ['charlie']);
        const contribution = await workflow.teamGenerate('gen-team', 'charlie', { name: 'string', value: 'number' }, 50, 'Team generation test');
        (0, vitest_1.expect)(contribution.author).toBe('charlie');
        (0, vitest_1.expect)(contribution.team).toBe('Generation Team');
    });
    (0, vitest_1.it)('should create review requests', async () => {
        await workflow.createTeam('review-team', 'Review Team', ['dave']);
        await workflow.teamGenerate('review-team', 'dave', { id: 'string' }, 25, 'Review test');
        const review = await workflow.createReviewRequest('review-team', 'dave', 'Test Review', 'Testing review process', ['alice']);
        (0, vitest_1.expect)(review.title).toBe('Test Review');
        (0, vitest_1.expect)(review.status).toBe('pending');
        (0, vitest_1.expect)(review.qualityGates.length).toBeGreaterThan(0);
    });
    (0, vitest_1.it)('should add comments to reviews', async () => {
        const review = await workflow.createReviewRequest('review-team', 'dave', 'Comment Test', 'Testing comments', ['alice']);
        await workflow.addComment(review.id, 'alice', 'Looks good!');
        // Comment addition is tested by not throwing
        (0, vitest_1.expect)(true).toBe(true);
    });
    (0, vitest_1.it)('should design collaborative schemas', async () => {
        const schema = await workflow.designCollaborativeSchema('test-schema', ['alice', 'bob'], { field1: 'string', field2: 'number' });
        (0, vitest_1.expect)(schema.name).toBe('test-schema');
        (0, vitest_1.expect)(schema.contributors.length).toBe(2);
    });
    (0, vitest_1.it)('should get team statistics', async () => {
        const stats = await workflow.getTeamStatistics('review-team');
        (0, vitest_1.expect)(stats).toBeDefined();
        (0, vitest_1.expect)(stats.team).toBe('Review Team');
    });
});
(0, vitest_1.describe)('Performance Benchmarks', () => {
    (0, vitest_1.it)('should benchmark version control operations', async () => {
        const repoPath = createTestRepo('perf-version-control');
        const generator = new version_control_integration_1.VersionControlledDataGenerator(repoPath);
        await generator.initializeRepository();
        const start = Date.now();
        const schema = { name: 'string', value: 'number' };
        for (let i = 0; i < 5; i++) {
            await generator.generateAndCommit(schema, 100, `Perf test ${i}`);
        }
        const duration = Date.now() - start;
        console.log(`Version control benchmark: 5 commits in ${duration}ms`);
        (0, vitest_1.expect)(duration).toBeLessThan(30000); // Should complete within 30 seconds
    });
    (0, vitest_1.it)('should benchmark multi-agent coordination', async () => {
        const repoPath = createTestRepo('perf-multi-agent');
        const coordinator = new multi_agent_data_generation_1.MultiAgentDataCoordinator(repoPath);
        await coordinator.initialize();
        // Register agents
        for (let i = 0; i < 3; i++) {
            await coordinator.registerAgent(`perf-agent-${i}`, `Agent ${i}`, `type${i}`, { id: 'string' });
        }
        const start = Date.now();
        await coordinator.coordinateParallelGeneration([
            { agentId: 'perf-agent-0', count: 100, description: 'Task 1' },
            { agentId: 'perf-agent-1', count: 100, description: 'Task 2' },
            { agentId: 'perf-agent-2', count: 100, description: 'Task 3' }
        ]);
        const duration = Date.now() - start;
        console.log(`Multi-agent benchmark: 3 agents, 300 records in ${duration}ms`);
        (0, vitest_1.expect)(duration).toBeLessThan(20000); // Should complete within 20 seconds
    });
});
(0, vitest_1.describe)('Error Handling', () => {
    (0, vitest_1.it)('should handle invalid repository paths', async () => {
        const generator = new version_control_integration_1.VersionControlledDataGenerator('/invalid/path/that/does/not/exist');
        await (0, vitest_1.expect)(async () => {
            await generator.generateAndCommit({}, 10, 'Test');
        }).rejects.toThrow();
    });
    (0, vitest_1.it)('should handle invalid agent operations', async () => {
        const repoPath = createTestRepo('error-handling');
        const coordinator = new multi_agent_data_generation_1.MultiAgentDataCoordinator(repoPath);
        await coordinator.initialize();
        await (0, vitest_1.expect)(async () => {
            await coordinator.agentGenerate('non-existent-agent', 10, 'Test');
        }).rejects.toThrow('not found');
    });
    (0, vitest_1.it)('should handle verification failures gracefully', async () => {
        const repoPath = createTestRepo('error-verification');
        const generator = new quantum_resistant_data_1.QuantumResistantDataGenerator(repoPath);
        await generator.initialize();
        const verified = await generator.verifyIntegrity('non-existent-id');
        (0, vitest_1.expect)(verified).toBe(false);
    });
});
// Run all tests
console.log('🧪 Running comprehensive test suite for agentic-jujutsu integration...\n');
//# sourceMappingURL=test-suite.js.map