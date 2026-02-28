# NPM Publishing Guide - @ruvector/core

**Date:** 2025-11-21
**Status:** ✅ Package Configuration Complete

## 📦 Package Structure

### Main Package: @ruvector/core
Located in `/workspaces/ruvector/npm/core`

```
@ruvector/core/
├── package.json          # Main package with platform detection
├── dist/                 # TypeScript compiled output
│   ├── index.js
│   ├── index.cjs
│   └── index.d.ts
└── platforms/           # Platform-specific binaries
    ├── linux-x64-gnu/
    ├── linux-arm64-gnu/
    ├── darwin-x64/
    ├── darwin-arm64/
    └── win32-x64-msvc/
```

### Platform Package Structure
Each platform package (e.g., `@ruvector/core-linux-x64-gnu`) contains:

```
@ruvector/core-linux-x64-gnu/
├── package.json         # Platform-specific configuration
├── index.js            # Native module loader
├── ruvector.node       # Native binary (4.3MB)
└── README.md           # Platform documentation
```

## 🔧 Package Configuration

### Main package.json (@ruvector/core)
```json
{
  "name": "@ruvector/core",
  "version": "0.1.1",
  "description": "High-performance Rust vector database for Node.js",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "type": "module",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "require": "./dist/index.cjs",
      "types": "./dist/index.d.ts"
    }
  },
  "engines": {
    "node": ">= 18"
  },
  "files": [
    "dist",
    "platforms",
    "native",
    "*.node",
    "README.md",
    "LICENSE"
  ],
  "optionalDependencies": {
    "@ruvector/core-darwin-arm64": "0.1.1",
    "@ruvector/core-darwin-x64": "0.1.1",
    "@ruvector/core-linux-arm64-gnu": "0.1.1",
    "@ruvector/core-linux-x64-gnu": "0.1.1",
    "@ruvector/core-win32-x64-msvc": "0.1.1"
  }
}
```

### Platform package.json (e.g., linux-x64-gnu)
```json
{
  "name": "@ruvector/core-linux-x64-gnu",
  "version": "0.1.1",
  "description": "Linux x64 GNU native binding for @ruvector/core",
  "main": "index.js",
  "type": "commonjs",
  "os": ["linux"],
  "cpu": ["x64"],
  "engines": {
    "node": ">= 18"
  },
  "files": [
    "index.js",
    "ruvector.node",
    "*.node",
    "README.md"
  ]
}
```

## 📋 Pre-Publishing Checklist

### 1. Build Native Binaries ✅
```bash
# Option A: Local build (current platform only)
cd npm/core
npm run build

# Option B: Multi-platform via GitHub Actions
git push origin main
# Workflow: .github/workflows/build-native.yml
```

### 2. Verify Binary Inclusion ✅
```bash
cd npm/core/platforms/linux-x64-gnu
npm pack --dry-run

# Expected output:
# - 4 files total
# - 4.5 MB unpacked size
# - ruvector.node (4.3MB)
# - index.js (330B)
# - package.json (612B)
# - README.md (272B)
```

### 3. Test Package Locally ✅
```bash
cd npm/core
node test-package.js

# Expected output:
# ✅ File structure test PASSED
# ✅ Native module test PASSED
# ✅ Database creation test PASSED
# ✅ Basic operations test PASSED
```

### 4. Update Version Numbers
```bash
# Update all package.json files to same version
npm version patch  # or minor, major
```

## 🚀 Publishing Process

### Step 1: Login to NPM
```bash
# If not already logged in
npm login

# Verify authentication
npm whoami
```

### Step 2: Publish Platform Packages
```bash
# Publish each platform package
cd npm/core/platforms/linux-x64-gnu
npm publish --access public

cd ../linux-arm64-gnu
npm publish --access public

cd ../darwin-x64
npm publish --access public

cd ../darwin-arm64
npm publish --access public

cd ../win32-x64-msvc
npm publish --access public
```

### Step 3: Build Main Package
```bash
cd npm/core
npm run build  # Compile TypeScript
```

### Step 4: Publish Main Package
```bash
npm publish --access public
```

## 🧪 Testing Installation

### Test on Current Platform
```bash
# In a test directory
npm install @ruvector/core

# Create test.js
node -e "
const { VectorDB } = require('@ruvector/core');
const db = new VectorDB({ dimensions: 3 });
console.log('✅ Package installed and working!');
"
```

