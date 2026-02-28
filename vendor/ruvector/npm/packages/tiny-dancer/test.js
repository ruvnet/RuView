const tinyDancer = require('./index.js');

console.log('Testing @ruvector/tiny-dancer...');

// Test version function
try {
  const ver = tinyDancer.version();
  console.log(`✓ version(): ${ver}`);
} catch (e) {
  console.error('✗ version() failed:', e.message);
}

// Test hello function
try {
  const msg = tinyDancer.hello();
  console.log(`✓ hello(): ${msg}`);
} catch (e) {
  console.error('✗ hello() failed:', e.message);
}

// Test Router class exists
try {
  if (typeof tinyDancer.Router === 'function') {
    console.log('✓ Router class available');
  } else {
    console.log('✗ Router class not found');
  }
} catch (e) {
  console.error('✗ Router check failed:', e.message);
}

console.log('\nAll basic tests completed!');
