# ADR-420: Classify equivalent ESPHome IPv6 addresses consistently

- Status: proposed
- Date: 2026-10-29
- Related: ADR-373 (private host device access)

## Context

The existing private-host policy allows loopback and mapped private IPv4 addresses, but compares raw IPv6 strings. Expanded loopback and hexadecimal mapped spellings are rejected before connection despite identifying the same allowed address (RFC 4291 sections 2.2, 2.5.3 and 2.5.5.2).

## Decision

Canonicalize validated IPv6 addresses for classification only. Decode only the exact mapped prefix before applying the existing IPv4 range policy. Preserve the original connection address, including link-local scope identifiers. Classify unique-local and link-local prefixes with numeric first-hextet masks. Public, multicast, unspecified and nonmapped lookalike addresses are refused. No dependency or device-access grant changes.

## Verification

The same focused suite reproduces 31 failures on the original source and passes all 125 cases on the candidate. Range-edge controls and injected connector observations cover notation equivalence and early rejection; scoped addresses reach the connector unchanged. Existing ESPHome tests additionally exercise owned TCP fixtures. These are software tests, not physical-device or sensing-accuracy evidence. Broader gate receipts accompany the PR qualification.
