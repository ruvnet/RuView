# ADR-400: Decode fused vitals into node diagnostics

- Status: proposed
- Date: 2026-10-11
- Related: ADR-063 (fusion), ADR-373 (node diagnostics)

## Context

The capture parser recognizes firmware magic `0xC5110004` as fused vitals but
returns only a byte count. Node aggregation then omits its RSSI and physiological
values. A valid fused-only stream therefore has null `lastVitals` even though the
firmware supplies the current fused heart and breathing estimates.

## Decision

Decode the exact 48-byte packed `edge_fused_vitals_pkt_t` from
`firmware/esp32-csi-node/main/edge_processing.h`. Share the common fields and fixed
point unit conversions with ordinary vitals, then expose the mmWave presence,
type, confidence, raw heart/breathing values, distance in centimetres, and target
count from their actual offsets. Aggregate fused reports into RSSI and latest
vitals while retaining their distinct packet kind counter. Reject truncated
fused reports before they can count as decoded node data.

## Verification and limits

A firmware-layout fixture enters the public capture over owned loopback UDP.
The baseline counts the packet but returns null RSSI and latest vitals. Fixed
assertions check 72 bpm fused heart, 15 bpm breathing, 120 cm distance, confidence
and target values, and timestamp units. Truncated-prefix tests and existing
ordinary-vitals tests remain controls. These are decoded device-reported values;
no clinical accuracy, real radio, or sensor-fusion quality is established.
