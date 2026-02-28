# RuVector Automation Scripts

This directory contains automation scripts organized by purpose.

## 📁 Directory Structure

```
scripts/
├── README.md           # This file
├── benchmark/          # Performance benchmarking
├── build/              # Build utilities
├── ci/                 # CI/CD automation
├── deploy/             # Deployment scripts
├── patches/            # Patch files
├── publish/            # Package publishing
├── test/               # Testing scripts
└── validate/           # Validation & verification
```

## 🚀 Deployment

Scripts for deploying to production.

| Script | Description |
|--------|-------------|
| `deploy/deploy.sh` | Comprehensive deployment (crates.io + npm) |
| `deploy/test-deploy.sh` | Test deployment without publishing |
| `deploy/DEPLOYMENT.md` | Full deployment documentation |
| `deploy/DEPLOYMENT-QUICKSTART.md` | Quick deployment guide |

**Usage:**
```bash
# Full deployment
./scripts/deploy/deploy.sh

# Dry run
./scripts/deploy/deploy.sh --dry-run

# Test deployment
./scripts/deploy/test-deploy.sh
```

## 📦 Publishing

Scripts for publishing packages to registries.

| Script | Description |
|--------|-------------|
| `publish/publish-all.sh` | Publish all packages |
| `publish/publish-crates.sh` | Publish Rust crates to crates.io |
| `publish/publish-cli.sh` | Publish CLI package |
| `publish/publish-router-wasm.sh` | Publish router WASM package |
| `publish/check-and-publish-router-wasm.sh` | Check and publish router WASM |

**Usage:**
```bash
# Set credentials first
export CRATES_API_KEY="your-crates-io-token"
export NPM_TOKEN="your-npm-token"

# Publish all
./scripts/publish/publish-all.sh

# Publish crates only
./scripts/publish/publish-crates.sh
```

## 📊 Benchmarking

Performance benchmarking scripts.

| Script | Description |
|--------|-------------|
| `benchmark/run_benchmarks.sh` | Run core benchmarks |
| `benchmark/run_llm_benchmarks.sh` | Run LLM inference benchmarks |

**Usage:**
```bash
# Run core benchmarks
./scripts/benchmark/run_benchmarks.sh

# Run LLM benchmarks
./scripts/benchmark/run_llm_benchmarks.sh
```

## 🧪 Testing

Testing and validation scripts.

| Script | Description |
|--------|-------------|
| `test/test-wasm.mjs` | Test WASM bindings |
| `test/test-graph-cli.sh` | Test graph CLI commands |
| `test/test-all-graph-commands.sh` | Test all graph commands |
| `test/test-docker-package.sh` | Test Docker packaging |

**Usage:**
```bash
# Test WASM
node ./scripts/test/test-wasm.mjs

# Test graph CLI
./scripts/test/test-graph-cli.sh
```

## ✅ Validation

Package and build verification scripts.

| Script | Description |
|--------|-------------|
| `validate/validate-packages.sh` | Validate package configs |
| `validate/validate-packages-simple.sh` | Simple package validation |
| `validate/verify-paper-impl.sh` | Verify paper implementation |
| `validate/verify_hnsw_build.sh` | Verify HNSW build |

**Usage:**
```bash
# Validate packages
./scripts/validate/validate-packages.sh

# Verify HNSW
./scripts/validate/verify_hnsw_build.sh
```

## 🔄 CI/CD

Continuous integration scripts.

| Script | Description |
|--------|-------------|
| `ci/ci-sync-lockfile.sh` | Auto-fix lock files in CI |
| `ci/sync-lockfile.sh` | Sync package-lock.json |
| `ci/install-hooks.sh` | Install git hooks |

**Usage:**
```bash
# Install git hooks (recommended)
./scripts/ci/install-hooks.sh

# Sync lockfile
./scripts/ci/sync-lockfile.sh
```

## 🛠️ Build

Build utility scripts located in `build/`.

## 🩹 Patches

Patch files for dependencies located in `patches/`.

## 🚀 Quick Start

### For Development

1. **Install git hooks** (recommended):
   ```bash
   ./scripts/ci/install-hooks.sh
   ```

2. **Run tests**:
   ```bash
   ./scripts/test/test-wasm.mjs
   ```

### For Deployment

1. **Set credentials**:
   ```bash
   export CRATES_API_KEY="your-crates-io-token"
   export NPM_TOKEN="your-npm-token"
   ```

2. **Dry run first**:
   ```bash
   ./scripts/deploy/deploy.sh --dry-run
   ```

3. **Deploy**:
   ```bash
   ./scripts/deploy/deploy.sh
   ```

## 🔐 Security

**Never commit credentials!** Always use environment variables or `.env` file.

See [deploy/DEPLOYMENT.md](deploy/DEPLOYMENT.md) for security best practices.
