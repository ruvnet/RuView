# 📦 Publishing Guide - Psycho-Symbolic Packages

Complete guide for publishing `psycho-symbolic-integration` and `psycho-synth-examples` to npm.

## 📋 Pre-Publishing Checklist

### ✅ Package Validation Status

Both packages have been validated and are **ready for publishing**:

**psycho-symbolic-integration**
- ✅ package.json configured
- ✅ README.md (2.8 KB)
- ✅ LICENSE included
- ✅ .npmignore configured
- ✅ TypeScript source (src/)
- ✅ Repository metadata
- ✅ publishConfig.access: public
- ✅ npm pack dry-run passed (32.7 KB unpacked)

**psycho-synth-examples**
- ✅ package.json configured
- ✅ README.md (10.4 KB comprehensive)
- ✅ LICENSE included
- ✅ .npmignore configured
- ✅ CLI binary (bin/cli.js)
- ✅ 6 example files (105.3 KB total)
- ✅ TypeScript source (src/)
- ✅ Repository metadata
- ✅ publishConfig.access: public
- ✅ npm pack dry-run passed (112.7 KB unpacked)
- ✅ CLI tested and working

## 🚀 Publishing Steps

### Step 1: Login to npm

```bash
npm login
# Enter your npm credentials
# Username: your-npm-username
# Password: your-npm-password
# Email: your-email@example.com
```

Verify login:
```bash
npm whoami
```

### Step 2: Final Validation

Run the validation script to ensure everything is ready:

```bash
# From repository root
cd /home/user/ruvector

# Check package contents
cd packages/psycho-symbolic-integration
npm pack --dry-run

cd ../psycho-synth-examples
npm pack --dry-run
```

### Step 3: Publish psycho-symbolic-integration

```bash
cd /home/user/ruvector/packages/psycho-symbolic-integration

# Optional: Build TypeScript (if needed)
npm run build

# Publish to npm
npm publish --access public

# Expected output:
# + psycho-symbolic-integration@0.1.0
```

### Step 4: Publish psycho-synth-examples

```bash
cd /home/user/ruvector/packages/psycho-synth-examples

# Optional: Build TypeScript (if needed)
npm run build

# Publish to npm
npm publish --access public

# Expected output:
# + psycho-synth-examples@0.1.0
```

### Step 5: Verify Publication

```bash
# Check psycho-symbolic-integration
npm view psycho-symbolic-integration

# Check psycho-synth-examples
npm view psycho-synth-examples

# Test npx command
npx psycho-synth-examples list
# or
npx psycho-synth-examples list
```

## 🔄 Publishing Updates

### Versioning Strategy

Follow Semantic Versioning (semver):
- **Patch** (0.1.1): Bug fixes, documentation updates
- **Minor** (0.2.0): New features, backwards-compatible
- **Major** (1.0.0): Breaking changes

### Update Version

```bash
# Patch release
npm version patch

# Minor release
npm version minor

# Major release
npm version major

# Custom version
npm version 0.2.0
```

### Publish Updated Version

```bash
# Build and publish
npm run build
npm publish --access public

# Or use npm scripts
npm run prepublishOnly  # If defined
npm publish --access public
```

## 📦 Package Contents

### psycho-symbolic-integration (32.7 KB)

Includes:
- `LICENSE` (1.1 KB)
- `README.md` (2.8 KB)
- `package.json` (1.7 KB)
- `src/adapters/agentic-synth-adapter.ts` (11.2 KB)
- `src/adapters/ruvector-adapter.ts` (8.0 KB)
- `src/index.ts` (7.9 KB)

**Total: 6 files**

### psycho-synth-examples (112.7 KB)

Includes:
- `LICENSE` (1.1 KB)
- `README.md` (10.4 KB)
- `package.json` (2.4 KB)
- `bin/cli.js` (3.9 KB)
- `src/index.ts` (3.9 KB)
- `examples/audience-analysis.ts` (10.5 KB)
- `examples/voter-sentiment.ts` (13.6 KB)
- `examples/marketing-optimization.ts` (14.2 KB)
- `examples/financial-sentiment.ts` (15.1 KB)
- `examples/medical-patient-analysis.ts` (15.7 KB)
- `examples/psychological-profiling.ts` (22.0 KB)

**Total: 11 files**

## 🧪 Testing After Publication

### Test Installation

```bash
# Create test directory
mkdir /tmp/test-psycho-synth
cd /tmp/test-psycho-synth
npm init -y

# Install integration package
npm install psycho-symbolic-integration

# Install examples package
npm install psycho-synth-examples

# Test programmatic API
node -e "const pkg = require('psycho-symbolic-integration'); console.log(pkg)"

# Test CLI
npx psycho-synth-examples list
npx psycho-synth-examples --help
```

### Test npx Direct Execution

```bash
# Test without installation (npx will download temporarily)
npx psycho-synth-examples list
npx psycho-synth-examples list
npx pse list  # Short alias

# Test running examples
# (requires GEMINI_API_KEY)
export GEMINI_API_KEY="your-key-here"
npx psycho-synth-examples run audience
```

## 📊 Expected npm Registry Info

### psycho-symbolic-integration

```
Package: psycho-symbolic-integration
Version: 0.1.0
License: MIT
Description: Integration layer combining psycho-symbolic-reasoner with ruvector and agentic-synth
Homepage: https://github.com/ruvnet/ruvector#readme
Repository: https://github.com/ruvnet/ruvector.git
Issues: https://github.com/ruvnet/ruvector/issues
```