### Test Platform Detection
```bash
# Should auto-select correct platform package
npm install @ruvector/core

# Verify correct platform loaded
node -e "
const path = require('path');
const pkg = require('@ruvector/core/package.json');
console.log('Platform packages:', Object.keys(pkg.optionalDependencies));
"
```

## 📊 Package Sizes

| Package | Unpacked Size | Compressed Size |
|---------|--------------|-----------------|
| @ruvector/core | ~10 KB | ~3 KB |
| @ruvector/core-linux-x64-gnu | 4.5 MB | 1.9 MB |
| @ruvector/core-linux-arm64-gnu | ~4.5 MB | ~1.9 MB |
| @ruvector/core-darwin-x64 | ~4.5 MB | ~1.9 MB |
| @ruvector/core-darwin-arm64 | ~4.5 MB | ~1.9 MB |
| @ruvector/core-win32-x64-msvc | ~4.5 MB | ~1.9 MB |

**Total when all platforms installed:** ~22 MB unpacked, ~9 MB compressed

**Per-platform install:** ~4.5 MB (only installs matching platform)

## 🔐 Security Notes

1. **Native Binaries**: All .node files are compiled Rust code (safe)
2. **No Postinstall Scripts**: No automatic code execution
3. **Optional Dependencies**: Platforms install only when needed
4. **Scoped Package**: Published under @ruvector namespace

## 🐛 Troubleshooting

### Binary Not Found Error
```
Error: Failed to load native binding for linux-x64-gnu
```

**Solution:**
1. Check platform package is installed: `npm ls @ruvector/core-linux-x64-gnu`
2. Verify binary exists: `ls node_modules/@ruvector/core-linux-x64-gnu/ruvector.node`
3. Reinstall: `npm install --force`

### Wrong Platform Detected
```
Error: Unsupported platform: freebsd-x64
```

**Solution:**
The package only supports: linux (x64/arm64), darwin (x64/arm64), win32 (x64)

### Module Load Failed
```
Error: dlopen failed: cannot open shared object file
```

**Solution:**
- Ensure Node.js >= 18
- Check system dependencies: `ldd ruvector.node`
- May need: glibc 2.31+, libstdc++

## 📈 Maintenance

### Updating Package Version
1. Update version in all package.json files (root + all platforms)
2. Rebuild native binaries with GitHub Actions
3. Test locally with `npm pack --dry-run`
4. Publish platform packages first
5. Publish main package last

### Adding New Platform
1. Add platform to GitHub Actions matrix
2. Create new platform package directory
3. Add to optionalDependencies in main package.json
4. Update platform detection logic
5. Build and publish

## 🔗 Related Documentation

- [Build Process](./BUILD_PROCESS.md) - Multi-platform build details
- [Publishing Complete](./PUBLISHING_COMPLETE.md) - Rust crates on crates.io
- [Phase 2 Complete](./PHASE2_MULTIPLATFORM_COMPLETE.md) - Multi-platform architecture
- [Phase 3 WASM Status](./PHASE3_WASM_STATUS.md) - WebAssembly implementation

## ✅ Verification Commands

```bash
# Verify package contents
npm pack --dry-run

# Check file sizes
du -sh npm/core/platforms/*/ruvector.node

# Test all platforms (if binaries available)
for platform in linux-x64-gnu linux-arm64-gnu darwin-x64 darwin-arm64 win32-x64-msvc; do
  echo "Testing $platform..."
  cd npm/core/platforms/$platform && npm pack --dry-run
  cd -
done

# Verify TypeScript compilation
cd npm/core && npm run build && ls -la dist/
```

## 🎯 Success Criteria

- ✅ All platform packages include 4.3MB+ ruvector.node binary
- ✅ npm pack shows correct file sizes (4.5MB unpacked)
- ✅ Test script passes all 4 tests
- ✅ TypeScript definitions generated
- ✅ Package.json files array includes all required files
- ✅ Platform detection works correctly
- ⏳ Published to npm registry (pending)
- ⏳ Installation tested on all platforms (pending)

---

**Last Updated:** 2025-11-21
**Next Steps:** Publish platform packages to npm registry
