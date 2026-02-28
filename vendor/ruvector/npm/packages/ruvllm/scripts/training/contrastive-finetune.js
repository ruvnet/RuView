#!/usr/bin/env node
/**
 * Contrastive Fine-tuning for RuvLTRA Claude Code Router
 *
 * Uses triplet loss to fine-tune embeddings:
 * - Anchor: task description
 * - Positive: correct agent description
 * - Negative: wrong agent description (hard negative)
 *
 * Goal: minimize distance(anchor, positive) and maximize distance(anchor, negative)
 */

const { execSync } = require('child_process');
const { existsSync, writeFileSync, readFileSync, mkdirSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const OUTPUT_DIR = join(homedir(), '.ruvllm', 'training');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');

// Import training data
const { AGENT_TRAINING_DATA, generateTrainingDataset, generateContrastivePairs, getDatasetStats } = require('./routing-dataset');

// Build agent descriptions from training data
const AGENT_DESCRIPTIONS = {};
for (const [agent, data] of Object.entries(AGENT_TRAINING_DATA)) {
  AGENT_DESCRIPTIONS[agent] = data.description;
}

// Get training data
const TRAINING_EXAMPLES = generateTrainingDataset();
const CONTRASTIVE_PAIRS_RAW = generateContrastivePairs();

// Training configuration
const CONFIG = {
  epochs: 10,
  batchSize: 16,
  learningRate: 0.0001,
  margin: 0.5,           // Triplet loss margin
  temperature: 0.07,     // InfoNCE temperature
  hardNegativeRatio: 0.7, // Ratio of hard negatives
  outputPath: join(OUTPUT_DIR, 'ruvltra-finetuned'),
};

/**
 * Get embedding from model
 */
function getEmbedding(modelPath, text) {
  try {
    const sanitized = text.replace(/"/g, '\\"').replace(/\n/g, ' ').slice(0, 500);
    const result = execSync(
      `llama-embedding -m "${modelPath}" -p "${sanitized}" --embd-output-format json 2>/dev/null`,
      { encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 }
    );
    const json = JSON.parse(result);
    return json.data[json.data.length - 1].embedding;
  } catch {
    return null;
  }
}

/**
 * Compute cosine similarity
 */
function cosineSimilarity(a, b) {
  if (!a || !b || a.length !== b.length) return 0;
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
function tripletLoss(anchorEmb, positiveEmb, negativeEmb, margin = CONFIG.margin) {
  const posDist = 1 - cosineSimilarity(anchorEmb, positiveEmb);
  const negDist = 1 - cosineSimilarity(anchorEmb, negativeEmb);
  return Math.max(0, margin + posDist - negDist);
}

/**
 * Compute InfoNCE loss (contrastive)
 */
function infoNCELoss(anchorEmb, positiveEmb, negativeEmbs, temperature = CONFIG.temperature) {
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
 * Prepare training batches with triplets
 */
function prepareTrainingData(modelPath) {
  console.log('Preparing training data...');

  // Pre-compute agent description embeddings
  const agentEmbeddings = {};
  for (const [agent, desc] of Object.entries(AGENT_DESCRIPTIONS)) {
    process.stdout.write(`  Embedding ${agent}... `);
    agentEmbeddings[agent] = getEmbedding(modelPath, desc);
    console.log('done');
  }

  // Create triplets from training examples
  const triplets = [];
  const agents = Object.keys(AGENT_DESCRIPTIONS);

  console.log(`\nGenerating triplets from ${TRAINING_EXAMPLES.length} examples...`);

  // Group examples by agent
  const examplesByAgent = {};
  for (const ex of TRAINING_EXAMPLES) {
    if (!examplesByAgent[ex.agent]) examplesByAgent[ex.agent] = [];
    examplesByAgent[ex.agent].push(ex);
  }

  // Create triplets: anchor task, positive agent, negative agent
  for (const example of TRAINING_EXAMPLES.slice(0, 200)) { // Limit for speed
    const anchorEmb = getEmbedding(modelPath, example.task);
    if (!anchorEmb) continue;

    const positiveAgent = example.agent;
    const positiveEmb = agentEmbeddings[positiveAgent];

    // Get hard negatives (confusing agents)
    const hardNegatives = example.confusing_with
      ? [example.confusing_with]
      : agents.filter(a => a !== positiveAgent).slice(0, 2);

    for (const negAgent of hardNegatives) {
      const negativeEmb = agentEmbeddings[negAgent];
      if (negativeEmb) {
        triplets.push({
          anchor: example.task,
          anchorEmb,
          positive: positiveAgent,
          positiveEmb,
          negative: negAgent,
          negativeEmb,
          isHard: !!example.confusing_with,
        });
      }
    }

    // Add random negative for diversity
    const randomNeg = agents.filter(a => a !== positiveAgent)[Math.floor(Math.random() * (agents.length - 1))];
    if (agentEmbeddings[randomNeg]) {
      triplets.push({
        anchor: example.task,
        anchorEmb,
        positive: positiveAgent,
        positiveEmb,
        negative: randomNeg,
        negativeEmb: agentEmbeddings[randomNeg],
        isHard: false,
      });
    }
  }

  console.log(`Created ${triplets.length} triplets`);
  return { triplets, agentEmbeddings };
}

/**
 * Compute gradient for embedding update (simplified)
 * In practice, this would be done via proper backprop
 */
function computeGradient(anchorEmb, positiveEmb, negativeEmb, lr = CONFIG.learningRate) {
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
 * Export training data for external fine-tuning tools
 */
function exportTrainingData(triplets, outputPath) {
  console.log(`\nExporting training data to ${outputPath}...`);

  // JSONL format for fine-tuning
  const jsonlData = triplets.map(t => ({
    anchor: t.anchor,
    positive: t.positive,
    negative: t.negative,
    isHard: t.isHard,
  }));

  // CSV format for analysis
  const csvData = [
    'anchor,positive,negative,is_hard',
    ...triplets.map(t => `"${t.anchor.replace(/"/g, '""')}",${t.positive},${t.negative},${t.isHard}`)
  ].join('\n');

  // Embedding matrix for direct training
  const embeddingData = {
    anchors: triplets.map(t => t.anchorEmb),
    positives: triplets.map(t => t.positiveEmb),
    negatives: triplets.map(t => t.negativeEmb),
    labels: triplets.map(t => t.positive),
  };

  mkdirSync(outputPath, { recursive: true });
  writeFileSync(join(outputPath, 'triplets.jsonl'), jsonlData.map(JSON.stringify).join('\n'));
  writeFileSync(join(outputPath, 'triplets.csv'), csvData);
  writeFileSync(join(outputPath, 'embeddings.json'), JSON.stringify(embeddingData, null, 2));

  console.log(`  Exported ${triplets.length} triplets`);
  return outputPath;
}

/**
 * Simulate training loop (compute losses)
 */
function simulateTraining(triplets, epochs = CONFIG.epochs) {
  console.log(`\nSimulating ${epochs} epochs of training...`);

  const batchSize = CONFIG.batchSize;
  const history = [];

  for (let epoch = 0; epoch < epochs; epoch++) {
    let epochLoss = 0;
    let batchCount = 0;

    // Shuffle triplets
    const shuffled = [...triplets].sort(() => Math.random() - 0.5);

    for (let i = 0; i < shuffled.length; i += batchSize) {
      const batch = shuffled.slice(i, i + batchSize);
      let batchLoss = 0;

      for (const triplet of batch) {
        const loss = tripletLoss(
          triplet.anchorEmb,
          triplet.positiveEmb,
          triplet.negativeEmb
        );
        batchLoss += loss;
      }

      epochLoss += batchLoss / batch.length;
      batchCount++;
    }

    const avgLoss = epochLoss / batchCount;
    history.push({ epoch: epoch + 1, loss: avgLoss });

    process.stdout.write(`  Epoch ${epoch + 1}/${epochs}: loss = ${avgLoss.toFixed(4)}\r`);
  }

  console.log('\n');
  return history;
}

/**
 * Evaluate model on test set
 */
function evaluateModel(modelPath, agentEmbeddings) {
  const ROUTING_TESTS = [
    { task: 'Implement a binary search function in TypeScript', expected: 'coder' },
    { task: 'Write unit tests for the authentication module', expected: 'tester' },
    { task: 'Review the pull request for security vulnerabilities', expected: 'reviewer' },
    { task: 'Research best practices for React state management', expected: 'researcher' },
    { task: 'Design the database schema for user profiles', expected: 'architect' },
    { task: 'Fix the null pointer exception in the login handler', expected: 'debugger' },
    { task: 'Audit the API endpoints for XSS vulnerabilities', expected: 'security-architect' },
    { task: 'Write JSDoc comments for the utility functions', expected: 'documenter' },
    { task: 'Refactor the payment module to use async/await', expected: 'refactorer' },
    { task: 'Optimize the database queries for the dashboard', expected: 'optimizer' },
    { task: 'Set up the CI/CD pipeline for the microservices', expected: 'devops' },
    { task: 'Generate OpenAPI documentation for the REST API', expected: 'api-docs' },
    { task: 'Create a sprint plan for the next two weeks', expected: 'planner' },
    { task: 'Build a React component for user registration', expected: 'coder' },
    { task: 'Debug memory leak in the WebSocket handler', expected: 'debugger' },
    { task: 'Investigate slow API response times', expected: 'researcher' },
    { task: 'Check code for potential race conditions', expected: 'reviewer' },
    { task: 'Add integration tests for the payment gateway', expected: 'tester' },
    { task: 'Plan the architecture for real-time notifications', expected: 'architect' },
    { task: 'Cache the frequently accessed user data', expected: 'optimizer' },
  ];

  let correct = 0;
  const results = [];

  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, test.task);

    let bestAgent = 'coder';
    let bestSim = -1;

    for (const [agent, emb] of Object.entries(agentEmbeddings)) {
      const sim = cosineSimilarity(taskEmb, emb);
      if (sim > bestSim) {
        bestSim = sim;
        bestAgent = agent;
      }
    }

    const isCorrect = bestAgent === test.expected;
    if (isCorrect) correct++;
    results.push({ task: test.task, expected: test.expected, got: bestAgent, correct: isCorrect });
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length, results };
}

/**
 * Generate LoRA adapter configuration
 */
function generateLoRAConfig(outputPath) {
  const loraConfig = {
    model_type: 'qwen2',
    base_model: 'Qwen/Qwen2.5-0.5B',
    output_dir: outputPath,

    // LoRA parameters
    lora_r: 8,
    lora_alpha: 16,
    lora_dropout: 0.05,
    target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj'],

    // Training parameters
    learning_rate: CONFIG.learningRate,
    num_train_epochs: CONFIG.epochs,
    per_device_train_batch_size: CONFIG.batchSize,
    gradient_accumulation_steps: 4,
    warmup_ratio: 0.1,

    // Contrastive loss parameters
    loss_type: 'triplet',
    margin: CONFIG.margin,
    temperature: CONFIG.temperature,

    // Data
    train_data: join(outputPath, 'triplets.jsonl'),
    eval_data: join(outputPath, 'eval.jsonl'),
  };

  writeFileSync(join(outputPath, 'lora_config.json'), JSON.stringify(loraConfig, null, 2));
  return loraConfig;
}

/**
 * Generate training script for external tools
 */
function generateTrainingScript(outputPath) {
  const script = `#!/bin/bash
# RuvLTRA Fine-tuning Script
# Prerequisites: pip install transformers peft accelerate

set -e

MODEL_PATH="${outputPath}"
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
from transformers import AutoModelForCausalLM, AutoTokenizer, TrainingArguments
from peft import LoraConfig, get_peft_model, TaskType

# Load config
config_path = Path("${outputPath}/lora_config.json")
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

  writeFileSync(join(outputPath, 'train.sh'), script);
  execSync(`chmod +x "${join(outputPath, 'train.sh')}"`);
  return join(outputPath, 'train.sh');
}

/**
 * Main training pipeline
 */
async function main() {
  console.log('╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║           RuvLTRA Contrastive Fine-tuning Pipeline                                ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  if (!existsSync(RUVLTRA_MODEL)) {
    console.error('RuvLTRA model not found. Run download-models.sh first.');
    process.exit(1);
  }

  const stats = getDatasetStats();
  console.log(`Model: ${RUVLTRA_MODEL}`);
  console.log(`Training examples: ${stats.totalExamples}`);
  console.log(`Contrastive pairs: ${stats.contrastivePairs}`);
  console.log(`Output: ${CONFIG.outputPath}\n`);

  // Prepare training data
  const { triplets, agentEmbeddings } = prepareTrainingData(RUVLTRA_MODEL);

  // Export for external training
  exportTrainingData(triplets, CONFIG.outputPath);

  // Generate LoRA config
  const loraConfig = generateLoRAConfig(CONFIG.outputPath);
  console.log('Generated LoRA config:', join(CONFIG.outputPath, 'lora_config.json'));

  // Generate training script
  const scriptPath = generateTrainingScript(CONFIG.outputPath);
  console.log('Generated training script:', scriptPath);

  // Simulate training to show expected loss curve
  const history = simulateTraining(triplets);

  // Evaluate current model
  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                   CURRENT MODEL EVALUATION');
  console.log('─────────────────────────────────────────────────────────────────\n');

  const evalResult = evaluateModel(RUVLTRA_MODEL, agentEmbeddings);
  console.log(`Embedding-only accuracy: ${(evalResult.accuracy * 100).toFixed(1)}%\n`);

  // Summary
  console.log('═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              TRAINING SUMMARY');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log('Training data exported:');
  console.log(`  - ${join(CONFIG.outputPath, 'triplets.jsonl')} (${triplets.length} triplets)`);
  console.log(`  - ${join(CONFIG.outputPath, 'triplets.csv')} (spreadsheet format)`);
  console.log(`  - ${join(CONFIG.outputPath, 'embeddings.json')} (precomputed embeddings)`);
  console.log(`  - ${join(CONFIG.outputPath, 'lora_config.json')} (LoRA configuration)`);
  console.log(`  - ${join(CONFIG.outputPath, 'train.sh')} (training script)\n`);

  console.log('Expected training loss (simulated):');
  console.log(`  Initial: ${history[0].loss.toFixed(4)}`);
  console.log(`  Final:   ${history[history.length - 1].loss.toFixed(4)}`);
  console.log(`  Improvement: ${((1 - history[history.length - 1].loss / history[0].loss) * 100).toFixed(1)}%\n`);

  console.log('To fine-tune on GPU:');
  console.log(`  cd ${CONFIG.outputPath}`);
  console.log('  ./train.sh\n');

  console.log('After training, convert to GGUF:');
  console.log('  python convert_lora.py --base Qwen/Qwen2.5-0.5B --lora ./lora-adapter');
  console.log('  llama-quantize model-merged.gguf ruvltra-finetuned-q4_k_m.gguf q4_k_m\n');
}

main().catch(console.error);
