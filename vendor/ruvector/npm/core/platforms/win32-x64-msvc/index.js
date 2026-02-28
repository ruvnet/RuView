const { join } = require('path');

let nativeBinding;
try {
  nativeBinding = require('./ruvector.node');
} catch (error) {
  throw new Error(
    'Failed to load native binding for win32-x64-msvc. ' +
    'This package may have been installed incorrectly. ' +
    'Ensure you have Visual C++ Redistributable installed. ' +
    'Error: ' + error.message
  );
}

module.exports = nativeBinding;
