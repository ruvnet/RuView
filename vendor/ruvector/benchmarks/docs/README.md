# RuVector Benchmarking Suite

Comprehensive benchmarking tool for testing the globally distributed RuVector vector search system at scale (500M+ concurrent connections).

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Benchmark Scenarios](#benchmark-scenarios)
- [Running Benchmarks](#running-benchmarks)
- [Understanding Results](#understanding-results)
- [Best Practices](#best-practices)
- [Cost Estimation](#cost-estimation)
- [Troubleshooting](#troubleshooting)
- [Advanced Usage](#advanced-usage)

## Overview

This benchmarking suite provides enterprise-grade load testing capabilities for RuVector, supporting:

- **Massive Scale**: Test up to 25B concurrent connections
- **Multi-Region**: Distributed load generation across 11 GCP regions
- **Comprehensive Metrics**: Latency, throughput, errors, resource utilization, costs
- **SLA Validation**: Automated checking against 99.99% availability, <50ms p99 latency targets
- **Advanced Analysis**: Statistical analysis, bottleneck identification, recommendations

## Features

### Load Generation
- Multi-protocol support (HTTP, HTTP/2, WebSocket, gRPC)
- Realistic query patterns (uniform, hotspot, Zipfian, burst)
- Configurable ramp-up/down rates
- Connection lifecycle management
- Geographic distribution

### Metrics Collection
- Latency distribution (p50, p90, p95, p99, p99.9)
- Throughput tracking (QPS, bandwidth)
- Error analysis by type and region
- Resource utilization (CPU, memory, network)
- Cost per million queries
- Regional performance comparison

### Analysis & Reporting
- Statistical analysis with anomaly detection
- SLA compliance checking
- Bottleneck identification
- Performance score calculation
- Actionable recommendations
- Interactive visualization dashboard
- Markdown and JSON reports
- CSV export for further analysis

## Prerequisites

### Required
- **Node.js**: v18+ (for TypeScript execution)
- **k6**: Latest version ([installation guide](https://k6.io/docs/getting-started/installation/))
- **Access**: RuVector cluster endpoint

### Optional
- **Claude Flow**: For hooks integration
  ```bash
  npm install -g claude-flow@alpha
  ```
- **Docker**: For containerized execution
- **GCP Account**: For multi-region load generation

## Installation

1. **Clone Repository**
   ```bash
   cd /home/user/ruvector/benchmarks
   ```

2. **Install Dependencies**
   ```bash
   npm install -g typescript ts-node
   npm install k6 @types/k6
   ```

3. **Verify Installation**
   ```bash
   k6 version
   ts-node --version
   ```

4. **Configure Environment**
   ```bash
   export BASE_URL="https://your-ruvector-cluster.example.com"
   export PARALLEL=2  # Number of parallel scenarios
   ```

## Quick Start

### Run a Single Scenario

```bash
# Quick validation (100M connections, 45 minutes)
ts-node benchmark-runner.ts run baseline_100m

# Full baseline test (500M connections, 3+ hours)
ts-node benchmark-runner.ts run baseline_500m

# Burst test (10x spike to 5B connections)
ts-node benchmark-runner.ts run burst_10x
```

### Run Scenario Groups

```bash
# Quick validation suite (~1 hour)
ts-node benchmark-runner.ts group quick_validation

# Standard test suite (~6 hours)
ts-node benchmark-runner.ts group standard_suite

# Full stress testing suite (~10 hours)
ts-node benchmark-runner.ts group stress_suite

# All scenarios (~48 hours)
ts-node benchmark-runner.ts group full_suite
```

### List Available Tests

```bash
ts-node benchmark-runner.ts list
```

## Benchmark Scenarios

### Baseline Tests

#### baseline_500m
- **Description**: Steady-state operation with 500M concurrent connections
- **Duration**: 3h 15m
- **Target**: P99 < 50ms, 99.99% availability
- **Use Case**: Production capacity validation

#### baseline_100m
- **Description**: Smaller baseline for quick validation
- **Duration**: 45m
- **Target**: P99 < 50ms, 99.99% availability
- **Use Case**: CI/CD integration, quick regression tests

### Burst Tests

#### burst_10x
- **Description**: Sudden spike to 5B concurrent (10x baseline)
- **Duration**: 20m
- **Target**: P99 < 100ms, 99.9% availability
- **Use Case**: Flash sale, viral event simulation

#### burst_25x
- **Description**: Extreme spike to 12.5B concurrent (25x baseline)
- **Duration**: 35m
- **Target**: P99 < 150ms, 99.5% availability
- **Use Case**: Major global event (Olympics, elections)

#### burst_50x
- **Description**: Maximum spike to 25B concurrent (50x baseline)
- **Duration**: 50m
- **Target**: P99 < 200ms, 99% availability
- **Use Case**: Stress testing absolute limits

### Failover Tests

#### regional_failover
- **Description**: Test recovery when one region fails
- **Duration**: 45m
- **Target**: <10% throughput degradation, <1% errors
- **Use Case**: Disaster recovery validation

#### multi_region_failover
- **Description**: Test recovery when multiple regions fail
- **Duration**: 55m
- **Target**: <20% throughput degradation, <2% errors
- **Use Case**: Multi-region outage preparation

### Workload Tests

#### read_heavy
- **Description**: 95% reads, 5% writes (typical production workload)
- **Duration**: 1h 50m
- **Target**: P99 < 50ms, 99.99% availability
- **Use Case**: Production simulation

#### write_heavy
- **Description**: 70% writes, 30% reads (batch indexing scenario)
- **Duration**: 1h 50m
- **Target**: P99 < 80ms, 99.95% availability
- **Use Case**: Bulk data ingestion

#### balanced_workload
- **Description**: 50% reads, 50% writes
- **Duration**: 1h 50m
- **Target**: P99 < 60ms, 99.98% availability
- **Use Case**: Mixed workload validation

### Real-World Scenarios

#### world_cup
- **Description**: Predictable spike with geographic concentration (Europe)
- **Duration**: 3h
- **Target**: P99 < 100ms during matches
- **Use Case**: Major sporting event

#### black_friday
- **Description**: Sustained high load with periodic spikes
- **Duration**: 14h
- **Target**: P99 < 80ms, 99.95% availability
- **Use Case**: E-commerce peak period

## Running Benchmarks

### Basic Usage

```bash
# Set environment variables
export BASE_URL="https://ruvector.example.com"
export REGION="us-east1"

# Run single test
ts-node benchmark-runner.ts run baseline_500m

# Run with custom config
BASE_URL="https://staging.example.com" \
PARALLEL=3 \
ts-node benchmark-runner.ts group standard_suite
```

### With Claude Flow Hooks

```bash
# Enable hooks (default)
export ENABLE_HOOKS=true

# Disable hooks
export ENABLE_HOOKS=false

ts-node benchmark-runner.ts run baseline_500m
```

Hooks will automatically:
- Execute `npx claude-flow@alpha hooks pre-task` before each test
- Store results in swarm memory
- Execute `npx claude-flow@alpha hooks post-task` after completion

### Multi-Region Execution

To distribute load across regions:

```bash
# Deploy load generators to GCP regions
for region in us-east1 us-west1 europe-west1 asia-east1; do
  gcloud compute instances create "k6-${region}" \
    --zone="${region}-a" \
    --machine-type="n2-standard-32" \
    --image-family="ubuntu-2004-lts" \
    --image-project="ubuntu-os-cloud" \
    --metadata-from-file=startup-script=setup-k6.sh
done

# Run distributed test
ts-node benchmark-runner.ts run baseline_500m
```

### Docker Execution

```bash
# Build container
docker build -t ruvector-benchmark .

# Run test
docker run \
  -e BASE_URL="https://ruvector.example.com" \
  -v $(pwd)/results:/results \
  ruvector-benchmark run baseline_500m
```

## Understanding Results

### Output Structure

```
results/
  run-{timestamp}/
    {scenario}-{timestamp}-raw.json       # Raw K6 metrics
    {scenario}-{timestamp}-metrics.json   # Processed metrics
    {scenario}-{timestamp}-metrics.csv    # CSV export
    {scenario}-{timestamp}-analysis.json  # Analysis report
    {scenario}-{timestamp}-report.md      # Markdown report
    SUMMARY.md                            # Multi-scenario summary
```

### Key Metrics

#### Latency
- **P50 (Median)**: 50% of requests faster than this
- **P90**: 90% of requests faster than this
- **P95**: 95% of requests faster than this
- **P99**: 99% of requests faster than this (SLA target)
- **P99.9**: 99.9% of requests faster than this

**Target**: P99 < 50ms for baseline, <100ms for burst

#### Throughput
- **QPS**: Queries per second
- **Peak QPS**: Maximum sustained throughput
- **Average QPS**: Mean throughput over test duration

**Target**: 50M QPS for 500M baseline connections

#### Error Rate
- **Total Errors**: Count of failed requests
- **Error Rate %**: Percentage of requests that failed
- **By Type**: Breakdown (timeout, connection, server, client)
- **By Region**: Geographic distribution

**Target**: < 0.01% error rate (99.99% success)

#### Availability
- **Uptime %**: Percentage of time system was available
- **Downtime**: Total milliseconds of unavailability
- **MTBF**: Mean time between failures
- **MTTR**: Mean time to recovery

**Target**: 99.99% availability (52 minutes/year downtime)

#### Resource Utilization
- **CPU %**: Average and peak CPU usage
- **Memory %**: Average and peak memory usage
- **Network**: Bandwidth, ingress/egress bytes
- **Per Region**: Resource usage by geographic location

**Alert Thresholds**: CPU > 80%, Memory > 85%

#### Cost
- **Total Cost**: Compute + network + storage
- **Cost Per Million**: Queries per million queries
- **Per Region**: Cost breakdown by location

**Target**: < $0.50 per million queries

### Performance Score

Overall score (0-100) calculated from:
- **Performance** (35%): Latency and throughput
- **Reliability** (35%): Availability and error rate
- **Scalability** (20%): Resource utilization efficiency
- **Efficiency** (10%): Cost effectiveness

**Grades**:
- 90-100: Excellent
- 80-89: Good
- 70-79: Fair
- 60-69: Needs Improvement
- <60: Poor

### SLA Compliance

✅ **PASSED** if all criteria met:
- P99 latency < 50ms (baseline) or scenario target
- Availability >= 99.99%
- Error rate < 0.01%

❌ **FAILED** if any criterion violated

### Analysis Report

Each test generates an analysis report with:

1. **Statistical Analysis**
   - Summary statistics
   - Distribution histograms
   - Time series charts
   - Anomaly detection

2. **SLA Compliance**
   - Pass/fail status
   - Violation details
   - Duration and severity

3. **Bottlenecks**
   - Identified constraints
   - Current vs. threshold values
   - Impact assessment
   - Recommendations

4. **Recommendations**
   - Prioritized action items
   - Implementation guidance
   - Estimated impact and cost

### Visualization Dashboard

Open `visualization-dashboard.html` in a browser to view:

- Real-time metrics
- Interactive charts
- Geographic heat maps
- Historical comparisons
- Cost analysis

## Best Practices

### Before Running Tests

1. **Baseline Environment**
   - Ensure cluster is healthy
   - No active deployments or maintenance
   - Stable configuration

2. **Resource Allocation**
   - Sufficient load generator capacity
   - Network bandwidth provisioned
   - Monitoring systems ready

3. **Communication**
   - Notify team of upcoming test
   - Schedule during low-traffic periods
   - Have rollback plan ready

### During Tests

1. **Monitoring**
   - Watch real-time metrics
   - Check for anomalies
   - Monitor costs

2. **Safety**
   - Start with smaller tests (baseline_100m)
   - Gradually increase load
   - Be ready to abort if issues detected

3. **Documentation**
   - Note any unusual events
   - Document configuration changes
   - Record observations

### After Tests

1. **Analysis**
   - Review all metrics
   - Identify bottlenecks
   - Compare to previous runs

2. **Reporting**
   - Share results with team
   - Document findings
   - Create action items

3. **Follow-Up**
   - Implement recommendations
   - Re-test after changes
   - Track improvements over time

### Test Frequency

- **Quick Validation**: Daily (CI/CD)
- **Standard Suite**: Weekly
- **Stress Testing**: Monthly
- **Full Suite**: Quarterly

## Cost Estimation

### Load Generation Costs

Per hour of testing:
- **Compute**: ~$1,000/hour (distributed load generators)
- **Network**: ~$200/hour (egress traffic)
- **Storage**: ~$10/hour (results storage)

**Total**: ~$1,200/hour

### Scenario Cost Estimates

| Scenario | Duration | Estimated Cost |
|----------|----------|----------------|
| baseline_100m | 45m | $900 |
| baseline_500m | 3h 15m | $3,900 |
| burst_10x | 20m | $400 |
| burst_25x | 35m | $700 |
| burst_50x | 50m | $1,000 |
| read_heavy | 1h 50m | $2,200 |
| world_cup | 3h | $3,600 |
| black_friday | 14h | $16,800 |
| **Full Suite** | ~48h | **~$57,600** |

### Cost Optimization

1. **Use Spot Instances**: 60-80% savings on load generators
2. **Regional Selection**: Test in fewer regions
3. **Shorter Duration**: Reduce steady-state phase
4. **Parallel Execution**: Minimize total runtime

## Troubleshooting

### Common Issues

#### K6 Not Found
```bash
# Install k6
brew install k6  # macOS
sudo apt install k6  # Linux
choco install k6  # Windows
```

#### Connection Refused
```bash
# Check cluster endpoint
curl -v https://your-ruvector-cluster.example.com/health

# Verify network connectivity
ping your-ruvector-cluster.example.com
```

#### Out of Memory
```bash
# Increase Node.js memory limit
export NODE_OPTIONS="--max-old-space-size=8192"

# Use smaller scenario
ts-node benchmark-runner.ts run baseline_100m
```

#### High Error Rate
- Check cluster health
- Verify capacity (not overloaded)
- Review network latency
- Check authentication/authorization

#### Slow Performance
- Insufficient load generator capacity
- Network bandwidth limitations
- Target cluster under-provisioned
- Configuration issues (connection limits, timeouts)

### Debug Mode

```bash
# Enable verbose logging
export DEBUG=true
export LOG_LEVEL=debug

ts-node benchmark-runner.ts run baseline_500m
```

### Support

For issues or questions:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://docs.ruvector.io
- Community: https://discord.gg/ruvector

## Advanced Usage

### Custom Scenarios

Create custom scenario in `benchmark-scenarios.ts`:

```typescript
export const SCENARIOS = {
  ...SCENARIOS,
  my_custom_test: {
    name: 'My Custom Test',
    description: 'Custom workload pattern',
    config: {
      targetConnections: 1000000000,
      rampUpDuration: '15m',
      steadyStateDuration: '1h',
      rampDownDuration: '10m',
      queriesPerConnection: 100,
      queryInterval: '1000',
      protocol: 'http',
      vectorDimension: 768,
      queryPattern: 'uniform',
    },
    k6Options: {
      // K6 configuration
    },
    expectedMetrics: {
      p99Latency: 50,
      errorRate: 0.01,
      throughput: 100000000,
      availability: 99.99,
    },
    duration: '1h25m',
    tags: ['custom'],
  },
};
```

### Integration with CI/CD

```yaml
# .github/workflows/benchmark.yml
name: Benchmark
on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly
  workflow_dispatch:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - name: Install k6
        run: |
          sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
          echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
          sudo apt-get update
          sudo apt-get install k6
      - name: Run benchmark
        env:
          BASE_URL: ${{ secrets.BASE_URL }}
        run: |
          cd benchmarks
          ts-node benchmark-runner.ts run baseline_100m
      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: benchmarks/results/
```

### Programmatic Usage

```typescript
import { BenchmarkRunner } from './benchmark-runner';

const runner = new BenchmarkRunner({
  baseUrl: 'https://ruvector.example.com',
  parallelScenarios: 2,
  enableHooks: true,
});

// Run single scenario
const run = await runner.runScenario('baseline_500m');
console.log(`Score: ${run.analysis?.score.overall}/100`);

// Run multiple scenarios
const results = await runner.runScenarios([
  'baseline_500m',
  'burst_10x',
  'read_heavy',
]);

// Check if all passed SLA
const allPassed = Array.from(results.values()).every(
  r => r.analysis?.slaCompliance.met
);
```

---

**Happy Benchmarking!** 🚀

For questions or contributions, please visit: https://github.com/ruvnet/ruvector
