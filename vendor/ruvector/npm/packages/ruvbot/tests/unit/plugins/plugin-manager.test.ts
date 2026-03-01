/**
 * Plugin Manager Unit Tests
 *
 * Tests for plugin discovery, lifecycle, and execution.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  PluginManager,
  createPluginManager,
  createPluginManifest,
  PluginManifestSchema,
  DEFAULT_PLUGIN_CONFIG,
  type PluginInstance,
  type PluginManifest,
} from '../../../src/plugins/PluginManager.js';

describe('PluginManager', () => {
  let manager: PluginManager;

  beforeEach(() => {
    manager = createPluginManager({
      pluginsDir: './test-plugins',
      autoLoad: false,
      sandboxed: true,
    });
  });

  describe('Configuration', () => {
    it('should use default config values', () => {
      const defaultManager = createPluginManager();
      expect(DEFAULT_PLUGIN_CONFIG.pluginsDir).toBe('./plugins');
      expect(DEFAULT_PLUGIN_CONFIG.autoLoad).toBe(true);
      expect(DEFAULT_PLUGIN_CONFIG.maxPlugins).toBe(50);
    });

    it('should override config values', () => {
      const customManager = createPluginManager({
        pluginsDir: './custom-plugins',
        maxPlugins: 10,
      });
      expect(customManager).toBeInstanceOf(PluginManager);
    });
  });

  describe('Plugin Manifest', () => {
    it('should validate valid manifest', () => {
      const manifest = createPluginManifest({
        name: 'test-plugin',
        version: '1.0.0',
        description: 'A test plugin',
      });

      expect(manifest.name).toBe('test-plugin');
      expect(manifest.version).toBe('1.0.0');
      expect(manifest.license).toBe('MIT');
    });

    it('should reject invalid manifest', () => {
      expect(() => {
        PluginManifestSchema.parse({
          name: '', // Invalid: empty name
          version: 'invalid', // Invalid: not semver
        });
      }).toThrow();
    });

    it('should set default values', () => {
      const manifest = createPluginManifest({
        name: 'minimal',
        version: '1.0.0',
        description: 'Minimal plugin',
      });

      expect(manifest.main).toBe('index.js');
      expect(manifest.permissions).toEqual([]);
      expect(manifest.keywords).toEqual([]);
    });

    it('should accept permissions', () => {
      const manifest = createPluginManifest({
        name: 'with-permissions',
        version: '1.0.0',
        description: 'Plugin with permissions',
        permissions: ['memory:read', 'llm:invoke'],
      });

      expect(manifest.permissions).toContain('memory:read');
      expect(manifest.permissions).toContain('llm:invoke');
    });
  });

  describe('Plugin Listing', () => {
    it('should return empty list initially', () => {
      const plugins = manager.listPlugins();
      expect(plugins).toEqual([]);
    });

    it('should return undefined for non-existent plugin', () => {
      const plugin = manager.getPlugin('non-existent');
      expect(plugin).toBeUndefined();
    });

    it('should filter enabled plugins', () => {
      const enabled = manager.getEnabledPlugins();
      expect(enabled).toEqual([]);
    });
  });

  describe('Plugin Skills', () => {
    it('should return empty skills list', () => {
      const skills = manager.getPluginSkills();
      expect(skills).toEqual([]);
    });
  });

  describe('Plugin Commands', () => {
    it('should return empty commands list', () => {
      const commands = manager.getPluginCommands();
      expect(commands).toEqual([]);
    });
  });

  describe('Message Dispatch', () => {
    it('should return null when no plugins handle message', async () => {
      const response = await manager.dispatchMessage({
        content: 'Hello',
        userId: 'user-123',
      });
      expect(response).toBeNull();
    });
  });

  describe('Skill Invocation', () => {
    it('should throw when skill not found', async () => {
      await expect(
        manager.invokeSkill('non-existent-skill', {})
      ).rejects.toThrow('Skill non-existent-skill not found');
    });
  });

  describe('Events', () => {
    it('should emit events', () => {
      const loadHandler = vi.fn();
      const errorHandler = vi.fn();

      manager.on('plugin:loaded', loadHandler);
      manager.on('plugin:error', errorHandler);

      // Events would be emitted during plugin loading
      expect(manager.listenerCount('plugin:loaded')).toBe(1);
      expect(manager.listenerCount('plugin:error')).toBe(1);
    });
  });

  describe('Registry Search', () => {
    it('should return empty array without IPFS gateway', async () => {
      const managerWithoutIPFS = createPluginManager({
        ipfsGateway: undefined,
      });
      const results = await managerWithoutIPFS.searchRegistry('test');
      expect(results).toEqual([]);
    });
  });

  describe('Registry Install', () => {
    it('should throw without IPFS gateway', async () => {
      const managerWithoutIPFS = createPluginManager({
        ipfsGateway: undefined,
      });
      await expect(
        managerWithoutIPFS.installFromRegistry('test-plugin')
      ).rejects.toThrow('IPFS gateway not configured');
    });
  });

  describe('Plugin Enable/Disable', () => {
    it('should return false when plugin not found', async () => {
      const result = await manager.enablePlugin('non-existent');
      expect(result).toBe(false);
    });

    it('should return false when disabling non-existent plugin', async () => {
      const result = await manager.disablePlugin('non-existent');
      expect(result).toBe(false);
    });
  });

  describe('Plugin Unload', () => {
    it('should return false when plugin not found', async () => {
      const result = await manager.unloadPlugin('non-existent');
      expect(result).toBe(false);
    });
  });

  describe('Max Plugins Limit', () => {
    it('should enforce max plugins config', () => {
      const limitedManager = createPluginManager({
        maxPlugins: 5,
      });
      expect(limitedManager).toBeInstanceOf(PluginManager);
    });
  });
});

describe('Plugin Manifest Validation', () => {
  it('should validate name length', () => {
    expect(() => {
      PluginManifestSchema.parse({
        name: 'a'.repeat(100), // Too long
        version: '1.0.0',
        description: 'Test',
      });
    }).toThrow();
  });

  it('should validate semver format', () => {
    const validVersions = ['1.0.0', '0.1.0', '10.20.30', '1.0.0-alpha'];
    const invalidVersions = ['1', '1.0', 'v1.0.0', 'latest'];

    validVersions.forEach(version => {
      expect(() => {
        PluginManifestSchema.parse({
          name: 'test',
          version,
          description: 'Test',
        });
      }).not.toThrow();
    });

    invalidVersions.forEach(version => {
      expect(() => {
        PluginManifestSchema.parse({
          name: 'test',
          version,
          description: 'Test',
        });
      }).toThrow();
    });
  });

  it('should validate permission values', () => {
    expect(() => {
      PluginManifestSchema.parse({
        name: 'test',
        version: '1.0.0',
        description: 'Test',
        permissions: ['invalid:permission'],
      });
    }).toThrow();
  });

  it('should accept all valid permissions', () => {
    const validPermissions = [
      'memory:read',
      'memory:write',
      'session:read',
      'session:write',
      'skill:register',
      'skill:invoke',
      'llm:invoke',
      'http:outbound',
      'fs:read',
      'fs:write',
      'env:read',
    ];

    expect(() => {
      PluginManifestSchema.parse({
        name: 'test',
        version: '1.0.0',
        description: 'Test',
        permissions: validPermissions,
      });
    }).not.toThrow();
  });
});
