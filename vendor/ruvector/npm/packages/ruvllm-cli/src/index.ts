/**
 * @ruvector/ruvllm-cli - CLI for LLM Inference and Benchmarking
 *
 * A command-line interface for running local LLM inference with
 * Metal/CUDA acceleration, model benchmarking, and serving.
 *
 * @example
 * ```bash
 * # Run inference
 * npx @ruvector/ruvllm-cli run --model ./model.gguf --prompt "Hello"
 *
 * # Benchmark a model
 * npx @ruvector/ruvllm-cli bench --model ./model.gguf --iterations 10
 *
 * # Start server
 * npx @ruvector/ruvllm-cli serve --model ./model.gguf --port 8080
 * ```
 *
 * @packageDocumentation
 */

export {
  ModelFormat,
  AccelerationBackend,
  QuantizationType,
  ModelConfig,
  GenerationParams,
  InferenceResult,
  BenchmarkResult,
  CLIConfig,
  ChatMessage,
  ChatCompletionOptions,
} from './types.js';

/** CLI version */
export const VERSION = '0.1.0';

/** Default CLI configuration */
export const DEFAULT_CONFIG: import('./types.js').CLIConfig = {
  defaultBackend: 'cpu' as import('./types.js').AccelerationBackend,
  modelsDir: '~/.ruvllm/models',
  cacheDir: '~/.ruvllm/cache',
  logLevel: 'info',
  streaming: true,
};

/**
 * Parse CLI arguments
 */
export function parseArgs(args: string[]): Record<string, string | boolean> {
  const result: Record<string, string | boolean> = {};

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      const next = args[i + 1];
      if (next && !next.startsWith('--')) {
        result[key] = next;
        i++;
      } else {
        result[key] = true;
      }
    } else if (arg.startsWith('-')) {
      const key = arg.slice(1);
      result[key] = true;
    }
  }

  return result;
}

/**
 * Format benchmark results as table
 */
export function formatBenchmarkTable(results: import('./types.js').BenchmarkResult[]): string {
  const headers = ['Model', 'Backend', 'Prompt TPS', 'Gen TPS', 'Memory (MB)'];
  const rows = results.map(r => [
    r.model,
    r.backend,
    r.promptTPS.toFixed(2),
    r.generationTPS.toFixed(2),
    r.memoryUsage.toFixed(0),
  ]);

  const widths = headers.map((h, i) =>
    Math.max(h.length, ...rows.map(r => String(r[i]).length))
  );

  const separator = widths.map(w => '-'.repeat(w)).join(' | ');
  const headerRow = headers.map((h, i) => h.padEnd(widths[i])).join(' | ');
  const dataRows = rows.map(row =>
    row.map((cell, i) => String(cell).padEnd(widths[i])).join(' | ')
  );

  return [headerRow, separator, ...dataRows].join('\n');
}

/**
 * Get available backends for current system
 */
export function getAvailableBackends(): import('./types.js').AccelerationBackend[] {
  const backends: import('./types.js').AccelerationBackend[] = ['cpu' as import('./types.js').AccelerationBackend];

  // Platform detection would go here
  // For now, return CPU as always available

  return backends;
}
