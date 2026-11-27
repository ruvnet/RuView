# ADR-422: Prepare room training bank output as a file

- Status: proposed
- Date: 2026-10-30
- Related: ADR-369 (training execution boundary)

## Context

The room trainer defaults to room-bank.json and writes a JSON file, while the harness creates that output itself as a directory before execution. Default and nested room runs therefore fail, and an existing regular bank file cannot be reused. Pose training outputs remain checkpoint directories.

## Decision

Create only the parent of room bank files. Preserve directory preparation for pose outputs. Before creating new parents, validate existing output ancestors through the existing trusted-checkout path rules. Return a structured preparation failure instead of an uncaught filesystem exception. Do not delete output directories left by prior failed runs.

## Verification

The same filesystem/public-function suite fails five of seven cases on upstream and passes all seven after the change; existing training controls also pass. Independent path-ancestry checks cover outside ancestors and dangling links without trainer execution. The Rust room trainer is a source oracle only; no Rust training, model inference or physical device was executed. Path validation does not prevent a concurrent replacement race.
