const { join } = require('path');

let nativeBinding;
try {
  nativeBinding = require('./ruvector.node');
} catch (error) {
  throw new Error(
    'Failed to load native binding for darwin-x64. ' +
    'This package may have been installed incorrectly. ' +
    'Error: ' + error.message
  );
}

module.exports = nativeBinding;