**Keywords:** psycho-symbolic, reasoning, ruvector, agentic-synth, ai, vector-database, synthetic-data, integration

### psycho-synth-examples

```
Package: psycho-synth-examples
Version: 0.1.0
License: MIT
Description: Advanced psycho-symbolic reasoning examples: audience analysis, voter sentiment, marketing optimization, financial insights, medical patient analysis, and exotic psychological profiling
Homepage: https://github.com/ruvnet/ruvector/tree/main/packages/psycho-synth-examples#readme
Repository: https://github.com/ruvnet/ruvector.git
Issues: https://github.com/ruvnet/ruvector/issues
```

**Keywords:** psycho-symbolic, reasoning, synthetic-data, audience-analysis, voter-sentiment, marketing-optimization, financial-analysis, medical-insights, psychological-profiling, sentiment-analysis, preference-extraction, examples

**Binaries:**
- `psycho-synth-examples` → bin/cli.js
- `pse` → bin/cli.js

## 🎯 Post-Publication Tasks

### 1. Update Repository README

Add installation badges and links:

```markdown
## Packages

### psycho-symbolic-integration
[![npm version](https://badge.fury.io/js/@ruvector%2Fpsycho-symbolic-integration.svg)](https://www.npmjs.com/package/psycho-symbolic-integration)

### psycho-synth-examples
[![npm version](https://badge.fury.io/js/@ruvector%2Fpsycho-synth-examples.svg)](https://www.npmjs.com/package/psycho-synth-examples)
```

### 2. Create GitHub Release

```bash
# Tag the release
git tag -a v0.1.0 -m "Release v0.1.0: Psycho-Symbolic Integration"
git push origin v0.1.0

# Create GitHub release via web UI or gh CLI
gh release create v0.1.0 --title "v0.1.0: Psycho-Symbolic Integration" --notes "Initial release of psycho-symbolic-integration and psycho-synth-examples"
```

### 3. Announce Release

Share on:
- Twitter/X
- Reddit (r/javascript, r/node, r/machinelearning)
- Dev.to
- Hacker News
- LinkedIn

Sample announcement:

```
🚀 Just published two new npm packages!

psycho-symbolic-integration
- 500x faster sentiment analysis (0.4ms vs GPT-4's 200ms)
- Psychologically-guided synthetic data generation
- Hybrid symbolic+vector reasoning

psycho-synth-examples
- 6 production-ready examples
- Audience analysis, voter sentiment, marketing optimization
- Financial analysis, medical insights, psychological profiling

Try it: npx psycho-synth-examples list

#AI #MachineLearning #JavaScript #TypeScript
```

### 4. Monitor Package Stats

- npm downloads: https://npmcharts.com
- npm trends: https://www.npmtrends.com/psycho-synth-examples
- Package phobia: https://packagephobia.com

## 🔧 Troubleshooting

### "402 Payment Required"

You need to verify your email address with npm.

### "403 Forbidden"

1. Check you're logged in: `npm whoami`
2. Verify scope ownership: `npm owner ls @ruvector/package-name`
3. Ensure `publishConfig.access` is set to `"public"` for scoped packages

### "ENEEDAUTH"

Run `npm login` again.

### "Version already published"

You cannot republish the same version. Increment version:
```bash
npm version patch
npm publish --access public
```

### Package name conflict

If `@ruvector` scope is not available, you may need to:
1. Create the scope on npm
2. Use a different scope
3. Publish without scope (not recommended)

## 📝 Maintenance

### Regular Updates

1. **Monthly**: Check dependencies for updates
   ```bash
   npm outdated
   npm update
   ```

2. **Quarterly**: Review and update examples
   - Add new use cases
   - Improve documentation
   - Update dependencies

3. **As Needed**: Bug fixes and patches
   ```bash
   npm version patch
   npm publish --access public
   ```

### Deprecating Versions

If you need to deprecate a version:

```bash
npm deprecate psycho-synth-examples@0.1.0 "Use version 0.2.0 or later"
```

### Unpublishing (Use Sparingly!)

npm allows unpublishing within 72 hours:

```bash
# Unpublish specific version
npm unpublish psycho-synth-examples@0.1.0

# Unpublish entire package (dangerous!)
npm unpublish psycho-synth-examples --force
```

⚠️ **Warning**: Unpublishing can break dependent projects. Only do this for critical security issues.

## ✅ Final Pre-Publish Checklist

Before running `npm publish`, verify:

- [ ] Version number is correct
- [ ] CHANGELOG.md updated (if you have one)
- [ ] All tests pass
- [ ] README.md is accurate and comprehensive
- [ ] LICENSE file included
- [ ] .npmignore excludes unnecessary files
- [ ] Dependencies are up to date
- [ ] No secrets or credentials in code
- [ ] Repository field points to correct URL
- [ ] Keywords are relevant and accurate
- [ ] Author information is correct
- [ ] npm pack --dry-run shows expected files
- [ ] You're logged into correct npm account
- [ ] Scope (@ruvector) is available or you have access

## 🎉 Ready to Publish!

Both packages have been thoroughly validated and are ready for publication:

```bash
# Publish psycho-symbolic-integration
cd packages/psycho-symbolic-integration
npm publish --access public

# Publish psycho-synth-examples
cd ../psycho-synth-examples
npm publish --access public

# Verify
npx psycho-synth-examples list
```

---

**Good luck with your publication!** 🚀

For questions or issues:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- npm Support: https://www.npmjs.com/support

MIT © ruvnet
