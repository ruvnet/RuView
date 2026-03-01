# RuVector Release Pipeline Documentation

## Overview

The RuVector release pipeline is a comprehensive CI/CD workflow that automates the building, testing, and publishing of Rust crates and npm packages across multiple platforms.

## Workflow Files

- **`release.yml`**: Main release pipeline workflow
- **`build-native.yml`**: Reusable workflow for building native Node.js modules
- **`validate-lockfile.yml`**: Validates package-lock.json integrity

## Trigger Methods

### 1. Tag-Based Release (Recommended)

```bash
# Create and push a version tag
git tag v0.1.3
git push origin v0.1.3
```

This automatically triggers the full release pipeline.

### 2. Manual Workflow Dispatch

Navigate to: **Actions → Release Pipeline → Run workflow**

Options:
- **Version**: Version to release (e.g., `0.1.3`)
- **Skip Tests**: Skip validation tests (not recommended)
- **Dry Run**: Build everything but don't publish

## Pipeline Stages

### Stage 1: Validation (`validate`)

**Runs on**: `ubuntu-22.04`

**Tasks**:
- ✅ Check code formatting with `cargo fmt`
- ✅ Run Clippy lints with all warnings as errors
- ✅ Run Rust test suite across all crates
- ✅ Run npm unit tests
- ✅ Generate validation summary

**Skip condition**: Set `skip_tests: true` in manual workflow dispatch

### Stage 2: Build Rust Crates (`build-crates`)

**Runs on**: `ubuntu-22.04`

**Tasks**:
- Build all workspace crates in release mode
- Run crate-specific tests
- Generate build summary with all crate versions

**Crates built** (26 total):
- Core: `ruvector-core`, `ruvector-metrics`, `ruvector-filter`
- Graph: `ruvector-graph`, `ruvector-gnn`
- Distributed: `ruvector-cluster`, `ruvector-raft`, `ruvector-replication`
- Bindings: `ruvector-node`, `ruvector-wasm`
- And 16 more specialized crates

### Stage 3: Build WASM Packages (`build-wasm`)

**Runs on**: `ubuntu-22.04`

**Tasks**:
- Install `wasm-pack` build tool
- Build WASM packages for:
  - `ruvector-wasm` (core WASM)
  - `ruvector-gnn-wasm` (graph neural networks)
  - `ruvector-graph-wasm` (graph database)
  - `ruvector-tiny-dancer-wasm` (tiny dancer)
- Upload WASM artifacts for later stages

**Caching**:
- Rust dependencies via `Swatinem/rust-cache`
- wasm-pack binary

### Stage 4: Build Native Modules (`build-native`)

**Runs on**: Multi-platform matrix

**Reuses**: `./.github/workflows/build-native.yml` as callable workflow

**Platforms built**:
- Linux x64 (GNU) - `ubuntu-22.04`
- Linux ARM64 (GNU) - `ubuntu-22.04` with cross-compilation
- macOS x64 (Intel) - `macos-13`
- macOS ARM64 (Apple Silicon) - `macos-14`
- Windows x64 (MSVC) - `windows-2022`

**Build matrix details**:
```yaml
- host: ubuntu-22.04, target: x86_64-unknown-linux-gnu
- host: ubuntu-22.04, target: aarch64-unknown-linux-gnu
- host: macos-13, target: x86_64-apple-darwin
- host: macos-14, target: aarch64-apple-darwin
- host: windows-2022, target: x86_64-pc-windows-msvc
```

**Output**: Binary artifacts for each platform uploaded to GitHub Actions

### Stage 5: Publish Rust Crates (`publish-crates`)

**Runs on**: `ubuntu-22.04`

**Requires**:
- ✅ Validation passed
- ✅ Build crates succeeded
- 🔑 `CARGO_REGISTRY_TOKEN` secret configured
- Tag starts with `v*` OR manual workflow dispatch
- NOT in dry-run mode

**Publishing order** (respects dependencies):

