#!/usr/bin/env node

/**
 * RuvBot CLI Entry Point
 *
 * Usage:
 *   npx ruvbot <command> [options]
 *   ruvbot <command> [options]
 *
 * Commands:
 *   start     Start the RuvBot server
 *   init      Initialize RuvBot in current directory
 *   doctor    Run diagnostics and health checks
 *   config    Manage configuration
 *   memory    Memory management commands
 *   security  Security scanning and audit
 *   plugins   Plugin management
 *   agent     Agent management
 *   status    Show bot status
 */

require('dotenv/config');

async function run() {
  try {
    // Try CJS build first
    const { main } = require('../dist/cli/index.js');
    await main();
  } catch (cjsError) {
    // Fall back to dynamic import for ESM
    try {
      const { main } = await import('../dist/esm/cli/index.js');
      await main();
    } catch (esmError) {
      console.error('Failed to load RuvBot CLI');
      console.error('CJS Error:', cjsError.message);
      console.error('ESM Error:', esmError.message);
      console.error('\nTry running: npm run build');
      process.exit(1);
    }
  }
}

run().catch((error) => {
  console.error('Fatal error:', error.message);
  process.exit(1);
});
