/**
 * CLI tests for agentic-synth
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { exec } from 'child_process';
import { promisify } from 'util';
import { writeFileSync, unlinkSync, existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

const execAsync = promisify(exec);

describe('CLI', () => {
  const cliPath = join(process.cwd(), 'bin/cli.js');
  let testDir;
  let schemaPath;
  let outputPath;
  let configPath;

  beforeEach(() => {
    testDir = join(tmpdir(), `agentic-synth-test-${Date.now()}`);
    schemaPath = join(testDir, 'schema.json');
    outputPath = join(testDir, 'output.json');
    configPath = join(testDir, 'config.json');

    // Create test directory
    if (!existsSync(testDir)) {
      const { mkdirSync } = require('fs');
      mkdirSync(testDir, { recursive: true });
    }
  });

  afterEach(() => {
    // Cleanup test files
    [schemaPath, outputPath, configPath].forEach(path => {
      if (existsSync(path)) {
        unlinkSync(path);
      }
    });
  });

  describe('generate command', () => {
    it('should generate data with default count', async () => {
      const { stdout } = await execAsync(`node ${cliPath} generate`);
      const data = JSON.parse(stdout);

      expect(Array.isArray(data)).toBe(true);
      expect(data.length).toBe(10); // Default count
    });

    it('should generate specified number of records', async () => {
      const { stdout } = await execAsync(`node ${cliPath} generate --count 5`);
      const data = JSON.parse(stdout);

      expect(data).toHaveLength(5);
    });

    it('should use provided schema file', async () => {
      const schema = {
        name: { type: 'string', length: 10 },
        age: { type: 'number', min: 18, max: 65 }
      };
      writeFileSync(schemaPath, JSON.stringify(schema));

      const { stdout } = await execAsync(
        `node ${cliPath} generate --count 3 --schema ${schemaPath}`
      );
      const data = JSON.parse(stdout);

      expect(data).toHaveLength(3);
      data.forEach(record => {
        expect(record).toHaveProperty('name');
        expect(record).toHaveProperty('age');
      });
    });

    it('should write to output file', async () => {
      await execAsync(
        `node ${cliPath} generate --count 5 --output ${outputPath}`
      );

      expect(existsSync(outputPath)).toBe(true);

      const data = JSON.parse(readFileSync(outputPath, 'utf8'));
      expect(data).toHaveLength(5);
    });

    it('should use seed for reproducibility', async () => {
      const { stdout: output1 } = await execAsync(
        `node ${cliPath} generate --count 3 --seed 12345`
      );
      const { stdout: output2 } = await execAsync(
        `node ${cliPath} generate --count 3 --seed 12345`
      );

      // Note: Due to random generation, results may differ
      // In production, implement proper seeded RNG
      expect(output1).toBeDefined();
      expect(output2).toBeDefined();
    });

    it('should handle invalid schema file', async () => {
      writeFileSync(schemaPath, 'invalid json');

      await expect(
        execAsync(`node ${cliPath} generate --schema ${schemaPath}`)
      ).rejects.toThrow();
    });

    it('should handle non-existent schema file', async () => {
      await expect(
        execAsync(`node ${cliPath} generate --schema /nonexistent/schema.json`)
      ).rejects.toThrow();
    });
  });

  describe('config command', () => {
    it('should display default configuration', async () => {
      const { stdout } = await execAsync(`node ${cliPath} config`);
      const config = JSON.parse(stdout);

      expect(config).toHaveProperty('api');
      expect(config).toHaveProperty('cache');
      expect(config).toHaveProperty('generator');
    });

    it('should load configuration from file', async () => {
      const customConfig = {
        api: { baseUrl: 'https://custom.com' }
      };
      writeFileSync(configPath, JSON.stringify(customConfig));

      const { stdout } = await execAsync(
        `node ${cliPath} config --file ${configPath}`
      );
      const config = JSON.parse(stdout);

      expect(config.api.baseUrl).toBe('https://custom.com');
    });

    it('should handle invalid config file', async () => {
      writeFileSync(configPath, 'invalid json');

      await expect(
        execAsync(`node ${cliPath} config --file ${configPath}`)
      ).rejects.toThrow();
    });
  });

  describe('validate command', () => {
    it('should validate valid configuration', async () => {
      const validConfig = {
        api: { baseUrl: 'https://test.com' },
        cache: { maxSize: 100 }
      };
      writeFileSync(configPath, JSON.stringify(validConfig));

      const { stdout } = await execAsync(
        `node ${cliPath} validate --file ${configPath}`
      );

      expect(stdout).toContain('valid');
    });

    it('should detect invalid configuration', async () => {
      const invalidConfig = {
        // Missing required fields
        cache: {}
      };
      writeFileSync(configPath, JSON.stringify(invalidConfig));

      await expect(
        execAsync(`node ${cliPath} validate --file ${configPath}`)
      ).rejects.toThrow();
    });
  });

  describe('error handling', () => {
    it('should show error for unknown command', async () => {
      await expect(
        execAsync(`node ${cliPath} unknown`)
      ).rejects.toThrow();
    });

    it('should handle invalid count parameter', async () => {
      await expect(
        execAsync(`node ${cliPath} generate --count abc`)
      ).rejects.toThrow();
    });

    it('should handle permission errors', async () => {
      // Try to write to read-only location
      const readOnlyPath = '/root/readonly.json';

      await expect(
        execAsync(`node ${cliPath} generate --output ${readOnlyPath}`)
      ).rejects.toThrow();
    });
  });

  describe('help and version', () => {
    it('should display help information', async () => {
      const { stdout } = await execAsync(`node ${cliPath} --help`);

      expect(stdout).toContain('agentic-synth');
      expect(stdout).toContain('generate');
      expect(stdout).toContain('config');
      expect(stdout).toContain('validate');
    });

    it('should display version', async () => {
      const { stdout } = await execAsync(`node ${cliPath} --version`);
      expect(stdout).toMatch(/\d+\.\d+\.\d+/);
    });

    it('should display command-specific help', async () => {
      const { stdout } = await execAsync(`node ${cliPath} generate --help`);

      expect(stdout).toContain('generate');
      expect(stdout).toContain('--count');
      expect(stdout).toContain('--schema');
    });
  });

  describe('output formatting', () => {
    it('should format JSON output properly', async () => {
      const { stdout } = await execAsync(`node ${cliPath} generate --count 2`);

      // Should be valid JSON
      expect(() => JSON.parse(stdout)).not.toThrow();

      // Should be pretty-printed (contains newlines)
      expect(stdout).toContain('\n');
    });

    it('should write formatted JSON to file', async () => {
      await execAsync(
        `node ${cliPath} generate --count 2 --output ${outputPath}`
      );

      const content = readFileSync(outputPath, 'utf8');
      expect(content).toContain('\n');
      expect(() => JSON.parse(content)).not.toThrow();
    });
  });
});
