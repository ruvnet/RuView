# Edge-Net Lifecycle Simulation - Documentation Index

## Quick Navigation

### Getting Started
1. **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** - Start here! Quick overview and reference
2. **[USAGE.md](USAGE.md)** - Complete usage guide with examples
3. **[README.md](README.md)** - Project overview (existing edge-net simulation docs)

### Technical Documentation
4. **[SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md)** - Deep dive into architecture and design

### Source Code
5. **[src/](src/)** - All TypeScript source files
   - `cell.ts` - Node simulation
   - `network.ts` - Network state management
   - `metrics.ts` - Performance tracking
   - `phases.ts` - Phase transition logic
   - `report.ts` - Report generation
   - `simulator.ts` - Main orchestrator

## Documentation Hierarchy

```
Index (you are here)
├── Quick Start
│   ├── PROJECT_SUMMARY.md ⭐ Start here
│   └── USAGE.md
├── Architecture
│   └── SIMULATION_OVERVIEW.md
├── Project Overview
│   └── README.md
└── Source Code
    └── src/*.ts
```

## By Use Case

### I want to run the simulation
→ **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** (Quick Reference section)
→ **[USAGE.md](USAGE.md)** (Quick Start section)

### I want to understand how it works
→ **[SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md)** (Architecture section)
→ **[USAGE.md](USAGE.md)** (Understanding Output section)

### I want to modify the simulation
→ **[SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md)** (Component Details)
→ **[USAGE.md](USAGE.md)** (Customizing section)
→ **Source code:** `src/*.ts`

### I want to understand the results
→ **[USAGE.md](USAGE.md)** (Understanding Output + Interpreting Results)
→ **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** (Output Example section)

### I want to integrate with Edge-Net
→ **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** (Integration section)
→ **[SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md)** (Integration section)

## By Topic

### Architecture
- **Components:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Component Details
- **Data Flow:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Execution Flow
- **Algorithms:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Network Topology

### Economics
- **Energy Model:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Economic Model
- **Sustainability:** [USAGE.md](USAGE.md) § Interpreting Results
- **Parameters:** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Configuration Defaults

### Phases
- **Phase 1 (Genesis):** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Simulation Phases
- **Phase 2 (Growth):** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Simulation Phases
- **Phase 3 (Maturation):** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Simulation Phases
- **Phase 4 (Independence):** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Simulation Phases
- **Transitions:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Phases

### Validation
- **Criteria:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Validation Framework
- **Interpreting:** [USAGE.md](USAGE.md) § Interpreting Results
- **Success/Failure:** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Exit Codes

### Performance
- **Metrics:** [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) § Performance
- **Optimization:** [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) § Performance Optimization
- **Benchmarks:** [USAGE.md](USAGE.md) § Performance Tips

## File Reference

### Documentation Files

| File | Size | Lines | Purpose |
|------|------|-------|---------|
| INDEX.md | This file | Quick navigation |
| PROJECT_SUMMARY.md | 15 KB | 540 | Quick reference and overview |
| USAGE.md | 10 KB | 420 | Complete usage guide |
| SIMULATION_OVERVIEW.md | 18 KB | 650 | Technical architecture |
| README.md | 2 KB | 63 | Project overview (existing) |

### Source Files

| File | Size | Lines | Purpose |
|------|------|-------|---------|
| src/cell.ts | 5.7 KB | 230 | Node simulation |
| src/network.ts | 9.6 KB | 310 | Network management |
| src/metrics.ts | 9.6 KB | 280 | Performance tracking |
| src/phases.ts | 7.3 KB | 180 | Phase transitions |
| src/report.ts | 8.4 KB | 270 | Report generation |
| src/simulator.ts | 6.1 KB | 210 | Main orchestrator |

### Configuration Files

| File | Purpose |
|------|---------|
| package.json | NPM dependencies and scripts |
| tsconfig.json | TypeScript compiler configuration |
| .gitignore | Git ignore rules |

## Quick Command Reference

```bash
# Installation
npm install

# Run simulation
npm run simulate          # Normal mode
npm run simulate:fast     # Fast mode
npm run simulate:verbose  # Verbose mode

# Build
npm run build            # Compile TypeScript
npm run clean            # Clean build artifacts
```

