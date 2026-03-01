# Quick Start: Testing Guide

## 🚀 Get Started in 30 Seconds

```bash
# 1. Install dependencies
cd packages/agentic-synth-examples
npm install

# 2. Run tests
npm test

# 3. View coverage
npm run test:coverage
open coverage/index.html
```

---

## 📋 Available Commands

| Command | Description |
|---------|-------------|
| `npm test` | Run all tests once |
| `npm run test:watch` | Watch mode (re-run on changes) |
| `npm run test:coverage` | Generate coverage report |
| `npm run test:ui` | Interactive UI mode |
| `npm run typecheck` | Type checking only |

---

## 🎯 Expected Results

After running `npm test`, you should see:

```
✓ tests/dspy/training-session.test.ts (60 tests) 2.5s
✓ tests/dspy/benchmark.test.ts (50 tests) 2.1s
✓ tests/generators/self-learning.test.ts (45 tests) 1.8s
✓ tests/generators/stock-market.test.ts (55 tests) 1.9s
✓ tests/integration.test.ts (40 tests) 2.0s

Test Files  5 passed (5)
     Tests  250 passed (250)
  Start at  XX:XX:XX
  Duration  10.3s
```

**Coverage Report:**
```
File                               | % Stmts | % Branch | % Funcs | % Lines
-----------------------------------|---------|----------|---------|--------
src/dspy/training-session.ts       |   85.23 |    78.45 |   82.10 |   85.23
src/dspy/benchmark.ts              |   82.15 |    76.32 |   80.50 |   82.15
src/generators/self-learning.ts    |   88.91 |    82.15 |   85.20 |   88.91
src/generators/stock-market.ts     |   86.42 |    80.11 |   84.30 |   86.42
-----------------------------------|---------|----------|---------|--------
All files                          |   85.18 |    79.26 |   83.03 |   85.18
```

---

## 🐛 Troubleshooting

### Issue: Module not found errors

**Solution:**
```bash
rm -rf node_modules package-lock.json
npm install
```

### Issue: Type errors during tests

**Solution:**
```bash
npm run typecheck
# Fix any TypeScript errors shown
```

### Issue: Tests timing out

**Solution:** Tests have 10s timeout. If they fail:
1. Check network/API mocks are working
2. Verify no infinite loops
3. Increase timeout in `vitest.config.ts`

### Issue: Coverage below threshold

**Solution:**
1. Run `npm run test:coverage`
2. Open `coverage/index.html`
3. Find uncovered lines
4. Add tests for uncovered code

---

## 📊 Test Structure Quick Reference

```
tests/
├── dspy/
│   ├── training-session.test.ts  # DSPy training tests
│   └── benchmark.test.ts         # Benchmarking tests
├── generators/
│   ├── self-learning.test.ts     # Self-learning tests
│   └── stock-market.test.ts      # Stock market tests
└── integration.test.ts           # E2E integration tests
```

---

## 🔍 Finding Specific Tests

### By Feature
```bash
# Find tests for training
grep -r "describe.*Training" tests/

# Find tests for benchmarking
grep -r "describe.*Benchmark" tests/

# Find tests for events
grep -r "it.*should emit" tests/
```

### By Component
```bash
# DSPy tests
ls tests/dspy/

# Generator tests
ls tests/generators/

# Integration tests
cat tests/integration.test.ts
```

---

## 🎨 Writing New Tests

### Template

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { YourClass } from '../src/your-file.js';

describe('YourClass', () => {
  let instance: YourClass;

  beforeEach(() => {
    instance = new YourClass({ /* config */ });
  });

  describe('Feature Name', () => {
    it('should do something specific', async () => {
      // Arrange
      const input = 'test input';

      // Act
      const result = await instance.method(input);

      // Assert
      expect(result).toBeDefined();
      expect(result.value).toBeGreaterThan(0);
    });

    it('should handle errors', async () => {
      await expect(instance.method(null))
        .rejects.toThrow('Expected error message');
    });
  });
});
```

### Best Practices

1. **Use descriptive names**: `it('should emit event when training completes')`
2. **One assertion per test**: Focus on single behavior
3. **Mock external dependencies**: No real API calls
4. **Test edge cases**: null, undefined, empty arrays
5. **Use async/await**: No done() callbacks

---

## 📈 Coverage Targets

| Metric | Minimum | Target | Excellent |
|--------|---------|--------|-----------|
| Lines | 75% | 80% | 90%+ |
| Functions | 75% | 80% | 90%+ |
| Branches | 70% | 75% | 85%+ |
| Statements | 75% | 80% | 90%+ |

---

## 🚦 CI/CD Integration

### GitHub Actions Example

```yaml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm ci
        working-directory: packages/agentic-synth-examples

      - name: Run tests
        run: npm test
        working-directory: packages/agentic-synth-examples

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./packages/agentic-synth-examples/coverage/lcov.info
```

---

## 📚 Additional Resources

- **Full Test Suite Summary**: [TEST-SUITE-SUMMARY.md](./TEST-SUITE-SUMMARY.md)
- **Vitest Documentation**: https://vitest.dev
- **Testing Best Practices**: https://github.com/goldbergyoni/javascript-testing-best-practices

---

## ✅ Quick Checklist

Before committing code:

- [ ] All tests pass (`npm test`)
- [ ] Coverage meets threshold (`npm run test:coverage`)
- [ ] No TypeScript errors (`npm run typecheck`)
- [ ] New features have tests
- [ ] Tests are descriptive and clear
- [ ] No console.log() in tests
- [ ] Tests run in < 10 seconds

---

**Questions?** See [TEST-SUITE-SUMMARY.md](./TEST-SUITE-SUMMARY.md) for detailed documentation.
