# ADR-413: Preserve complete UDP datagrams in the CSI relay

- Status: proposed
- Date: 2026-10-11
- Related: ADR-018 (ESP32 CSI packet layout)

## Context

The public Python relay promises byte-for-byte forwarding without parsing or
mutation. Its 2048-byte receive buffer truncates larger datagrams; a valid
ADR-018 shape with four antennas and 256 subcarriers occupies 2068 bytes. Its
truthiness check also discards empty UDP datagrams instead of forwarding them.
Both behaviors violate the transparent relay contract.

## Decision

Receive up to 65535 bytes, sufficient for a complete IPv4 UDP payload, and
forward every received datagram, including an empty payload. Retain None as the
socket-timeout sentinel. Preserve bind defaults, destination selection,
reporting and cleanup. Introduce no packet interpretation or hardware policy.

## Verification

Actual public Python CLI relays traffic between owned loopback sockets. The
baseline truncates a complete 2068-byte ADR-018 packet and fails to forward an
empty datagram; a small binary payload is unchanged. Fixed runs compare every
byte and retain both controls. All eight harness gates run with these actual
subprocess tests. This proves local datagram preservation, not physical CSI
capture quality or network performance.
