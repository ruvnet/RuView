"use strict";
/**
 * Contrastive Fine-tuning for RuvLTRA Claude Code Router
 *
 * Uses triplet loss to fine-tune embeddings:
 * - Anchor: task description
 * - Positive: correct agent description
 * - Negative: wrong agent description (hard negative)
 *
 * Goal: minimize distance(anchor, positive) and maximize distance(anchor, negative)
 *
 * @example
 * ```typescript
 * import { ContrastiveTrainer, tripletLoss, infoNCELoss } from '@ruvector/ruvllm';
 *
 * const trainer = new ContrastiveTrainer({
 *   epochs: 10,
 *   batchSize: 16,
 *   margin: 0.5,
 * });
 *
 * // Add triplets
 * trainer.addTriplet(anchorEmb, positiveEmb, negativeEmb, true);
 *
 * // Train and export
 * const results = trainer.train();
 * trainer.exportTrainingData('./output');
 * ```
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.AGENT_TRAINING_DATA = exports.ContrastiveTrainer = void 0;
exports.cosineSimilarity = cosineSimilarity;
exports.tripletLoss = tripletLoss;
exports.infoNCELoss = infoNCELoss;
exports.computeGradient = computeGradient;
exports.generateTrainingDataset = generateTrainingDataset;
exports.generateContrastivePairs = generateContrastivePairs;
exports.getDatasetStats = getDatasetStats;
const fs_1 = require("fs");
const path_1 = require("path");
/**
 * Default contrastive config
 */
const DEFAULT_CONTRASTIVE_CONFIG = {
    epochs: 10,
    batchSize: 16,
    learningRate: 0.0001,
    margin: 0.5,
    temperature: 0.07,
    hardNegativeRatio: 0.7,
    outputPath: './training-output',
};
/**
 * Compute cosine similarity between two embeddings
 */
function cosineSimilarity(a, b) {
    if (!a || !b || a.length !== b.length)
        return 0;
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
        dot += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
    }
    return dot / (Math.sqrt(normA) * Math.sqrt(normB) || 1);
}
/**
 * Compute triplet loss
 * L = max(0, margin + d(anchor, positive) - d(anchor, negative))
 */
function tripletLoss(anchorEmb, positiveEmb, negativeEmb, margin = 0.5) {
    const posDist = 1 - cosineSimilarity(anchorEmb, positiveEmb);
    const negDist = 1 - cosineSimilarity(anchorEmb, negativeEmb);
    return Math.max(0, margin + posDist - negDist);
}
/**
 * Compute InfoNCE loss (contrastive)
 */
function infoNCELoss(anchorEmb, positiveEmb, negativeEmbs, temperature = 0.07) {
    const posSim = cosineSimilarity(anchorEmb, positiveEmb) / temperature;
    const negSims = negativeEmbs.map(neg => cosineSimilarity(anchorEmb, neg) / temperature);
    // Softmax denominator
    const maxSim = Math.max(posSim, ...negSims);
    const expPos = Math.exp(posSim - maxSim);
    const expNegs = negSims.map(sim => Math.exp(sim - maxSim));
    const denominator = expPos + expNegs.reduce((a, b) => a + b, 0);
    // Cross-entropy loss
    return -Math.log(expPos / denominator);
}
/**
 * Compute gradient for embedding update (simplified)
 */
function computeGradient(anchorEmb, positiveEmb, negativeEmb, lr = 0.0001) {
    const dim = anchorEmb.length;
    const gradient = new Array(dim).fill(0);
    // Pull anchor towards positive
    for (let i = 0; i < dim; i++) {
        gradient[i] += lr * (positiveEmb[i] - anchorEmb[i]);
    }
    // Push anchor away from negative
    for (let i = 0; i < dim; i++) {
        gradient[i] -= lr * 0.5 * (negativeEmb[i] - anchorEmb[i]);
    }
    return gradient;
}
/**
 * Contrastive Trainer for RuvLTRA models
 *
 * Implements triplet loss and InfoNCE loss for embedding fine-tuning.
 */
