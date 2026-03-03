#!/usr/bin/env node

/**
 * Release Script for Psycho-Symbolic Reasoner
 *
 * This script automates the release process:
 * 1. Version validation and bumping
 * 2. Changelog generation
 * 3. Build validation
 * 4. Testing
 * 5. Git tagging
 * 6. NPM publishing
 * 7. GitHub release creation
 */

import { spawn } from 'child_process';
import { promises as fs } from 'fs';
import { join, resolve } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, '..');
const rootDir = resolve(__dirname, '..');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
};

function log(color, prefix, message) {
  console.log(`${color}${prefix}${colors.reset} ${message}`);
}

function info(message) {
  log(colors.blue, '[INFO]', message);
}

function success(message) {
  log(colors.green, '[SUCCESS]', message);
}

function warn(message) {
  log(colors.yellow, '[WARN]', message);
}

function error(message) {
  log(colors.red, '[ERROR]', message);
}

async function runCommand(command, args = [], options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: options.silent ? 'pipe' : 'inherit',
      cwd: options.cwd || rootDir,
      ...options
    });

    let stdout = '';
    let stderr = '';

    if (options.silent) {
      child.stdout?.on('data', (data) => {
        stdout += data.toString();
      });

      child.stderr?.on('data', (data) => {
        stderr += data.toString();
      });
    }

    child.on('close', (code) => {
      if (code === 0) {
        resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() });
      } else {
        reject(new Error(`Command failed: ${command} ${args.join(' ')}\\n${stderr}`));
      }
    });

    child.on('error', reject);
  });
}

async function getCurrentVersion() {
  const packageJson = JSON.parse(
    await fs.readFile(resolve(rootDir, 'package.json'), 'utf8')
  );
  return packageJson.version;
}

async function validateWorkingDirectory() {
  info('Validating working directory...');

  try {
    const { stdout } = await runCommand('git', ['status', '--porcelain'], { silent: true });
    if (stdout.trim()) {
      error('Working directory is not clean. Please commit or stash your changes.');
      console.log('Uncommitted changes:');
      console.log(stdout);
      process.exit(1);
    }
  } catch (error) {
    error(`Failed to check git status: ${error.message}`);
    process.exit(1);
  }

  // Check if we're on main or develop branch
  try {
    const { stdout } = await runCommand('git', ['branch', '--show-current'], { silent: true });
    const currentBranch = stdout.trim();

    if (!['main', 'develop'].includes(currentBranch)) {
      warn(`You are on branch '${currentBranch}'. Releases should typically be made from 'main' or 'develop'.`);

      const readline = await import('readline');
      const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
      });

      const answer = await new Promise(resolve => {
        rl.question('Continue anyway? (y/N): ', resolve);
      });
      rl.close();

      if (answer.toLowerCase() !== 'y') {
        info('Release cancelled.');
        process.exit(0);
      }
    }
  } catch (error) {
    warn(`Could not determine current branch: ${error.message}`);
  }

  success('Working directory validated');
}

async function runTests() {
  info('Running tests...');

  try {
    await runCommand('npm', ['test']);
    success('All tests passed');
  } catch (error) {
    error(`Tests failed: ${error.message}`);
    process.exit(1);
  }
}

async function runLinting() {
  info('Running linter...');

  try {
    await runCommand('npm', ['run', 'lint']);
    success('Linting passed');
  } catch (error) {
    error(`Linting failed: ${error.message}`);
    process.exit(1);
  }
}

async function buildProject() {
  info('Building project...');

  try {
    await runCommand('npm', ['run', 'build']);
    success('Build completed');
  } catch (error) {
    error(`Build failed: ${error.message}`);
    process.exit(1);
  }
}

async function bumpVersion(type) {
  info(`Bumping version: ${type}`);

  try {
    const { stdout } = await runCommand('npm', ['version', type, '--no-git-tag-version'], { silent: true });
    const newVersion = stdout.trim();
    success(`Version bumped to ${newVersion}`);
    return newVersion.replace('v', '');
  } catch (error) {
    error(`Failed to bump version: ${error.message}`);
    process.exit(1);
  }
}

async function updateChangelog(version) {
  info('Updating CHANGELOG.md...');

  const changelogPath = resolve(rootDir, 'CHANGELOG.md');
  let changelog;

  try {
    changelog = await fs.readFile(changelogPath, 'utf8');
  } catch (error) {
    warn('CHANGELOG.md not found, creating new one');
    changelog = '# Changelog\\n\\nAll notable changes to this project will be documented in this file.\\n\\n';
  }

  const today = new Date().toISOString().split('T')[0];
  const unreleased = '## [Unreleased]';
  const newVersionSection = `## [${version}] - ${today}\\n\\n### Added\\n- Release ${version}\\n\\n`;

  if (changelog.includes(unreleased)) {
    changelog = changelog.replace(unreleased, `${unreleased}\\n\\n${newVersionSection}`);
  } else {
    const lines = changelog.split('\\n');
    lines.splice(3, 0, '', newVersionSection);
    changelog = lines.join('\\n');
  }

  await fs.writeFile(changelogPath, changelog);
  success('CHANGELOG.md updated');
}

