/**
 * Error Handling and Edge Case Tests
 * Comprehensive tests for error conditions and boundary cases
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';

describe('Error Handling and Edge Cases', () => {
  beforeAll(() => {
    console.log('Initializing error handling tests...');
  });

  describe('Input Validation Errors', () => {
    test('should handle null and undefined inputs gracefully', () => {
      const nullInputTests = [
        { input: null, operation: 'add_fact', expected_error: 'Invalid input' },
        { input: undefined, operation: 'query', expected_error: 'Invalid input' },
        { input: '', operation: 'analyze_sentiment', expected_error: 'Empty input' },
        { input: '   ', operation: 'extract_preferences', expected_error: 'Empty input' }
      ];

      for (const test of nullInputTests) {
        expect(() => {
          // Mock function that validates input
          const validateInput = (input: any, operation: string) => {
            if (input === null || input === undefined) {
              throw new Error('Invalid input: null or undefined');
            }
            if (typeof input === 'string' && input.trim() === '') {
              throw new Error('Empty input not allowed');
            }
            return true;
          };

          validateInput(test.input, test.operation);
        }).toThrow();
      }
    });

    test('should handle malformed JSON inputs', () => {
      const malformedJsonTests = [
        '{"subject": "Alice", "predicate": "knows"', // Missing closing brace
        '{"subject": Alice, "predicate": "knows"}', // Missing quotes
        '{subject: "Alice", "predicate": "knows"}', // Missing quotes on key
        '{"subject": "Alice" "predicate": "knows"}', // Missing comma
        'not json at all',
        '{"circular": circular}', // Invalid reference
        '{true: false}', // Invalid key type
      ];

      for (const malformedJson of malformedJsonTests) {
        expect(() => {
          JSON.parse(malformedJson);
        }).toThrow();

        // Test graceful handling
        const safeJsonParse = (jsonString: string) => {
          try {
            return { success: true, data: JSON.parse(jsonString) };
          } catch (error) {
            return { success: false, error: (error as Error).message };
          }
        };

        const result = safeJsonParse(malformedJson);
        expect(result.success).toBe(false);
        expect(result.error).toBeDefined();
      }
    });

    test('should handle oversized inputs', () => {
      const oversizedTests = [
        { type: 'text', size: 1000000, limit: 10000 }, // 1MB text, 10KB limit
        { type: 'array', size: 100000, limit: 1000 }, // 100K array, 1K limit
        { type: 'object_depth', size: 10000, limit: 100 }, // Deep nesting
        { type: 'string_length', size: 50000, limit: 1000 } // Long string
      ];

      for (const test of oversizedTests) {
        const validateSize = (input: any, type: string, limit: number) => {
          switch (type) {
            case 'text':
              if (typeof input === 'string' && input.length > limit) {
                throw new Error(`Text too long: ${input.length} > ${limit}`);
              }
              break;
            case 'array':
              if (Array.isArray(input) && input.length > limit) {
                throw new Error(`Array too large: ${input.length} > ${limit}`);
              }
              break;
            case 'string_length':
              if (typeof input === 'string' && input.length > limit) {
                throw new Error(`String too long: ${input.length} > ${limit}`);
              }
              break;
          }
        };

        // Create oversized input
        let oversizedInput: any;
        switch (test.type) {
          case 'text':
          case 'string_length':
            oversizedInput = 'a'.repeat(test.size);
            break;
          case 'array':
            oversizedInput = new Array(test.size).fill(0);
            break;
          case 'object_depth':
            oversizedInput = {};
            let current = oversizedInput;
            for (let i = 0; i < test.size; i++) {
              current.next = {};
              current = current.next;
            }
            break;
        }

        expect(() => {
          validateSize(oversizedInput, test.type, test.limit);
        }).toThrow();
      }
    });

    test('should handle invalid data types', () => {
      const typeValidationTests = [
        { input: 123, expected_type: 'string', operation: 'add_fact_subject' },
        { input: 'not_a_number', expected_type: 'number', operation: 'set_confidence' },
        { input: 'not_boolean', expected_type: 'boolean', operation: 'set_flag' },
        { input: 123, expected_type: 'object', operation: 'add_rule' },
        { input: {}, expected_type: 'array', operation: 'set_preconditions' }
      ];

      for (const test of typeValidationTests) {
        const validateType = (input: any, expectedType: string) => {
          if (expectedType === 'array' && !Array.isArray(input)) {
            throw new Error(`Expected array, got ${typeof input}`);
          } else if (expectedType !== 'array' && typeof input !== expectedType) {
            throw new Error(`Expected ${expectedType}, got ${typeof input}`);
          }
        };

        expect(() => {
          validateType(test.input, test.expected_type);
        }).toThrow();
      }
    });
  });

  describe('Resource Exhaustion Handling', () => {
    test('should handle memory pressure gracefully', async () => {
      const detector = testUtils.memoryLeakDetector;
      detector.start();

      const memoryPressureTest = async () => {
        const allocations = [];
        let allocationCount = 0;
        const maxAllocations = 10000;

        try {
          while (allocationCount < maxAllocations) {
            // Allocate progressively larger chunks
            const size = Math.min(1000 + allocationCount, 100000);
            const allocation = new Array(size).fill(Math.random());
            allocations.push(allocation);
            allocationCount++;

            // Check memory usage periodically
            if (allocationCount % 100 === 0) {
              detector.snapshot();
              const analysis = detector.checkForLeaks();

              // If memory usage is too high, start cleanup
              if (analysis.memoryIncrease > 100 * 1024 * 1024) { // 100MB
                console.log(`Memory pressure detected at ${allocationCount} allocations`);

                // Clean up half the allocations
                const toRemove = Math.floor(allocations.length / 2);
                allocations.splice(0, toRemove);

                if (global.gc) {
                  global.gc();
                }

                detector.snapshot();
                break;
              }
            }

            // Yield control periodically
            if (allocationCount % 500 === 0) {
              await testUtils.asyncUtils.sleep(1);
            }
          }
        } catch (error) {
          console.log(`Memory exhaustion at ${allocationCount} allocations:`, (error as Error).message);
        }

        return { allocationCount, finalSize: allocations.length };
      };

      const result = await memoryPressureTest();

      expect(result.allocationCount).toBeGreaterThan(100); // Should allocate some memory
      expect(result.finalSize).toBeLessThan(result.allocationCount); // Should have cleaned up

      const finalAnalysis = detector.checkForLeaks();
      console.log(`Memory pressure test: ${result.allocationCount} allocations, final memory: ${Math.round(finalAnalysis.memoryIncrease / 1024)}KB`);
    });

    test('should handle stack overflow conditions', () => {
      const stackOverflowTests = [
        {
          name: 'infinite_recursion',
          fn: function infiniteRecursion(depth: number = 0): number {
            if (depth > 10000) { // Safety limit
              throw new Error('Maximum recursion depth exceeded');
            }
            return infiniteRecursion(depth + 1);
          }
        },
        {
          name: 'deep_object_traversal',
          fn: function deepTraversal(obj: any, depth: number = 0): any {
            if (depth > 5000) { // Safety limit
              throw new Error('Maximum traversal depth exceeded');
            }
            if (obj && obj.next) {
              return deepTraversal(obj.next, depth + 1);
            }
            return obj;
          }
        }
      ];

      for (const test of stackOverflowTests) {
        expect(() => {
          if (test.name === 'infinite_recursion') {
            test.fn();
          } else if (test.name === 'deep_object_traversal') {
            // Create deep object
            const deepObj: any = {};
            let current = deepObj;
            for (let i = 0; i < 6000; i++) {
              current.next = { value: i };
              current = current.next;
            }
            test.fn(deepObj);
          }
        }).toThrow();
      }
    });

    test('should handle timeout conditions', async () => {
      const timeoutTests = [
        { name: 'slow_operation', timeout: 100, expected_duration: 200 },
        { name: 'network_simulation', timeout: 500, expected_duration: 1000 },
        { name: 'cpu_intensive', timeout: 50, expected_duration: 150 }
      ];

      for (const test of timeoutTests) {
        const slowOperation = async (duration: number) => {
          return new Promise<string>((resolve) => {
            setTimeout(() => resolve('completed'), duration);
          });
        };

        // Test timeout handling
        await expect(
          testUtils.asyncUtils.withTimeout(
            slowOperation(test.expected_duration),
            test.timeout
          )
        ).rejects.toThrow('timed out');

        console.log(`${test.name} timeout test passed`);
      }
    });
  });

  describe('Concurrent Access Errors', () => {
    test('should handle race conditions safely', async () => {
      const sharedResource = {
        counter: 0,
        data: new Map<string, any>(),
        lock: false
      };

      const concurrentOperations = async (operationId: number) => {
        for (let i = 0; i < 100; i++) {
          // Simulate acquiring lock
          while (sharedResource.lock) {
            await testUtils.asyncUtils.sleep(1);
          }

          try {
            sharedResource.lock = true;

            // Critical section
            const currentCounter = sharedResource.counter;
            await testUtils.asyncUtils.sleep(Math.random() * 2); // Simulate work
            sharedResource.counter = currentCounter + 1;
            sharedResource.data.set(`op_${operationId}_${i}`, Date.now());

          } finally {
            sharedResource.lock = false;
          }

          await testUtils.asyncUtils.sleep(1);
        }
      };

      // Run concurrent operations
      const operations = [];
      for (let i = 0; i < 5; i++) {
        operations.push(concurrentOperations(i));
      }

      await Promise.all(operations);

      // Verify data integrity
      expect(sharedResource.counter).toBe(500); // 5 operations × 100 iterations
      expect(sharedResource.data.size).toBe(500);
      expect(sharedResource.lock).toBe(false);

      console.log('Race condition test passed: counter =', sharedResource.counter);
    });

    test('should handle deadlock prevention', async () => {
      const resources = {
        resourceA: { locked: false, lockHolder: null as string | null },
        resourceB: { locked: false, lockHolder: null as string | null }
      };

      const acquireResource = async (resourceName: 'resourceA' | 'resourceB', holder: string, timeout: number = 1000) => {
        const startTime = Date.now();

        while (resources[resourceName].locked) {
          if (Date.now() - startTime > timeout) {
            throw new Error(`Timeout acquiring ${resourceName} for ${holder}`);
          }
          await testUtils.asyncUtils.sleep(10);
        }

        resources[resourceName].locked = true;
        resources[resourceName].lockHolder = holder;
      };

      const releaseResource = (resourceName: 'resourceA' | 'resourceB', holder: string) => {
        if (resources[resourceName].lockHolder === holder) {
          resources[resourceName].locked = false;
          resources[resourceName].lockHolder = null;
        }
      };

      const deadlockProneOperation1 = async () => {
        try {
          await acquireResource('resourceA', 'operation1', 500);
          await testUtils.asyncUtils.sleep(100);
          await acquireResource('resourceB', 'operation1', 500);

          // Work with both resources
          await testUtils.asyncUtils.sleep(50);

        } catch (error) {
          // Expected timeout due to deadlock prevention
          console.log('Operation1 timed out (expected):', (error as Error).message);
        } finally {
          releaseResource('resourceB', 'operation1');
          releaseResource('resourceA', 'operation1');
        }
      };

      const deadlockProneOperation2 = async () => {
        try {
          await acquireResource('resourceB', 'operation2', 500);
          await testUtils.asyncUtils.sleep(100);
          await acquireResource('resourceA', 'operation2', 500);

          // Work with both resources
          await testUtils.asyncUtils.sleep(50);

        } catch (error) {
          // Expected timeout due to deadlock prevention
          console.log('Operation2 timed out (expected):', (error as Error).message);
        } finally {
          releaseResource('resourceA', 'operation2');
          releaseResource('resourceB', 'operation2');
        }
      };

      // Run potentially deadlocking operations
      await Promise.all([
        deadlockProneOperation1(),
        deadlockProneOperation2()
      ]);

      // Verify resources are properly released
      expect(resources.resourceA.locked).toBe(false);
      expect(resources.resourceB.locked).toBe(false);
      expect(resources.resourceA.lockHolder).toBe(null);
      expect(resources.resourceB.lockHolder).toBe(null);

      console.log('Deadlock prevention test passed');
    });
  });

  describe('Data Corruption Scenarios', () => {
    test('should detect and handle corrupted data structures', () => {
      const corruptionTests = [
        {
          name: 'missing_required_fields',
          data: { subject: 'Alice' }, // Missing predicate and object
          expectedError: 'Missing required field'
        },
        {
          name: 'invalid_references',
          data: { id: 'fact1', subject: 'Alice', predicate: 'knows', object: 'NonExistentEntity' },
          expectedError: 'Invalid reference'
        },
        {
          name: 'circular_references',
          data: (() => {
            const obj: any = { id: 'circular' };
            obj.self = obj;
            return obj;
          })(),
          expectedError: 'Circular reference'
        },
        {
          name: 'type_mismatch',
          data: { confidence: 'not_a_number' },
          expectedError: 'Type mismatch'
        }
      ];

      for (const test of corruptionTests) {
        const validateDataStructure = (data: any) => {
          // Check for required fields
          if (data.subject !== undefined && (!data.predicate || !data.object)) {
            throw new Error('Missing required field: predicate or object');
          }

          // Check for type mismatches
          if (data.confidence !== undefined && typeof data.confidence !== 'number') {
            throw new Error('Type mismatch: confidence must be number');
          }

          // Check for circular references (simple detection)
          const seen = new WeakSet();
          const checkCircular = (obj: any): void => {
            if (obj && typeof obj === 'object') {
              if (seen.has(obj)) {
                throw new Error('Circular reference detected');
              }
              seen.add(obj);
              for (const value of Object.values(obj)) {
                checkCircular(value);
              }
            }
          };
          checkCircular(data);

          return true;
        };

        expect(() => {
          validateDataStructure(test.data);
        }).toThrow();

        console.log(`${test.name} corruption test passed`);
      }
    });

    test('should handle data recovery scenarios', () => {
      const recoveryScenarios = [
        {
          name: 'partial_data_loss',
          corruptedData: [
            { id: 'fact1', subject: 'Alice', predicate: 'knows', object: 'Bob' },
            { id: 'fact2', subject: 'Bob' }, // Incomplete fact
            { id: 'fact3', subject: 'Charlie', predicate: 'likes', object: 'Coffee' }
          ],
          expectedRecovered: 2
        },
        {
          name: 'format_corruption',
          corruptedData: [
            '{"id":"fact1","subject":"Alice","predicate":"knows","object":"Bob"}',
            'corrupted json string',
            '{"id":"fact3","subject":"Charlie","predicate":"likes","object":"Coffee"}'
          ],
          expectedRecovered: 2
        }
      ];

      for (const scenario of recoveryScenarios) {
        const recoverData = (corruptedItems: any[]) => {
          const recovered = [];
          const errors = [];

          for (const item of corruptedItems) {
            try {
              let parsedItem = item;

              // Try to parse if it's a string
              if (typeof item === 'string') {
                parsedItem = JSON.parse(item);
              }

              // Validate required fields
              if (parsedItem.id && parsedItem.subject && parsedItem.predicate && parsedItem.object) {
                recovered.push(parsedItem);
              } else {
                errors.push({ item, error: 'Missing required fields' });
              }
            } catch (error) {
              errors.push({ item, error: (error as Error).message });
            }
          }

          return { recovered, errors };
        };

        const result = recoverData(scenario.corruptedData);

        expect(result.recovered.length).toBe(scenario.expectedRecovered);
        expect(result.errors.length).toBe(scenario.corruptedData.length - scenario.expectedRecovered);

        console.log(`${scenario.name} recovery: ${result.recovered.length}/${scenario.corruptedData.length} items recovered`);
      }
    });
  });

  describe('Edge Case Scenarios', () => {
    test('should handle boundary value conditions', () => {
      const boundaryTests = [
        { name: 'zero_confidence', value: 0, min: 0, max: 1, valid: true },
        { name: 'max_confidence', value: 1, min: 0, max: 1, valid: true },
        { name: 'negative_confidence', value: -0.1, min: 0, max: 1, valid: false },
        { name: 'over_max_confidence', value: 1.1, min: 0, max: 1, valid: false },
        { name: 'empty_string', value: '', min: 1, max: 1000, valid: false },
        { name: 'max_length_string', value: 'a'.repeat(1000), min: 1, max: 1000, valid: true },
        { name: 'over_max_length', value: 'a'.repeat(1001), min: 1, max: 1000, valid: false }
      ];

      for (const test of boundaryTests) {
        const validateBoundary = (value: any, min: number, max: number) => {
          if (typeof value === 'number') {
            return value >= min && value <= max;
          } else if (typeof value === 'string') {
            return value.length >= min && value.length <= max;
          }
          return false;
        };

        const isValid = validateBoundary(test.value, test.min, test.max);
        expect(isValid).toBe(test.valid);

        console.log(`${test.name}: ${isValid ? 'valid' : 'invalid'} (expected: ${test.valid ? 'valid' : 'invalid'})`);
      }
    });

    test('should handle unusual character encodings', () => {
      const encodingTests = [
        { name: 'unicode_emoji', text: 'I love coffee! ☕😍🎉', expected_length: 19 },
        { name: 'unicode_accents', text: 'Café, naïve, résumé', expected_length: 19 },
        { name: 'unicode_symbols', text: '© ® ™ € £ ¥ § ¶', expected_length: 15 },
        { name: 'unicode_chinese', text: '你好世界', expected_length: 4 },
        { name: 'unicode_arabic', text: 'مرحبا بالعالم', expected_length: 12 },
        { name: 'unicode_mixed', text: 'Hello 世界 🌍', expected_length: 9 }
      ];

      for (const test of encodingTests) {
        const processUnicodeText = (text: string) => {
          // Test that Unicode text is handled correctly
          const length = text.length;
          const bytes = new TextEncoder().encode(text);
          const decoded = new TextDecoder().decode(bytes);

          return {
            originalLength: length,
            byteLength: bytes.length,
            roundTrip: decoded === text,
            firstChar: text.charCodeAt(0),
            lastChar: text.charCodeAt(text.length - 1)
          };
        };

        const result = processUnicodeText(test.text);

        expect(result.originalLength).toBe(test.expected_length);
        expect(result.roundTrip).toBe(true);
        expect(result.byteLength).toBeGreaterThanOrEqual(result.originalLength);

        console.log(`${test.name}: ${result.originalLength} chars, ${result.byteLength} bytes, round-trip: ${result.roundTrip}`);
      }
    });

    test('should handle floating point precision issues', () => {
      const precisionTests = [
        { a: 0.1, b: 0.2, expected: 0.3, tolerance: 1e-10 },
        { a: 0.1 * 3, b: 0.3, expected: 0.3, tolerance: 1e-10 },
        { a: Math.PI, b: Math.E, expected: Math.PI + Math.E, tolerance: 1e-15 },
        { a: 1e-16, b: 1e-16, expected: 2e-16, tolerance: 1e-17 }
      ];

      for (const test of precisionTests) {
        const result = test.a + test.b;
        const difference = Math.abs(result - test.expected);
        const withinTolerance = difference <= test.tolerance;

        expect(withinTolerance).toBe(true);

        console.log(`Precision test: ${test.a} + ${test.b} = ${result} (expected: ${test.expected}, diff: ${difference})`);
      }
    });

    test('should handle date and time edge cases', () => {
      const dateTests = [
        { name: 'epoch_zero', timestamp: 0, expected_year: 1970 },
        { name: 'y2k_boundary', timestamp: 946684800000, expected_year: 2000 },
        { name: 'leap_year', timestamp: 1582934400000, expected_year: 2020 },
        { name: 'dst_transition', timestamp: 1615708800000, expected_valid: true },
        { name: 'max_safe_integer', timestamp: Number.MAX_SAFE_INTEGER, expected_valid: true }
      ];

      for (const test of dateTests) {
        const processTimestamp = (timestamp: number) => {
          try {
            const date = new Date(timestamp);
            return {
              valid: !isNaN(date.getTime()),
              year: date.getFullYear(),
              iso: date.toISOString()
            };
          } catch (error) {
            return {
              valid: false,
              error: (error as Error).message
            };
          }
        };

        const result = processTimestamp(test.timestamp);

        if (test.expected_year) {
          expect(result.year).toBe(test.expected_year);
        }
        if (test.expected_valid !== undefined) {
          expect(result.valid).toBe(test.expected_valid);
        }

        console.log(`${test.name}: ${result.valid ? `year ${result.year}` : `invalid: ${result.error}`}`);
      }
    });
  });
});