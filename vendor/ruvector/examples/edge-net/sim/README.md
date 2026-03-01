# Edge-Net Genesis Phase Simulation

A comprehensive simulation framework for testing the Edge-Net distributed compute network lifecycle, from genesis bootstrap to full decentralization.

## Overview

This simulation models the complete lifecycle of the Edge-Net network across four distinct phases:

1. **Genesis Phase (0 - 10K nodes)**: Network bootstrap with genesis nodes providing foundation
2. **Transition Phase (10K - 50K nodes)**: Genesis sunset preparation and network resilience testing
3. **Maturity Phase (50K - 100K nodes)**: Genesis read-only mode, full self-sustenance
4. **Post-Genesis Phase (100K+ nodes)**: Complete decentralization, genesis retirement

## Features

- Realistic Node Behavior: Simulates node joining, leaving, task processing, and economic activity
- Economic Modeling: Tracks rUv (Resource Utility Vouchers) distribution, treasury, and protocol sustainability
- Phase Transitions: Automatic detection and validation of lifecycle phase transitions
- Genesis Sunset: Models the graceful retirement of genesis nodes as the network matures
- Health Monitoring: Comprehensive network health metrics and economic indicators
- Visualization: ASCII charts and detailed reports of simulation results
- Validation: Test suite to ensure simulation accuracy

## Installation

```bash
cd /workspaces/ruvector/examples/edge-net/sim
npm install
```

## Quick Start

Run a full lifecycle simulation:

```bash
npm run sim:full
```

Run specific phases:

```bash
npm run sim:genesis      # Genesis phase only (0-10K nodes)
npm run sim:transition   # Through transition (0-50K nodes)
npm run sim:maturity     # Through maturity (0-100K nodes)
```

## Testing

```bash
npm test
```

## Documentation

See full documentation in this README file for:
- Command line options
- Simulation architecture
- Phase details
- Economic model
- Visualization and reports
- E2B integration

Built with edge-net for distributed compute intelligence.
