#!/usr/bin/env node

/**
 * RuvBot CLI entry point
 *
 * Usage:
 *   npx @ruvector/ruvbot init
 *   npx @ruvector/ruvbot start
 *   npx @ruvector/ruvbot config
 *   npx @ruvector/ruvbot skills list
 *   npx @ruvector/ruvbot status
 */

import { main } from '../dist/cli/index.mjs';

main().catch((error) => {
  console.error('Error:', error.message);
  process.exit(1);
});
