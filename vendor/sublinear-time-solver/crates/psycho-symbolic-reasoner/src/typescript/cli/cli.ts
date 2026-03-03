#!/usr/bin/env node

/**
 * CLI entry point for npx execution
 * This file provides the main executable entry point
 */

// Import and run the main CLI
import('./index.js').then(({ PsychoSymbolicReasonerCLI }) => {
  const cli = new PsychoSymbolicReasonerCLI();
  return cli.run();
}).catch((error) => {
  console.error('Fatal CLI error:', error);
  process.exit(1);
});