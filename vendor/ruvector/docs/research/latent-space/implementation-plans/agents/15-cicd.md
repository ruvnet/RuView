# Agent 15: CI/CD Pipeline Implementation

**Agent**: CI/CD Engineer
**Focus**: Complete GitHub Actions workflows for testing, benchmarking, and release automation
**Status**: Ready for Implementation

## Overview

This document provides complete CI/CD pipeline configuration for RuVector, covering:
- Continuous integration testing across platforms
- Automated releases to multiple package registries
- Daily performance benchmarking
- Quality gates and dependency management

## 1. CI Workflow - Test on Every PR

**File**: `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    name: Test - ${{ matrix.os }} - ${{ matrix.arch }} - Node ${{ matrix.node }}
    runs-on: ${{ matrix.os }}

    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        arch: [x64, arm64]
        node: [18, 20, 22]
        exclude:
          # Windows ARM64 runners not widely available
          - os: windows-latest
            arch: arm64
          # Ubuntu ARM64 requires special runners
          - os: ubuntu-latest
            arch: arm64

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Setup Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          cache-on-failure: true

      - name: Setup Node.js ${{ matrix.node }}
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node }}
          cache: 'npm'
          architecture: ${{ matrix.arch }}

      - name: Install dependencies
        run: npm ci

      - name: Run Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Run Rust tests
        run: cargo test --all-features --verbose

      - name: Build NAPI bindings
        run: npm run build
        env:
          CARGO_PROFILE_RELEASE_LTO: 'false'

      - name: Run Node.js tests
        run: npm test

      - name: Generate coverage
        if: matrix.os == 'ubuntu-latest' && matrix.node == '20' && matrix.arch == 'x64'
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --output-dir ./coverage

      - name: Upload coverage to Codecov
        if: matrix.os == 'ubuntu-latest' && matrix.node == '20' && matrix.arch == 'x64'
        uses: codecov/codecov-action@v4
        with:
          files: ./coverage/cobertura.xml
          flags: unittests
          fail_ci_if_error: true
          token: ${{ secrets.CODECOV_TOKEN }}

      - name: Check coverage threshold
        if: matrix.os == 'ubuntu-latest' && matrix.node == '20' && matrix.arch == 'x64'
        run: |
          COVERAGE=$(cargo tarpaulin --out Json | jq -r '.files | map(.coverage) | add / length')
          echo "Coverage: $COVERAGE%"
          if (( $(echo "$COVERAGE < 80" | bc -l) )); then
            echo "Coverage $COVERAGE% is below threshold 80%"
            exit 1
          fi

  lint:
    name: Lint and Type Check
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run ESLint
        run: npm run lint
        continue-on-error: true

      - name: Run TypeScript check
        run: npm run typecheck

  security:
    name: Security Audit
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run cargo audit
        run: |
          cargo install cargo-audit
          cargo audit

      - name: Run npm audit
        run: npm audit --audit-level=moderate

  wasm:
    name: WASM Build
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Build WASM
        run: wasm-pack build --target web --out-dir pkg/wasm

      - name: Test WASM
        run: wasm-pack test --headless --firefox

  quality-gate:
    name: Quality Gate
    runs-on: ubuntu-latest
    needs: [test, lint, security, wasm]
    if: always()

    steps:
      - name: Check job results
        run: |
          if [[ "${{ needs.test.result }}" != "success" ]] || \
             [[ "${{ needs.lint.result }}" != "success" ]] || \
             [[ "${{ needs.security.result }}" != "success" ]] || \
             [[ "${{ needs.wasm.result }}" != "success" ]]; then
            echo "Quality gate failed"
            exit 1
          fi
          echo "Quality gate passed"
```

## 2. Release Workflow - Publish on Tag

**File**: `.github/workflows/release.yml`

