# Contributing to Psycho-Symbolic Reasoner

Thank you for your interest in contributing to the Psycho-Symbolic Reasoner! This document provides guidelines and information for contributors.

## 🤝 Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## 🚀 Getting Started

### Prerequisites

- **Node.js**: Version 18.0.0 or higher
- **Rust**: Latest stable version (install via [rustup](https://rustup.rs/))
- **wasm-pack**: For building WebAssembly modules
- **Git**: For version control

### Development Setup

1. **Fork and clone the repository**:
   ```bash
   git clone https://github.com/your-username/sublinear-time-solver.git
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

## 📋 Development Workflow

### Branch Strategy

- `main` - Production-ready code
- `develop` - Integration branch for features
- `feature/*` - Feature development branches
- `bugfix/*` - Bug fix branches
- `hotfix/*` - Critical fixes for production

### Making Changes

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following our coding standards

3. **Write or update tests** for your changes

4. **Run the test suite**:
   ```bash
   npm test
   npm run lint
   npm run typecheck
   ```

5. **Commit your changes** using conventional commits:
   ```bash
   git commit -m "feat: add new reasoning algorithm"
   ```

6. **Push and create a pull request**

## 🎯 Types of Contributions

### 🐛 Bug Reports

When reporting bugs, please include:

- **Clear description** of the issue
- **Steps to reproduce** the problem
- **Expected vs actual behavior**
- **Environment details** (OS, Node.js version, etc.)
- **Relevant logs or error messages**

Use our [bug report template](.github/ISSUE_TEMPLATE/bug_report.md).

### ✨ Feature Requests

For new features, please provide:

- **Clear use case** and motivation
- **Detailed description** of the proposed feature
- **Potential implementation approach**
- **Examples** of how it would be used

Use our [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).

### 📚 Documentation

Documentation improvements are always welcome:

- **API documentation** improvements
- **Tutorial and example** additions
- **README** enhancements
- **Code comments** and inline documentation

### 🧪 Testing

Help improve our test coverage:

- **Unit tests** for Rust components
- **Integration tests** for TypeScript layer
- **End-to-end tests** for CLI and MCP functionality
- **Performance benchmarks**

## 💻 Coding Standards

### Rust Code

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write comprehensive unit tests
- Document public APIs with rustdoc

```rust
/// Extracts sentiment from text input
///
/// # Arguments
/// * `text` - The input text to analyze
///
/// # Returns
/// * `SentimentResult` - Analysis results with score and confidence
///
/// # Examples
/// ```
/// let result = extract_sentiment("I love this!");
/// assert!(result.score > 0.5);
/// ```
pub fn extract_sentiment(text: &str) -> SentimentResult {
    // Implementation...
}
```

### TypeScript Code

- Follow [TypeScript ESLint rules](.eslintrc.js)
- Use Prettier for code formatting
- Write JSDoc comments for public APIs
- Prefer functional programming patterns
- Use strict type checking

```typescript
/**
 * Creates a new planning instance with the given configuration
 *
 * @param config - Planner configuration options
 * @returns Promise resolving to configured planner instance
 *
 * @example
 * ```typescript
 * const planner = await createPlanner({
 *   rulesPath: './rules.json',
 *   enableLogging: true
 * });
 * ```
 */
export async function createPlanner(config: PlannerConfig): Promise<Planner> {
  // Implementation...
}
```

### Commit Messages

We use [Conventional Commits](https://conventionalcommits.org/):

- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes (no logic changes)
- `refactor:` - Code refactoring
- `test:` - Test additions or modifications
- `chore:` - Build process or auxiliary tool changes

Examples:
```
feat: add sentiment analysis caching
fix: resolve memory leak in graph traversal
docs: update MCP integration examples
test: add integration tests for planner
```

## 🧪 Testing Guidelines

### Unit Tests

Write unit tests for all new functionality:

**Rust tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_analysis() {
        let result = extract_sentiment("I'm happy!");
        assert!(result.score > 0.0);
        assert_eq!(result.primary_emotion, "joy");
    }
}
```

**TypeScript tests**:
```typescript
import { test } from 'node:test';
import { strictEqual } from 'node:assert';
import { PsychoSymbolicReasoner } from '../src/index.js';

test('should initialize reasoner correctly', async () => {
  const reasoner = new PsychoSymbolicReasoner();
  await reasoner.initialize();
  strictEqual(reasoner.isReady(), true);
});
```

### Integration Tests

Test component interactions:

```typescript
test('MCP integration end-to-end', async () => {
  const server = new MCPServer();
  await server.start();

  const response = await server.callTool('extractSentiment', {
    text: 'I love this framework!'
  });

  strictEqual(response.score > 0.5, true);
});
```

### Performance Tests

Include performance benchmarks for critical paths:

```typescript
test('graph query performance', async () => {
  const start = performance.now();
  await reasoner.queryGraph(complexQuery);
  const duration = performance.now() - start;

  // Should complete within 100ms for 1000 node graph
  assert(duration < 100);
});
```

## 📦 Build and Release Process

### Local Development

```bash
# Build everything
npm run build

# Build with watch mode
npm run build:dev

# Test with coverage
npm run test

# Lint and format
npm run lint:fix
npm run format

# Type checking
npm run typecheck
```

### Release Process

1. **Update version** in `package.json`
2. **Update CHANGELOG.md** with new features and fixes
3. **Run full test suite**: `npm test`
4. **Build production version**: `npm run build`
5. **Create release PR** to `main` branch
6. **Tag release** after merge: `git tag v1.0.1`
7. **Publish to NPM**: `npm publish`

## 🏗️ Architecture Guidelines

### Project Structure

```
psycho-symbolic-reasoner/
├── src/                    # TypeScript source
│   ├── cli/               # CLI interface
│   ├── mcp/               # MCP server and tools
│   ├── api/               # REST API
│   ├── lib/               # Core library
│   └── types/             # TypeScript definitions
├── graph_reasoner/        # Rust graph reasoning
├── extractors/            # Rust affect/preference extraction
├── planner/               # Rust planning algorithms
├── tests/                 # Test suites
├── examples/              # Usage examples
├── docs/                  # Documentation
└── benchmarks/            # Performance tests
```

### Design Principles

1. **Separation of Concerns**: Keep Rust performance-critical code separate from TypeScript integration
2. **Type Safety**: Use strict TypeScript and Rust type systems
3. **Performance First**: Optimize hot paths, especially in Rust core
4. **Security**: Maintain WASM sandboxing and input validation
5. **Extensibility**: Design for easy addition of new reasoning modules
6. **Documentation**: Code should be self-documenting with comprehensive comments

### Adding New Features

1. **Rust Core**: Implement performance-critical algorithms in Rust
2. **WASM Bindings**: Create TypeScript bindings for Rust functionality
3. **TypeScript API**: Provide high-level, easy-to-use TypeScript APIs
4. **MCP Integration**: Expose functionality as MCP tools when applicable
5. **CLI Support**: Add CLI commands for new features
6. **Documentation**: Update docs and examples

## 🔧 Troubleshooting

### Common Issues

**WASM build failures**:
```bash
# Reinstall wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Clean and rebuild
npm run clean
npm run build:wasm
```

**TypeScript compilation errors**:
```bash
# Check TypeScript version
npx tsc --version

# Clean TypeScript cache
rm -rf node_modules/.cache
npm run build:ts
```

**Test failures**:
```bash
# Run specific test
npm test -- --grep "sentiment"

# Run with verbose output
npm test -- --verbose
```

### Getting Help

- 📚 Check our [documentation](docs/)
- 🐛 Search existing [issues](https://github.com/ruvnet/sublinear-time-solver/issues)
- 💬 Join [discussions](https://github.com/ruvnet/sublinear-time-solver/discussions)
- 📧 Email maintainers: [github@ruv.net](mailto:github@ruv.net)

## 🎖️ Recognition

Contributors are recognized in:

- **README.md** - Major contributors listed
- **CHANGELOG.md** - Feature and fix attributions
- **GitHub releases** - Contributor highlights
- **Annual reports** - Top contributor recognition

## 📄 License

By contributing to Psycho-Symbolic Reasoner, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to the future of psycho-symbolic reasoning! 🧠✨