class ContrastiveTrainer {
    constructor(config) {
        this.triplets = [];
        this.history = [];
        this.agentEmbeddings = new Map();
        this.config = { ...DEFAULT_CONTRASTIVE_CONFIG, ...config };
    }
    /**
     * Add a training triplet
     */
    addTriplet(anchor, anchorEmb, positive, positiveEmb, negative, negativeEmb, isHard = false) {
        this.triplets.push({
            anchor,
            anchorEmb,
            positive,
            positiveEmb,
            negative,
            negativeEmb,
            isHard,
        });
    }
    /**
     * Add agent embedding for reference
     */
    addAgentEmbedding(agentName, embedding) {
        this.agentEmbeddings.set(agentName, embedding);
    }
    /**
     * Get all agent embeddings
     */
    getAgentEmbeddings() {
        return this.agentEmbeddings;
    }
    /**
     * Get triplet count
     */
    getTripletCount() {
        return this.triplets.length;
    }
    /**
     * Simulate training (compute losses without actual backprop)
     * In a full implementation, this would use proper gradient descent
     */
    train() {
        const startTime = Date.now();
        const { epochs, batchSize, margin } = this.config;
        if (this.triplets.length === 0) {
            return {
                tripletCount: 0,
                finalLoss: 0,
                initialLoss: 0,
                improvement: 0,
                history: [],
                durationMs: 0,
            };
        }
        for (let epoch = 0; epoch < epochs; epoch++) {
            let epochLoss = 0;
            let batchCount = 0;
            // Shuffle triplets
            const shuffled = [...this.triplets].sort(() => Math.random() - 0.5);
            for (let i = 0; i < shuffled.length; i += batchSize) {
                const batch = shuffled.slice(i, i + batchSize);
                let batchLoss = 0;
                for (const triplet of batch) {
                    const loss = tripletLoss(triplet.anchorEmb, triplet.positiveEmb, triplet.negativeEmb, margin);
                    batchLoss += loss;
                }
                epochLoss += batchLoss / batch.length;
                batchCount++;
            }
            const avgLoss = epochLoss / batchCount;
            this.history.push({ epoch: epoch + 1, loss: avgLoss });
        }
        const initialLoss = this.history[0]?.loss || 0;
        const finalLoss = this.history[this.history.length - 1]?.loss || 0;
        const improvement = initialLoss > 0 ? (1 - finalLoss / initialLoss) * 100 : 0;
        return {
            tripletCount: this.triplets.length,
            finalLoss,
            initialLoss,
            improvement,
            history: this.history,
            durationMs: Date.now() - startTime,
        };
    }
    /**
     * Export training data for external fine-tuning tools
     */
    exportTrainingData(outputPath) {
        const outDir = outputPath || this.config.outputPath;
        if (!(0, fs_1.existsSync)(outDir)) {
            (0, fs_1.mkdirSync)(outDir, { recursive: true });
        }
        // JSONL format for fine-tuning
        const jsonlData = this.triplets.map(t => ({
            anchor: t.anchor,
            positive: t.positive,
            negative: t.negative,
            isHard: t.isHard,
        }));
        // CSV format for analysis
        const csvData = [
            'anchor,positive,negative,is_hard',
            ...this.triplets.map(t => `"${t.anchor.replace(/"/g, '""')}",${t.positive},${t.negative},${t.isHard}`),
        ].join('\n');
        // Embedding matrix for direct training
        const embeddingData = {
            anchors: this.triplets.map(t => t.anchorEmb),
            positives: this.triplets.map(t => t.positiveEmb),
            negatives: this.triplets.map(t => t.negativeEmb),
            labels: this.triplets.map(t => t.positive),
        };
        (0, fs_1.writeFileSync)((0, path_1.join)(outDir, 'triplets.jsonl'), jsonlData.map(item => JSON.stringify(item)).join('\n'));
        (0, fs_1.writeFileSync)((0, path_1.join)(outDir, 'triplets.csv'), csvData);
        (0, fs_1.writeFileSync)((0, path_1.join)(outDir, 'embeddings.json'), JSON.stringify(embeddingData, null, 2));
        return outDir;
    }
    /**
     * Generate LoRA adapter configuration
     */
    generateLoRAConfig(outputPath) {
        const outDir = outputPath || this.config.outputPath;
        const loraConfig = {
            model_type: 'qwen2',
            base_model: 'Qwen/Qwen2.5-0.5B',
            output_dir: outDir,
            lora_r: 8,
            lora_alpha: 16,
            lora_dropout: 0.05,
            target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj'],
            learning_rate: this.config.learningRate,
            num_train_epochs: this.config.epochs,
            per_device_train_batch_size: this.config.batchSize,
            gradient_accumulation_steps: 4,
            warmup_ratio: 0.1,
            loss_type: 'triplet',
            margin: this.config.margin,
            temperature: this.config.temperature,
            train_data: (0, path_1.join)(outDir, 'triplets.jsonl'),
            eval_data: (0, path_1.join)(outDir, 'eval.jsonl'),
        };
        if (!(0, fs_1.existsSync)(outDir)) {
            (0, fs_1.mkdirSync)(outDir, { recursive: true });
        }
        (0, fs_1.writeFileSync)((0, path_1.join)(outDir, 'lora_config.json'), JSON.stringify(loraConfig, null, 2));
        return loraConfig;
    }
    /**
     * Generate training script for external tools
     */
    generateTrainingScript(outputPath) {
        const outDir = outputPath || this.config.outputPath;
        const script = `#!/bin/bash
# RuvLTRA Fine-tuning Script
# Prerequisites: pip install transformers peft accelerate

set -e

MODEL_PATH="${outDir}"
BASE_MODEL="Qwen/Qwen2.5-0.5B"

echo "=== RuvLTRA Contrastive Fine-tuning ==="
echo "Base model: $BASE_MODEL"
echo "Output: $MODEL_PATH"

# Check for training data
if [ ! -f "$MODEL_PATH/triplets.jsonl" ]; then
  echo "Error: Training data not found at $MODEL_PATH/triplets.jsonl"
  exit 1
fi

# Install dependencies if needed
python3 -c "import transformers, peft" 2>/dev/null || {
  echo "Installing dependencies..."
  pip install transformers peft accelerate sentencepiece
}

# Fine-tune with LoRA
python3 << 'PYTHON'
import json
import torch
from pathlib import Path
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import LoraConfig, get_peft_model, TaskType

# Load config
config_path = Path("${outDir}/lora_config.json")
with open(config_path) as f:
    config = json.load(f)

print(f"Loading base model: {config['base_model']}")

# Load model and tokenizer
tokenizer = AutoTokenizer.from_pretrained(config['base_model'])
model = AutoModelForCausalLM.from_pretrained(
    config['base_model'],
    torch_dtype=torch.float16,
    device_map='auto'
)

# Configure LoRA
lora_config = LoraConfig(
    r=config['lora_r'],
    lora_alpha=config['lora_alpha'],
    lora_dropout=config['lora_dropout'],
    target_modules=config['target_modules'],
    task_type=TaskType.CAUSAL_LM,
)

model = get_peft_model(model, lora_config)
model.print_trainable_parameters()

print("Model ready for fine-tuning!")
print(f"Training data: {config['train_data']}")
print("Note: Full training requires GPU. This script validates the setup.")
PYTHON

echo ""
echo "=== Setup Complete ==="
echo "To train on GPU, run the full training pipeline."
echo "Training data exported to: $MODEL_PATH/triplets.jsonl"
`;
        if (!(0, fs_1.existsSync)(outDir)) {
            (0, fs_1.mkdirSync)(outDir, { recursive: true });
        }
        const scriptPath = (0, path_1.join)(outDir, 'train.sh');
        (0, fs_1.writeFileSync)(scriptPath, script);
        return scriptPath;
    }
    /**
     * Get training history
     */
    getHistory() {
        return [...this.history];
    }
    /**
     * Reset trainer
     */
    reset() {
        this.triplets = [];
        this.history = [];
    }
}
exports.ContrastiveTrainer = ContrastiveTrainer;
/**
 * Agent Training Data for Claude Code Router
 */
