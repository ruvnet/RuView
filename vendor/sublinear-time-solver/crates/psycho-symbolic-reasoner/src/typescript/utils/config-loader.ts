import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';
import * as yaml from 'yaml';
import { AppConfig, CLIArgs } from '../types/config.js';
import { Logger } from './logger.js';

/**
 * Configuration loader utility
 */
export class ConfigLoader {
  private static readonly DEFAULT_CONFIG_FILES = [
    'psycho-symbolic-reasoner.config.json',
    'psycho-symbolic-reasoner.config.yaml',
    'psycho-symbolic-reasoner.config.yml',
    '.psycho-symbolic-reasoner.json',
    '.psycho-symbolic-reasoner.yaml',
    '.psycho-symbolic-reasoner.yml'
  ];

  /**
   * Load configuration from file and merge with CLI arguments
   */
  public static async loadConfig(cliArgs: CLIArgs): Promise<AppConfig> {
    let fileConfig: Partial<AppConfig> = {};

    // Load from specified config file or search for default files
    const configFile = cliArgs.config || ConfigLoader.findDefaultConfigFile();

    if (configFile) {
      try {
        fileConfig = ConfigLoader.loadConfigFile(configFile);
        try {
          Logger.info(`Loaded configuration from: ${configFile}`);
        } catch {
          // Logger not initialized yet, that's ok
        }
      } catch (error) {
        try {
          Logger.error(`Failed to load config file: ${configFile}`, error);
        } catch {
          // Logger not initialized yet, that's ok
        }
        if (cliArgs.config) {
          // If explicitly specified, fail hard
          throw new Error(`Failed to load specified config file: ${configFile}`);
        }
        // If auto-discovered, just warn and continue with defaults
        try {
          Logger.warn(`Using default configuration due to config file error`);
        } catch {
          // Logger not initialized yet, that's ok
        }
      }
    }

    // Merge file config with CLI arguments
    const mergedConfig = ConfigLoader.mergeConfigs(fileConfig, cliArgs);

    // Validate the final configuration
    const validatedConfig = AppConfig.parse(mergedConfig);

    return validatedConfig;
  }

  /**
   * Find default configuration file in current directory
   */
  private static findDefaultConfigFile(): string | null {
    for (const filename of ConfigLoader.DEFAULT_CONFIG_FILES) {
      const filepath = resolve(process.cwd(), filename);
      if (existsSync(filepath)) {
        return filepath;
      }
    }
    return null;
  }

  /**
   * Load configuration from a specific file
   */
  private static loadConfigFile(filepath: string): Partial<AppConfig> {
    if (!existsSync(filepath)) {
      throw new Error(`Configuration file not found: ${filepath}`);
    }

    const content = readFileSync(filepath, 'utf-8');
    const ext = filepath.toLowerCase();

    if (ext.endsWith('.json')) {
      try {
        return JSON.parse(content);
      } catch (error) {
        throw new Error(`Invalid JSON in config file: ${filepath}`);
      }
    } else if (ext.endsWith('.yaml') || ext.endsWith('.yml')) {
      try {
        return yaml.parse(content);
      } catch (error) {
        throw new Error(`Invalid YAML in config file: ${filepath}`);
      }
    } else {
      throw new Error(`Unsupported config file format: ${filepath}`);
    }
  }

  /**
   * Merge file configuration with CLI arguments
   */
  private static mergeConfigs(fileConfig: Partial<AppConfig>, cliArgs: CLIArgs): any {
    const merged: any = { ...fileConfig };

    // CLI args override file config
    if (cliArgs.transport) {
      merged.server = { ...merged.server, transport: cliArgs.transport };
    }

    if (cliArgs.port) {
      merged.server = { ...merged.server, port: cliArgs.port };
    }

    if (cliArgs.host) {
      merged.server = { ...merged.server, host: cliArgs.host };
    }

    if (cliArgs.knowledgeBase) {
      merged.knowledgeBase = { ...merged.knowledgeBase, file: cliArgs.knowledgeBase };
    }

    if (cliArgs.logLevel) {
      merged.logging = { ...merged.logging, level: cliArgs.logLevel };
    }

    if (cliArgs.logFile) {
      merged.logging = { ...merged.logging, file: cliArgs.logFile };
    }

    if (cliArgs.quiet) {
      merged.logging = { ...merged.logging, console: false };
    }

    if (cliArgs.verbose) {
      merged.logging = { ...merged.logging, level: 'debug' };
    }

    return merged;
  }

  /**
   * Generate sample configuration file
   */
  public static generateSampleConfig(): string {
    const sampleConfig: AppConfig = AppConfig.parse({});

    return JSON.stringify(sampleConfig, null, 2);
  }

  /**
   * Validate configuration object
   */
  public static validateConfig(config: unknown): AppConfig {
    try {
      return AppConfig.parse(config);
    } catch (error) {
      throw new Error(`Configuration validation failed: ${error}`);
    }
  }
}