```yaml
name: Release

on:
  push:
    tags:
      - 'v*.*.*'

permissions:
  contents: write
  packages: write

env:
  CARGO_TERM_COLOR: always

jobs:
  create-release:
    name: Create Release
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
      version: ${{ steps.get_version.outputs.version }}

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Get version
        id: get_version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Generate changelog
        id: changelog
        run: |
          PREV_TAG=$(git describe --abbrev=0 --tags $(git rev-list --tags --skip=1 --max-count=1) 2>/dev/null || echo "")
          if [ -z "$PREV_TAG" ]; then
            CHANGELOG=$(git log --pretty=format:"- %s (%h)" --reverse)
          else
            CHANGELOG=$(git log ${PREV_TAG}..HEAD --pretty=format:"- %s (%h)" --reverse)
          fi
          echo "changelog<<EOF" >> $GITHUB_OUTPUT
          echo "$CHANGELOG" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        id: create_release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Release v${{ steps.get_version.outputs.version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.changelog }}

            ## Downloads
            - Cargo: `cargo install ruvector`
            - npm: `npm install ruvector`
            - WASM: Available in release assets
          draft: false
          prerelease: false

  build-binaries:
    name: Build - ${{ matrix.target }}
    needs: create-release
    runs-on: ${{ matrix.os }}

    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            node: '20'
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            node: '20'
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            node: '20'
          - os: macos-latest
            target: x86_64-apple-darwin
            node: '20'
          - os: macos-latest
            target: aarch64-apple-darwin
            node: '20'

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node }}
          cache: 'npm'

      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu

      - name: Install dependencies
        run: npm ci

      - name: Build
        run: npm run build -- --target ${{ matrix.target }}
        env:
          CARGO_PROFILE_RELEASE_LTO: 'true'

      - name: Package binary
        id: package
        run: |
          ARCHIVE_NAME="ruvector-${{ needs.create-release.outputs.version }}-${{ matrix.target }}"
          if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
            7z a ${ARCHIVE_NAME}.zip ./target/${{ matrix.target }}/release/*.node
            echo "archive=${ARCHIVE_NAME}.zip" >> $GITHUB_OUTPUT
          else
            tar czf ${ARCHIVE_NAME}.tar.gz -C ./target/${{ matrix.target }}/release *.node
            echo "archive=${ARCHIVE_NAME}.tar.gz" >> $GITHUB_OUTPUT
          fi

      - name: Upload Release Asset
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./${{ steps.package.outputs.archive }}
          asset_name: ${{ steps.package.outputs.archive }}
          asset_content_type: application/octet-stream

  publish-cargo:
    name: Publish to crates.io
    needs: create-release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Publish to crates.io
        run: cargo publish --token ${{ secrets.CARGO_TOKEN }}
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_TOKEN }}

  publish-npm:
    name: Publish to npm
    needs: [create-release, build-binaries]
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'

      - name: Install dependencies
        run: npm ci

      - name: Build
        run: npm run build

      - name: Publish to npm
        run: npm publish --access public
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}

  publish-wasm:
    name: Publish WASM Package
    needs: create-release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Build WASM
        run: wasm-pack build --target web --out-dir pkg/wasm

      - name: Package WASM
        run: |
          cd pkg/wasm
          tar czf ../../ruvector-${{ needs.create-release.outputs.version }}-wasm.tar.gz .

      - name: Upload WASM Asset
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./ruvector-${{ needs.create-release.outputs.version }}-wasm.tar.gz
          asset_name: ruvector-${{ needs.create-release.outputs.version }}-wasm.tar.gz
          asset_content_type: application/gzip

  publish-github:
    name: Publish to GitHub Packages
    needs: create-release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://npm.pkg.github.com'
          scope: '@ruvector'

      - name: Install dependencies
        run: npm ci

      - name: Build
        run: npm run build

      - name: Update package name for GitHub
        run: |
          node -e "
            const pkg = require('./package.json');
            pkg.name = '@ruvector/ruvector';
            require('fs').writeFileSync('package.json', JSON.stringify(pkg, null, 2));
          "

      - name: Publish to GitHub Packages
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## 3. Benchmark Workflow - Daily Performance

**File**: `.github/workflows/benchmark.yml`

```yaml
name: Benchmark

on:
  schedule:
    # Run daily at 2 AM UTC
    - cron: '0 2 * * *'
  push:
    branches: [main]
    paths:
      - 'src/**'
      - 'benches/**'
      - 'Cargo.toml'
  workflow_dispatch:

permissions:
  contents: write
  deployments: write

env:
  CARGO_TERM_COLOR: always

