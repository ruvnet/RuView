# 🚀 Running Agentic-Jujutsu Examples

This guide shows you how to run and test all agentic-jujutsu examples with agentic-synth.

---

## Prerequisites

```bash
# Install agentic-jujutsu globally (optional)
npm install -g agentic-jujutsu@latest

# Or use with npx (recommended)
npx agentic-jujutsu@latest --version
```

## Environment Setup

```bash
# Navigate to examples directory
cd /home/user/ruvector/packages/agentic-synth/examples/agentic-jujutsu

# Set API key for agentic-synth
export GEMINI_API_KEY=your-api-key-here

# Initialize test repository (one-time setup)
npx agentic-jujutsu@latest init test-repo
cd test-repo
```

---

## Running Examples

### 1. Version Control Integration

**Basic Usage:**
```bash
npx tsx version-control-integration.ts
```

**What it demonstrates:**
- Repository initialization
- Committing generated data with metadata
- Creating branches for different strategies
- Comparing datasets across branches
- Merging data from multiple branches
- Rolling back to previous generations
- Tagging important versions

**Expected Output:**
```
✅ Initialized jujutsu repository
✅ Generated 100 user records
✅ Committed to branch: main (commit: abc123)
✅ Created branch: strategy-A
✅ Generated 100 records with strategy A
✅ Compared datasets: 15 differences found
✅ Rolled back to version abc123
```

---

### 2. Multi-Agent Data Generation

**Basic Usage:**
```bash
npx tsx multi-agent-data-generation.ts
```

**What it demonstrates:**
- Registering multiple agents
- Each agent on dedicated branch
- Parallel data generation
- Automatic conflict resolution
- Merging agent contributions
- Agent activity tracking

**Expected Output:**
```
✅ Registered 3 agents
✅ Agent 1 (user-gen): Generated 500 users
✅ Agent 2 (product-gen): Generated 1000 products
✅ Agent 3 (order-gen): Generated 2000 orders
✅ Merged all contributions (octopus merge)
✅ Total records: 3500
```

---

### 3. ReasoningBank Learning

**Basic Usage:**
```bash
npx tsx reasoning-bank-learning.ts
```

**What it demonstrates:**
- Tracking generation trajectories
- Learning from successful patterns
- Adaptive schema evolution
- Quality improvement over time
- Memory distillation
- Self-optimization

**Expected Output:**
```
✅ Generation 1: Quality score 0.72
✅ Learned pattern: "high quality uses X constraint"
✅ Generation 2: Quality score 0.85 (+18%)
✅ Evolved schema: Added field Y
✅ Generation 3: Quality score 0.92 (+7%)
✅ Distilled 3 patterns for future use
```

---

### 4. Quantum-Resistant Data

**Basic Usage:**
```bash
npx tsx quantum-resistant-data.ts
```

**What it demonstrates:**
- Quantum-safe key generation
- Cryptographic data signing
- Integrity verification
- Merkle tree proofs
- Audit trail generation
- Tamper detection

**Expected Output:**
```
✅ Generated quantum-resistant keypair
✅ Signed dataset with Ed25519
✅ Verified signature: VALID
✅ Created Merkle tree with 100 leaves
✅ Generated audit trail: 5 operations
✅ Integrity check: PASSED
```

---

### 5. Collaborative Workflows

**Basic Usage:**
```bash
npx tsx collaborative-workflows.ts
```

**What it demonstrates:**
- Team creation with permissions
- Team workspaces
- Review requests
- Quality gates
- Approval workflows
- Collaborative schema design

**Expected Output:**
```
✅ Created team: data-science (5 members)
✅ Created workspace: experiments/team-data-science
✅ Generated dataset: 1000 records
✅ Submitted for review
✅ Review approved by 2/3 reviewers
✅ Quality gate passed (score: 0.89)
✅ Merged to production branch
```

---

### 6. Test Suite

**Run all tests:**
```bash
npx tsx test-suite.ts
```

**What it tests:**
- All version control operations
- Multi-agent coordination
- ReasoningBank learning
- Quantum security
- Collaborative workflows
- Performance benchmarks
- Error handling

**Expected Output:**
```
🧪 Running Test Suite...

Version Control Tests: ✅ 8/8 passed
Multi-Agent Tests: ✅ 6/6 passed
ReasoningBank Tests: ✅ 7/7 passed
Quantum Security Tests: ✅ 5/5 passed
Collaborative Tests: ✅ 9/9 passed
Performance Tests: ✅ 10/10 passed

Total: ✅ 45/45 passed (100%)
Duration: 12.5s
```

---

## Running All Examples

**Sequential Execution:**
```bash
#!/bin/bash
echo "Running all agentic-jujutsu examples..."

npx tsx version-control-integration.ts
npx tsx multi-agent-data-generation.ts
npx tsx reasoning-bank-learning.ts
npx tsx quantum-resistant-data.ts
npx tsx collaborative-workflows.ts
npx tsx test-suite.ts

echo "✅ All examples completed!"
```

