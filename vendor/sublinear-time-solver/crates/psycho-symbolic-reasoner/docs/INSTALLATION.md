# Installation Guide - Psycho-Symbolic Reasoner

This guide covers various installation methods and setup procedures for the Psycho-Symbolic Reasoner.

## Table of Contents

- [System Requirements](#system-requirements)
- [Quick Installation](#quick-installation)
- [Development Setup](#development-setup)
- [Docker Installation](#docker-installation)
- [Building from Source](#building-from-source)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

## System Requirements

### Minimum Requirements
- **Node.js**: 18.0.0 or higher
- **NPM**: 9.0.0 or higher
- **Memory**: 2GB RAM minimum, 4GB recommended
- **Storage**: 500MB available space

### Additional Requirements for Development
- **Rust**: Latest stable version (1.70+)
- **wasm-pack**: Latest version
- **Git**: For version control

### Supported Platforms
- Linux (Ubuntu 20.04+, CentOS 8+, Alpine 3.15+)
- macOS (10.15+)
- Windows 10/11 (with WSL2 recommended)

## Quick Installation

### Global Installation (Recommended)

Install globally to use the CLI commands anywhere:

```bash
npm install -g psycho-symbolic-reasoner
```

Verify installation:
```bash
psycho-symbolic-reasoner --version
psr --help
```

### Project-specific Installation

Install as a dependency in your project:

```bash
npm install psycho-symbolic-reasoner
```

### NPX Usage (No Installation)

Run directly without installation:

```bash
npx psycho-symbolic-reasoner --help
npx psycho-symbolic-reasoner serve --port 3000
```

## Development Setup

### Prerequisites

1. **Install Node.js and NPM**:
   ```bash
   # Using Node Version Manager (recommended)
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
   nvm install 20
   nvm use 20

   # Or download from nodejs.org
   # https://nodejs.org/
   ```

2. **Install Rust**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   rustup target add wasm32-unknown-unknown
   ```

3. **Install wasm-pack**:
   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

### Clone and Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/ruvnet/sublinear-time-solver.git
   cd sublinear-time-solver/psycho-symbolic-reasoner
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Build the project**:
   ```bash
   npm run build
   ```

4. **Run tests**:
   ```bash
   npm test
   ```

5. **Start development server**:
   ```bash
   npm run dev:serve
   ```

## Docker Installation

### Using Pre-built Image

```bash
# Pull the image
docker pull ghcr.io/ruvnet/psycho-symbolic-reasoner:latest

# Run with MCP server
docker run -p 3000:3000 ghcr.io/ruvnet/psycho-symbolic-reasoner:latest serve

# Run CLI commands
docker run --rm ghcr.io/ruvnet/psycho-symbolic-reasoner:latest --help
```

### Building Docker Image

```bash
# Clone repository
git clone https://github.com/ruvnet/sublinear-time-solver.git
cd sublinear-time-solver/psycho-symbolic-reasoner

# Build image
docker build -t psycho-symbolic-reasoner .

# Run container
docker run -p 3000:3000 psycho-symbolic-reasoner serve
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'
services:
  psycho-reasoner:
    image: ghcr.io/ruvnet/psycho-symbolic-reasoner:latest
    ports:
      - "3000:3000"
    environment:
      - PSR_LOG_LEVEL=info
      - PSR_PORT=3000
    volumes:
      - ./knowledge-base.json:/app/knowledge-base.json
    command: serve --config /app/config.json
```

Run with:
```bash
docker-compose up -d
```

## Building from Source

### Complete Build Process

1. **Prerequisites Setup**:
   ```bash
   # Ensure all tools are installed
   node --version    # Should be 18.0.0+
   npm --version     # Should be 9.0.0+
   cargo --version   # Should be 1.70.0+
   wasm-pack --version
   ```

2. **Clone and Navigate**:
   ```bash
   git clone https://github.com/ruvnet/sublinear-time-solver.git
   cd sublinear-time-solver/psycho-symbolic-reasoner
   ```

3. **Install Node Dependencies**:
   ```bash
   npm ci
   ```

4. **Build WASM Components**:
   ```bash
   npm run build:wasm
   ```

5. **Build TypeScript**:
   ```bash
   npm run build:ts
   ```

6. **Validate Build**:
   ```bash
   npm run test
   npm run lint
   ```

### Manual WASM Build

If you need to build WASM components manually:

```bash
# Graph reasoner
wasm-pack build --target nodejs --out-dir ../wasm/pkg graph_reasoner

# Extractors
wasm-pack build --target nodejs --out-dir ../wasm/extractors extractors

# Planner
wasm-pack build --target nodejs --out-dir ../wasm/planner planner
```

### Custom Build Script

You can also use the custom build script:

```bash
node scripts/build.js
```

Options:
```bash
node scripts/build.js --clean-only     # Clean only
node scripts/build.js --wasm-only      # WASM only
node scripts/build.js --ts-only        # TypeScript only
```

## Verification

### Basic Functionality Test

```bash
# Test CLI
psycho-symbolic-reasoner --version

# Test sentiment analysis
echo "I'm feeling great today!" | psycho-symbolic-reasoner analyze sentiment

# Test MCP server
psycho-symbolic-reasoner serve --port 3000 &
curl http://localhost:3000/health
```

### Comprehensive Test Suite

```bash
npm test
npm run test:integration
npm run benchmark
```

### WASM Verification

```bash
# Check WASM files exist
ls -la wasm/pkg/
ls -la wasm/extractors/
ls -la wasm/planner/

# Test WASM loading
node -e "import('./wasm/pkg/index.js').then(m => console.log('Graph reasoner loaded'))"
```

## Troubleshooting

### Common Issues

#### 1. Node.js Version Issues

**Problem**: `Error: Unsupported Node.js version`

**Solution**:
```bash
nvm install 20
nvm use 20
# Or update Node.js to latest LTS
```

#### 2. Rust/WASM Build Failures

**Problem**: `wasm-pack not found` or build errors

**Solution**:
```bash
# Reinstall wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Ensure Rust is up to date
rustup update stable
rustup target add wasm32-unknown-unknown

# Clean and rebuild
cargo clean
npm run clean
npm run build:wasm
```

#### 3. Permission Errors

**Problem**: `EACCES` permission errors

**Solution**:
```bash
# Fix NPM permissions
npm config set prefix ~/.npm-global
export PATH=~/.npm-global/bin:$PATH

# Or use sudo (not recommended)
sudo npm install -g psycho-symbolic-reasoner
```

#### 4. Memory Issues

**Problem**: `JavaScript heap out of memory`

**Solution**:
```bash
# Increase Node.js memory limit
export NODE_OPTIONS="--max-old-space-size=4096"
npm run build
```

#### 5. TypeScript Compilation Errors

**Problem**: TypeScript compilation failures

**Solution**:
```bash
# Clean TypeScript cache
rm -rf node_modules/.cache
rm -f tsconfig.tsbuildinfo

# Reinstall dependencies
npm ci
npm run build:ts
```

### Platform-Specific Issues

#### Windows

- **Issue**: Path separator problems
- **Solution**: Use WSL2 or Git Bash

#### macOS

- **Issue**: Missing development tools
- **Solution**: `xcode-select --install`

#### Linux (Alpine)

- **Issue**: Missing glibc
- **Solution**: Install compatibility layer or use musl builds

### Getting Help

If you encounter issues not covered here:

1. **Check GitHub Issues**: [Issues Page](https://github.com/ruvnet/sublinear-time-solver/issues)
2. **View Logs**: Enable debug logging with `PSR_LOG_LEVEL=debug`
3. **Community Support**: [Discussions](https://github.com/ruvnet/sublinear-time-solver/discussions)
4. **Direct Support**: [Email](mailto:github@ruv.net)

### Debug Mode

Enable detailed logging:

```bash
export PSR_LOG_LEVEL=debug
export PSR_ENABLE_VERBOSE=true
psycho-symbolic-reasoner serve
```

### Performance Monitoring

Monitor performance during installation:

```bash
# Build with timing
time npm run build

# Memory usage monitoring
npm run build --verbose

# Benchmark after installation
npm run benchmark
```

## Next Steps

After successful installation:

1. **Read the [Quick Start Guide](../README.md#quick-start)**
2. **Explore [Examples](../examples/)**
3. **Review [API Documentation](./API.md)**
4. **Set up [MCP Integration](../README.md#mcp-integration)**
5. **Join the [Community](https://github.com/ruvnet/sublinear-time-solver/discussions)**

---

For more detailed information, see the complete [documentation](../README.md) or visit our [GitHub repository](https://github.com/ruvnet/sublinear-time-solver).