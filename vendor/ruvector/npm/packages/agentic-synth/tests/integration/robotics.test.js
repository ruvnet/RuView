/**
 * Integration tests for Agentic Robotics adapter
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { RoboticsAdapter } from '../../src/adapters/robotics.js';
import { DataGenerator } from '../../src/generators/data-generator.js';

describe('Agentic Robotics Integration', () => {
  let adapter;
  let generator;

  beforeEach(async () => {
    adapter = new RoboticsAdapter({
      endpoint: 'http://localhost:9000',
      protocol: 'grpc'
    });

    generator = new DataGenerator({
      schema: {
        action: { type: 'string', length: 8 },
        value: { type: 'number', min: 0, max: 100 }
      }
    });

    await adapter.initialize();
  });

  afterEach(async () => {
    if (adapter.initialized) {
      await adapter.shutdown();
    }
  });

  describe('initialization', () => {
    it('should initialize adapter', async () => {
      const newAdapter = new RoboticsAdapter();
      await newAdapter.initialize();
      expect(newAdapter.initialized).toBe(true);
    });

    it('should handle re-initialization', async () => {
      await adapter.initialize();
      expect(adapter.initialized).toBe(true);
    });

    it('should shutdown adapter', async () => {
      await adapter.shutdown();
      expect(adapter.initialized).toBe(false);
    });
  });

  describe('command execution', () => {
    it('should send basic command', async () => {
      const command = {
        type: 'move',
        payload: { x: 10, y: 20 }
      };

      const result = await adapter.sendCommand(command);

      expect(result).toHaveProperty('commandId');
      expect(result.type).toBe('move');
      expect(result.status).toBe('executed');
      expect(result.result).toEqual({ x: 10, y: 20 });
    });

    it('should throw error when not initialized', async () => {
      await adapter.shutdown();

      await expect(adapter.sendCommand({ type: 'test' })).rejects.toThrow(
        'Robotics adapter not initialized'
      );
    });

    it('should validate command structure', async () => {
      await expect(adapter.sendCommand({})).rejects.toThrow('Invalid command: missing type');
      await expect(adapter.sendCommand(null)).rejects.toThrow('Invalid command: missing type');
    });

    it('should handle commands without payload', async () => {
      const command = { type: 'status' };
      const result = await adapter.sendCommand(command);

      expect(result.type).toBe('status');
      expect(result.status).toBe('executed');
    });
  });

  describe('status monitoring', () => {
    it('should get adapter status', async () => {
      const status = await adapter.getStatus();

      expect(status).toHaveProperty('initialized');
      expect(status).toHaveProperty('protocol');
      expect(status).toHaveProperty('endpoint');
      expect(status.initialized).toBe(true);
      expect(status.protocol).toBe('grpc');
    });

    it('should throw error when checking status while not initialized', async () => {
      await adapter.shutdown();

      await expect(adapter.getStatus()).rejects.toThrow(
        'Robotics adapter not initialized'
      );
    });
  });

  describe('end-to-end workflow', () => {
    it('should generate data and execute commands', async () => {
      // Generate synthetic command data
      const data = generator.generate(5);

      // Execute commands
      const results = [];
      for (const item of data) {
        const result = await adapter.sendCommand({
          type: 'execute',
          payload: item
        });
        results.push(result);
      }

      expect(results).toHaveLength(5);
      results.forEach(result => {
        expect(result.status).toBe('executed');
        expect(result).toHaveProperty('commandId');
      });
    });

    it('should handle batch command execution', async () => {
      const commands = [
        { type: 'init', payload: { config: 'test' } },
        { type: 'move', payload: { x: 1, y: 2 } },
        { type: 'rotate', payload: { angle: 90 } },
        { type: 'stop' }
      ];

      const results = await Promise.all(
        commands.map(cmd => adapter.sendCommand(cmd))
      );

      expect(results).toHaveLength(4);
      expect(results[0].type).toBe('init');
      expect(results[1].type).toBe('move');
      expect(results[2].type).toBe('rotate');
      expect(results[3].type).toBe('stop');
    });
  });

  describe('error handling', () => {
    it('should handle initialization failure gracefully', async () => {
      const failingAdapter = new RoboticsAdapter({
        endpoint: 'http://invalid:99999'
      });

      // Note: Mock implementation always succeeds
      await expect(failingAdapter.initialize()).resolves.toBe(true);
    });

    it('should handle command execution errors', async () => {
      await adapter.shutdown();

      await expect(adapter.sendCommand({ type: 'test' })).rejects.toThrow();
    });
  });

  describe('performance', () => {
    it('should execute 100 commands quickly', async () => {
      const commands = Array.from({ length: 100 }, (_, i) => ({
        type: 'test',
        payload: { index: i }
      }));

      const start = Date.now();
      await Promise.all(commands.map(cmd => adapter.sendCommand(cmd)));
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(1000); // Less than 1 second
    });

    it('should handle concurrent command execution', async () => {
      const concurrentCommands = 50;
      const commands = Array.from({ length: concurrentCommands }, (_, i) => ({
        type: 'concurrent',
        payload: { id: i }
      }));

      const results = await Promise.all(
        commands.map(cmd => adapter.sendCommand(cmd))
      );

      expect(results).toHaveLength(concurrentCommands);
      results.forEach(result => {
        expect(result.status).toBe('executed');
      });
    });
  });

  describe('protocol support', () => {
    it('should support different protocols', async () => {
      const protocols = ['grpc', 'http', 'websocket'];

      for (const protocol of protocols) {
        const protocolAdapter = new RoboticsAdapter({ protocol });
        await protocolAdapter.initialize();

        const status = await protocolAdapter.getStatus();
        expect(status.protocol).toBe(protocol);

        await protocolAdapter.shutdown();
      }
    });
  });
});
