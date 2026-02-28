#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const platforms = [
  'linux-x64',
  'linux-arm64',
  'darwin-x64',
  'darwin-arm64',
  'win32-x64'
];

const basePackage = {
  version: '0.1.2',
  repository: {
    type: 'git',
    url: 'https://github.com/ruvnet/ruvector.git'
  },
  license: 'MIT',
  keywords: ['vector', 'database', 'native', 'napi', 'rust'],
  os: [],
  cpu: []
};

// Platform-specific configurations
const platformConfigs = {
  'linux-x64': { os: ['linux'], cpu: ['x64'] },
  'linux-arm64': { os: ['linux'], cpu: ['arm64'] },
  'darwin-x64': { os: ['darwin'], cpu: ['x64'] },
  'darwin-arm64': { os: ['darwin'], cpu: ['arm64'] },
  'win32-x64': { os: ['win32'], cpu: ['x64'] }
};

function createPlatformPackage(platform) {
  const packageDir = path.join(__dirname, '..', platform);
  const nativeDir = path.join(__dirname, '..', 'native', platform);

  // Check if native module exists
  if (!fs.existsSync(nativeDir)) {
    console.log(`⏭️  Skipping ${platform} (no native module found)`);
    return false;
  }

  // Create platform package directory
  if (!fs.existsSync(packageDir)) {
    fs.mkdirSync(packageDir, { recursive: true });
  }

  // Create package.json
  const packageJson = {
    name: `@ruvector/core-${platform}`,
    description: `Native NAPI bindings for Ruvector (${platform})`,
    main: 'index.js',
    ...basePackage,
    ...platformConfigs[platform]
  };

  fs.writeFileSync(
    path.join(packageDir, 'package.json'),
    JSON.stringify(packageJson, null, 2)
  );

  // Create index.js that loads the native module
  const extension = platform.startsWith('win32') ? '.dll' : '.node';
  const nativeFile = `ruvector${extension}`;

  const indexJs = `
const { join } = require('path');

let nativeBinding;
try {
  nativeBinding = require('./${nativeFile}');
} catch (error) {
  throw new Error(
    'Failed to load native binding for ${platform}. ' +
    'This package may have been installed incorrectly. ' +
    'Error: ' + error.message
  );
}

module.exports = nativeBinding;
`.trim();

  fs.writeFileSync(path.join(packageDir, 'index.js'), indexJs);

  // Copy native module
  const sourceFile = path.join(nativeDir, 'ruvector.node');
  const targetFile = path.join(packageDir, nativeFile);

  if (fs.existsSync(sourceFile)) {
    fs.copyFileSync(sourceFile, targetFile);
  }

  // Copy README
  const readme = `# @ruvector/core-${platform}

Native NAPI bindings for Ruvector vector database (${platform}).

This package is automatically installed as an optional dependency of \`@ruvector/core\`.
You should not need to install it directly.

## Platform Support

- OS: ${platformConfigs[platform].os.join(', ')}
- CPU: ${platformConfigs[platform].cpu.join(', ')}

## Installation

\`\`\`bash
npm install @ruvector/core
\`\`\`

## License

MIT
`;

  fs.writeFileSync(path.join(packageDir, 'README.md'), readme);

  return packageDir;
}

function publishPlatform(packageDir) {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(packageDir, 'package.json'))
  );

  console.log(`📦 Publishing ${packageJson.name}...`);

  try {
    execSync('npm publish --access public', {
      cwd: packageDir,
      stdio: 'inherit'
    });
    console.log(`✅ Published ${packageJson.name}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to publish ${packageJson.name}:`, error.message);
    return false;
  }
}

// Main execution
console.log('🚀 Creating and publishing platform packages...\n');

let successCount = 0;
let failCount = 0;

for (const platform of platforms) {
  const packageDir = createPlatformPackage(platform);

  if (packageDir) {
    const published = publishPlatform(packageDir);
    if (published) {
      successCount++;
    } else {
      failCount++;
    }
  }
  console.log('');
}

console.log(`\n📊 Summary: ${successCount} published, ${failCount} failed`);

if (failCount > 0) {
  process.exit(1);
}
