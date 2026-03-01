# Publishing Guide

Complete guide for publishing rUvector packages to npm.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Publishing Process](#publishing-process)
- [Version Management](#version-management)
- [CI/CD Workflow](#cicd-workflow)
- [Manual Publishing](#manual-publishing)
- [Troubleshooting](#troubleshooting)
- [Post-Publication](#post-publication)

## Overview

rUvector uses automated publishing via GitHub Actions. When you push a version tag, the CI/CD pipeline:

1. Builds native binaries for all 5 platforms
2. Publishes platform-specific packages
3. Publishes the main `@ruvector/core` package

### Published Packages

The publishing workflow creates these npm packages:

**Platform-specific packages** (native binaries):
- `ruvector-core-darwin-arm64` - macOS Apple Silicon (M1/M2/M3)
- `ruvector-core-darwin-x64` - macOS Intel
- `ruvector-core-linux-arm64-gnu` - Linux ARM64
- `ruvector-core-linux-x64-gnu` - Linux x64
- `ruvector-core-win32-x64-msvc` - Windows x64

**Main package**:
- `@ruvector/core` - Main package with TypeScript types and platform detection

## Prerequisites

### 1. NPM Authentication

Ensure you have npm publish permissions:

```bash
npm whoami
# Should show your npm username
```

### 2. GitHub Secrets

The repository must have the `NPM_TOKEN` secret configured:

1. Go to **Settings** → **Secrets and variables** → **Actions**
2. Add `NPM_TOKEN` with your npm authentication token
3. Generate token at: https://www.npmjs.com/settings/[username]/tokens

### 3. Git Configuration

```bash
# Verify git user
git config user.name
git config user.email

# Verify remote
git remote -v
```

### 4. Build System Check

All platforms must build successfully:

```bash
# View latest build status
gh run list --workflow "Build Native Modules" --limit 1

# Or check PR directly
gh pr checks <PR-NUMBER>
```

## Publishing Process

### Option 1: Automated Publishing (Recommended)

This is the standard workflow for releases:

#### Step 1: Ensure All Builds Pass

```bash
# Check current build status
gh run list --limit 1
```

All 5 platform builds should be passing:
- ✅ darwin-arm64 (Apple Silicon)
- ✅ darwin-x64 (Intel Mac)
- ✅ linux-arm64-gnu
- ✅ linux-x64-gnu
- ✅ win32-x64-msvc

#### Step 2: Update Version

```bash
# Navigate to package directory
cd npm/core

# Bump version (choose one)
npm version patch   # 0.1.1 -> 0.1.2
npm version minor   # 0.1.1 -> 0.2.0
npm version major   # 0.1.1 -> 1.0.0

# Or manually edit package.json
```

Also update platform package versions to match:

```bash
# Edit npm/core/package.json
# Update optionalDependencies versions to match
```

#### Step 3: Commit Version Bump

```bash
cd /workspaces/ruvector

git add npm/core/package.json npm/package-lock.json
git commit -m "chore: bump version to X.Y.Z"
git push
```

#### Step 4: Create and Push Tag

```bash
# Create annotated tag
git tag vX.Y.Z -a -m "Release vX.Y.Z

- Feature 1
- Feature 2
- Bug fix 3"

# Push tag to trigger publishing
git push origin vX.Y.Z
```

#### Step 5: Monitor Publishing Workflow

```bash
# Watch workflow progress
gh run watch

# Or view in browser
gh run list --limit 1
```

The workflow will:
1. ⏳ Build all 5 platforms (~7 minutes)
2. ⏳ Upload artifacts
3. ⏳ Run publish job
4. ✅ Publish packages to npm

#### Step 6: Verify Publication

```bash
# Check npm registry
npm view @ruvector/core versions
npm view ruvector-core-darwin-arm64 versions

# Or search npm
npm search @ruvector
```

### Option 2: Quick Release from PR

If you have a PR with passing builds:

```bash
# 1. Update version
npm version patch -w npm/core

# 2. Commit and push
git add -A
git commit -m "chore: bump version to X.Y.Z"
git push

# 3. Create and push tag
git tag vX.Y.Z
git push origin vX.Y.Z
```

### Option 3: Release After Merge

```bash
# 1. Merge PR to main
gh pr merge <PR-NUMBER> --squash

# 2. Pull latest
git checkout main
git pull

# 3. Create tag
git tag vX.Y.Z -a -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

## Version Management

### Versioning Strategy

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0): Breaking changes
- **MINOR** (0.1.0): New features, backwards compatible
- **PATCH** (0.0.1): Bug fixes, backwards compatible

### Version Synchronization

**Critical**: All packages must use synchronized versions:

```json
// npm/core/package.json
{
  "version": "0.1.2",
  "optionalDependencies": {
    "ruvector-core-darwin-arm64": "0.1.2",    // ← Must match
    "ruvector-core-darwin-x64": "0.1.2",      // ← Must match
    "ruvector-core-linux-arm64-gnu": "0.1.2", // ← Must match
    "ruvector-core-linux-x64-gnu": "0.1.2",   // ← Must match
    "ruvector-core-win32-x64-msvc": "0.1.2"   // ← Must match
  }
}
```

### Pre-release Versions

For beta/alpha releases:

```bash
# Create pre-release version
npm version prerelease --preid=beta
# 0.1.1 -> 0.1.2-beta.0

git tag v0.1.2-beta.0
git push origin v0.1.2-beta.0
```

## CI/CD Workflow

### Workflow Triggers

The build workflow (`.github/workflows/build-native.yml`) triggers on:

1. **Pull Requests** to main
   - Builds all platforms
   - Does NOT publish

2. **Pushes** to main
   - Builds all platforms
   - Does NOT publish

3. **Tags** matching `v*`
   - Builds all platforms
   - **PUBLISHES** to npm ✅

### Build Matrix

The workflow builds for 5 platforms in parallel:

```yaml
matrix:
  settings:
    - host: ubuntu-22.04
      platform: linux-x64-gnu

    - host: ubuntu-22.04
      platform: linux-arm64-gnu

    - host: macos-15-intel
      platform: darwin-x64

    - host: macos-14
      platform: darwin-arm64

    - host: windows-2022
      platform: win32-x64-msvc
```

### Publish Job

The publish job runs **only on tags**:

```yaml
publish:
  needs: build
  if: startsWith(github.ref, 'refs/tags/v')
  steps:
    - Download all artifacts
    - Copy binaries to platform packages
    - Publish platform packages
    - Publish main package
```

### Workflow Files

```
.github/workflows/
├── build-native.yml      # Main build & publish workflow
└── agentic-synth-ci.yml  # Additional CI checks
```

## Manual Publishing

If you need to publish manually (not recommended):

### Prerequisites

```bash
# Login to npm
npm login

# Verify authentication
npm whoami
```

### Build All Platforms

You'll need access to all 5 platforms or use cross-compilation:

```bash
# Linux x64
cargo build --release --target x86_64-unknown-linux-gnu

# Linux ARM64
cargo build --release --target aarch64-unknown-linux-gnu

# macOS Intel
cargo build --release --target x86_64-apple-darwin

# macOS ARM64
cargo build --release --target aarch64-apple-darwin

# Windows
cargo build --release --target x86_64-pc-windows-msvc
```

### Publish Platform Packages

```bash
cd npm/core/platforms

# Publish each platform
cd darwin-arm64 && npm publish --access public
cd ../darwin-x64 && npm publish --access public
cd ../linux-arm64-gnu && npm publish --access public
cd ../linux-x64-gnu && npm publish --access public
cd ../win32-x64-msvc && npm publish --access public
```

### Publish Main Package

```bash
cd npm/core
npm run build  # Compile TypeScript
npm publish --access public
```

## Troubleshooting

### Build Failures

#### Issue: Version Mismatch Error

```
npm error Invalid: lock file's ruvector-core-darwin-arm64@0.1.1
does not satisfy ruvector-core-darwin-arm64@0.1.2
```

**Solution**: Synchronize versions in `npm/core/package.json`:

```bash
# Update optionalDependencies to match platform package versions
cd npm
npm install
git add package-lock.json
git commit -m "fix: sync package versions"
```

#### Issue: macOS Build Failures

```
Error: macos-13 runner deprecated
```

**Solution**: Update workflow to use `macos-15-intel`:

```yaml
- host: macos-15-intel  # Not macos-13
  platform: darwin-x64
```

#### Issue: Windows PowerShell Errors

```
Could not find a part of the path 'D:\dev\null'
```

**Solution**: Add `shell: bash` to Windows-compatible steps:

```yaml
- name: Find built .node files
  shell: bash  # ← Add this
  run: |
    find . -name "*.node" 2>/dev/null || true
```

### Publishing Failures

#### Issue: NPM Authentication Failed

```
npm error code ENEEDAUTH
npm error need auth This command requires you to be logged in
```

**Solution**:
1. Verify `NPM_TOKEN` secret in GitHub repository settings
2. Regenerate token if expired: https://www.npmjs.com/settings/[username]/tokens
3. Update repository secret

#### Issue: Permission Denied

```
npm error code E403
npm error 403 Forbidden - PUT https://registry.npmjs.org/@ruvector%2fcore
```

**Solution**:
1. Verify you have publish permissions for `@ruvector` scope
2. Contact npm org admin to grant permissions
3. Ensure package name is available on npm

#### Issue: Build Artifacts Not Found

```
Error: No files were found with the provided path: npm/core/platforms/*/
```

**Solution**:
1. Check that all 5 builds completed successfully
2. Verify artifact upload step succeeded
3. Check artifact names match expected pattern: `bindings-{platform}`

### Version Issues

#### Issue: Tag Already Exists

```
error: tag 'v0.1.2' already exists
```

**Solution**:
```bash
# Delete local tag
git tag -d v0.1.2

# Delete remote tag
git push origin :refs/tags/v0.1.2

# Create tag again
git tag v0.1.2
git push origin v0.1.2
```

#### Issue: Wrong Version Published

If you published the wrong version:

```bash
# Deprecate the bad version
npm deprecate @ruvector/core@0.1.2 "Incorrect version, use 0.1.3"

# Unpublish (only within 72 hours)
npm unpublish @ruvector/core@0.1.2

# Publish correct version
git tag v0.1.3
git push origin v0.1.3
```

## Post-Publication

### Verify Published Packages

```bash
# Check versions
npm view @ruvector/core versions
npm view ruvector-core-darwin-arm64 versions

# Test installation
npm create vite@latest test-project
cd test-project
npm install @ruvector/core
npm run dev
```

### Update Documentation

After publishing:

1. Update `CHANGELOG.md` with release notes
2. Update `README.md` version badges
3. Create GitHub Release with notes
4. Update documentation site if applicable

### Create GitHub Release

```bash
# Using gh CLI
gh release create v0.1.2 \
  --title "Release v0.1.2" \
  --notes "## Changes

- Feature 1
- Feature 2
- Bug fix 3

**Full Changelog**: https://github.com/ruvnet/ruvector/compare/v0.1.1...v0.1.2"

# Or create manually at:
# https://github.com/ruvnet/ruvector/releases/new
```

### Announce Release

- Post on social media
- Update project website
- Notify users on Discord/Slack
- Send newsletter if applicable

## Release Checklist

Use this checklist for each release:

```markdown
## Pre-Release
- [ ] All tests passing
- [ ] All 5 platform builds passing
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in package.json
- [ ] Version synchronized across all packages

## Release
- [ ] Version committed and pushed
- [ ] Git tag created
- [ ] Tag pushed to trigger workflow
- [ ] Workflow completed successfully
- [ ] Packages published to npm

## Post-Release
- [ ] Verified packages on npm
- [ ] Test installation works
- [ ] GitHub Release created
- [ ] Documentation updated
- [ ] Announced release
```

## Additional Resources

- [npm Publishing Documentation](https://docs.npmjs.com/cli/v8/commands/npm-publish)
- [Semantic Versioning](https://semver.org/)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [NAPI-RS Documentation](https://napi.rs/)
- [Rust Cross-Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)

## Support

If you encounter issues not covered in this guide:

1. Check [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
2. Review [GitHub Actions Logs](https://github.com/ruvnet/ruvector/actions)
3. Ask in [Discussions](https://github.com/ruvnet/ruvector/discussions)
4. Contact maintainers

---

Last Updated: 2025-11-25
Version: 1.0.0
