# RuVector Benchmarks - Quick Start Guide

Get up and running with RuVector benchmarks in 5 minutes!

## Prerequisites

- Node.js 18+ and npm
- k6 load testing tool
- Access to RuVector cluster

## Installation

### Step 1: Install k6

**macOS:**
```bash
brew install k6
```

**Linux (Debian/Ubuntu):**
```bash
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg \
  --keyserver hkp://keyserver.ubuntu.com:80 \
  --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | \
  sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6
```

**Windows:**
```powershell
choco install k6
```

### Step 2: Run Setup Script

```bash
cd /home/user/ruvector/benchmarks
./setup.sh
```

This will:
- Check dependencies
- Install TypeScript/ts-node
- Create results directory
- Configure environment

### Step 3: Configure Environment

Edit `.env` file with your cluster URL:

```bash
BASE_URL=https://your-ruvector-cluster.example.com
PARALLEL=1
ENABLE_HOOKS=true
```

## Running Your First Test

### Quick Validation (45 minutes)

```bash
npm run test:quick
```

This runs `baseline_100m` scenario:
- 100M concurrent connections
- 30 minutes steady-state
- Validates basic functionality

### View Results

```bash
# Start visualization dashboard
npm run dashboard

# Open in browser
open http://localhost:8000/visualization-dashboard.html
```

## Common Scenarios

### Baseline Test (500M connections)
```bash
npm run test:baseline
```
Duration: 3h 15m

### Burst Test (10x spike)
```bash
npm run test:burst
```
Duration: 20m

### Standard Test Suite
```bash
npm run test:standard
```
Duration: ~6 hours

## Understanding Results

After a test completes, check:

```bash
results/
  run-{timestamp}/
    {scenario}-metrics.json     # Raw metrics
    {scenario}-analysis.json    # Analysis report
    {scenario}-report.md        # Human-readable report
    SUMMARY.md                  # Overall summary
```

### Key Metrics

- **P99 Latency**: Should be < 50ms (baseline)
- **Throughput**: Queries per second
- **Error Rate**: Should be < 0.01%
- **Availability**: Should be > 99.99%

### Performance Score

Each test gets a score 0-100:
- 90+: Excellent
- 80-89: Good
- 70-79: Fair
- <70: Needs improvement

## Troubleshooting

### Connection Failed
```bash
# Test cluster connectivity
curl -v https://your-cluster.example.com/health
```

### k6 Errors
```bash
# Verify k6 installation
k6 version

# Reinstall if needed
brew reinstall k6  # macOS
```

### High Memory Usage
```bash
# Increase Node.js memory
export NODE_OPTIONS="--max-old-space-size=8192"
```

## Docker Usage

### Build Image
```bash
docker build -t ruvector-benchmark .
```

### Run Test
```bash
docker run \
  -e BASE_URL="https://your-cluster.example.com" \
  -v $(pwd)/results:/benchmarks/results \
  ruvector-benchmark run baseline_100m
```

## Next Steps

1. **Review README.md** for comprehensive documentation
2. **Explore scenarios** in `benchmark-scenarios.ts`
3. **Customize tests** for your workload
4. **Set up CI/CD** for continuous benchmarking

## Quick Command Reference

```bash
# List all scenarios
npm run list

# Run specific scenario
ts-node benchmark-runner.ts run <scenario-name>

# Run scenario group
ts-node benchmark-runner.ts group <group-name>

# View dashboard
npm run dashboard

# Clean results
npm run clean
```

## Available Scenarios

### Baseline Tests
- `baseline_100m` - Quick validation (45m)
- `baseline_500m` - Full baseline (3h 15m)

### Burst Tests
- `burst_10x` - 10x spike (20m)
- `burst_25x` - 25x spike (35m)
- `burst_50x` - 50x spike (50m)

### Workload Tests
- `read_heavy` - 95% reads (1h 50m)
- `write_heavy` - 70% writes (1h 50m)
- `balanced_workload` - 50/50 split (1h 50m)

### Failover Tests
- `regional_failover` - Single region failure (45m)
- `multi_region_failover` - Multiple region failure (55m)

### Real-World Tests
- `world_cup` - Sporting event simulation (3h)
- `black_friday` - E-commerce peak (14h)

### Scenario Groups
- `quick_validation` - Fast validation suite
- `standard_suite` - Standard test suite
- `stress_suite` - Stress testing
- `reliability_suite` - Failover tests
- `full_suite` - All scenarios

## Support

- **Documentation**: See README.md
- **Issues**: https://github.com/ruvnet/ruvector/issues
- **Slack**: https://ruvector.slack.com

---

**Ready to benchmark!** 🚀

Start with: `npm run test:quick`
