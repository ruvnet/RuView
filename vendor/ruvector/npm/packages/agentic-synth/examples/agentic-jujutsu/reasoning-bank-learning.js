"use strict";
/**
 * ReasoningBank Learning Integration Example
 *
 * Demonstrates using agentic-jujutsu's ReasoningBank intelligence features
 * to learn from data generation patterns, track quality over time,
 * implement adaptive schema evolution, and create self-improving generators.
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
exports.ReasoningBankDataGenerator = void 0;
const synth_1 = require("../../src/core/synth");
const child_process_1 = require("child_process");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
class ReasoningBankDataGenerator {
    constructor(repoPath) {
        this.synth = new synth_1.AgenticSynth();
        this.repoPath = repoPath;
        this.trajectories = [];
        this.patterns = new Map();
        this.schemas = new Map();
    }
    /**
     * Initialize ReasoningBank-enabled repository
     */
    async initialize() {
        try {
            console.log('🧠 Initializing ReasoningBank learning system...');
            // Initialize jujutsu with ReasoningBank features
            if (!fs.existsSync(path.join(this.repoPath, '.jj'))) {
                (0, child_process_1.execSync)('npx agentic-jujutsu@latest init --reasoning-bank', {
                    cwd: this.repoPath,
                    stdio: 'inherit'
                });
            }
            // Create learning directories
            const dirs = [
                'data/trajectories',
                'data/patterns',
                'data/schemas',
                'data/verdicts',
                'data/memories'
            ];
            for (const dir of dirs) {
                const fullPath = path.join(this.repoPath, dir);
                if (!fs.existsSync(fullPath)) {
                    fs.mkdirSync(fullPath, { recursive: true });
                }
            }
            // Load existing learning data
            await this.loadLearningState();
            console.log('✅ ReasoningBank system initialized');
        }
        catch (error) {
            throw new Error(`Failed to initialize: ${error.message}`);
        }
    }
    /**
     * Generate data with trajectory tracking
     */
    async generateWithLearning(schema, parameters, description) {
        try {
            console.log(`🎲 Generating data with learning enabled...`);
            const startTime = Date.now();
            const trajectoryId = `traj_${Date.now()}`;
            // Generate data
            let data = [];
            let errors = 0;
            try {
                data = await this.synth.generate(schema, parameters);
            }
            catch (error) {
                errors++;
                console.error('Generation error:', error);
            }
            const duration = Date.now() - startTime;
            const quality = this.calculateQuality(data);
            // Create trajectory
            const trajectory = {
                id: trajectoryId,
                timestamp: new Date(),
                schema,
                parameters,
                quality,
                performance: {
                    duration,
                    recordCount: data.length,
                    errorRate: data.length > 0 ? errors / data.length : 1
                },
                verdict: this.judgeVerdict(quality, errors),
                lessons: this.extractLessons(schema, parameters, quality, errors)
            };
            this.trajectories.push(trajectory);
            // Save trajectory
            await this.saveTrajectory(trajectory);
            // Commit with reasoning metadata
            await this.commitWithReasoning(data, trajectory, description);
            // Learn from trajectory
            await this.learnFromTrajectory(trajectory);
            console.log(`✅ Generated ${data.length} records (quality: ${(quality * 100).toFixed(1)}%)`);
            console.log(`📊 Verdict: ${trajectory.verdict}`);
            console.log(`💡 Lessons learned: ${trajectory.lessons.length}`);
            return { data, trajectory };
        }
        catch (error) {
            throw new Error(`Generation with learning failed: ${error.message}`);
        }
    }
    /**
     * Learn from generation trajectory and update patterns
     */
    async learnFromTrajectory(trajectory) {
        try {
            console.log('🧠 Learning from trajectory...');
            // Extract patterns from successful generations
            if (trajectory.verdict === 'success') {
                const patternId = this.generatePatternId(trajectory);
                let pattern = this.patterns.get(patternId);
                if (!pattern) {
                    pattern = {
                        patternId,
                        type: 'schema',
                        description: this.describePattern(trajectory),
                        successRate: 0,
                        timesApplied: 0,
                        averageQuality: 0,
                        recommendations: []
                    };
                }
                // Update pattern statistics
                pattern.timesApplied++;
                pattern.averageQuality =
                    (pattern.averageQuality * (pattern.timesApplied - 1) + trajectory.quality) /
                        pattern.timesApplied;
                pattern.successRate =
                    (pattern.successRate * (pattern.timesApplied - 1) + 1) /
                        pattern.timesApplied;
                // Generate recommendations
                pattern.recommendations = this.generateRecommendations(pattern, trajectory);
                this.patterns.set(patternId, pattern);
                // Save pattern
                await this.savePattern(pattern);
                console.log(`   📝 Updated pattern: ${patternId}`);
                console.log(`   📊 Success rate: ${(pattern.successRate * 100).toFixed(1)}%`);
            }
            // Distill memory from trajectory
            await this.distillMemory(trajectory);
        }
        catch (error) {
            console.error('Learning failed:', error);
        }
    }
    /**
     * Adaptive schema evolution based on learning
     */
    async evolveSchema(baseSchema, targetQuality = 0.95, maxGenerations = 10) {
        try {
            console.log(`\n🧬 Evolving schema to reach ${(targetQuality * 100).toFixed(0)}% quality...`);
            let currentSchema = baseSchema;
            let generation = 0;
            let bestQuality = 0;
            let bestSchema = baseSchema;
            while (generation < maxGenerations && bestQuality < targetQuality) {
                generation++;
                console.log(`\n   Generation ${generation}/${maxGenerations}`);
                // Generate test data
                const { data, trajectory } = await this.generateWithLearning(currentSchema, { count: 100 }, `Schema evolution - Generation ${generation}`);
                // Track quality
                if (trajectory.quality > bestQuality) {
                    bestQuality = trajectory.quality;
                    bestSchema = currentSchema;
                    console.log(`   🎯 New best quality: ${(bestQuality * 100).toFixed(1)}%`);
                }
                // Apply learned patterns to mutate schema
                if (trajectory.quality < targetQuality) {
                    const mutations = this.applyLearningToSchema(currentSchema, trajectory);
                    currentSchema = this.mutateSchema(currentSchema, mutations);
                    console.log(`   🔄 Applied ${mutations.length} mutations`);
                }
                else {
                    console.log(`   ✅ Target quality reached!`);
                    break;
                }
            }
            // Save evolved schema
            const adaptiveSchema = {
                version: `v${generation}`,
                schema: bestSchema,
                performance: bestQuality,
                generation,
                mutations: []
            };
            const schemaId = `schema_${Date.now()}`;
            this.schemas.set(schemaId, adaptiveSchema);
            await this.saveSchema(schemaId, adaptiveSchema);
            console.log(`\n🏆 Evolution complete:`);
            console.log(`   Final quality: ${(bestQuality * 100).toFixed(1)}%`);
            console.log(`   Generations: ${generation}`);
            return adaptiveSchema;
        }
        catch (error) {
            throw new Error(`Schema evolution failed: ${error.message}`);
        }
    }
    /**
     * Pattern recognition across trajectories
     */
    async recognizePatterns() {
        try {
            console.log('\n🔍 Recognizing patterns from trajectories...');
            const recognizedPatterns = [];
            // Analyze successful trajectories
            const successfulTrajectories = this.trajectories.filter(t => t.verdict === 'success' && t.quality > 0.8);
            // Group by schema similarity
            const schemaGroups = this.groupBySchemaStructure(successfulTrajectories);
            for (const [structure, trajectories] of schemaGroups.entries()) {
                const avgQuality = trajectories.reduce((sum, t) => sum + t.quality, 0) / trajectories.length;
                const pattern = {
                    patternId: `pattern_${structure}`,
                    type: 'schema',
                    description: `Schema structure with ${trajectories.length} successful generations`,
                    successRate: 1.0,
                    timesApplied: trajectories.length,
                    averageQuality: avgQuality,
                    recommendations: this.synthesizeRecommendations(trajectories)
                };
                recognizedPatterns.push(pattern);
            }
            console.log(`✅ Recognized ${recognizedPatterns.length} patterns`);
            return recognizedPatterns;
        }
        catch (error) {
            throw new Error(`Pattern recognition failed: ${error.message}`);
        }
    }
    /**
     * Self-improvement through continuous learning
     */
    async continuousImprovement(iterations = 5) {
        try {
            console.log(`\n🔄 Starting continuous improvement (${iterations} iterations)...\n`);
            const improvementLog = {
                iterations: [],
                qualityTrend: [],
                patternsLearned: 0,
                bestQuality: 0
            };
            for (let i = 0; i < iterations; i++) {
                console.log(`\n━━━ Iteration ${i + 1}/${iterations} ━━━`);
                // Get best learned pattern
                const bestPattern = this.getBestPattern();
                // Generate using best known approach
                const schema = bestPattern
                    ? this.schemaFromPattern(bestPattern)
                    : this.getBaseSchema();
                const { trajectory } = await this.generateWithLearning(schema, { count: 500 }, `Continuous improvement iteration ${i + 1}`);
                // Track improvement
                improvementLog.iterations.push({
                    iteration: i + 1,
                    quality: trajectory.quality,
                    verdict: trajectory.verdict,
                    lessonsLearned: trajectory.lessons.length
                });
                improvementLog.qualityTrend.push(trajectory.quality);
                if (trajectory.quality > improvementLog.bestQuality) {
                    improvementLog.bestQuality = trajectory.quality;
                }
                // Recognize new patterns
                const newPatterns = await this.recognizePatterns();
                improvementLog.patternsLearned = newPatterns.length;
                console.log(`   📊 Quality: ${(trajectory.quality * 100).toFixed(1)}%`);
                console.log(`   🧠 Total patterns: ${improvementLog.patternsLearned}`);
            }
            // Calculate improvement rate
            const qualityImprovement = improvementLog.qualityTrend.length > 1
                ? improvementLog.qualityTrend[improvementLog.qualityTrend.length - 1] -
                    improvementLog.qualityTrend[0]
                : 0;
            console.log(`\n📈 Improvement Summary:`);
            console.log(`   Quality increase: ${(qualityImprovement * 100).toFixed(1)}%`);
            console.log(`   Best quality: ${(improvementLog.bestQuality * 100).toFixed(1)}%`);
            console.log(`   Patterns learned: ${improvementLog.patternsLearned}`);
            return improvementLog;
        }
        catch (error) {
            throw new Error(`Continuous improvement failed: ${error.message}`);
        }
    }
    // Helper methods
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
    judgeVerdict(quality, errors) {
        if (errors > 0)
            return 'failure';
        if (quality >= 0.9)
            return 'success';
        if (quality >= 0.7)
            return 'partial';
        return 'failure';
    }
    extractLessons(schema, parameters, quality, errors) {
        const lessons = [];
        if (quality > 0.9) {
            lessons.push('High quality achieved with current schema structure');
        }
        if (errors === 0) {
            lessons.push('Error-free generation with current parameters');
        }
        if (Object.keys(schema).length > 10) {
            lessons.push('Complex schemas may benefit from validation');
        }
        return lessons;
    }
    generatePatternId(trajectory) {
        const schemaKeys = Object.keys(trajectory.schema).sort().join('_');
        return `pattern_${schemaKeys}_${trajectory.verdict}`;
    }
    describePattern(trajectory) {
        const fieldCount = Object.keys(trajectory.schema).length;
        return `${trajectory.verdict} pattern with ${fieldCount} fields, quality ${(trajectory.quality * 100).toFixed(0)}%`;
    }
    generateRecommendations(pattern, trajectory) {
        const recs = [];
        if (pattern.averageQuality > 0.9) {
            recs.push('Maintain current schema structure');
        }
        if (pattern.timesApplied > 5) {
            recs.push('Consider this a proven pattern');
        }
        return recs;
    }
    applyLearningToSchema(schema, trajectory) {
        const mutations = [];
        // Apply learned improvements
        if (trajectory.quality < 0.8) {
            mutations.push('add_validation');
        }
        if (trajectory.performance.errorRate > 0.1) {
            mutations.push('simplify_types');
        }
        return mutations;
    }
    mutateSchema(schema, mutations) {
        const mutated = { ...schema };
        for (const mutation of mutations) {
            if (mutation === 'add_validation') {
                // Add validation constraints
                for (const key of Object.keys(mutated)) {
                    if (typeof mutated[key] === 'string') {
                        mutated[key] = { type: mutated[key], required: true };
                    }
                }
            }
        }
        return mutated;
    }
    groupBySchemaStructure(trajectories) {
        const groups = new Map();
        for (const trajectory of trajectories) {
            const structure = Object.keys(trajectory.schema).sort().join('_');
            if (!groups.has(structure)) {
                groups.set(structure, []);
            }
            groups.get(structure).push(trajectory);
        }
        return groups;
    }
    synthesizeRecommendations(trajectories) {
        return [
            `Based on ${trajectories.length} successful generations`,
            'Recommended for production use',
            'High reliability pattern'
        ];
    }
    getBestPattern() {
        let best = null;
        for (const pattern of this.patterns.values()) {
            if (!best || pattern.averageQuality > best.averageQuality) {
                best = pattern;
            }
        }
        return best;
    }
    schemaFromPattern(pattern) {
        // Extract schema from pattern (simplified)
        return this.getBaseSchema();
    }
    getBaseSchema() {
        return {
            name: 'string',
            email: 'email',
            age: 'number',
            city: 'string'
        };
    }
    async saveTrajectory(trajectory) {
        const file = path.join(this.repoPath, 'data/trajectories', `${trajectory.id}.json`);
        fs.writeFileSync(file, JSON.stringify(trajectory, null, 2));
    }
    async savePattern(pattern) {
        const file = path.join(this.repoPath, 'data/patterns', `${pattern.patternId}.json`);
        fs.writeFileSync(file, JSON.stringify(pattern, null, 2));
    }
    async saveSchema(id, schema) {
        const file = path.join(this.repoPath, 'data/schemas', `${id}.json`);
        fs.writeFileSync(file, JSON.stringify(schema, null, 2));
    }
    async commitWithReasoning(data, trajectory, description) {
        const dataFile = path.join(this.repoPath, 'data', `gen_${Date.now()}.json`);
        fs.writeFileSync(dataFile, JSON.stringify(data, null, 2));
        (0, child_process_1.execSync)(`npx agentic-jujutsu@latest add "${dataFile}"`, {
            cwd: this.repoPath,
            stdio: 'pipe'
        });
        const message = `${description}\n\nReasoning:\n${JSON.stringify({
            quality: trajectory.quality,
            verdict: trajectory.verdict,
            lessons: trajectory.lessons
        }, null, 2)}`;
        (0, child_process_1.execSync)(`npx agentic-jujutsu@latest commit -m "${message}"`, {
            cwd: this.repoPath,
            stdio: 'pipe'
        });
    }
    async distillMemory(trajectory) {
        const memoryFile = path.join(this.repoPath, 'data/memories', `memory_${Date.now()}.json`);
        fs.writeFileSync(memoryFile, JSON.stringify({
            trajectory: trajectory.id,
            timestamp: trajectory.timestamp,
            key_lessons: trajectory.lessons,
            quality: trajectory.quality
        }, null, 2));
    }
    async loadLearningState() {
        // Load trajectories
        const trajDir = path.join(this.repoPath, 'data/trajectories');
        if (fs.existsSync(trajDir)) {
            const files = fs.readdirSync(trajDir);
            for (const file of files) {
                if (file.endsWith('.json')) {
                    const content = fs.readFileSync(path.join(trajDir, file), 'utf-8');
                    this.trajectories.push(JSON.parse(content));
                }
            }
        }
        // Load patterns
        const patternDir = path.join(this.repoPath, 'data/patterns');
        if (fs.existsSync(patternDir)) {
            const files = fs.readdirSync(patternDir);
            for (const file of files) {
                if (file.endsWith('.json')) {
                    const content = fs.readFileSync(path.join(patternDir, file), 'utf-8');
                    const pattern = JSON.parse(content);
                    this.patterns.set(pattern.patternId, pattern);
                }
            }
        }
    }
}
exports.ReasoningBankDataGenerator = ReasoningBankDataGenerator;
// Example usage
async function main() {
    console.log('🚀 ReasoningBank Learning Integration Example\n');
    const repoPath = path.join(process.cwd(), 'reasoning-bank-repo');
    const generator = new ReasoningBankDataGenerator(repoPath);
    try {
        // Initialize
        await generator.initialize();
        // Generate with learning
        const schema = {
            name: 'string',
            email: 'email',
            age: 'number',
            city: 'string',
            active: 'boolean'
        };
        await generator.generateWithLearning(schema, { count: 1000 }, 'Initial learning generation');
        // Evolve schema
        const evolved = await generator.evolveSchema(schema, 0.95, 5);
        console.log('\n🧬 Evolved schema:', evolved);
        // Continuous improvement
        const improvement = await generator.continuousImprovement(3);
        console.log('\n📈 Improvement log:', improvement);
        console.log('\n✅ ReasoningBank learning example completed!');
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
//# sourceMappingURL=reasoning-bank-learning.js.map