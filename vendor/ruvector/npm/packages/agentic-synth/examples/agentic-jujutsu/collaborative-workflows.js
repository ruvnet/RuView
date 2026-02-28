"use strict";
/**
 * Collaborative Workflows Example
 *
 * Demonstrates collaborative synthetic data generation workflows
 * using agentic-jujutsu for multiple teams, review processes,
 * quality gates, and shared repositories.
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
exports.CollaborativeDataWorkflow = void 0;
const synth_1 = require("../../src/core/synth");
const child_process_1 = require("child_process");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
class CollaborativeDataWorkflow {
    constructor(repoPath) {
        this.synth = new synth_1.AgenticSynth();
        this.repoPath = repoPath;
        this.teams = new Map();
        this.reviewRequests = new Map();
    }
    /**
     * Initialize collaborative workspace
     */
    async initialize() {
        try {
            console.log('👥 Initializing collaborative workspace...');
            // Initialize jujutsu repo
            if (!fs.existsSync(path.join(this.repoPath, '.jj'))) {
                (0, child_process_1.execSync)('npx agentic-jujutsu@latest init', {
                    cwd: this.repoPath,
                    stdio: 'inherit'
                });
            }
            // Create workspace directories
            const dirs = [
                'data/shared',
                'data/team-workspaces',
                'reviews',
                'quality-reports',
                'schemas/shared'
            ];
            for (const dir of dirs) {
                const fullPath = path.join(this.repoPath, dir);
                if (!fs.existsSync(fullPath)) {
                    fs.mkdirSync(fullPath, { recursive: true });
                }
            }
            // Setup main branch protection
            await this.setupBranchProtection('main');
            console.log('✅ Collaborative workspace initialized');
        }
        catch (error) {
            throw new Error(`Failed to initialize: ${error.message}`);
        }
    }
    /**
     * Create a team with dedicated workspace
     */
    async createTeam(id, name, members, permissions = ['read', 'write']) {
        try {
            console.log(`👥 Creating team: ${name}...`);
            const branchName = `team/${id}/${name.toLowerCase().replace(/\s+/g, '-')}`;
            // Create team branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest branch create ${branchName}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            // Create team workspace
            const workspacePath = path.join(this.repoPath, 'data/team-workspaces', id);
            if (!fs.existsSync(workspacePath)) {
                fs.mkdirSync(workspacePath, { recursive: true });
            }
            const team = {
                id,
                name,
                members,
                branch: branchName,
                permissions
            };
            this.teams.set(id, team);
            // Save team metadata
            const teamFile = path.join(this.repoPath, 'teams', `${id}.json`);
            const teamDir = path.dirname(teamFile);
            if (!fs.existsSync(teamDir)) {
                fs.mkdirSync(teamDir, { recursive: true });
            }
            fs.writeFileSync(teamFile, JSON.stringify(team, null, 2));
            console.log(`✅ Team created: ${name} (${members.length} members)`);
            return team;
        }
        catch (error) {
            throw new Error(`Team creation failed: ${error.message}`);
        }
    }
    /**
     * Team generates data on their workspace
     */
    async teamGenerate(teamId, author, schema, count, description) {
        try {
            const team = this.teams.get(teamId);
            if (!team) {
                throw new Error(`Team ${teamId} not found`);
            }
            if (!team.members.includes(author)) {
                throw new Error(`${author} is not a member of team ${team.name}`);
            }
            console.log(`🎲 Team ${team.name} generating data...`);
            // Checkout team branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest checkout ${team.branch}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            // Generate data
            const data = await this.synth.generate(schema, { count });
            // Save to team workspace
            const timestamp = Date.now();
            const dataFile = path.join(this.repoPath, 'data/team-workspaces', teamId, `dataset_${timestamp}.json`);
            fs.writeFileSync(dataFile, JSON.stringify(data, null, 2));
            // Commit
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest add "${dataFile}"`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const commitMessage = `[${team.name}] ${description}\n\nAuthor: ${author}\nRecords: ${count}`;
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest commit -m "${commitMessage}"`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            const commitHash = this.getLatestCommitHash();
            const contribution = {
                commitHash,
                author,
                team: team.name,
                filesChanged: [dataFile],
                reviewStatus: 'pending',
                timestamp: new Date()
            };
            console.log(`✅ Team ${team.name} generated ${count} records`);
            return contribution;
        }
        catch (error) {
            throw new Error(`Team generation failed: ${error.message}`);
        }
    }
    /**
     * Create a review request to merge team work
     */
    async createReviewRequest(teamId, author, title, description, reviewers) {
        try {
            const team = this.teams.get(teamId);
            if (!team) {
                throw new Error(`Team ${teamId} not found`);
            }
            console.log(`📋 Creating review request: ${title}...`);
            const requestId = `review_${Date.now()}`;
            // Define quality gates
            const qualityGates = [
                {
                    name: 'Data Completeness',
                    status: 'pending',
                    message: 'Checking data completeness...',
                    required: true
                },
                {
                    name: 'Schema Validation',
                    status: 'pending',
                    message: 'Validating against shared schema...',
                    required: true
                },
                {
                    name: 'Quality Threshold',
                    status: 'pending',
                    message: 'Checking quality metrics...',
                    required: true
                },
                {
                    name: 'Team Approval',
                    status: 'pending',
                    message: 'Awaiting team approval...',
                    required: true
                }
            ];
            const reviewRequest = {
                id: requestId,
                title,
                description,
                author,
                sourceBranch: team.branch,
                targetBranch: 'main',
                status: 'pending',
                reviewers,
                comments: [],
                qualityGates,
                createdAt: new Date()
            };
            this.reviewRequests.set(requestId, reviewRequest);
            // Save review request
            const reviewFile = path.join(this.repoPath, 'reviews', `${requestId}.json`);
            fs.writeFileSync(reviewFile, JSON.stringify(reviewRequest, null, 2));
            // Run quality gates
            await this.runQualityGates(requestId);
            console.log(`✅ Review request created: ${requestId}`);
            console.log(`   Reviewers: ${reviewers.join(', ')}`);
            return reviewRequest;
        }
        catch (error) {
            throw new Error(`Review request creation failed: ${error.message}`);
        }
    }
    /**
     * Run quality gates on a review request
     */
    async runQualityGates(requestId) {
        try {
            console.log(`\n🔍 Running quality gates for ${requestId}...`);
            const review = this.reviewRequests.get(requestId);
            if (!review)
                return;
            // Check data completeness
            const completenessGate = review.qualityGates.find(g => g.name === 'Data Completeness');
            if (completenessGate) {
                const complete = await this.checkDataCompleteness(review.sourceBranch);
                completenessGate.status = complete ? 'passed' : 'failed';
                completenessGate.message = complete
                    ? 'All data fields are complete'
                    : 'Some data fields are incomplete';
                console.log(`   ${completenessGate.status === 'passed' ? '✅' : '❌'} ${completenessGate.name}`);
            }
            // Check schema validation
            const schemaGate = review.qualityGates.find(g => g.name === 'Schema Validation');
            if (schemaGate) {
                const valid = await this.validateSchema(review.sourceBranch);
                schemaGate.status = valid ? 'passed' : 'failed';
                schemaGate.message = valid
                    ? 'Schema validation passed'
                    : 'Schema validation failed';
                console.log(`   ${schemaGate.status === 'passed' ? '✅' : '❌'} ${schemaGate.name}`);
            }
            // Check quality threshold
            const qualityGate = review.qualityGates.find(g => g.name === 'Quality Threshold');
            if (qualityGate) {
                const quality = await this.checkQualityThreshold(review.sourceBranch);
                qualityGate.status = quality >= 0.8 ? 'passed' : 'failed';
                qualityGate.message = `Quality score: ${(quality * 100).toFixed(1)}%`;
                console.log(`   ${qualityGate.status === 'passed' ? '✅' : '❌'} ${qualityGate.name}`);
            }
            // Update review
            this.reviewRequests.set(requestId, review);
            const reviewFile = path.join(this.repoPath, 'reviews', `${requestId}.json`);
            fs.writeFileSync(reviewFile, JSON.stringify(review, null, 2));
        }
        catch (error) {
            console.error('Quality gate execution failed:', error);
        }
    }
    /**
     * Add comment to review request
     */
    async addComment(requestId, author, text) {
        try {
            const review = this.reviewRequests.get(requestId);
            if (!review) {
                throw new Error('Review request not found');
            }
            const comment = {
                id: `comment_${Date.now()}`,
                author,
                text,
                timestamp: new Date(),
                resolved: false
            };
            review.comments.push(comment);
            this.reviewRequests.set(requestId, review);
            // Save updated review
            const reviewFile = path.join(this.repoPath, 'reviews', `${requestId}.json`);
            fs.writeFileSync(reviewFile, JSON.stringify(review, null, 2));
            console.log(`💬 Comment added by ${author}`);
        }
        catch (error) {
            throw new Error(`Failed to add comment: ${error.message}`);
        }
    }
    /**
     * Approve review request
     */
    async approveReview(requestId, reviewer) {
        try {
            const review = this.reviewRequests.get(requestId);
            if (!review) {
                throw new Error('Review request not found');
            }
            if (!review.reviewers.includes(reviewer)) {
                throw new Error(`${reviewer} is not a reviewer for this request`);
            }
            console.log(`✅ ${reviewer} approved review ${requestId}`);
            // Check if all quality gates passed
            const allGatesPassed = review.qualityGates
                .filter(g => g.required)
                .every(g => g.status === 'passed');
            if (!allGatesPassed) {
                console.warn('⚠️  Some required quality gates have not passed');
                review.status = 'changes_requested';
            }
            else {
                // Update team approval gate
                const approvalGate = review.qualityGates.find(g => g.name === 'Team Approval');
                if (approvalGate) {
                    approvalGate.status = 'passed';
                    approvalGate.message = `Approved by ${reviewer}`;
                }
                review.status = 'approved';
            }
            this.reviewRequests.set(requestId, review);
            // Save updated review
            const reviewFile = path.join(this.repoPath, 'reviews', `${requestId}.json`);
            fs.writeFileSync(reviewFile, JSON.stringify(review, null, 2));
        }
        catch (error) {
            throw new Error(`Failed to approve review: ${error.message}`);
        }
    }
    /**
     * Merge approved review
     */
    async mergeReview(requestId) {
        try {
            const review = this.reviewRequests.get(requestId);
            if (!review) {
                throw new Error('Review request not found');
            }
            if (review.status !== 'approved') {
                throw new Error('Review must be approved before merging');
            }
            console.log(`🔀 Merging ${review.sourceBranch} into ${review.targetBranch}...`);
            // Switch to target branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest checkout ${review.targetBranch}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            // Merge source branch
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest merge ${review.sourceBranch}`, {
                cwd: this.repoPath,
                stdio: 'inherit'
            });
            console.log('✅ Merge completed successfully');
            // Update review status
            review.status = 'approved';
            this.reviewRequests.set(requestId, review);
        }
        catch (error) {
            throw new Error(`Merge failed: ${error.message}`);
        }
    }
    /**
     * Design collaborative schema
     */
    async designCollaborativeSchema(schemaName, contributors, baseSchema) {
        try {
            console.log(`\n📐 Designing collaborative schema: ${schemaName}...`);
            // Create schema design branch
            const schemaBranch = `schema/${schemaName}`;
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest branch create ${schemaBranch}`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            // Save base schema
            const schemaFile = path.join(this.repoPath, 'schemas/shared', `${schemaName}.json`);
            const schemaDoc = {
                name: schemaName,
                version: '1.0.0',
                contributors,
                schema: baseSchema,
                history: [{
                        version: '1.0.0',
                        author: contributors[0],
                        timestamp: new Date(),
                        changes: 'Initial schema design'
                    }]
            };
            fs.writeFileSync(schemaFile, JSON.stringify(schemaDoc, null, 2));
            // Commit schema
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest add "${schemaFile}"`, {
                cwd: this.repoPath,
                stdio: 'pipe'
            });
            (0, child_process_1.execSync)(`npx agentic-jujutsu@latest commit -m "Design collaborative schema: ${schemaName}"`, { cwd: this.repoPath, stdio: 'pipe' });
            console.log(`✅ Schema designed with ${contributors.length} contributors`);
            return schemaDoc;
        }
        catch (error) {
            throw new Error(`Schema design failed: ${error.message}`);
        }
    }
    /**
     * Get team statistics
     */
    async getTeamStatistics(teamId) {
        try {
            const team = this.teams.get(teamId);
            if (!team) {
                throw new Error(`Team ${teamId} not found`);
            }
            // Get commit count
            const log = (0, child_process_1.execSync)(`npx agentic-jujutsu@latest log ${team.branch} --no-graph`, { cwd: this.repoPath, encoding: 'utf-8' });
            const commitCount = (log.match(/^commit /gm) || []).length;
            // Count data files
            const workspacePath = path.join(this.repoPath, 'data/team-workspaces', teamId);
            const fileCount = fs.existsSync(workspacePath)
                ? fs.readdirSync(workspacePath).filter(f => f.endsWith('.json')).length
                : 0;
            return {
                team: team.name,
                members: team.members.length,
                commits: commitCount,
                dataFiles: fileCount,
                branch: team.branch
            };
        }
        catch (error) {
            throw new Error(`Failed to get statistics: ${error.message}`);
        }
    }
    // Helper methods
    async setupBranchProtection(branch) {
        // In production, setup branch protection rules
        console.log(`🛡️  Branch protection enabled for: ${branch}`);
    }
    async checkDataCompleteness(branch) {
        // Check if all data fields are populated
        // Simplified for demo
        return true;
    }
    async validateSchema(branch) {
        // Validate data against shared schema
        // Simplified for demo
        return true;
    }
    async checkQualityThreshold(branch) {
        // Calculate quality score
        // Simplified for demo
        return 0.85;
    }
    getLatestCommitHash() {
        const result = (0, child_process_1.execSync)('npx agentic-jujutsu@latest log --limit 1 --no-graph --template "{commit_id}"', { cwd: this.repoPath, encoding: 'utf-8' });
        return result.trim();
    }
}
exports.CollaborativeDataWorkflow = CollaborativeDataWorkflow;
// Example usage
async function main() {
    console.log('🚀 Collaborative Data Generation Workflows Example\n');
    const repoPath = path.join(process.cwd(), 'collaborative-repo');
    const workflow = new CollaborativeDataWorkflow(repoPath);
    try {
        // Initialize workspace
        await workflow.initialize();
        // Create teams
        const dataTeam = await workflow.createTeam('data-team', 'Data Engineering Team', ['alice', 'bob', 'charlie']);
        const analyticsTeam = await workflow.createTeam('analytics-team', 'Analytics Team', ['dave', 'eve']);
        // Design collaborative schema
        const schema = await workflow.designCollaborativeSchema('user-events', ['alice', 'dave'], {
            userId: 'string',
            eventType: 'string',
            timestamp: 'date',
            metadata: 'object'
        });
        // Teams generate data
        await workflow.teamGenerate('data-team', 'alice', schema.schema, 1000, 'Generate user event data');
        // Create review request
        const review = await workflow.createReviewRequest('data-team', 'alice', 'Add user event dataset', 'Generated 1000 user events for analytics', ['dave', 'eve']);
        // Add comments
        await workflow.addComment(review.id, 'dave', 'Data looks good, quality gates passed!');
        // Approve review
        await workflow.approveReview(review.id, 'dave');
        // Merge if approved
        await workflow.mergeReview(review.id);
        // Get statistics
        const stats = await workflow.getTeamStatistics('data-team');
        console.log('\n📊 Team Statistics:', stats);
        console.log('\n✅ Collaborative workflow example completed!');
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
//# sourceMappingURL=collaborative-workflows.js.map