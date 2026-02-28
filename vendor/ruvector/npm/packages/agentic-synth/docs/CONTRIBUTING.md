# Contributing to Agentic-Synth

Thank you for your interest in contributing to Agentic-Synth! We welcome contributions from the community.

## 🌟 Ways to Contribute

- **Bug Reports**: Report issues and bugs
- **Feature Requests**: Suggest new features and improvements
- **Code Contributions**: Submit pull requests
- **Documentation**: Improve guides, examples, and API docs
- **Templates**: Share domain-specific schemas
- **Testing**: Add test coverage
- **Examples**: Create example use cases

## 🚀 Getting Started

### Prerequisites

- Node.js >= 18.0.0
- npm, yarn, or pnpm
- Git

### Development Setup

1. **Fork and clone the repository**

```bash
git clone https://github.com/your-username/ruvector.git
cd ruvector/packages/agentic-synth
```

2. **Install dependencies**

```bash
npm install
```

3. **Run tests**

```bash
npm test
```

4. **Build the package**

```bash
npm run build
```

5. **Run examples**

```bash
npm run example:customer-support
```

## 📝 Development Workflow

### Creating a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

### Making Changes

1. Write your code following our style guide
2. Add tests for new functionality
3. Update documentation as needed
4. Run linting and type checking:

```bash
npm run lint
npm run typecheck
```

### Committing Changes

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```bash
git commit -m "feat: add new generator for medical data"
git commit -m "fix: resolve streaming memory leak"
git commit -m "docs: update API reference"
```

**Commit types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

### Creating a Pull Request

1. Push your changes:
```bash
git push origin feature/your-feature-name
```

2. Open a pull request on GitHub
3. Fill out the PR template
4. Wait for review

## 🧪 Testing

### Running Tests

```bash
# Run all tests
npm test

# Run tests in watch mode
npm run test:watch

# Run tests with coverage
npm run test:coverage
```

### Writing Tests

```typescript
import { describe, it, expect } from 'vitest';
import { SynthEngine, Schema } from '../src';

describe('SynthEngine', () => {
  it('should generate data matching schema', async () => {
    const synth = new SynthEngine();
    const schema = Schema.define({
      name: 'User',
      type: 'object',
      properties: {
        name: { type: 'string' },
        age: { type: 'number' },
      },
    });

    const result = await synth.generate({ schema, count: 10 });

    expect(result.data).toHaveLength(10);
    expect(result.data[0]).toHaveProperty('name');
    expect(result.data[0]).toHaveProperty('age');
  });
});
```

## 📚 Documentation

### Updating Documentation

Documentation is located in:
- `README.md` - Main package documentation
- `docs/API.md` - Complete API reference
- `docs/EXAMPLES.md` - Usage examples
- `docs/INTEGRATIONS.md` - Integration guides

### Documentation Style

- Use clear, concise language
- Include code examples
- Add type signatures for TypeScript
- Link to related documentation

## 🎨 Code Style

### TypeScript Style Guide

```typescript
// Use explicit types
function generateData(count: number): Promise<Data[]> {
  // ...
}

// Use async/await instead of promises
async function fetchData() {
  const result = await api.get('/data');
  return result;
}

// Use descriptive variable names
const userSchema = Schema.define({ /* ... */ });
const generatedUsers = await synth.generate({ schema: userSchema, count: 100 });

// Document complex functions
/**
 * Generates synthetic data based on schema
 * @param options - Generation options
 * @returns Generated data with metadata
 */
async function generate(options: GenerateOptions): Promise<GeneratedData> {
  // ...
}
```

### Linting

We use ESLint and Prettier:

```bash
npm run lint        # Check for issues
npm run lint:fix    # Auto-fix issues
npm run format      # Format code
```

## 🐛 Reporting Bugs

### Before Reporting

1. Check if the bug has already been reported
2. Try the latest version
3. Create a minimal reproduction

### Bug Report Template

```markdown
**Description**
A clear description of the bug.

**To Reproduce**
Steps to reproduce the behavior:
1. Initialize with config '...'
2. Call function '...'
3. See error

**Expected Behavior**
What you expected to happen.

**Actual Behavior**
What actually happened.

**Environment**
- Agentic-Synth version:
- Node.js version:
- OS:

**Code Sample**
\`\`\`typescript
// Minimal reproduction code
\`\`\`

**Error Messages**
\`\`\`
Full error messages and stack traces
\`\`\`
```

## 💡 Feature Requests

### Feature Request Template

```markdown
**Feature Description**
A clear description of the feature.

**Use Case**
Why this feature would be useful.

**Proposed API**
\`\`\`typescript
// How the API might look
\`\`\`

**Alternatives Considered**
Other approaches you've considered.

**Additional Context**
Any other context or screenshots.
```

## 🔍 Code Review Process

### What We Look For

- **Correctness**: Does it work as intended?
- **Tests**: Are there adequate tests?
- **Documentation**: Is it well documented?
- **Style**: Does it follow our style guide?
- **Performance**: Are there any performance concerns?
- **Breaking Changes**: Does it break existing APIs?

### Review Timeline

- Initial review: 1-3 business days
- Follow-up reviews: 1-2 business days
- Merge: After approval and CI passes

## 📦 Publishing (Maintainers Only)

### Release Process

1. Update version in `package.json`
2. Update `CHANGELOG.md`
3. Create git tag
4. Publish to npm:

```bash
npm run build
npm test
npm publish
```

## 🏆 Recognition

Contributors will be:
- Listed in `package.json` contributors
- Mentioned in release notes
- Featured in project README

## 📞 Getting Help

- **Discord**: [Join our community](https://discord.gg/ruvnet)
- **GitHub Discussions**: [Ask questions](https://github.com/ruvnet/ruvector/discussions)
- **Email**: support@ruv.io

## 📜 Code of Conduct

### Our Pledge

We pledge to make participation in our project a harassment-free experience for everyone.

### Our Standards

**Positive behavior:**
- Using welcoming and inclusive language
- Being respectful of differing viewpoints
- Gracefully accepting constructive criticism
- Focusing on what is best for the community

**Unacceptable behavior:**
- Trolling, insulting/derogatory comments
- Public or private harassment
- Publishing others' private information
- Other conduct which could reasonably be considered inappropriate

### Enforcement

Violations may be reported to support@ruv.io. All complaints will be reviewed and investigated.

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to Agentic-Synth! 🎉
