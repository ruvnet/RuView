# ADR-421: Validate direct training gate score ranges

- Status: proposed
- Date: 2026-10-30
- Related: ADR-369 (training evidence gate)

## Context

The exported trainingGate API accepts finite scores outside the existing 0–100 schema contract, allowing meaningless baseline-relative results. MCP and SDK schema validation already bounds those inputs.

## Decision

Apply the same finite 0–100 range inside the public training function. Refuse out-of-range model or baseline scores before computing a delta or evidence statement. Keep valid fractional and percentage interpretations unchanged.

## Verification

Identical public-export regressions produce two failures and one passing control on upstream, then all three pass with this change. Existing training tests run alongside them. This establishes software input validation only; it does not measure a model, hardware or sensing accuracy.
