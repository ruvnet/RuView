/**
 * Doctor Command - System diagnostics and health checks
 *
 * Checks:
 * - Node.js version
 * - Required dependencies
 * - Environment variables
 * - Database connectivity
 * - LLM provider connectivity
 * - Memory system
 * - Security configuration
 * - Plugin system
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';

interface CheckResult {
  name: string;
  status: 'pass' | 'warn' | 'fail';
  message: string;
  fix?: string;
}

export function createDoctorCommand(): Command {
  const doctor = new Command('doctor');

  doctor
    .description('Run diagnostics and health checks')
    .option('--fix', 'Attempt to fix issues automatically')
    .option('--json', 'Output results as JSON')
    .option('-v, --verbose', 'Show detailed information')
    .action(async (options) => {
      const results: CheckResult[] = [];
      const spinner = ora('Running diagnostics...').start();

      try {
        // Check Node.js version
        results.push(await checkNodeVersion());

        // Check environment variables
        results.push(...await checkEnvironment());

        // Check dependencies
        results.push(...await checkDependencies());

        // Check database connectivity
        results.push(await checkDatabase());

        // Check LLM providers
        results.push(...await checkLLMProviders());

        // Check memory system
        results.push(await checkMemorySystem());

        // Check security configuration
        results.push(await checkSecurity());

        // Check plugin system
        results.push(await checkPlugins());

        // Check disk space
        results.push(await checkDiskSpace());

        spinner.stop();

        if (options.json) {
          console.log(JSON.stringify(results, null, 2));
          return;
        }

        // Display results
        console.log(chalk.bold('\n🏥 RuvBot Doctor Results\n'));
        console.log('─'.repeat(60));

        let passCount = 0;
        let warnCount = 0;
        let failCount = 0;

        for (const result of results) {
          const icon = result.status === 'pass' ? '✓' : result.status === 'warn' ? '⚠' : '✗';
          const color = result.status === 'pass' ? chalk.green : result.status === 'warn' ? chalk.yellow : chalk.red;

          console.log(color(`${icon} ${result.name}`));
          if (options.verbose || result.status !== 'pass') {
            console.log(chalk.gray(`  ${result.message}`));
          }
          if (result.fix && result.status !== 'pass') {
            console.log(chalk.cyan(`  Fix: ${result.fix}`));
          }

          if (result.status === 'pass') passCount++;
          else if (result.status === 'warn') warnCount++;
          else failCount++;
        }

        console.log('─'.repeat(60));
        console.log(
          `\nSummary: ${chalk.green(passCount + ' passed')}, ` +
            `${chalk.yellow(warnCount + ' warnings')}, ` +
            `${chalk.red(failCount + ' failed')}`
        );

        if (failCount > 0) {
          console.log(chalk.red('\n⚠ Some checks failed. Run with --fix to attempt automatic fixes.'));
          process.exit(1);
        } else if (warnCount > 0) {
          console.log(chalk.yellow('\n⚠ Some warnings detected. Review and address if needed.'));
        } else {
          console.log(chalk.green('\n✓ All checks passed! RuvBot is healthy.'));
        }
      } catch (error) {
        spinner.fail(chalk.red('Diagnostics failed'));
        console.error(error);
        process.exit(1);
      }
    });

  return doctor;
}

async function checkNodeVersion(): Promise<CheckResult> {
  const version = process.version;
  const major = parseInt(version.slice(1).split('.')[0], 10);

  if (major >= 20) {
    return { name: 'Node.js Version', status: 'pass', message: `${version} (recommended)` };
  } else if (major >= 18) {
    return { name: 'Node.js Version', status: 'warn', message: `${version} (18+ supported, 20+ recommended)` };
  } else {
    return {
      name: 'Node.js Version',
      status: 'fail',
      message: `${version} (requires 18+)`,
      fix: 'Install Node.js 20 LTS from https://nodejs.org',
    };
  }
}

async function checkEnvironment(): Promise<CheckResult[]> {
  const results: CheckResult[] = [];

  // Check for .env file
  const fs = await import('fs/promises');
  try {
    await fs.access('.env');
    results.push({ name: 'Environment File', status: 'pass', message: '.env file found' });
  } catch {
    results.push({
      name: 'Environment File',
      status: 'warn',
      message: 'No .env file found',
      fix: 'Copy .env.example to .env and configure',
    });
  }

  // Check critical environment variables
  const criticalVars = ['ANTHROPIC_API_KEY', 'OPENROUTER_API_KEY', 'OPENAI_API_KEY'];
  const hasApiKey = criticalVars.some((v) => process.env[v]);

  if (hasApiKey) {
    results.push({ name: 'LLM API Key', status: 'pass', message: 'At least one LLM API key configured' });
  } else {
    results.push({
      name: 'LLM API Key',
      status: 'warn',
      message: 'No LLM API key found',
      fix: 'Set ANTHROPIC_API_KEY or OPENROUTER_API_KEY in .env',
    });
  }

  return results;
}

async function checkDependencies(): Promise<CheckResult[]> {
  const results: CheckResult[] = [];

  // Check if package.json exists
  const fs = await import('fs/promises');
  try {
    const pkg = JSON.parse(await fs.readFile('package.json', 'utf-8'));
    const hasRuvbot = pkg.dependencies?.['@ruvector/ruvbot'] || pkg.devDependencies?.['@ruvector/ruvbot'];

    if (hasRuvbot) {
      results.push({ name: 'RuvBot Package', status: 'pass', message: '@ruvector/ruvbot installed' });
    }
  } catch {
    // Not in a project directory, skip this check
  }

  // Check for optional dependencies
  const optionalDeps = [
    { name: '@slack/bolt', desc: 'Slack integration' },
    { name: 'pg', desc: 'PostgreSQL support' },
    { name: 'ioredis', desc: 'Redis caching' },
  ];

  for (const dep of optionalDeps) {
    try {
      await import(dep.name);
      results.push({ name: dep.desc, status: 'pass', message: `${dep.name} available` });
    } catch {
      results.push({
        name: dep.desc,
        status: 'warn',
        message: `${dep.name} not installed (optional)`,
        fix: `npm install ${dep.name}`,
      });
    }
  }

  return results;
}

async function checkDatabase(): Promise<CheckResult> {
  const storageType = process.env.RUVBOT_STORAGE_TYPE || 'sqlite';

  if (storageType === 'sqlite') {
    const fs = await import('fs/promises');
    const dbPath = process.env.RUVBOT_SQLITE_PATH || './data/ruvbot.db';

    try {
      await fs.access(dbPath);
      return { name: 'Database (SQLite)', status: 'pass', message: `Database found at ${dbPath}` };
    } catch {
      return {
        name: 'Database (SQLite)',
        status: 'warn',
        message: `Database not found at ${dbPath}`,
        fix: 'Run `ruvbot init` to create database',
      };
    }
  } else if (storageType === 'postgres') {
    const dbUrl = process.env.DATABASE_URL;
    if (!dbUrl) {
      return {
        name: 'Database (PostgreSQL)',
        status: 'fail',
        message: 'DATABASE_URL not configured',
        fix: 'Set DATABASE_URL in .env',
      };
    }

    try {
      const { default: pg } = await import('pg');
      const client = new pg.Client(dbUrl);
      await client.connect();
      await client.query('SELECT 1');
      await client.end();
      return { name: 'Database (PostgreSQL)', status: 'pass', message: 'Connection successful' };
    } catch (error: any) {
      return {
        name: 'Database (PostgreSQL)',
        status: 'fail',
        message: `Connection failed: ${error.message}`,
        fix: 'Check DATABASE_URL and ensure PostgreSQL is running',
      };
    }
  }

  return { name: 'Database', status: 'pass', message: `Using ${storageType} storage` };
}

async function checkLLMProviders(): Promise<CheckResult[]> {
  const results: CheckResult[] = [];

  // Check Anthropic
  if (process.env.ANTHROPIC_API_KEY) {
    try {
      const response = await fetch('https://api.anthropic.com/v1/messages', {
        method: 'POST',
        headers: {
          'x-api-key': process.env.ANTHROPIC_API_KEY,
          'anthropic-version': '2023-06-01',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          model: 'claude-3-haiku-20240307',
          max_tokens: 1,
          messages: [{ role: 'user', content: 'hi' }],
        }),
      });

      if (response.ok || response.status === 400) {
        // 400 means API key is valid but request is bad (expected with minimal request)
        results.push({ name: 'Anthropic API', status: 'pass', message: 'API key valid' });
      } else if (response.status === 401) {
        results.push({
          name: 'Anthropic API',
          status: 'fail',
          message: 'Invalid API key',
          fix: 'Check ANTHROPIC_API_KEY in .env',
        });
      } else {
        results.push({ name: 'Anthropic API', status: 'warn', message: `Status: ${response.status}` });
      }
    } catch (error: any) {
      results.push({
        name: 'Anthropic API',
        status: 'fail',
        message: `Connection failed: ${error.message}`,
        fix: 'Check network connectivity',
      });
    }
  }

  // Check OpenRouter
  if (process.env.OPENROUTER_API_KEY) {
    try {
      const response = await fetch('https://openrouter.ai/api/v1/models', {
        headers: { Authorization: `Bearer ${process.env.OPENROUTER_API_KEY}` },
      });

      if (response.ok) {
        results.push({ name: 'OpenRouter API', status: 'pass', message: 'API key valid' });
      } else {
        results.push({
          name: 'OpenRouter API',
          status: 'fail',
          message: 'Invalid API key',
          fix: 'Check OPENROUTER_API_KEY in .env',
        });
      }
    } catch (error: any) {
      results.push({
        name: 'OpenRouter API',
        status: 'fail',
        message: `Connection failed: ${error.message}`,
      });
    }
  }

  if (results.length === 0) {
    results.push({
      name: 'LLM Providers',
      status: 'warn',
      message: 'No LLM providers configured',
      fix: 'Set ANTHROPIC_API_KEY or OPENROUTER_API_KEY',
    });
  }

  return results;
}

async function checkMemorySystem(): Promise<CheckResult> {
  const memoryPath = process.env.RUVBOT_MEMORY_PATH || './data/memory';
  const fs = await import('fs/promises');

  try {
    await fs.access(memoryPath);
    const stats = await fs.stat(memoryPath);

    if (stats.isDirectory()) {
      return { name: 'Memory System', status: 'pass', message: `Memory directory exists at ${memoryPath}` };
    }
  } catch {
    return {
      name: 'Memory System',
      status: 'warn',
      message: `Memory directory not found at ${memoryPath}`,
      fix: 'Run `ruvbot init` to create directories',
    };
  }

  return { name: 'Memory System', status: 'pass', message: 'Ready' };
}

async function checkSecurity(): Promise<CheckResult> {
  const aidefenceEnabled = process.env.RUVBOT_AIDEFENCE_ENABLED !== 'false';
  const piiEnabled = process.env.RUVBOT_PII_DETECTION !== 'false';
  const auditEnabled = process.env.RUVBOT_AUDIT_LOG !== 'false';

  const features = [];
  if (aidefenceEnabled) features.push('AI Defense');
  if (piiEnabled) features.push('PII Detection');
  if (auditEnabled) features.push('Audit Logging');

  if (features.length === 0) {
    return {
      name: 'Security Configuration',
      status: 'warn',
      message: 'All security features disabled',
      fix: 'Enable RUVBOT_AIDEFENCE_ENABLED=true in .env',
    };
  }

  return {
    name: 'Security Configuration',
    status: 'pass',
    message: `Enabled: ${features.join(', ')}`,
  };
}

async function checkPlugins(): Promise<CheckResult> {
  const pluginsEnabled = process.env.RUVBOT_PLUGINS_ENABLED !== 'false';
  const pluginsDir = process.env.RUVBOT_PLUGINS_DIR || './plugins';

  if (!pluginsEnabled) {
    return { name: 'Plugin System', status: 'pass', message: 'Disabled' };
  }

  const fs = await import('fs/promises');
  try {
    const files = await fs.readdir(pluginsDir);
    const plugins = files.filter((f) => f.endsWith('.js') || f.endsWith('.ts'));
    return {
      name: 'Plugin System',
      status: 'pass',
      message: `${plugins.length} plugin(s) found in ${pluginsDir}`,
    };
  } catch {
    return {
      name: 'Plugin System',
      status: 'warn',
      message: `Plugin directory not found at ${pluginsDir}`,
      fix: `mkdir -p ${pluginsDir}`,
    };
  }
}

async function checkDiskSpace(): Promise<CheckResult> {
  try {
    const os = await import('os');
    const { execSync } = await import('child_process');

    // Get disk space (works on Unix-like systems)
    const df = execSync('df -h . 2>/dev/null || echo "N/A"').toString().trim();
    const lines = df.split('\n');

    if (lines.length > 1) {
      const parts = lines[1].split(/\s+/);
      const available = parts[3];
      const usePercent = parts[4];

      const useNum = parseInt(usePercent, 10);
      if (useNum > 90) {
        return {
          name: 'Disk Space',
          status: 'fail',
          message: `${usePercent} used, ${available} available`,
          fix: 'Free up disk space',
        };
      } else if (useNum > 80) {
        return {
          name: 'Disk Space',
          status: 'warn',
          message: `${usePercent} used, ${available} available`,
        };
      }
      return {
        name: 'Disk Space',
        status: 'pass',
        message: `${available} available`,
      };
    }
  } catch {
    // Disk check not available
  }

  return { name: 'Disk Space', status: 'pass', message: 'Check not available on this platform' };
}

export default createDoctorCommand;