## Reading Order for New Users

### Option 1: Quick Start (10 minutes)
1. [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) - Read "Quick Reference" section
2. Run `npm install && npm run simulate:fast`
3. Review console output and JSON report

### Option 2: Comprehensive (30 minutes)
1. [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) - Full read
2. [USAGE.md](USAGE.md) - "Understanding Output" section
3. Run `npm run simulate`
4. [USAGE.md](USAGE.md) - "Interpreting Results" section

### Option 3: Technical Deep Dive (1-2 hours)
1. [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) - Overview
2. [SIMULATION_OVERVIEW.md](SIMULATION_OVERVIEW.md) - Full read
3. [USAGE.md](USAGE.md) - "Customizing" section
4. Source code review: `src/*.ts`
5. Run multiple scenarios

## Key Concepts

### Must-Know Terms
- **Cell:** Individual network node (simulated E2B sandbox)
- **Energy (rUv):** Simulated cryptocurrency for operations
- **Genesis Node:** Bootstrap node with 10x multiplier
- **Phase:** Lifecycle stage (Genesis, Growth, Maturation, Independence)
- **Sustainability:** Earned/spent energy ratio (must be > 1.0)
- **Preferential Attachment:** New nodes connect to high-fitness nodes

### Phase Milestones
- **10K nodes:** Genesis → Growth
- **50K nodes:** Growth → Maturation
- **100K nodes:** Maturation → Independence
- **120K nodes:** Simulation complete

### Validation Thresholds
- **Genesis multiplier:** 10.0x initially
- **Energy accumulation:** > 1000 rUv in genesis
- **Success rate:** > 70% task completion
- **Sustainability:** > 1.0 earned/spent ratio
- **Connectivity:** > 5 avg connections (genesis), > 10 (maturation)

## Troubleshooting Guide

### Build Errors
→ [USAGE.md](USAGE.md) § Troubleshooting

### Runtime Errors
→ [USAGE.md](USAGE.md) § Troubleshooting

### Validation Failures
→ [USAGE.md](USAGE.md) § Interpreting Results § Critical Issues

### Performance Issues
→ [USAGE.md](USAGE.md) § Performance Tips

## External References

### Related Edge-Net Documentation
- `/workspaces/ruvector/examples/edge-net/architecture.md` - Network architecture
- `/workspaces/ruvector/examples/edge-net/economic-model.md` - Economic details
- `/workspaces/ruvector/examples/edge-net/deployment.md` - Deployment guide

### RuVector Project
- `/workspaces/ruvector/README.md` - Main project README
- `/workspaces/ruvector/docs/` - RuVector documentation

## Glossary

| Term | Definition |
|------|------------|
| Cell | Simulated network node (maps to E2B sandbox) |
| rUv | Resource Utility Voucher (simulated energy/currency) |
| Genesis Node | Bootstrap node with 10x earning multiplier |
| Regular Node | Standard network node with 1x multiplier |
| Phase | Lifecycle stage of network development |
| Sustainability | Economic viability (earned/spent > 1.0) |
| Preferential Attachment | Topology algorithm favoring high-fitness nodes |
| Fitness Score | Weighted capability score for node selection |
| Genesis Sunset | Graceful retirement of bootstrap nodes |
| P2P Independence | Fully decentralized network operation |

## Version History

### v1.0.0 (2025-12-31)
- ✅ Initial release
- ✅ Complete 4-phase lifecycle simulation
- ✅ Economic model with sustainability tracking
- ✅ Automatic validation framework
- ✅ JSON report generation
- ✅ Comprehensive documentation

## Contact & Support

For issues, questions, or contributions:
1. Check this documentation first
2. Review source code comments
3. Consult Edge-Net architecture docs
4. Refer to RuVector project documentation

---

**Navigation Tips:**
- Use Ctrl+F to search within documents
- All links are relative and work in GitHub/VSCode
- Start with PROJECT_SUMMARY.md for quickest orientation
- SIMULATION_OVERVIEW.md for technical deep dive
- USAGE.md for practical how-to guides

**Last Updated:** 2025-12-31
**Documentation Version:** 1.0.0