**Save as `run-all.sh` and execute:**
```bash
chmod +x run-all.sh
./run-all.sh
```

---

## Parallel Execution

**Run examples in parallel (faster):**
```bash
#!/bin/bash
echo "Running examples in parallel..."

npx tsx version-control-integration.ts &
npx tsx multi-agent-data-generation.ts &
npx tsx reasoning-bank-learning.ts &
npx tsx quantum-resistant-data.ts &
npx tsx collaborative-workflows.ts &

wait
echo "✅ All examples completed!"
```

---

## Performance Benchmarks

**Benchmark script:**
```bash
#!/bin/bash
echo "Benchmarking agentic-jujutsu operations..."

# Measure commit performance
time npx agentic-jujutsu@latest commit -m "benchmark" data.json

# Measure branch performance
time npx agentic-jujutsu@latest new-branch test-branch

# Measure merge performance
time npx agentic-jujutsu@latest merge test-branch

# Measure status performance
time npx agentic-jujutsu@latest status

echo "✅ Benchmarking complete!"
```

**Expected Results:**
- Commit: ~50-100ms
- Branch: ~10-20ms
- Merge: ~100-200ms
- Status: ~5-10ms

---

## Testing with Different Data Sizes

**Small datasets (100 records):**
```bash
npx tsx version-control-integration.ts --count 100
```

**Medium datasets (10,000 records):**
```bash
npx tsx version-control-integration.ts --count 10000
```

**Large datasets (100,000 records):**
```bash
npx tsx version-control-integration.ts --count 100000
```

---

## Integration with CI/CD

**GitHub Actions Example:**
```yaml
name: Test Agentic-Jujutsu Examples

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm install

      - name: Run examples
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
        run: |
          cd packages/agentic-synth/examples/agentic-jujutsu
          npx tsx test-suite.ts

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: test-results
          path: test-results.json
```

---

## Troubleshooting

### Issue: "agentic-jujutsu: command not found"

**Solution:**
```bash
# Use npx to run without installing
npx agentic-jujutsu@latest --version

# Or install globally
npm install -g agentic-jujutsu@latest
```

### Issue: "Repository not initialized"

**Solution:**
```bash
# Initialize jujutsu repository
npx agentic-jujutsu@latest init
```

### Issue: "GEMINI_API_KEY not set"

**Solution:**
```bash
export GEMINI_API_KEY=your-api-key-here
```

### Issue: "Module not found"

**Solution:**
```bash
# Install dependencies
npm install
npm install -g tsx
```

### Issue: "Merge conflicts"

**Solution:**
```bash
# View conflicts
npx agentic-jujutsu@latest status

# Resolve conflicts manually or use automatic resolution
npx tsx collaborative-workflows.ts --auto-resolve
```

---

## Advanced Usage

### Custom Configuration

Create `jujutsu.config.json`:
```json
{
  "reasoningBank": {
    "enabled": true,
    "minQualityScore": 0.8,
    "learningRate": 0.1
  },
  "quantum": {
    "algorithm": "Ed25519",
    "hashFunction": "SHA-512"
  },
  "collaboration": {
    "requireReviews": 2,
    "qualityGateThreshold": 0.85
  }
}
```

### Environment Variables

```bash
# Enable debug logging
export JUJUTSU_DEBUG=true

# Set custom repository path
export JUJUTSU_REPO_PATH=/path/to/repo

# Configure cache
export JUJUTSU_CACHE_SIZE=1000

# Set timeout
export JUJUTSU_TIMEOUT=30000
```

---

## Monitoring and Metrics

**View statistics:**
```bash
npx agentic-jujutsu@latest stats

# Output:
# Total commits: 1,234
# Total branches: 56
# Active agents: 3
# Average quality score: 0.87
# Cache hit rate: 92%
```

**Export metrics:**
```bash
npx agentic-jujutsu@latest export-metrics metrics.json
```

---

## Cleanup

**Remove test repositories:**
```bash
rm -rf test-repo .jj
```

**Clear cache:**
```bash
npx agentic-jujutsu@latest cache clear
```

---

## Next Steps

1. Read the main [README.md](./README.md) for detailed documentation
2. Explore individual example files for code samples
3. Run the test suite to verify functionality
4. Integrate with your CI/CD pipeline
5. Customize examples for your use case

---

## Support

- **Issues**: https://github.com/ruvnet/agentic-jujutsu/issues
- **Documentation**: https://github.com/ruvnet/agentic-jujutsu
- **Examples**: This directory

---

**Last Updated**: 2025-11-22
**Version**: 0.1.0
**Status**: Production Ready ✅