jobs:
  benchmark:
    name: Run Benchmarks
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Rust cache
        uses: Swatinem/rust-cache@v2

      - name: Install criterion
        run: cargo install cargo-criterion

      - name: Run benchmarks
        run: cargo criterion --message-format=json > benchmark-results.json

      - name: Parse benchmark results
        id: parse
        run: |
          python3 <<EOF
          import json
          import os

          with open('benchmark-results.json') as f:
              results = [json.loads(line) for line in f if line.strip()]

          benchmarks = []
          for result in results:
              if result.get('reason') == 'benchmark-complete':
                  name = result['id']
                  mean = result['mean']['estimate']
                  benchmarks.append({
                      'name': name,
                      'unit': 'ns',
                      'value': mean
                  })

          with open('parsed-results.json', 'w') as f:
              json.dump(benchmarks, f)

          print(f"Parsed {len(benchmarks)} benchmarks")
          EOF

      - name: Download previous benchmark data
        uses: actions/cache@v4
        with:
          path: ./cache
          key: benchmark-${{ github.ref }}

      - name: Check for regression
        id: regression
        run: |
          python3 <<EOF
          import json
          import os

          # Load current results
          with open('parsed-results.json') as f:
              current = {b['name']: b['value'] for b in json.load(f)}

          # Load previous results
          previous = {}
          if os.path.exists('./cache/benchmark.json'):
              with open('./cache/benchmark.json') as f:
                  previous = {b['name']: b['value'] for b in json.load(f)}

          # Check for regressions (>5% slower)
          regressions = []
          for name, curr_val in current.items():
              if name in previous:
                  prev_val = previous[name]
                  change = ((curr_val - prev_val) / prev_val) * 100
                  if change > 5.0:
                      regressions.append({
                          'name': name,
                          'previous': prev_val,
                          'current': curr_val,
                          'change': change
                      })

          if regressions:
              print("⚠️ Performance regressions detected:")
              for r in regressions:
                  print(f"  {r['name']}: +{r['change']:.2f}% slower")
              with open(os.environ['GITHUB_OUTPUT'], 'a') as f:
                  f.write('has_regression=true\n')
              exit(1)
          else:
              print("✅ No performance regressions detected")
              with open(os.environ['GITHUB_OUTPUT'], 'a') as f:
                  f.write('has_regression=false\n')
          EOF
        continue-on-error: true

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          name: Rust Benchmark
          tool: 'cargo'
          output-file-path: benchmark-results.json
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
          alert-threshold: '105%'
          comment-on-alert: true
          fail-on-alert: true
          alert-comment-cc-users: '@ruvector/maintainers'

      - name: Save results for next run
        run: |
          mkdir -p ./cache
          cp parsed-results.json ./cache/benchmark.json

      - name: Generate benchmark report
        run: |
          python3 <<EOF
          import json

          with open('parsed-results.json') as f:
              benchmarks = json.load(f)

          print("# Benchmark Results\n")
          print("| Benchmark | Time (ns) |")
          print("|-----------|-----------|")
          for b in sorted(benchmarks, key=lambda x: x['name']):
              print(f"| {b['name']} | {b['value']:.2f} |")
          EOF

      - name: Comment on PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const benchmarks = JSON.parse(fs.readFileSync('parsed-results.json', 'utf8'));

            let comment = '## 📊 Benchmark Results\n\n';
            comment += '| Benchmark | Time (ns) |\n';
            comment += '|-----------|-----------||\n';

            benchmarks.sort((a, b) => a.name.localeCompare(b.name));
            for (const b of benchmarks) {
              comment += `| ${b.name} | ${b.value.toFixed(2)} |\n`;
            }

            if ('${{ steps.regression.outputs.has_regression }}' === 'true') {
              comment += '\n⚠️ **Performance regression detected!** See logs for details.\n';
            }

            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: comment
            });

  memory-benchmark:
    name: Memory Usage Benchmark
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install valgrind
        run: sudo apt-get update && sudo apt-get install -y valgrind

      - name: Build with debug info
        run: cargo build --release --tests

      - name: Run memory profiling
        run: |
          valgrind --tool=massif --massif-out-file=massif.out \
            cargo test --release -- --test-threads=1

      - name: Parse memory results
        run: |
          ms_print massif.out > memory-report.txt
          grep "peak" memory-report.txt || true

      - name: Upload memory report
        uses: actions/upload-artifact@v4
        with:
          name: memory-report
          path: memory-report.txt

  platform-benchmark:
    name: Platform Benchmark - ${{ matrix.os }}
    runs-on: ${{ matrix.os }}

    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run platform-specific benchmarks
        run: cargo bench --bench platform_benchmarks

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-${{ matrix.os }}
          path: target/criterion/
