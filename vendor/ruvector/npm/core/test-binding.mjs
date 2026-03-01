/**
 * Test to inspect what's actually exported from the native binding
 */

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);

try {
  const nativeBinding = require('./native/linux-x64/ruvector.node');

  console.log('=== Native Binding Inspection ===\n');
  console.log('Type:', typeof nativeBinding);
  console.log('Is null:', nativeBinding === null);
  console.log('Is undefined:', nativeBinding === undefined);
  console.log('\nKeys:', Object.keys(nativeBinding));
  console.log('\nProperties:');

  for (const key of Object.keys(nativeBinding)) {
    const value = nativeBinding[key];
    console.log(`  ${key}: ${typeof value}`);

    if (typeof value === 'object' && value !== null) {
      console.log(`    Methods:`, Object.keys(value));
    }
    if (typeof value === 'function') {
      console.log(`    Is constructor:`, value.prototype !== undefined);
      if (value.prototype) {
        console.log(`    Prototype methods:`, Object.getOwnPropertyNames(value.prototype));
      }
    }
  }

  console.log('\n=== Testing Functions ===\n');

  if (nativeBinding.version) {
    console.log('version():', nativeBinding.version());
  }

  if (nativeBinding.hello) {
    console.log('hello():', nativeBinding.hello());
  }

} catch (error) {
  console.error('Error:', error.message);
  console.error(error.stack);
}
