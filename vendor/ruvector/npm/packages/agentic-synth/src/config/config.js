/**
 * Configuration management
 */

import { readFileSync } from 'fs';
import yaml from 'js-yaml';
import { config as dotenvConfig } from 'dotenv';

export class Config {
  constructor(options = {}) {
    this.values = {};
    this.envPrefix = options.envPrefix || 'AGENTIC_SYNTH_';

    if (options.loadEnv !== false) {
      dotenvConfig();
    }

    if (options.configPath) {
      this.loadFromFile(options.configPath);
    }

    this.values = {
      ...this._getDefaults(),
      ...this.values,
      ...options
    };
  }

  get(key, defaultValue = undefined) {
    const envKey = `${this.envPrefix}${key.toUpperCase().replace(/\./g, '_')}`;
    if (process.env[envKey]) {
      return this._parseValue(process.env[envKey]);
    }

    const keys = key.split('.');
    let value = this.values;

    for (const k of keys) {
      if (value && typeof value === 'object' && k in value) {
        value = value[k];
      } else {
        return defaultValue;
      }
    }

    return value !== undefined ? value : defaultValue;
  }

  set(key, value) {
    const keys = key.split('.');
    let target = this.values;

    for (let i = 0; i < keys.length - 1; i++) {
      const k = keys[i];
      if (!(k in target) || typeof target[k] !== 'object') {
        target[k] = {};
      }
      target = target[k];
    }

    target[keys[keys.length - 1]] = value;
  }

  loadFromFile(path) {
    try {
      const content = readFileSync(path, 'utf8');

      if (path.endsWith('.json')) {
        this.values = { ...this.values, ...JSON.parse(content) };
      } else if (path.endsWith('.yaml') || path.endsWith('.yml')) {
        this.values = { ...this.values, ...yaml.load(content) };
      } else {
        throw new Error('Unsupported config file format');
      }
    } catch (error) {
      throw new Error(`Failed to load config from ${path}: ${error.message}`);
    }
  }

  validate(requiredKeys = []) {
    const missing = [];

    for (const key of requiredKeys) {
      if (this.get(key) === undefined) {
        missing.push(key);
      }
    }

    if (missing.length > 0) {
      throw new Error(`Missing required configuration: ${missing.join(', ')}`);
    }

    return true;
  }

  getAll() {
    return { ...this.values };
  }

  _getDefaults() {
    return {
      api: {
        baseUrl: 'https://api.example.com',
        timeout: 5000,
        retries: 3
      },
      cache: {
        maxSize: 100,
        ttl: 3600000
      },
      generator: {
        seed: Date.now(),
        format: 'json'
      },
      router: {
        strategy: 'round-robin'
      }
    };
  }

  _parseValue(value) {
    if (value.startsWith('{') || value.startsWith('[')) {
      try {
        return JSON.parse(value);
      } catch {
        return value;
      }
    }

    if (value === 'true') return true;
    if (value === 'false') return false;

    if (!isNaN(value) && value.trim() !== '') {
      return Number(value);
    }

    return value;
  }
}