```

## 4. Dependabot Configuration

**File**: `.github/dependabot.yml`

```yaml
version: 2

updates:
  # Cargo dependencies
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "06:00"
    open-pull-requests-limit: 10
    reviewers:
      - "ruvector/maintainers"
    labels:
      - "dependencies"
      - "rust"
    commit-message:
      prefix: "chore(deps):"
      include: "scope"
    ignore:
      # Ignore major version updates for breaking changes
      - dependency-name: "*"
        update-types: ["version-update:semver-major"]
    groups:
      rust-dependencies:
        patterns:
          - "*"
        update-types:
          - "minor"
          - "patch"

  # npm dependencies
  - package-ecosystem: "npm"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "06:00"
    open-pull-requests-limit: 10
    reviewers:
      - "ruvector/maintainers"
    labels:
      - "dependencies"
      - "npm"
    commit-message:
      prefix: "chore(deps):"
      include: "scope"
    ignore:
      - dependency-name: "*"
        update-types: ["version-update:semver-major"]
    groups:
      npm-dependencies:
        patterns:
          - "*"
        update-types:
          - "minor"
          - "patch"

  # GitHub Actions
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
      time: "06:00"
    open-pull-requests-limit: 5
    reviewers:
      - "ruvector/maintainers"
    labels:
      - "dependencies"
      - "github-actions"
    commit-message:
      prefix: "chore(ci):"
      include: "scope"
```

## 5. Quality Gates Configuration

**File**: `.github/workflows/quality-gate.yml`

```yaml
name: Quality Gate

on:
  pull_request:
    branches: [main, develop]
  push:
    branches: [main]

jobs:
  coverage-gate:
    name: Coverage Gate
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: cargo tarpaulin --out Xml --out Json --output-dir ./coverage

      - name: Check coverage threshold
        run: |
          COVERAGE=$(jq -r '.files | map(.coverage) | add / length' coverage/tarpaulin.json)
          echo "Coverage: ${COVERAGE}%"

          if (( $(echo "$COVERAGE < 80" | bc -l) )); then
            echo "❌ Coverage ${COVERAGE}% is below threshold 80%"
            exit 1
          fi

          echo "✅ Coverage ${COVERAGE}% meets threshold"

      - name: Comment coverage on PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const coverage = JSON.parse(fs.readFileSync('coverage/tarpaulin.json', 'utf8'));
            const total = coverage.files.reduce((sum, f) => sum + f.coverage, 0) / coverage.files.length;

            const comment = `## 📊 Code Coverage Report

            **Total Coverage**: ${total.toFixed(2)}%
            **Threshold**: 80%
            **Status**: ${total >= 80 ? '✅ PASS' : '❌ FAIL'}

            ### File Coverage
            | File | Coverage |
            |------|----------|
            ${coverage.files.slice(0, 10).map(f =>
              `| ${f.path} | ${f.coverage.toFixed(2)}% |`
            ).join('\n')}
            `;

            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: comment
            });

  performance-gate:
    name: Performance Gate
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 2

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench --bench performance_gate -- --save-baseline current

      - name: Checkout base branch
        run: git checkout HEAD~1

      - name: Run baseline benchmarks
        run: cargo bench --bench performance_gate -- --save-baseline baseline

      - name: Compare benchmarks
        run: |
          cargo install critcmp
          critcmp baseline current > comparison.txt
          cat comparison.txt

          # Check if any benchmark is >5% slower
          if grep -q "change:.* +[0-9.]*%" comparison.txt; then
            REGRESSION=$(grep "change:.* +[0-9.]*%" comparison.txt | awk '{print $NF}' | tr -d '%')
            if (( $(echo "$REGRESSION > 5" | bc -l) )); then
              echo "❌ Performance regression detected: ${REGRESSION}%"
              exit 1
            fi
          fi

          echo "✅ No significant performance regression"

  clippy-gate:
    name: Clippy Gate
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Run Clippy
        run: |
          cargo clippy --all-targets --all-features \
            --message-format=json -- -D warnings \
            > clippy-results.json

      - name: Check for warnings
        run: |
          WARNINGS=$(jq -r 'select(.reason == "compiler-message") | select(.message.level == "warning")' clippy-results.json | wc -l)

          echo "Clippy warnings: $WARNINGS"

          if [ "$WARNINGS" -gt 0 ]; then
            echo "❌ Clippy warnings detected"
            jq -r 'select(.reason == "compiler-message") | select(.message.level == "warning") | .message.message' clippy-results.json
            exit 1
          fi

          echo "✅ No Clippy warnings"

  security-gate:
    name: Security Gate
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Run cargo-deny
        uses: EmbarkStudios/cargo-deny-action@v1
        with:
          log-level: warn
          command: check
          arguments: --all-features

      - name: Run cargo-audit
        run: |
          cargo install cargo-audit
          cargo audit --deny warnings

      - name: Run npm audit
        run: npm audit --audit-level=high

  size-gate:
    name: Binary Size Gate
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build release binary
        run: cargo build --release

      - name: Check binary size
        run: |
          SIZE=$(stat -c%s target/release/libruvector.so)
          SIZE_MB=$(echo "scale=2; $SIZE / 1048576" | bc)

          echo "Binary size: ${SIZE_MB}MB"

          # Fail if binary is larger than 50MB
          if (( $(echo "$SIZE_MB > 50" | bc -l) )); then
            echo "❌ Binary size ${SIZE_MB}MB exceeds 50MB limit"
            exit 1
          fi

          echo "✅ Binary size is acceptable"
