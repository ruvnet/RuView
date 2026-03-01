#!/usr/bin/env node
/**
 * Publish platform-specific @ruvector/graph-node packages to npm
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const platforms = [
  { name: 'linux-x64-gnu', nodeFile: 'index.linux-x64-gnu.node' },
  { name: 'linux-arm64-gnu', nodeFile: 'index.linux-arm64-gnu.node' },
  { name: 'darwin-x64', nodeFile: 'index.darwin-x64.node' },
  { name: 'darwin-arm64', nodeFile: 'index.darwin-arm64.node' },
  { name: 'win32-x64-msvc', nodeFile: 'index.win32-x64-msvc.node' },
];

const rootDir = path.join(__dirname, '..');
const version = require(path.join(rootDir, 'package.json')).version;

console.log('Publishing @ruvector/graph-node platform packages v' + version + '\n');

for (const platform of platforms) {
  const pkgName = '@ruvector/graph-node-' + platform.name;
  const nodeFile = path.join(rootDir, platform.nodeFile);

  if (!fs.existsSync(nodeFile)) {
    console.log('Skipping ' + pkgName + ' - ' + platform.nodeFile + ' not found');
    continue;
  }

  const tmpDir = path.join(rootDir, 'npm', platform.name);
  fs.mkdirSync(tmpDir, { recursive: true });

  // Create package.json for platform package
  const pkgJson = {
    name: pkgName,
    version: version,
    description: 'RuVector Graph Node.js bindings for ' + platform.name,
    main: 'ruvector-graph.node',
    files: ['ruvector-graph.node'],
    os: platform.name.includes('linux') ? ['linux'] :
        platform.name.includes('darwin') ? ['darwin'] :
        platform.name.includes('win32') ? ['win32'] : [],
    cpu: platform.name.includes('x64') ? ['x64'] :
         platform.name.includes('arm64') ? ['arm64'] : [],
    engines: { node: '>=18.0.0' },
    license: 'MIT',
    repository: {
      type: 'git',
      url: 'https://github.com/ruvnet/ruvector.git',
      directory: 'npm/packages/graph-node'
    },
    publishConfig: { access: 'public' }
  };

  fs.writeFileSync(
    path.join(tmpDir, 'package.json'),
    JSON.stringify(pkgJson, null, 2)
  );

  // Copy the .node file
  fs.copyFileSync(nodeFile, path.join(tmpDir, 'ruvector-graph.node'));

  // Publish
  console.log('Publishing ' + pkgName + '@' + version + '...');
  try {
    execSync('npm publish --access public', { cwd: tmpDir, stdio: 'inherit' });
    console.log('Published ' + pkgName + '@' + version + '\n');
  } catch (e) {
    console.error('Failed to publish ' + pkgName + ': ' + e.message + '\n');
  }

  // Cleanup
  fs.rmSync(tmpDir, { recursive: true, force: true });
}

console.log('Done!');
