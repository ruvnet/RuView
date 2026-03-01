const { platform, arch } = process;

// Platform mapping
const platformMap = {
  'linux': {
    'x64': 'ruvector-core-linux-x64-gnu',
    'arm64': 'ruvector-core-linux-arm64-gnu'
  },
  'darwin': {
    'x64': 'ruvector-core-darwin-x64',
    'arm64': 'ruvector-core-darwin-arm64'
  },
  'win32': {
    'x64': 'ruvector-core-win32-x64-msvc'
  }
};

function loadNativeModule() {
  const platformPackage = platformMap[platform]?.[arch];

  if (!platformPackage) {
    throw new Error(
      `Unsupported platform: ${platform}-${arch}\n` +
      `Ruvector native module is available for:\n` +
      `- Linux (x64, ARM64)\n` +
      `- macOS (x64, ARM64)\n` +
      `- Windows (x64)`
    );
  }

  try {
    return require(platformPackage);
  } catch (error) {
    if (error.code === 'MODULE_NOT_FOUND') {
      throw new Error(
        `Native module not found for ${platform}-${arch}\n` +
        `Please install: npm install ${platformPackage}\n` +
        `Or reinstall ruvector-core to get optional dependencies`
      );
    }
    throw error;
  }
}

module.exports = loadNativeModule();
