# ADR-397: Distinguish ESPHome received updates from retained samples

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 Amendment 3 (bounded ESPHome sessions)

## Context

ESPHome sessions count every incoming state update, but each entity retains only
its first 10,000 samples. The entity report used the retained length as `updates`,
understating actual arrivals once this cap was reached. Mean values also came from
that bounded subset without exposing how many updates were omitted.

## Decision

Maintain a separate arrival counter per entity. Report that count as `updates`,
retain `sampledUpdates` for the aggregation subset, and disclose `droppedSamples`.
Preserve the existing buffer bound and latest-value behavior. Existing hand-built
summary sessions without the new counter fall back to their retained sample count.

## Verification

An owned loopback native-protocol fixture emits 10,001 distance state updates
through the public reader. The baseline reports session=10,001 but entity=10,000.
The correction reports received=10,001, retained=10,000, omitted=1, with unchanged
50 cm mean. An existing small multi-entity fixture verifies zero omitted samples
and correct ordinary counts. This is software aggregation verification only.
