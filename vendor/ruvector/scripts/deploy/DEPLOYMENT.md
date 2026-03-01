# RuVector Deployment Guide

This guide covers the comprehensive deployment process for ruvector using the `deploy.sh` script.

## Prerequisites

### Required Tools

- **Rust toolchain** (rustc, cargo) - v1.77 or later
- **Node.js** - v18 or later
- **npm** - Latest version
- **wasm-pack** - For WASM builds
- **jq** - For JSON manipulation

Install missing tools:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js and npm (using nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18

# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Install jq (Ubuntu/Debian)
sudo apt-get install jq

# Install jq (macOS)
brew install jq
```

### Required Credentials

1. **crates.io API Token**
   - Visit https://crates.io/me
   - Generate a new API token
   - Set as environment variable: `export CRATES_API_KEY="your-token"`

2. **NPM Authentication Token**
   - Login to npm: `npm login`
   - Or create token: `npm token create`
   - Set as environment variable: `export NPM_TOKEN="your-token"`

3. **GitHub Personal Access Token** (Optional, for GitHub Actions)
   - Visit https://github.com/settings/tokens
   - Generate token with `repo` and `workflow` scopes
   - Set as environment variable: `export GITHUB_TOKEN="your-token"`

## Quick Start

### Full Deployment

```bash
# Export required credentials
export CRATES_API_KEY="your-crates-io-token"
export NPM_TOKEN="your-npm-token"

# Run deployment
./scripts/deploy.sh
```

### Dry Run (Test Without Publishing)

```bash
./scripts/deploy.sh --dry-run
```

## Usage Options

### Command-Line Flags

| Flag | Description |
|------|-------------|
| `--dry-run` | Test deployment without publishing |
| `--skip-tests` | Skip test suite execution |
| `--skip-crates` | Skip crates.io publishing |
| `--skip-npm` | Skip NPM publishing |
| `--skip-checks` | Skip clippy and formatting checks |
| `--force` | Skip confirmation prompts |
| `--version VERSION` | Set explicit version (default: read from Cargo.toml) |
| `-h, --help` | Show help message |

### Common Scenarios

**Publish only to crates.io:**
```bash
./scripts/deploy.sh --skip-npm
```

**Publish only to npm:**
```bash
./scripts/deploy.sh --skip-crates
```

**Quick deployment (skip all checks):**
```bash
# ⚠️ Not recommended for production
./scripts/deploy.sh --skip-tests --skip-checks --force
```

**Test deployment process:**
```bash
./scripts/deploy.sh --dry-run
```

**Deploy specific version:**
```bash
./scripts/deploy.sh --version 0.2.0
```

## Deployment Process

The script performs the following steps in order:

### 1. Prerequisites Check
- Verifies required tools (cargo, npm, wasm-pack, jq)
- Checks for required environment variables
- Displays version information

### 2. Version Management
- Reads version from workspace `Cargo.toml`
- Synchronizes version to all `package.json` files
- Updates:
  - Root `package.json`
  - `crates/ruvector-node/package.json`
  - `crates/ruvector-wasm/package.json`
  - All other NPM package manifests

### 3. Pre-Deployment Checks
- **Test Suite**: `cargo test --all`
- **Clippy Linter**: `cargo clippy --all-targets --all-features`
- **Format Check**: `cargo fmt --all -- --check`

### 4. WASM Package Builds
Builds all WASM packages:
- `ruvector-wasm`
- `ruvector-gnn-wasm`
- `ruvector-graph-wasm`
- `ruvector-tiny-dancer-wasm`

### 5. Crate Publishing
Publishes crates to crates.io in dependency order:

**Core crates:**
- `ruvector-core`
- `ruvector-metrics`
- `ruvector-filter`

**Cluster crates:**
- `ruvector-collections`
- `ruvector-snapshot`
- `ruvector-raft`
- `ruvector-cluster`
- `ruvector-replication`

**Graph and GNN:**
- `ruvector-graph`
- `ruvector-gnn`

**Router:**
- `ruvector-router-core`
- `ruvector-router-ffi`
- `ruvector-router-wasm`
- `ruvector-router-cli`

**Tiny Dancer:**
- `ruvector-tiny-dancer-core`
- `ruvector-tiny-dancer-wasm`
- `ruvector-tiny-dancer-node`

**Bindings:**
- `ruvector-node`
- `ruvector-wasm`
- `ruvector-gnn-node`
- `ruvector-gnn-wasm`
- `ruvector-graph-node`
- `ruvector-graph-wasm`

**CLI/Server:**
- `ruvector-cli`
- `ruvector-server`
- `ruvector-bench`

### 6. NPM Publishing
Publishes NPM packages:
- `@ruvector/node`
- `@ruvector/wasm`
- `@ruvector/gnn`
- `@ruvector/gnn-wasm`
- `@ruvector/graph-node`
- `@ruvector/graph-wasm`
- `@ruvector/tiny-dancer`
- `@ruvector/tiny-dancer-wasm`

### 7. GitHub Actions Trigger
Triggers cross-platform native builds (if `GITHUB_TOKEN` set)

## Version Management

### Automatic Version Sync

The script automatically synchronizes versions across all package manifests:

1. Reads version from workspace `Cargo.toml`
2. Updates all `package.json` files
3. Ensures consistency across the monorepo

### Manual Version Update

To bump version manually:

```bash
# 1. Update workspace Cargo.toml
sed -i 's/^version = .*/version = "0.2.0"/' Cargo.toml

# 2. Run deployment (will sync all packages)
./scripts/deploy.sh
```

### Semantic Versioning

Follow [Semantic Versioning](https://semver.org/):
- **MAJOR** (0.x.0): Breaking changes
- **MINOR** (x.1.0): New features, backward compatible
- **PATCH** (x.x.1): Bug fixes, backward compatible

## Troubleshooting

### Common Issues

**1. "CRATES_API_KEY not set"**
```bash
export CRATES_API_KEY="your-token"
```

**2. "NPM_TOKEN not set"**
```bash
export NPM_TOKEN="your-token"
```

**3. "Tests failed"**
```bash
# Run tests manually to see details
cargo test --all --verbose

# Skip tests if needed (not recommended)
./scripts/deploy.sh --skip-tests
```

**4. "Clippy found issues"**
```bash
# Fix clippy warnings
cargo clippy --all-targets --all-features --fix

# Or skip checks (not recommended)
./scripts/deploy.sh --skip-checks
```

**5. "Code formatting issues"**
```bash
# Format code
cargo fmt --all

# Then retry deployment
./scripts/deploy.sh
```

**6. "Crate already published"**

The script automatically skips already-published crates. If you need to publish a new version:
```bash
# Bump version in Cargo.toml
./scripts/deploy.sh --version 0.2.1
```

**7. "WASM build failed"**
```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build manually to see errors
cd crates/ruvector-wasm
wasm-pack build --target web --release
```

### Logs

Deployment logs are saved to `logs/deployment/deploy-YYYYMMDD-HHMMSS.log`

View recent logs:
```bash
ls -lt logs/deployment/
tail -f logs/deployment/deploy-*.log
```

## CI/CD Integration

### GitHub Actions

Create `.github/workflows/deploy.yml`:

```yaml
name: Deploy

on:
  push:
    tags:
      - 'v*'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: 18

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Install jq
        run: sudo apt-get install -y jq

      - name: Deploy
        env:
          CRATES_API_KEY: ${{ secrets.CRATES_API_KEY }}
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: ./scripts/deploy.sh --force
```

### Manual Deployment Checklist

- [ ] All tests passing locally
- [ ] Code formatted (`cargo fmt --all`)
- [ ] No clippy warnings
- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG updated
- [ ] Environment variables set
- [ ] Dry run successful
- [ ] Ready to publish

## Security Best Practices

### Credentials Management

**Never commit credentials to git!**

Use environment variables or secure vaults:

```bash
# Use .env file (add to .gitignore)
cat > .env << EOF
CRATES_API_KEY=your-token
NPM_TOKEN=your-token
GITHUB_TOKEN=your-token
EOF

# Source before deployment
source .env
./scripts/deploy.sh
```

Or use a password manager:
```bash
# Example with pass
export CRATES_API_KEY=$(pass show crates-io/api-key)
export NPM_TOKEN=$(pass show npm/token)
```

## Support

For issues or questions:
- **GitHub Issues**: https://github.com/ruvnet/ruvector/issues
- **Documentation**: https://github.com/ruvnet/ruvector
- **Deployment Logs**: `logs/deployment/`

## License

MIT License - See LICENSE file for details
