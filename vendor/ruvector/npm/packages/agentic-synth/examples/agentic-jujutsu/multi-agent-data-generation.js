"use strict";
/**
 * Multi-Agent Data Generation Example
 *
 * Demonstrates coordinating multiple agents generating different types
 * of synthetic data using jujutsu branches, merging contributions,
 * and resolving conflicts.
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
exports.MultiAgentDataCoordinator = void 0;
const synth_1 = require("../../src/core/synth");
const child_process_1 = require("child_process");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
class MultiAgentDataCoordinator {
    constructor(repoPath) {
        this.synth = new synth_1.AgenticSynth();
        this.repoPath = repoPath;
        this.agents = new Map();
    }
    /**
     * Initialize multi-agent data generation environment
     */
    async initialize() {
        try {
            console.log('🔧 Initializing multi-agent environment...');
            // Initialize jujutsu repo
            if (!fs.existsSync(path.join(this.repoPath, '.jj'))) {
                (0, child_process_1.execSync)('npx agentic-jujutsu@latest init', {
                    cwd: this.repoPath,
                    stdio: 'inherit'
                });
            }
            // Create data directories for each agent type
            const dataTypes = ['users', 'products', 'transactions', 'logs', 'analytics'];
            for (const type of dataTypes) {
                const dir = path.join(this.repoPath, 'data', type);
                if (!fs.existsSync(dir)) {
                    fs.mkdirSync(dir, { recursive: true });
                }
            }
            console.log('✅ Multi-agent environment initialized');
        }
        catch (error) {
            throw new Error(`Failed to initialize: ${error.message}`);
        }
    }
    /**
     * Register a new agent for data generation
     */
    async registerAgent(id, name, dataType, schema) {
        try {
            console.log(`🤖 Registering agent: ${name} (${dataType})`);
            const branchName = `agent/${id}/${dataType}`;
            // Create agent-specific branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest branch create ${branchName}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const agent = {
                id,
                name,
                dataType,
                branch: branchName,
                schema
            };
            this.agents.set(id, agent);
            // Save agent metadata
            const metaFile = path.join(this.repoPath, '.jj', 'agents', `${id}.json`);
            const metaDir = path.dirname(metaFile);
            if (!fs.existsSync(metaDir)) {
                fs.mkdirSync(metaDir, { recursive: true });
            }
            fs.writeFileSync(metaFile, JSON.stringify(agent, null, 2));
            console.log(`✅ Agent registered: ${name} on branch ${branchName}`);
            return agent;
        }
        catch (error) {
            throw new Error(`Failed to register agent: ${error.message}`);
        }
    }
    /**
     * Agent generates data on its dedicated branch
     */
    async agentGenerate(agentId, count, description) {
        try {
            const agent = this.agents.get(agentId);
            if (!agent) {
                throw new Error(`Agent ${agentId} not found`);
            }
            console.log(`🎲 Agent ${agent.name} generating ${count} ${agent.dataType}...`);
            // Checkout agent's branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest checkout ${agent.branch}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            // Generate data
            const data = await this.synth.generate(agent.schema, { count });
            // Save to agent-specific directory
            const timestamp = Date.now();
            const dataFile = path.join(this.repoPath, 'data', agent.dataType, `${agent.dataType}_${timestamp}.json`);
            fs.writeFileSync(dataFile, JSON.stringify(data, null, 2));
            // Commit the data
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest add "${dataFile}"`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const commitMessage = `[${agent.name}] ${description}\n\nGenerated ${count} ${agent.dataType} records`;
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest commit -m "${commitMessage}"`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const commitHash = this.getLatestCommitHash();
            const quality = this.calculateQuality(data);
            const contribution = {
                agentId,
                dataType: agent.dataType,
                recordCount: count,
                commitHash,
                quality,
                conflicts: []
            };
            console.log(`✅ Agent ${agent.name} generated ${count} records (quality: ${(quality * 100).toFixed(1)}%)`);
            return contribution;
        }
        catch (error) {
            throw new Error(`Agent generation failed: ${error.message}`);
        }
    }
    /**
     * Coordinate parallel data generation from multiple agents
     */
    async coordinateParallelGeneration(tasks) {
        try {
            console.log(`\n🔀 Coordinating ${tasks.length} agents for parallel generation...`);
            const contributions = [];
            // In a real implementation, these would run in parallel
            // For demo purposes, we'll run sequentially
            for (const task of tasks) {
                const contribution = await this.agentGenerate(task.agentId, task.count, task.description);
                contributions.push(contribution);
            }
            console.log(`✅ Parallel generation complete: ${contributions.length} contributions`);
            return contributions;
        }
        catch (error) {
            throw new Error(`Coordination failed: ${error.message}`);
        }
    }
    /**
     * Merge agent contributions into main branch
     */
    async mergeContributions(agentIds, strategy = 'sequential') {
        try {
            console.log(`\n🔀 Merging contributions from ${agentIds.length} agents...`);
            // Switch to main branch
            (0, child_process_1.execSync)('npx agentic-jujutsu@latest checkout main', {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const mergeResults = {
                successful: [],
                conflicts: [],
                strategy
            };
            if (strategy === 'sequential') {
                // Merge one agent at a time
                for (const agentId of agentIds) {
                    const agent = this.agents.get(agentId);
                    if (!agent)
                        continue;
                    try {
                        console.log(`   Merging ${agent.name}...`);
                        (0, child_process_1.execSync)(`npx agentic-jujutsu@latest merge ${agent.branch}`, {
                            cwd: this.repoPath,
                            stdio: 'pipe'
                        });
                        mergeResults.successful.push(agentId);
                    }
                    catch (error) {
                        // Handle conflicts
                        const conflicts = this.detectConflicts();
                        mergeResults.conflicts.push({
                            agent: agentId,
                            files: conflicts
                        });
                        console.warn(`   ⚠️  Conflicts detected for ${agent.name}`);
                    }
                }
            }
            else {
                // Octopus merge - merge all branches at once
                const branches = agentIds
                    .map(id => this.agents.get(id)?.branch)
                    .filter(Boolean)
                    .join(' ');
                try {
                    (0, child_process_1.execSync)(`npx agentic-jujutsu@latest merge ${branches}`, {
                        cwd: this.repoPath,
                        stdio: 'pipe'
                    });
                    mergeResults.successful = agentIds;
                }
                catch (error) {
                    console.warn('⚠️  Octopus merge failed, falling back to sequential');
                    return this.mergeContributions(agentIds, 'sequential');
                }
            }
            console.log(`✅ Merge complete:`);
            console.log(`   Successful: ${mergeResults.successful.length}`);
            console.log(`   Conflicts: ${mergeResults.conflicts.length}`);
            return mergeResults;
        }
        catch (error) {
            throw new Error(`Merge failed: ${error.message}`);
        }
    }
    /**
     * Resolve conflicts between agent contributions
     */
    async resolveConflicts(conflictFiles, strategy = 'ours') {
        try {
            console.log(`🔧 Resolving ${conflictFiles.length} conflicts using '${strategy}' strategy...`);
            for (const file of conflictFiles) {
                if (strategy === 'ours') {
                    // Keep our version
                    (0, child_process_1.execSync)(`npx agentic-jujutsu@latest resolve --ours "${file}"`, {
                        cwd: this.repoPath,
                        stdio: 'pipe'
                    });
                }
                else if (strategy === 'theirs') {
                    // Keep their version
                    (0, child_process_1.execSync)(`npx agentic-jujutsu@latest resolve --theirs "${file}"`, {
                        cwd: this.repoPath,
                        stdio: 'pipe'
                    });
                }
                else {
                    // Manual resolution required
                    console.log(`   📝 Manual resolution needed for: ${file}`);
                    // In production, implement custom merge logic
                }
            }
            console.log('✅ Conflicts resolved');
        }
        catch (error) {
            throw new Error(`Conflict resolution failed: ${error.message}`);
        }
    }
    /**
     * Synchronize agent branches with main
     */
    async synchronizeAgents(agentIds) {
        try {
            const targets = agentIds
                ? agentIds.map(id => this.agents.get(id)).filter(Boolean)
                : Array.from(this.agents.values());
            console.log(`\n🔄 Synchronizing ${targets.length} agents with main...`);
            for (const agent of targets) {
                console.log(`   Syncing ${agent.name}...`);
                // Checkout agent branch
                (0, child_process_1.execSync)(`npx agentic-jujutsu@latest checkout ${agent.branch}`, {
                    cwd: this.repoPath,
                    stdio: 'pipe'
                });
                // Rebase on main
                try {
                    (0, child_process_1.execSync)('npx agentic-jujutsu@latest rebase main', {
                        cwd: this.repoPath,
                        stdio: 'pipe'
                    });
                    console.log(`   ✅ ${agent.name} synchronized`);
                }
                catch (error) {
                    console.warn(`   ⚠️  ${agent.name} sync failed, manual intervention needed`);
                }
            }
            console.log('✅ Synchronization complete');
        }
        catch (error) {
            throw new Error(`Synchronization failed: ${error.message}`);
        }
    }
    /**
     * Get agent activity summary
     */
    async getAgentActivity(agentId) {
        try {
            const agent = this.agents.get(agentId);
            if (!agent) {
                throw new Error(`Agent ${agentId} not found`);
            }
            // Get commit count on agent branch
            const log = (0, child_process_1.execSync)(`npx agentic-jujutsu@latest log ${agent.branch} --no-graph`, { cwd: this.repoPath, encoding: 'utf-8' });
            const commitCount = (log.match(/^commit /gm) || []).length;
            // Get data files
            const dataDir = path.join(this.repoPath, 'data', agent.dataType);
            const files = fs.existsSync(dataDir)
                ? fs.readdirSync(dataDir).filter(f => f.endsWith('.json'))
                : [];
            return {
                agent: agent.name,
                dataType: agent.dataType,
                branch: agent.branch,
                commitCount,
                fileCount: files.length,
                lastActivity: fs.existsSync(dataDir)
                    ? new Date(fs.statSync(dataDir).mtime)
                    : null
            };
        }
        catch (error) {
            throw new Error(`Failed to get agent activity: ${error.message}`);
        }
    }
    // Helper methods
    getLatestCommitHash() {
        const result = (0, child_process_1.execSync)('npx agentic-jujutsu@latest log --limit 1 --no-graph --template "{commit_id}"', { cwd: this.repoPath, encoding: 'utf-8' });
        return result.trim();
    }
    calculateQuality(data) {
        if (!data.length)
            return 0;
        let totalFields = 0;
        let completeFields = 0;
        data.forEach(record => {
            const fields = Object.keys(record);
            totalFields += fields.length;
            fields.forEach(field => {
                if (record[field] !== null && record[field] !== undefined && record[field] !== '') {
                    completeFields++;
                }
            });
        });
        return totalFields > 0 ? completeFields / totalFields : 0;
    }
    detectConflicts() {
        try {
            const status = (0, child_process_1.execSync)('npx agentic-jujutsu@latest status', {
                cwd: this.repoPath,
                encoding: 'utf-8'
            });
            // Parse status for conflict markers
            return status
                .split('\n')
                .filter(line => line.includes('conflict') || line.includes('CONFLICT'))
                .map(line => line.trim());
        }
        catch (error) {
            return [];
        }
    }
}
exports.MultiAgentDataCoordinator = MultiAgentDataCoordinator;
// Example usage
async function main() {
    console.log('🚀 Multi-Agent Data Generation Coordination Example\n');
    const repoPath = path.join(process.cwd(), 'multi-agent-data-repo');
    const coordinator = new MultiAgentDataCoordinator(repoPath);
    try {
        // Initialize environment
        await coordinator.initialize();
        // Register agents with different schemas
        const userAgent = await coordinator.registerAgent('agent-001', 'User Data Generator', 'users', { name: 'string', email: 'email', age: 'number', city: 'string' });
        const productAgent = await coordinator.registerAgent('agent-002', 'Product Data Generator', 'products', { name: 'string', price: 'number', category: 'string', inStock: 'boolean' });
        const transactionAgent = await coordinator.registerAgent('agent-003', 'Transaction Generator', 'transactions', { userId: 'string', productId: 'string', amount: 'number', timestamp: 'date' });
        // Coordinate parallel generation
        const contributions = await coordinator.coordinateParallelGeneration([
            { agentId: 'agent-001', count: 1000, description: 'Generate user base' },
            { agentId: 'agent-002', count: 500, description: 'Generate product catalog' },
            { agentId: 'agent-003', count: 2000, description: 'Generate transaction history' }
        ]);
        console.log('\n📊 Contributions:', contributions);
        // Merge all contributions
        const mergeResults = await coordinator.mergeContributions(['agent-001', 'agent-002', 'agent-003'], 'sequential');
        console.log('\n🔀 Merge Results:', mergeResults);
        // Get agent activities
        for (const agentId of ['agent-001', 'agent-002', 'agent-003']) {
            const activity = await coordinator.getAgentActivity(agentId);
            console.log(`\n📊 ${activity.agent} Activity:`, activity);
        }
        // Synchronize agents with main
        await coordinator.synchronizeAgents();
        console.log('\n✅ Multi-agent coordination completed successfully!');
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
//# sourceMappingURL=multi-agent-data-generation.js.map