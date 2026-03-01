#!/usr/bin/env node

/**
 * RuvLLM CLI Entry Point
 *
 * Usage:
 *   ruvllm run --model <path> --prompt <text>
 *   ruvllm bench --model <path> [--iterations <n>]
 *   ruvllm serve --model <path> [--port <n>]
 *   ruvllm list
 *   ruvllm download <model-id>
 */

import { parseArgs, VERSION, DEFAULT_CONFIG } from '../dist/index.js';

const args = process.argv.slice(2);
const command = args[0];
const options = parseArgs(args.slice(1));

function printHelp() {
  console.log(`
RuvLLM CLI v${VERSION}

Usage:
  ruvllm <command> [options]

Commands:
  run       Run inference on a prompt
  bench     Benchmark model performance
  serve     Start HTTP server for inference
  list      List available models
  download  Download a model from HuggingFace
  chat      Interactive chat session

Options:
  --model, -m     Path to model file (GGUF)
  --prompt, -p    Input prompt text
  --backend, -b   Acceleration backend (metal, cuda, cpu)
  --port          Server port (default: 8080)
  --iterations    Benchmark iterations (default: 10)
  --temperature   Sampling temperature (default: 0.7)
  --max-tokens    Maximum tokens to generate (default: 256)
  --help, -h      Show this help message
  --version, -v   Show version

Examples:
  ruvllm run --model ./model.gguf --prompt "Hello, world"
  ruvllm bench --model ./model.gguf --iterations 20
  ruvllm serve --model ./model.gguf --port 3000
  ruvllm chat --model ./model.gguf
`);
}

function printVersion() {
  console.log(`ruvllm v${VERSION}`);
}

async function main() {
  if (options.help || options.h || !command) {
    printHelp();
    process.exit(0);
  }

  if (options.version || options.v) {
    printVersion();
    process.exit(0);
  }

  switch (command) {
    case 'run':
      console.log('Running inference...');
      console.log('Model:', options.model || 'Not specified');
      console.log('Prompt:', options.prompt || 'Not specified');
      console.log('\nNote: Full inference requires the native ruvllm binary.');
      console.log('Install with: cargo install ruvllm-cli');
      break;

    case 'bench':
      console.log('Running benchmark...');
      console.log('Model:', options.model || 'Not specified');
      console.log('Iterations:', options.iterations || 10);
      console.log('\nNote: Full benchmarking requires the native ruvllm binary.');
      console.log('Install with: cargo install ruvllm-cli');
      break;

    case 'serve':
      console.log('Starting server...');
      console.log('Model:', options.model || 'Not specified');
      console.log('Port:', options.port || 8080);
      console.log('\nNote: Server mode requires the native ruvllm binary.');
      console.log('Install with: cargo install ruvllm-cli');
      break;

    case 'list':
      console.log('Available models in', DEFAULT_CONFIG.modelsDir);
      console.log('\nNote: Model listing requires the native ruvllm binary.');
      break;

    case 'download':
      console.log('Downloading model:', args[1] || 'Not specified');
      console.log('\nNote: Model download requires the native ruvllm binary.');
      break;

    case 'chat':
      console.log('Starting chat session...');
      console.log('Model:', options.model || 'Not specified');
      console.log('\nNote: Chat mode requires the native ruvllm binary.');
      console.log('Install with: cargo install ruvllm-cli');
      break;

    default:
      console.error(`Unknown command: ${command}`);
      printHelp();
      process.exit(1);
  }
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