exports.AGENT_TRAINING_DATA = {
    coder: {
        description: 'Implementation specialist for writing clean, efficient code. Handles coding tasks, feature implementation, and code generation.',
        keywords: ['implement', 'code', 'write', 'build', 'create', 'develop', 'function', 'class', 'component', 'feature'],
        examples: [
            'Implement a binary search function',
            'Write a React component for user registration',
            'Create a REST API endpoint for user authentication',
            'Build a caching layer for the database queries',
        ],
        confusing_with: ['refactorer', 'debugger'],
    },
    tester: {
        description: 'Testing specialist for writing and maintaining tests. Creates unit tests, integration tests, and ensures code quality through testing.',
        keywords: ['test', 'unit test', 'integration test', 'coverage', 'mock', 'assertion', 'spec', 'jest', 'pytest'],
        examples: [
            'Write unit tests for the authentication module',
            'Add integration tests for the payment gateway',
            'Create test coverage for the user service',
            'Write e2e tests for the checkout flow',
        ],
        confusing_with: ['reviewer'],
    },
    reviewer: {
        description: 'Code review specialist for analyzing code quality, identifying issues, and suggesting improvements.',
        keywords: ['review', 'analyze', 'check', 'inspect', 'audit', 'evaluate', 'assess', 'critique'],
        examples: [
            'Review the pull request for code quality',
            'Check the code for potential security vulnerabilities',
            'Analyze the implementation for best practices',
            'Evaluate the architecture decisions in this PR',
        ],
        confusing_with: ['tester', 'security-architect'],
    },
    researcher: {
        description: 'Research specialist for investigating technologies, gathering information, and analyzing options.',
        keywords: ['research', 'investigate', 'explore', 'analyze', 'study', 'compare', 'evaluate', 'learn'],
        examples: [
            'Research best practices for React state management',
            'Investigate the performance issues in the dashboard',
            'Compare different authentication strategies',
            'Study the codebase architecture for the new feature',
        ],
        confusing_with: ['planner'],
    },
    architect: {
        description: 'System architect for designing software architecture, making technical decisions, and planning system structure.',
        keywords: ['design', 'architect', 'structure', 'plan', 'schema', 'model', 'pattern', 'system'],
        examples: [
            'Design the database schema for user profiles',
            'Plan the architecture for real-time notifications',
            'Create a system design for the microservices migration',
            'Design the API structure for the new product catalog',
        ],
        confusing_with: ['planner'],
    },
    debugger: {
        description: 'Debugging specialist for finding and fixing bugs, analyzing errors, and troubleshooting issues.',
        keywords: ['debug', 'fix', 'bug', 'error', 'issue', 'crash', 'exception', 'troubleshoot'],
        examples: [
            'Fix the null pointer exception in the login handler',
            'Debug the memory leak in the WebSocket handler',
            'Troubleshoot the race condition in the payment processor',
            'Find the root cause of the intermittent test failures',
        ],
        confusing_with: ['coder'],
    },
    'security-architect': {
        description: 'Security specialist for auditing code security, identifying vulnerabilities, and implementing security measures.',
        keywords: ['security', 'vulnerability', 'xss', 'sql injection', 'auth', 'encryption', 'audit', 'penetration'],
        examples: [
            'Audit the API endpoints for XSS vulnerabilities',
            'Review the authentication flow for security issues',
            'Implement input validation for the user forms',
            'Check for SQL injection vulnerabilities in the search',
        ],
        confusing_with: ['reviewer'],
    },
    documenter: {
        description: 'Documentation specialist for writing technical documentation, comments, and API docs.',
        keywords: ['document', 'comment', 'jsdoc', 'readme', 'docs', 'explain', 'describe', 'annotate'],
        examples: [
            'Write JSDoc comments for the utility functions',
            'Create README documentation for the new module',
            'Document the API endpoints with examples',
            'Add inline comments explaining the algorithm',
        ],
        confusing_with: ['api-docs'],
    },
    refactorer: {
        description: 'Refactoring specialist for improving code structure, cleaning up technical debt, and modernizing codebases.',
        keywords: ['refactor', 'clean', 'restructure', 'modernize', 'improve', 'simplify', 'extract', 'rename'],
        examples: [
            'Refactor the payment module to use async/await',
            'Clean up the legacy authentication code',
            'Extract common logic into a shared utility',
            'Simplify the complex conditional logic in checkout',
        ],
        confusing_with: ['coder'],
    },
    optimizer: {
        description: 'Performance optimization specialist for improving speed, reducing memory usage, and optimizing queries.',
        keywords: ['optimize', 'performance', 'speed', 'memory', 'cache', 'index', 'query', 'latency'],
        examples: [
            'Optimize the database queries for the dashboard',
            'Improve the page load time for the homepage',
            'Add caching to reduce API response times',
            'Reduce memory usage in the image processing pipeline',
        ],
        confusing_with: ['researcher'],
    },
    devops: {
        description: 'DevOps specialist for CI/CD pipelines, deployment automation, and infrastructure management.',
        keywords: ['deploy', 'ci/cd', 'pipeline', 'docker', 'kubernetes', 'terraform', 'aws', 'infrastructure'],
        examples: [
            'Set up the CI/CD pipeline for the microservices',
            'Configure Docker containers for the application',
            'Deploy the application to the staging environment',
            'Create Terraform scripts for the AWS infrastructure',
        ],
        confusing_with: [],
    },
    'api-docs': {
        description: 'API documentation specialist for creating OpenAPI specs, Swagger documentation, and API references.',
        keywords: ['openapi', 'swagger', 'api docs', 'endpoint', 'specification', 'schema', 'rest'],
        examples: [
            'Generate OpenAPI documentation for the REST API',
            'Create Swagger specs for the user endpoints',
            'Document the API authentication requirements',
            'Update the API reference with new endpoints',
        ],
        confusing_with: ['documenter'],
    },
    planner: {
        description: 'Project planning specialist for creating task plans, sprint planning, and roadmap development.',
        keywords: ['plan', 'roadmap', 'sprint', 'milestone', 'timeline', 'estimate', 'breakdown', 'prioritize'],
        examples: [
            'Create a sprint plan for the next two weeks',
            'Break down the feature into smaller tasks',
            'Estimate the effort for the migration project',
            'Prioritize the bug fixes for the release',
        ],
        confusing_with: ['architect', 'researcher'],
    },
};
/**
 * Generate training dataset from agent data
 */
