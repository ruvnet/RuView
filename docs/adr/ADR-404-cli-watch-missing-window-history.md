# ADR-404: Preserve missing capture windows in CLI watch trends

- Status: proposed
- Date: 2026-10-11
- Related: ADR-375 (per-node watch rate sparklines)

## Context

Watch draws repeated timed capture windows, but appends rate history only for
nodes present in a window. A present–absent–present node is therefore shown as
two adjacent observations, hiding the intervening outage. The sparkline helper
also discards missing positions, so merely adding null history entries would
still compress time. An actual terminal-mode CLI and owned loopback UDP replay
reproduces the false continuous two-bar trend.

## Decision

Append a missing entry for each previously observed node absent from a capture
window. Keep the existing 30-window bound. Render each missing position as `·`,
while scaling finite rates using only finite observations. Missing means no
observation in that window; it is not a measured zero rate. Initial history
still starts when a node is first observed. JSON capture output is unchanged.

## Verification

The public CLI receives one valid packet, an empty one-second capture, and one
returning packet. Its terminal row must show `▄·▄` instead of `▄▄`. A real child
process executes the CLI; only stdout's TTY capability is supplied by the test.
The test owns and cleans up its loopback sockets and child. Finite, zero,
all-missing and empty sparkline controls preserve scale and positions. This is
software replay, not a claim about radio hardware availability.
