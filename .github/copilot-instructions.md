# Quality Engineering Standards (Agentic QE)

## AQE MCP Server

This project uses Agentic QE for AI-powered quality engineering. The AQE MCP server provides tools for test generation, coverage analysis, quality assessment, and learning.

## Setup

Always call `fleet_init` before using other AQE tools to initialize the QE fleet.

## Available Tools

### Test Generation
- `test_generate_enhanced` — AI-powered test generation with pattern recognition and anti-pattern detection
- Supports unit, integration, and e2e test types

### Coverage Analysis
- `coverage_analyze_sublinear` — O(log n) coverage gap detection with ML-powered analysis
- Target: 80% statement coverage minimum, focus on risk-weighted coverage

### Quality Assessment
- `quality_assess` — Quality gate evaluation with configurable thresholds
- Run before marking tasks complete

### Security Scanning
- `security_scan_comprehensive` — SAST/DAST vulnerability scanning
- Run after changes to auth, security, or middleware code

### Defect Prediction
- `defect_predict` — AI analysis of code complexity and change history

### Learning & Memory
- `memory_store` — Store patterns and learnings for future reference
- `memory_query` — Query past patterns before starting work
- Always store successful patterns after task completion

## Best Practices

1. **Test Pyramid**: 70% unit, 20% integration, 10% e2e
2. **AAA Pattern**: Arrange-Act-Assert for clear test structure
3. **One assertion per test**: Test one behavior at a time
4. **Descriptive names**: `should_returnValue_when_condition`
5. **Mock at boundaries**: Only mock external dependencies
6. **Edge cases first**: Test boundary conditions, not just happy paths