```

## 6. Additional Workflow Files

### 6.1 PR Labeler

**File**: `.github/workflows/labeler.yml`

```yaml
name: Label PRs

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  label:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/labeler@v5
        with:
          repo-token: ${{ secrets.GITHUB_TOKEN }}
          configuration-path: .github/labeler.yml
```

**File**: `.github/labeler.yml`

```yaml
rust:
  - changed-files:
    - any-glob-to-any-file: 'src/**/*.rs'
    - any-glob-to-any-file: 'Cargo.toml'

typescript:
  - changed-files:
    - any-glob-to-any-file: '**/*.ts'
    - any-glob-to-any-file: 'package.json'

documentation:
  - changed-files:
    - any-glob-to-any-file: 'docs/**/*'
    - any-glob-to-any-file: '**/*.md'

tests:
  - changed-files:
    - any-glob-to-any-file: '**/*.test.ts'
    - any-glob-to-any-file: 'tests/**/*'

ci:
  - changed-files:
    - any-glob-to-any-file: '.github/**/*'
```

### 6.2 Stale Issue Management

**File**: `.github/workflows/stale.yml`

```yaml
name: Mark Stale Issues and PRs

on:
  schedule:
    - cron: '0 0 * * *'

permissions:
  issues: write
  pull-requests: write

jobs:
  stale:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/stale@v9
        with:
          repo-token: ${{ secrets.GITHUB_TOKEN }}
          stale-issue-message: 'This issue has been automatically marked as stale because it has not had recent activity. It will be closed if no further activity occurs. Thank you for your contributions.'
          stale-pr-message: 'This PR has been automatically marked as stale because it has not had recent activity. It will be closed if no further activity occurs. Thank you for your contributions.'
          stale-issue-label: 'stale'
          stale-pr-label: 'stale'
          days-before-stale: 60
          days-before-close: 7
          exempt-issue-labels: 'pinned,security'
          exempt-pr-labels: 'pinned,security'
```

## 7. Setup Instructions

### 7.1 Required Secrets

Add these secrets to your GitHub repository:

```bash
# Repository Settings > Secrets and Variables > Actions

CARGO_TOKEN          # crates.io API token
NPM_TOKEN            # npm registry token
CODECOV_TOKEN        # Codecov upload token
```

### 7.2 Branch Protection Rules

Configure branch protection for `main`:

- Require pull request reviews before merging (1 approval)
- Require status checks to pass:
  - `test` (all matrix jobs)
  - `lint`
  - `security`
  - `wasm`
  - `quality-gate`
  - `coverage-gate`
  - `performance-gate`
  - `clippy-gate`
- Require branches to be up to date
- Require conversation resolution before merging
- Include administrators

### 7.3 CODEOWNERS

**File**: `.github/CODEOWNERS`

```
# Default owners for everything
* @ruvector/maintainers