```
1. ruvector-core (foundation)
2. ruvector-metrics, ruvector-filter, ruvector-snapshot
3. ruvector-collections, ruvector-router-core
4. ruvector-raft, ruvector-cluster, ruvector-replication
5. ruvector-gnn, ruvector-graph
6. ruvector-server, ruvector-tiny-dancer-core
7. ruvector-router-cli, ruvector-router-ffi, ruvector-router-wasm
8. ruvector-cli, ruvector-bench
9. ruvector-wasm, ruvector-node
10. ruvector-gnn-wasm, ruvector-gnn-node
11. ruvector-graph-wasm, ruvector-graph-node
12. ruvector-tiny-dancer-wasm, ruvector-tiny-dancer-node
```

**Rate limiting**: 10 second delay between publishes to avoid crates.io rate limits

**Error handling**: Continues if a crate already exists (409 error)

### Stage 6: Publish npm Packages (`publish-npm`)

**Runs on**: `ubuntu-22.04`

**Requires**:
- ✅ Validation passed
- ✅ Build native succeeded
- ✅ Build WASM succeeded
- 🔑 `NPM_TOKEN` secret configured
- Tag starts with `v*` OR manual workflow dispatch
- NOT in dry-run mode

**Publishing order**:

```
1. Platform-specific packages (@ruvector/core-*)
   - @ruvector/core-linux-x64-gnu
   - @ruvector/core-linux-arm64-gnu
   - @ruvector/core-darwin-x64
   - @ruvector/core-darwin-arm64
   - @ruvector/core-win32-x64-msvc

2. @ruvector/wasm (WebAssembly bindings)
3. @ruvector/cli (Command-line interface)
4. @ruvector/extensions (Extensions)
5. @ruvector/core (Main package - depends on platform packages)
```

**Artifact handling**:
- Downloads native binaries from `build-native` job
- Downloads WASM packages from `build-wasm` job
- Copies to appropriate package directories
- Runs `npm ci` and `npm run build`
- Publishes with `--access public`

### Stage 7: Create GitHub Release (`create-release`)

**Runs on**: `ubuntu-22.04`

**Requires**:
- ✅ All build jobs succeeded
- Tag starts with `v*` OR manual workflow dispatch

**Tasks**:

1. **Download all artifacts**
   - Native binaries for all platforms
   - WASM packages

2. **Package artifacts**
   - `ruvector-native-linux-x64-gnu.tar.gz`
   - `ruvector-native-linux-arm64-gnu.tar.gz`
   - `ruvector-native-darwin-x64.tar.gz`
   - `ruvector-native-darwin-arm64.tar.gz`
   - `ruvector-native-win32-x64-msvc.tar.gz`
   - `ruvector-wasm.tar.gz`

3. **Generate release notes**
   - What's new section
   - Package lists (Rust crates and npm)
   - Platform support matrix
   - Installation instructions
   - Links to registries
   - Build metrics

4. **Create GitHub release**
   - Uses `softprops/action-gh-release@v1`
   - Attaches packaged artifacts
   - Marks as prerelease if version contains `alpha` or `beta`

### Stage 8: Release Summary (`release-summary`)

**Runs on**: `ubuntu-22.04`

**Always runs**: Even if previous jobs fail

**Tasks**:
- Generate comprehensive status table
- Show success/failure for each job
- Provide next steps and verification links

## Required Secrets

### CARGO_REGISTRY_TOKEN

**Purpose**: Publish Rust crates to crates.io

**Setup**:
1. Go to https://crates.io/settings/tokens
2. Create new token with `publish-new` and `publish-update` scopes
3. Add to GitHub: **Settings → Secrets → Actions → New secret**
   - Name: `CARGO_REGISTRY_TOKEN`
   - Value: Your crates.io token

### NPM_TOKEN

**Purpose**: Publish npm packages to npmjs.com

**Setup**:
1. Login to npmjs.com
2. Go to **Access Tokens → Generate New Token**
3. Select **Automation** type
4. Add to GitHub: **Settings → Secrets → Actions → New secret**
   - Name: `NPM_TOKEN`
   - Value: Your npm token

## Environments

The workflow uses GitHub Environments for additional security:

### `crates-io` Environment
- Used for `publish-crates` job
- Can add required reviewers
- Can add environment-specific secrets

