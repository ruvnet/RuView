"use strict";
/**
 * Memory Command - Vector memory management
 *
 * Note: Full memory operations require initialized MemoryManager with
 * vector index and embedder. This CLI provides basic operations and
 * demonstrates the memory system capabilities.
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.createMemoryCommand = createMemoryCommand;
const commander_1 = require("commander");
const chalk_1 = __importDefault(require("chalk"));
function createMemoryCommand() {
    const memory = new commander_1.Command('memory');
    memory.description('Memory management commands');
    // Stats command (doesn't require initialization)
    memory
        .command('stats')
        .description('Show memory configuration')
        .option('--json', 'Output as JSON')
        .action(async (options) => {
        try {
            // Get stats from environment/config
            const stats = {
                configured: true,
                dimensions: parseInt(process.env.RUVBOT_EMBEDDING_DIM || '384', 10),
                maxVectors: parseInt(process.env.RUVBOT_MAX_VECTORS || '100000', 10),
                indexType: 'HNSW',
                hnswM: parseInt(process.env.RUVBOT_HNSW_M || '16', 10),
                efConstruction: parseInt(process.env.RUVBOT_HNSW_EF_CONSTRUCTION || '200', 10),
                memoryPath: process.env.RUVBOT_MEMORY_PATH || './data/memory',
            };
            if (options.json) {
                console.log(JSON.stringify(stats, null, 2));
                return;
            }
            console.log(chalk_1.default.bold('\n📊 Memory Configuration\n'));
            console.log('─'.repeat(40));
            console.log(`Dimensions:      ${chalk_1.default.cyan(stats.dimensions)}`);
            console.log(`Max Vectors:     ${chalk_1.default.cyan(stats.maxVectors.toLocaleString())}`);
            console.log(`Index Type:      ${chalk_1.default.cyan(stats.indexType)}`);
            console.log(`HNSW M:          ${chalk_1.default.cyan(stats.hnswM)}`);
            console.log(`EF Construction: ${chalk_1.default.cyan(stats.efConstruction)}`);
            console.log(`Memory Path:     ${chalk_1.default.cyan(stats.memoryPath)}`);
            console.log('─'.repeat(40));
            console.log(chalk_1.default.gray('\nNote: Start RuvBot server for full memory operations'));
        }
        catch (error) {
            console.error(chalk_1.default.red(`Stats failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Store command
    memory
        .command('store')
        .description('Store content in memory (requires running server)')
        .requiredOption('-c, --content <content>', 'Content to store')
        .option('-t, --tags <tags>', 'Comma-separated tags')
        .option('-i, --importance <importance>', 'Importance score (0-1)', '0.5')
        .action(async (options) => {
        console.log(chalk_1.default.yellow('\n⚠ Memory store requires a running RuvBot server'));
        console.log(chalk_1.default.gray('\nTo store memory programmatically:'));
        console.log(chalk_1.default.cyan(`
  import { RuvBot } from '@ruvector/ruvbot';

  const bot = new RuvBot(config);
  await bot.start();

  const entry = await bot.memory.store('${options.content}', {
    tags: [${(options.tags || '').split(',').map((t) => `'${t.trim()}'`).join(', ')}],
    importance: ${options.importance}
  });
`));
        console.log(chalk_1.default.gray('Or use the REST API:'));
        console.log(chalk_1.default.cyan(`
  curl -X POST http://localhost:3000/api/memory \\
    -H "Content-Type: application/json" \\
    -d '{"content": "${options.content}", "tags": [${(options.tags || '').split(',').map((t) => `"${t.trim()}"`).join(', ')}]}'
`));
    });
    // Search command
    memory
        .command('search')
        .description('Search memory (requires running server)')
        .requiredOption('-q, --query <query>', 'Search query')
        .option('-l, --limit <limit>', 'Maximum results', '10')
        .option('--threshold <threshold>', 'Similarity threshold (0-1)', '0.5')
        .action(async (options) => {
        console.log(chalk_1.default.yellow('\n⚠ Memory search requires a running RuvBot server'));
        console.log(chalk_1.default.gray('\nTo search memory programmatically:'));
        console.log(chalk_1.default.cyan(`
  const results = await bot.memory.search('${options.query}', {
    topK: ${options.limit},
    threshold: ${options.threshold}
  });
`));
        console.log(chalk_1.default.gray('Or use the REST API:'));
        console.log(chalk_1.default.cyan(`
  curl "http://localhost:3000/api/memory/search?q=${encodeURIComponent(options.query)}&limit=${options.limit}"
`));
    });
    // Export command
    memory
        .command('export')
        .description('Export memory to file (requires running server)')
        .requiredOption('-o, --output <path>', 'Output file path')
        .option('--format <format>', 'Format: json, jsonl', 'json')
        .action(async (options) => {
        console.log(chalk_1.default.yellow('\n⚠ Memory export requires a running RuvBot server'));
        console.log(chalk_1.default.gray('\nTo export memory:'));
        console.log(chalk_1.default.cyan(`
  const data = await bot.memory.export();
  await fs.writeFile('${options.output}', JSON.stringify(data, null, 2));
`));
    });
    // Import command
    memory
        .command('import')
        .description('Import memory from file (requires running server)')
        .requiredOption('-i, --input <path>', 'Input file path')
        .action(async (options) => {
        console.log(chalk_1.default.yellow('\n⚠ Memory import requires a running RuvBot server'));
        console.log(chalk_1.default.gray('\nTo import memory:'));
        console.log(chalk_1.default.cyan(`
  const data = JSON.parse(await fs.readFile('${options.input}', 'utf-8'));
  const count = await bot.memory.import(data);
  console.log('Imported', count, 'entries');
`));
    });
    // Clear command
    memory
        .command('clear')
        .description('Clear all memory (DANGEROUS - requires running server)')
        .option('-y, --yes', 'Skip confirmation')
        .action(async (options) => {
        if (!options.yes) {
            console.log(chalk_1.default.red('\n⚠ DANGER: This will clear ALL memory entries!'));
            console.log(chalk_1.default.yellow('Use --yes flag to confirm'));
            return;
        }
        console.log(chalk_1.default.yellow('\n⚠ Memory clear requires a running RuvBot server'));
        console.log(chalk_1.default.gray('\nTo clear memory:'));
        console.log(chalk_1.default.cyan(`
  await bot.memory.clear();
`));
    });
    // Info command
    memory
        .command('info')
        .description('Show memory system information')
        .action(async () => {
        console.log(chalk_1.default.bold('\n🧠 RuvBot Memory System\n'));
        console.log('─'.repeat(50));
        console.log(chalk_1.default.cyan('Features:'));
        console.log('  • HNSW vector indexing (150x-12,500x faster search)');
        console.log('  • Semantic similarity search');
        console.log('  • Multi-source memory (conversation, learning, skill, user)');
        console.log('  • Importance-based eviction');
        console.log('  • TTL support for temporary memories');
        console.log('  • Tag-based filtering');
        console.log('');
        console.log(chalk_1.default.cyan('Supported Embeddings:'));
        console.log('  • MiniLM-L6-v2 (384 dimensions, default)');
        console.log('  • Custom embedders via WASM');
        console.log('');
        console.log(chalk_1.default.cyan('Configuration (via .env):'));
        console.log('  RUVBOT_EMBEDDING_DIM=384');
        console.log('  RUVBOT_MAX_VECTORS=100000');
        console.log('  RUVBOT_HNSW_M=16');
        console.log('  RUVBOT_HNSW_EF_CONSTRUCTION=200');
        console.log('  RUVBOT_MEMORY_PATH=./data/memory');
        console.log('─'.repeat(50));
    });
    return memory;
}
exports.default = createMemoryCommand;
//# sourceMappingURL=memory.js.map