#!/usr/bin/env node

/**
 * Post-install script for @ruvector/ruvbot
 *
 * Downloads optional native binaries and initializes data directories.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

async function main() {
  console.log('[ruvbot] Running post-install...');

  // Create data directory if it doesn't exist
  const dataDir = path.join(rootDir, 'data');
  if (!fs.existsSync(dataDir)) {
    fs.mkdirSync(dataDir, { recursive: true });
    console.log('[ruvbot] Created data directory');
  }

  // Check for optional dependencies
  const optionalDeps = [
    { name: '@slack/bolt', purpose: 'Slack integration' },
    { name: 'discord.js', purpose: 'Discord integration' },
    { name: 'better-sqlite3', purpose: 'SQLite storage' },
    { name: 'pg', purpose: 'PostgreSQL storage' },
  ];

  console.log('\n[ruvbot] Optional features:');
  for (const dep of optionalDeps) {
    try {
      await import(dep.name);
      console.log(`  [x] ${dep.purpose} (${dep.name})`);
    } catch {
      console.log(`  [ ] ${dep.purpose} - install ${dep.name} to enable`);
    }
  }

  console.log('\n[ruvbot] Installation complete!');
  console.log('[ruvbot] Run `npx @ruvector/ruvbot start` to begin.\n');
}

main().catch((error) => {
  // Post-install failures should not break npm install
  console.warn('[ruvbot] Post-install warning:', error.message);
});