function generateTrainingDataset() {
    const examples = [];
    for (const [agent, data] of Object.entries(exports.AGENT_TRAINING_DATA)) {
        // Add direct examples
        for (const example of data.examples) {
            examples.push({
                task: example,
                agent,
                complexity: 'medium',
            });
        }
        // Generate variations with keywords
        for (const keyword of data.keywords) {
            examples.push({
                task: `${keyword} a solution for the authentication system`,
                agent,
                complexity: 'low',
            });
        }
        // Add confusing pairs for hard negatives
        if (data.confusing_with) {
            for (const confusingAgent of data.confusing_with) {
                for (const example of data.examples.slice(0, 2)) {
                    examples.push({
                        task: example,
                        agent,
                        complexity: 'hard',
                        confusing_with: confusingAgent,
                    });
                }
            }
        }
    }
    return examples;
}
/**
 * Generate contrastive pairs for training
 */
function generateContrastivePairs() {
    const pairs = [];
    const agents = Object.keys(exports.AGENT_TRAINING_DATA);
    for (const [agent, data] of Object.entries(exports.AGENT_TRAINING_DATA)) {
        for (const example of data.examples) {
            // Hard negatives from confusing agents
            if (data.confusing_with) {
                for (const negAgent of data.confusing_with) {
                    pairs.push({
                        anchor: example,
                        positive: agent,
                        negative: negAgent,
                        isHard: true,
                    });
                }
            }
            // Random negatives
            const randomNegs = agents.filter(a => a !== agent).slice(0, 2);
            for (const negAgent of randomNegs) {
                pairs.push({
                    anchor: example,
                    positive: agent,
                    negative: negAgent,
                    isHard: false,
                });
            }
        }
    }
    return pairs;
}
/**
 * Get dataset statistics
 */
function getDatasetStats() {
    const examples = generateTrainingDataset();
    const pairs = generateContrastivePairs();
    const agents = Object.keys(exports.AGENT_TRAINING_DATA);
    return {
        totalExamples: examples.length,
        contrastivePairs: pairs.length,
        agentTypes: agents.length,
        agents,
    };
}
//# sourceMappingURL=contrastive.js.map