# Rust code
*.rs @ruvector/rust-team
Cargo.toml @ruvector/rust-team

# TypeScript/JavaScript
*.ts @ruvector/typescript-team
*.js @ruvector/typescript-team
package.json @ruvector/typescript-team

# CI/CD workflows
.github/workflows/ @ruvector/devops-team

# Documentation
/docs/ @ruvector/docs-team
*.md @ruvector/docs-team
```

## 8. Monitoring and Alerts

### 8.1 Workflow Failure Notifications

**File**: `.github/workflows/notify-failures.yml`

```yaml
name: Notify on Workflow Failures

on:
  workflow_run:
    workflows: ["CI", "Release", "Benchmark"]
    types: [completed]

jobs:
  notify:
    runs-on: ubuntu-latest
    if: ${{ github.event.workflow_run.conclusion == 'failure' }}

    steps:
      - name: Send Slack notification
        uses: slackapi/slack-github-action@v1
        with:
          webhook-url: ${{ secrets.SLACK_WEBHOOK_URL }}
          payload: |
            {
              "text": "❌ Workflow failed: ${{ github.event.workflow_run.name }}",
              "blocks": [
                {
                  "type": "section",
                  "text": {
                    "type": "mrkdwn",
                    "text": "*Workflow Failed*\n*Workflow*: ${{ github.event.workflow_run.name }}\n*Branch*: ${{ github.event.workflow_run.head_branch }}\n*Run*: ${{ github.event.workflow_run.html_url }}"
                  }
                }
              ]
            }
```

## 9. Performance Optimization Tips

### 9.1 Caching Strategy

```yaml
# Rust cache
- uses: Swatinem/rust-cache@v2
  with:
    cache-on-failure: true
    shared-key: "rust-cache"

# npm cache
- uses: actions/setup-node@v4
  with:
    cache: 'npm'

# Custom cache for build artifacts
- uses: actions/cache@v4
  with:
    path: |
      target/
      node_modules/
    key: ${{ runner.os }}-build-${{ hashFiles('**/Cargo.lock', '**/package-lock.json') }}
```

### 9.2 Parallel Job Execution

```yaml
jobs:
  test:
    strategy:
      fail-fast: false
      max-parallel: 10  # Run up to 10 jobs in parallel
      matrix:
        # Matrix configuration
```

### 9.3 Conditional Execution

```yaml
- name: Skip if docs only
  if: |
    !contains(github.event.head_commit.message, '[skip ci]') &&
    !contains(github.event.head_commit.message, '[docs only]')
  run: cargo test
```

## 10. Troubleshooting Guide

### Common Issues

1. **Build failures on ARM64**
   - Ensure cross-compilation tools are installed
   - Use appropriate runners or QEMU emulation

2. **Coverage threshold failures**
   - Review untested code paths
   - Add integration tests
   - Consider adjusting threshold for infrastructure code

3. **Benchmark regressions**
   - Check for algorithmic changes
   - Review recent dependency updates
   - Profile hot paths with flamegraph

4. **Release failures**
   - Verify version bumps in Cargo.toml and package.json
   - Ensure tokens are valid and have correct permissions
   - Check that all platform builds succeeded

## 11. Success Metrics

The CI/CD pipeline should achieve:

- **Test coverage**: ≥80%
- **Build time**: <15 minutes per platform
- **Benchmark stability**: <5% variance
- **Release automation**: 100% (zero manual steps)
- **Security scan**: 0 high/critical vulnerabilities
- **Workflow reliability**: >99% success rate

## Implementation Checklist

- [ ] Create all workflow files in `.github/workflows/`
- [ ] Configure Dependabot in `.github/dependabot.yml`
- [ ] Add required secrets to repository settings
- [ ] Configure branch protection rules
- [ ] Set up CODEOWNERS file
- [ ] Enable GitHub Actions permissions
- [ ] Test CI workflow with draft PR
- [ ] Validate release workflow with pre-release tag
- [ ] Confirm benchmark automation works
- [ ] Set up notification channels (Slack, email)
- [ ] Document workflow for team

## Next Steps

1. Review and merge this CI/CD configuration
2. Test each workflow individually
3. Monitor first few automated releases
4. Iterate based on team feedback
5. Add platform-specific optimizations as needed

---

**Agent 15 Status**: Complete CI/CD pipeline ready for deployment
