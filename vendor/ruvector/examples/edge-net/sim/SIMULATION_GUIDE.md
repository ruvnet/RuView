# Edge-Net Genesis Phase Simulation Guide

## Overview

This simulation framework models the complete lifecycle of the Edge-Net distributed compute network from genesis bootstrap through full decentralization.

## Quick Start

```bash
# Install dependencies
npm install

# Run quick demo (60 seconds)
node examples/quick-demo.js

# Run tests
npm test

# Run full simulation
npm run sim:full
```

## Architecture

### Components

1. **SimNode** - Individual network node with economic state and behavior
2. **NetworkSimulation** - Overall network orchestration
3. **EconomicTracker** - rUv distribution and economic health
4. **PhaseManager** - Lifecycle phase management

### Phases

| Phase | Nodes | Key Features |
|-------|-------|--------------|
| Genesis | 0-10K | 10x multiplier, network bootstrap |
| Transition | 10K-50K | Genesis connection limiting, multiplier decay |
| Maturity | 50K-100K | Genesis read-only, self-sustaining |
| Post-Genesis | 100K+ | Genesis retired, full decentralization |

## Key Metrics

### Network Health
- Active node count
- Task completion rate
- Success rate (target: >85%)
- Network health score (target: >0.7)

### Economic Health
- Total rUv supply and distribution
- Economic velocity (target: >0.3)
- Utilization rate (target: >0.5)
- Stability index (target: >0.6)

### Genesis Sunset
- Genesis node count and status
- Connection limits over time
- Multiplier decay effectiveness
- Network resilience without genesis

## Distribution Model

All rUv rewards distributed as:
- 70% → Contributors (direct rewards)
- 15% → Treasury (network operations)
- 10% → Protocol Fund (core development)
- 5% → Founders (vested rewards)

## Contribution Multiplier

```
multiplier = 1 + 9 * e^(-network_compute / 1,000,000)

Milestones:
  0 hours → 10.0x (genesis)
  100K hours → 9.1x
  500K hours → 6.1x
  1M hours → 4.0x
  10M+ hours → 1.0x (baseline)
```

## Validation Criteria

### Genesis Phase
- ✓ At least 1 genesis node active
- ✓ High multiplier (≥5.0x)
- ✓ Stable connectivity

### Transition Phase
- ✓ Genesis connections limited (≤500)
- ✓ Network resilience (≥0.7)
- ✓ Task routing success (≥0.85)

### Maturity Phase
- ✓ Genesis read-only
- ✓ Economic health (≥0.75)
- ✓ Self-sustaining

### Post-Genesis
- ✓ All genesis retired
- ✓ Network stability (≥0.8)
- ✓ Economic equilibrium (≥0.7)

## Usage Examples

### Run Specific Phase

```bash
# Genesis only
npm run sim:genesis

# Through transition
npm run sim:transition

# Through maturity
npm run sim:maturity
```

### Visualize Results

```bash
# Auto-detect latest report
npm run visualize

# Specific report
node scripts/visualize.js reports/simulation-all-2025-01-01.json
```

### Generate Reports

```bash
npm run report
```

Creates markdown reports with:
- Executive summary
- Network & economic metrics
- Phase transition timeline
- Genesis node performance
- Validation results
- Recommendations

## E2B Integration (Optional)

For cloud-scale simulation:

```javascript
import { Sandbox } from '@e2b/sdk';

const sandbox = await Sandbox.create();
await sandbox.filesystem.write('/sim/config.json', config);
await sandbox.process.start('npm run sim:full');
const report = await sandbox.filesystem.read('/sim/reports/latest.json');
```

## Troubleshooting

**Slow simulation?**
- Use `--fast` flag
- Target specific phase
- Reduce node count

**Out of memory?**
- Limit target nodes
- Use E2B sandbox
- Reduce history tracking

**Phase not transitioning?**
- Check node join rate
- Review phase thresholds
- Verify node churn rate

## Performance

| Target | Time | Real-Time |
|--------|------|-----------|
| 10K nodes | ~10s | ~30 days |
| 50K nodes | ~45s | ~150 days |
| 100K nodes | ~90s | ~300 days |
| 150K nodes | ~135s | ~450 days |

*With 10,000x acceleration*

## Output Files

Saved to `reports/`:
- `simulation-{phase}-{timestamp}.json` - Raw data
- `simulation-{phase}-{timestamp}.md` - Report

## Contributing

Focus areas:
- Additional economic models
- Advanced node behaviors
- Real-world network patterns
- Performance optimizations
- Visualization enhancements

## License

MIT License

---

Built for the Edge-Net distributed compute intelligence network.