### `npm` Environment
- Used for `publish-npm` job
- Can add required reviewers
- Can add environment-specific secrets

**Setup environments**:
1. Go to **Settings → Environments**
2. Create `crates-io` and `npm` environments
3. (Optional) Add required reviewers for production releases

## Caching Strategy

### Rust Cache
```yaml
uses: Swatinem/rust-cache@v2
with:
  prefix-key: 'v1-rust'
  shared-key: 'validate|build-crates|wasm'
```

**Caches**:
- `~/.cargo/registry`
- `~/.cargo/git`
- `target/` directory

**Benefits**: 2-5x faster builds

### Node.js Cache
```yaml
uses: actions/setup-node@v4
with:
  cache: 'npm'
  cache-dependency-path: npm/package-lock.json
```

**Caches**: `~/.npm` directory

## Build Matrix

The native build job uses a strategic matrix to cover all platforms:

| Platform | Host Runner | Rust Target | NAPI Platform | Cross-Compile |
|----------|-------------|-------------|---------------|---------------|
| Linux x64 | ubuntu-22.04 | x86_64-unknown-linux-gnu | linux-x64-gnu | No |
| Linux ARM64 | ubuntu-22.04 | aarch64-unknown-linux-gnu | linux-arm64-gnu | Yes (gcc-aarch64) |
| macOS Intel | macos-13 | x86_64-apple-darwin | darwin-x64 | No |
| macOS ARM | macos-14 | aarch64-apple-darwin | darwin-arm64 | No |
| Windows | windows-2022 | x86_64-pc-windows-msvc | win32-x64-msvc | No |

## Artifact Retention

- **Native binaries**: 7 days
- **WASM packages**: 7 days
- **Release packages**: Permanent (attached to GitHub release)

## Common Scenarios

### Regular Release

```bash
# 1. Update versions in Cargo.toml files
# 2. Update npm package.json files
# 3. Commit changes
git add .
git commit -m "chore: Bump version to 0.1.3"

# 4. Create and push tag
git tag v0.1.3
git push origin main
git push origin v0.1.3

# 5. Monitor workflow at:
# https://github.com/ruvnet/ruvector/actions/workflows/release.yml
```

### Dry Run (Test Release)

1. Go to **Actions → Release Pipeline**
2. Click **Run workflow**
3. Set:
   - Version: `0.1.3-test`
   - Dry run: `true`
4. Click **Run workflow**

This builds everything but skips publishing.

### Emergency Hotfix

```bash
# 1. Create hotfix branch
git checkout -b hotfix/critical-fix

# 2. Make fixes
# 3. Bump patch version
# 4. Commit and tag
git commit -m "fix: Critical security patch"
git tag v0.1.3-hotfix.1
git push origin hotfix/critical-fix
git push origin v0.1.3-hotfix.1

# 5. Manually trigger release workflow if needed
```

### Republish Failed Package

If a single npm package fails to publish:

```bash
# 1. Check error in workflow logs
# 2. Fix issue locally
# 3. Manually publish that package:
cd npm/packages/wasm
npm publish --access public

# Or trigger just the npm publishing:
# Manually run workflow_dispatch with skip_tests: true
```

## Troubleshooting

### Build Failures

**Symptom**: `build-crates` job fails

**Solutions**:
1. Check Rust version compatibility
2. Verify all dependencies are available
3. Look for compilation errors in logs
4. Test locally: `cargo build --workspace --release`

### Publishing Failures

**Symptom**: `publish-crates` or `publish-npm` fails

**Solutions**:

1. **Rate limiting**:
   - Wait and re-run workflow
   - Increase delay between publishes

