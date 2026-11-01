# ADR-364: BFI capture admission and continuity

- **Status**: proposed
- **Date**: 2026-09-29
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: bfi, capture, evidence, continuity, hardware

## Context

The Archer BE550, associated iPhone and MT7927 capture host have produced
readable beamforming feedback information (BFI). Monitor-mode support alone
does not establish this capability, and readable reports do not establish
continuous sensing. Router channel width also does not guarantee an identical
feedback grid in every report.

**MEASURED:** two saved trials contain 66 and 96 unique reports. During their
approximately 30-second sender intervals, rates were 2.2 and 3.2 reports/s,
with maximum silence of 8.392 and 6.437 seconds. No reports followed in the
approximately 15-second post-traffic intervals. All 162 reports received an
independent tshark angle/control/SNR comparison. Reproducer commands, capture
hashes and limitations are in the [capture results](../../tools/bfi/OPTIMIZATION.md)
and [continuity report](../../tools/bfi_learning/CAPTURE-CONTINUITY.md).

## Decision

Admit a hardware/driver/profile combination only from its actual packet
evidence. Apply these boundaries before using its output for sensing:

1. Keep originals and receipts outside Git, with raw own-network captures on
   the capture host. Record hardware, driver, channel, source hashes, capture
   interval and restoration outcome. Select the authorized AP/client direction
   explicitly; missing provenance cannot be inferred from an SSID.
2. Enforce container, radiotap, MAC, length, fragmentation and supported-format
   checks in [bfi_decode.py](../../tools/bfi/bfi_decode.py). Validate FCS when
   present and reject flagged corruption; absent FCS is a recorded limitation.
   Unsupported tuples remain counted rejections. Compare every admitted angle
   against an independent dissector before extending a format's support claim.
3. Deduplicate retries over the whole capture before slicing time intervals.
   Preserve integer timestamps and separate feedback grids. Sequence numbers
   and sounding tokens do not measure lost packets. BFI steering matrices are
   not full CSI; the identity product of a square unitary matrix with its
   conjugate transpose cannot serve as a changing channel feature.
4. Preserve full-observation quality metrics and add separate before-traffic,
   sender-interval and after-traffic metrics. Include leading/trailing silence,
   report rate, maximum gap and duration-weighted occupied one-second bins.
   Sender timestamps bound attempted UDP sends, not verified over-air sounding.
5. Use an initial continuity gate of reports in at least 27 of the first 30
   complete one-second sender bins and no silence longer than two seconds
   anywhere in the actual sender interval, repeated in three comparable trials.
   Report any partial final bin separately; never count it as a complete second.
   This is a project engineering gate, not a published sensing accuracy result.

The current traffic receipt lacks a PCAP hash and AP/client binding. The
analyzer checks caller-selected file associations and hashes all inputs in its
output; this establishes content integrity, not authenticated session binding.
Stronger binding is required before automated capture admission can treat
receipt association as independently verified.

## Alternatives

Monitor-mode declarations, average report rate alone and interpolating missing
reports would each hide a different failure. Retain explicit packet validation
and interval coverage instead. ESP32/Realtek CSI remains a separate modality;
multiple unsynchronized boards do not establish a coherent BFI array.

## Consequences

### Positive

Packet validity, capture continuity and sensing accuracy have separate,
reproducible acceptance criteria. Changes in denominators cannot appear as RF
performance gains.

### Negative

Strict format and continuity gates can exclude usable but sparse observations.
New PHY formats and hardware require additional captures and independent checks.

### Neutral

The current trials fail continuity. Further motion modeling requires a new
capture result; this ADR does not authorize hardware changes or establish
identity recognition.

## Validation and links

Run `python -B -m unittest discover -s tools/bfi -v` and the focused
`test_traffic_coverage.py` suite under `tools/bfi_learning`. Synthetic tests
check parsing and interval rules; acceptance additionally requires retained
real captures and the three successful trials above.

- Extends proposed [ADR-118](ADR-118-bfld-beamforming-feedback-layer-for-detection.md)
  and [ADR-123](ADR-123-bfld-capture-path-nexmon-and-esp32.md); neither is superseded.
- Preserves [ADR-120](ADR-120-bfld-privacy-class-and-hash-rotation.md)'s privacy boundary.
- Public-data admission has a separate contract in
  [ADR-365](ADR-365-public-bfi-dataset-and-decoder-contract.md).
