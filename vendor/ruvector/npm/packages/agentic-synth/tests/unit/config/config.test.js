/**
 * Unit tests for Config
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { Config } from '../../../src/config/config.js';
import { writeFileSync, unlinkSync, existsSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

describe('Config', () => {
  let testConfigPath;
  let originalEnv;

  beforeEach(() => {
    originalEnv = { ...process.env };
    testConfigPath = join(tmpdir(), `test-config-${Date.now()}.json`);
  });

  afterEach(() => {
    process.env = originalEnv;
    if (existsSync(testConfigPath)) {
      unlinkSync(testConfigPath);
    }
  });

  describe('constructor', () => {
    it('should create config with defaults', () => {
      const config = new Config({ loadEnv: false });
      expect(config.values).toBeDefined();
      expect(config.get('api.baseUrl')).toBeDefined();
    });

    it('should accept custom options', () => {
      const config = new Config({
        loadEnv: false,
        api: { baseUrl: 'https://custom.com' }
      });
      expect(config.get('api.baseUrl')).toBe('https://custom.com');
    });

    it('should load from file if provided', () => {
      writeFileSync(testConfigPath, JSON.stringify({
        custom: { value: 'test' }
      }));

      const config = new Config({
        loadEnv: false,
        configPath: testConfigPath
      });

      expect(config.get('custom.value')).toBe('test');
    });
  });

  describe('get', () => {
    let config;

    beforeEach(() => {
      config = new Config({
        loadEnv: false,
        api: {
          baseUrl: 'https://test.com',
          timeout: 5000
        },
        nested: {
          deep: {
            value: 'found'
          }
        }
      });
    });

    it('should get top-level value', () => {
      expect(config.get('api')).toEqual({
        baseUrl: 'https://test.com',
        timeout: 5000
      });
    });

    it('should get nested value with dot notation', () => {
      expect(config.get('api.baseUrl')).toBe('https://test.com');
      expect(config.get('nested.deep.value')).toBe('found');
    });

    it('should return default for non-existent key', () => {
      expect(config.get('nonexistent', 'default')).toBe('default');
    });

    it('should return undefined for non-existent key without default', () => {
      expect(config.get('nonexistent')).toBeUndefined();
    });

    it('should read from environment variables', () => {
      process.env.AGENTIC_SYNTH_API_KEY = 'env-key-123';
      const config = new Config({ loadEnv: false });

      expect(config.get('api.key')).toBe('env-key-123');
    });

    it('should prioritize environment over config file', () => {
      process.env.AGENTIC_SYNTH_CUSTOM_VALUE = 'from-env';

      const config = new Config({
        loadEnv: false,
        custom: { value: 'from-config' }
      });

      expect(config.get('custom.value')).toBe('from-env');
    });
  });

  describe('set', () => {
    let config;

    beforeEach(() => {
      config = new Config({ loadEnv: false });
    });

    it('should set top-level value', () => {
      config.set('newKey', 'newValue');
      expect(config.get('newKey')).toBe('newValue');
    });

    it('should set nested value with dot notation', () => {
      config.set('nested.deep.value', 'test');
      expect(config.get('nested.deep.value')).toBe('test');
    });

    it('should create nested structure if not exists', () => {
      config.set('a.b.c.d', 'deep');
      expect(config.get('a.b.c.d')).toBe('deep');
    });

    it('should update existing value', () => {
      config.set('api.baseUrl', 'https://new.com');
      expect(config.get('api.baseUrl')).toBe('https://new.com');
    });
  });

  describe('loadFromFile', () => {
    it('should load JSON config', () => {
      const configData = {
        api: { baseUrl: 'https://json.com' }
      };
      writeFileSync(testConfigPath, JSON.stringify(configData));

      const config = new Config({ loadEnv: false });
      config.loadFromFile(testConfigPath);

      expect(config.get('api.baseUrl')).toBe('https://json.com');
    });

    it('should load YAML config', () => {
      const yamlPath = testConfigPath.replace('.json', '.yaml');
      writeFileSync(yamlPath, 'api:\n  baseUrl: https://yaml.com');

      const config = new Config({ loadEnv: false });
      config.loadFromFile(yamlPath);

      expect(config.get('api.baseUrl')).toBe('https://yaml.com');

      unlinkSync(yamlPath);
    });

    it('should throw error for invalid JSON', () => {
      writeFileSync(testConfigPath, 'invalid json');

      const config = new Config({ loadEnv: false });
      expect(() => config.loadFromFile(testConfigPath)).toThrow();
    });

    it('should throw error for unsupported format', () => {
      const txtPath = testConfigPath.replace('.json', '.txt');
      writeFileSync(txtPath, 'text');

      const config = new Config({ loadEnv: false });
      expect(() => config.loadFromFile(txtPath)).toThrow('Unsupported config file format');

      unlinkSync(txtPath);
    });

    it('should throw error for non-existent file', () => {
      const config = new Config({ loadEnv: false });
      expect(() => config.loadFromFile('/nonexistent/file.json')).toThrow();
    });
  });

  describe('validate', () => {
    let config;

    beforeEach(() => {
      config = new Config({
        loadEnv: false,
        api: { baseUrl: 'https://test.com' },
        cache: { maxSize: 100 }
      });
    });

    it('should pass validation for existing keys', () => {
      expect(() => config.validate(['api.baseUrl', 'cache.maxSize'])).not.toThrow();
    });

    it('should throw error for missing required keys', () => {
      expect(() => config.validate(['nonexistent'])).toThrow('Missing required configuration: nonexistent');
    });

    it('should list all missing keys', () => {
      expect(() => config.validate(['missing1', 'missing2'])).toThrow('missing1, missing2');
    });

    it('should return true on successful validation', () => {
      expect(config.validate(['api.baseUrl'])).toBe(true);
    });
  });

  describe('getAll', () => {
    it('should return all configuration', () => {
      const config = new Config({
        loadEnv: false,
        custom: { value: 'test' }
      });

      const all = config.getAll();
      expect(all).toHaveProperty('custom');
      expect(all.custom.value).toBe('test');
    });

    it('should return copy not reference', () => {
      const config = new Config({ loadEnv: false });
      const all = config.getAll();

      all.modified = true;
      expect(config.get('modified')).toBeUndefined();
    });
  });

  describe('_parseValue', () => {
    let config;

    beforeEach(() => {
      config = new Config({ loadEnv: false });
    });

    it('should parse JSON strings', () => {
      expect(config._parseValue('{"key":"value"}')).toEqual({ key: 'value' });
      expect(config._parseValue('[1,2,3]')).toEqual([1, 2, 3]);
    });

    it('should parse booleans', () => {
      expect(config._parseValue('true')).toBe(true);
      expect(config._parseValue('false')).toBe(false);
    });

    it('should parse numbers', () => {
      expect(config._parseValue('123')).toBe(123);
      expect(config._parseValue('45.67')).toBe(45.67);
    });

    it('should return string for unparseable values', () => {
      expect(config._parseValue('plain text')).toBe('plain text');
    });
  });

  describe('default configuration', () => {
    it('should have sensible defaults', () => {
      const config = new Config({ loadEnv: false });

      expect(config.get('api.timeout')).toBe(5000);
      expect(config.get('cache.maxSize')).toBe(100);
      expect(config.get('cache.ttl')).toBe(3600000);
      expect(config.get('router.strategy')).toBe('round-robin');
    });
  });
});