2. **Already published**:
   - Bump version number
   - Or skip that package (it's already live)

3. **Authentication**:
   - Verify secrets are set correctly
   - Check token hasn't expired
   - Verify token has correct permissions

4. **Dependency issues**:
   - Check publishing order
   - Ensure dependencies are published first

### Cross-Compilation Issues

**Symptom**: Linux ARM64 build fails

**Solutions**:
1. Verify cross-compilation tools installed
2. Check linker configuration
3. Test with: `cargo build --target aarch64-unknown-linux-gnu`

### WASM Build Issues

**Symptom**: `build-wasm` job fails

**Solutions**:
1. Verify `wasm-pack` installation
2. Check for incompatible dependencies
3. Ensure `wasm32-unknown-unknown` target installed
4. Test locally: `wasm-pack build --target nodejs`

## Performance Optimization

### Parallel Builds

The workflow runs these jobs in parallel:
- `build-crates`
- `build-wasm`
- `build-native` (5 platform builds in parallel)

Total time: ~15-25 minutes (vs. 60+ minutes sequential)

### Cache Hit Rates

With proper caching:
- Rust builds: 70-90% cache hit rate
- npm installs: 90-95% cache hit rate

### Build Time Breakdown

| Job | Uncached | Cached |
|-----|----------|--------|
| Validate | 8-12 min | 3-5 min |
| Build Crates | 15-20 min | 5-8 min |
| Build WASM | 10-15 min | 4-6 min |
| Build Native (per platform) | 8-12 min | 3-5 min |
| Publish Crates | 5-10 min | 5-10 min |
| Publish npm | 3-5 min | 2-3 min |
| Create Release | 2-3 min | 2-3 min |

**Total (worst case)**: ~25-30 minutes with cache
**Total (cold start)**: ~45-60 minutes without cache

## Best Practices

1. **Always test locally first**
   ```bash
   cargo test --workspace
   cargo build --workspace --release
   cd npm && npm run build
   ```

2. **Use semantic versioning**
   - MAJOR.MINOR.PATCH (e.g., 0.1.3)
   - Breaking changes: bump MAJOR
   - New features: bump MINOR
   - Bug fixes: bump PATCH

3. **Write clear commit messages**
   ```bash
   feat: Add new vector search capability
   fix: Resolve memory leak in HNSW index
   chore: Bump dependencies
   ```

4. **Review workflow logs**
   - Check for warnings
   - Verify all tests passed
   - Confirm all packages published

5. **Update CHANGELOG.md**
   - Document breaking changes
   - List new features
   - Mention bug fixes

## Monitoring and Alerts

### GitHub Actions Notifications

1. Go to **Settings → Notifications**
2. Enable: "Actions - Only notify for failed workflows"

### Slack/Discord Integration

Add webhook to workflow:

```yaml
- name: Notify Slack
  if: failure()
  uses: slackapi/slack-github-action@v1
  with:
    webhook-url: ${{ secrets.SLACK_WEBHOOK }}
    payload: |
      {
        "text": "Release failed: ${{ github.ref }}"
      }
```

## Version Management

### Cargo.toml Versions

All crates use workspace version:

```toml
[workspace.package]
version = "0.1.2"
```

Update once in root `Cargo.toml`, applies to all crates.

### package.json Versions

Update independently:
- `npm/packages/core/package.json`
- `npm/packages/wasm/package.json`
- `npm/packages/cli/package.json`

Or use `npm version`:
```bash
cd npm/packages/core
npm version patch  # 0.1.2 -> 0.1.3
```

## Security Considerations

1. **Secrets**: Never log or expose `CARGO_REGISTRY_TOKEN` or `NPM_TOKEN`
2. **Branch protection**: Require reviews for version tags
3. **Environment protection**: Add reviewers for production environments
4. **Dependency scanning**: Enabled via GitHub security features
5. **Code signing**: Consider GPG signing for releases

## Future Enhancements

- [ ] Add code signing for native binaries
- [ ] Implement changelog generation from commits
- [ ] Add performance benchmarks to release notes
- [ ] Create Docker images as release artifacts
- [ ] Add automatic version bumping
- [ ] Implement release candidate (RC) workflow
- [ ] Add rollback capabilities
- [ ] Create platform-specific installers
- [ ] Add integration tests for published packages
- [ ] Implement canary releases

## Support

- **Issues**: https://github.com/ruvnet/ruvector/issues
- **Discussions**: https://github.com/ruvnet/ruvector/discussions
- **Documentation**: https://github.com/ruvnet/ruvector

## License

This workflow is part of the RuVector project and follows the same MIT license.