async function commitAndTag(version) {
  info(`Creating git commit and tag for version ${version}...`);

  try {
    // Add all changes
    await runCommand('git', ['add', '.']);

    // Commit
    await runCommand('git', ['commit', '-m', `chore(release): ${version}`]);

    // Create tag
    await runCommand('git', ['tag', `v${version}`]);

    success(`Committed and tagged version ${version}`);
  } catch (error) {
    error(`Failed to commit and tag: ${error.message}`);
    process.exit(1);
  }
}

async function publishToNpm(tag = 'latest') {
  info(`Publishing to NPM with tag: ${tag}`);

  try {
    // Check if user is logged in
    try {
      await runCommand('npm', ['whoami'], { silent: true });
    } catch {
      error('You are not logged in to NPM. Please run: npm login');
      process.exit(1);
    }

    // Publish
    const args = ['publish'];
    if (tag !== 'latest') {
      args.push('--tag', tag);
    }

    await runCommand('npm', args);
    success(`Published to NPM with tag: ${tag}`);
  } catch (error) {
    error(`Failed to publish to NPM: ${error.message}`);
    process.exit(1);
  }
}

async function createGitHubRelease(version) {
  info(`Creating GitHub release for version ${version}...`);

  try {
    // Check if gh CLI is available
    await runCommand('gh', ['--version'], { silent: true });

    // Create release
    await runCommand('gh', [
      'release', 'create',
      `v${version}`,
      '--title', `Release ${version}`,
      '--notes', `Release notes for version ${version}. See CHANGELOG.md for details.`,
      '--verify-tag'
    ]);

    success(`GitHub release created for version ${version}`);
  } catch (error) {
    warn(`Could not create GitHub release: ${error.message}`);
    warn('You may need to install GitHub CLI (gh) or create the release manually');
  }
}

async function pushToRemote() {
  info('Pushing to remote repository...');

  try {
    await runCommand('git', ['push']);
    await runCommand('git', ['push', '--tags']);
    success('Pushed to remote repository');
  } catch (error) {
    error(`Failed to push to remote: ${error.message}`);
    process.exit(1);
  }
}

async function main() {
  const startTime = Date.now();

  // Parse command line arguments
  const args = process.argv.slice(2);
  const versionType = args[0] || 'patch';
  const npmTag = args.includes('--beta') ? 'beta' : 'latest';
  const skipTests = args.includes('--skip-tests');
  const dryRun = args.includes('--dry-run');

  if (!['patch', 'minor', 'major', 'prerelease'].includes(versionType)) {
    error('Invalid version type. Use: patch, minor, major, or prerelease');
    process.exit(1);
  }

  try {
    log(colors.cyan, '[RELEASE]', `Starting release process for ${versionType} version...`);

    if (dryRun) {
      warn('DRY RUN MODE - No changes will be made');
    }

    const currentVersion = await getCurrentVersion();
    info(`Current version: ${currentVersion}`);

    await validateWorkingDirectory();

    if (!skipTests) {
      await runLinting();
      await runTests();
    }

    await buildProject();

    if (!dryRun) {
      const newVersion = await bumpVersion(versionType);
      await updateChangelog(newVersion);
      await commitAndTag(newVersion);
      await pushToRemote();
      await publishToNpm(npmTag);
      await createGitHubRelease(newVersion);

      const duration = ((Date.now() - startTime) / 1000).toFixed(2);
      success(`Release ${newVersion} completed successfully in ${duration}s`);

      console.log('\\n📦 Release Summary:');
      console.log(`   Version: ${newVersion}`);
      console.log(`   NPM Tag: ${npmTag}`);
      console.log(`   Duration: ${duration}s`);
      console.log('\\n🚀 Your package is now available:');
      console.log(`   npm install psycho-symbolic-reasoner@${newVersion}`);
      console.log(`   npx psycho-symbolic-reasoner@${newVersion}`);

    } else {
      success('Dry run completed - no changes made');
    }

  } catch (error) {
    error(`Release failed: ${error.message}`);
    process.exit(1);
  }
}

// Handle command line arguments
const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  console.log(`
Usage: node scripts/release.js [version-type] [options]

Version Types:
  patch       Increment patch version (1.0.0 -> 1.0.1)
  minor       Increment minor version (1.0.0 -> 1.1.0)
  major       Increment major version (1.0.0 -> 2.0.0)
  prerelease  Increment prerelease version (1.0.0 -> 1.0.1-0)

Options:
  --help, -h       Show this help message
  --beta           Publish with 'beta' tag instead of 'latest'
  --skip-tests     Skip running tests
  --dry-run        Simulate the release without making changes

Examples:
  node scripts/release.js patch              # Patch release
  node scripts/release.js minor --beta       # Minor beta release
  node scripts/release.js major --dry-run    # Simulate major release
`);
  process.exit(0);
}

main().catch(console.error);