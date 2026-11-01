# ADR-389: Measure CSI rates over the observed capture interval

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (receive-only device diagnostics)

## Context

The public `captureEsp32` producer may stop at `max_packets` before its configured
`seconds` deadline. Its node CSI rates still divided received packets by that
requested deadline. Two packets received in approximately 150 milliseconds with a
30-second deadline therefore appeared as 0.07 Hz, misleading operators evaluating
whether a node streams normally.

## Decision

Start a monotonic clock when the UDP socket successfully binds. At completion,
calculate `elapsedSeconds` and use it as the denominator for node CSI rates.
Retain `seconds` as the configured deadline for compatibility and expose the
observed interval separately. Close and analysis processing time are excluded.
A socket error before binding has a zero observed interval and no received data.

## Verification and limits

A deterministic-format ESP32 fixture is transmitted through an owned loopback UDP
socket into the public capture function. The baseline fails because the observed
interval is absent and the rate is 0.07 Hz. The fixed capture checks early
termination, preserves the deadline, and checks the rate against observed elapsed
time. Existing CSI decoding and capture tests remain controls. This is software
transport verification; it makes no radio, hardware, sensing-accuracy, or firmware
throughput claim. Normal deadline captures may report a slightly longer interval
because timer callbacks are scheduled by the host event loop.
