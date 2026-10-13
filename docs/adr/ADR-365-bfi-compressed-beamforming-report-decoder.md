# ADR 365: BFI compressed-beamforming-report decoder

| Field | Value |
|-------|-------|
| **Status** | Proposed (VHT SU decoder implemented + unit-tested on SYNTHETIC frames; no live capture yet) |
| **Date** | 2026-09-29 |
| **Deciders** | ruv |
| **Relates to** | ADR-118..123 (BFLD), ADR-141 (BFLD privacy control plane), ADR-323 (RTL8721Dx CSI), ADR-029 (multistatic) |

## Context

BFLD (ADR-118) is the plan to sense from 802.11 beamforming feedback (BFI) —
the compressed steering matrix a client returns to an AP after sounding. The
existing `wifi-densepose-bfld` crate implements the downstream envelope,
privacy gating, and sensing/identity surfaces, but it consumes the beamforming
angles only as an opaque `compressed_angle_matrix` blob. **Nothing in the tree
decodes a real 802.11 Compressed Beamforming Report** into structured angles.
That decoder is the missing front-end for every BFI use.

Two independent constraints shape this ADR:

1. **Capture is unproven on the hardware on hand.** Five monitor-mode captures
   on ruvultra's MediaTek MT7927 (idle, loaded, co-bound, and dedicated monitor
   on 5 GHz ch153/80 MHz with an active client speed test) yielded **zero**
   valid VHT/HE beamforming reports — only beacons and vendor action frames.
   The card's monitor path did not surface high-MCS unicast feedback at the
   sniffer's position (−75 dBm to the AP). Live capture therefore needs an
   Intel AX210 (the card KIT used) and/or a controlled BE550 network with the
   sniffer near the beamformed client. This is a hardware gate, not a code gate.

2. **Purpose boundary.** BFI is the signal KIT used to re-identify 197 people.
   This work targets device-free **presence/motion/tracking** and the
   **defensive privacy-exposure** analysis (ADR-121), on the operator's own
   network. Person re-identification of non-consenting people is out of scope;
   identity remains gated behind enrollment + consent per ADR-120/141.

## Decision

Add a pure-Rust `cbr` module to `wifi-densepose-bfld` that decodes a VHT
Compressed Beamforming Action frame body into structured feedback:

- `VhtMimoControl` — Nc, Nr, channel width, grouping Ng, codebook, SU/MU,
  segmentation, sounding token (802.11-2020 §9.4.1.50).
- `VhtBeamform` — per-stream average SNR, per-subcarrier Φ/ψ angle codes, the
  quantization widths, and dequantization to radians.
- Angle count from the ordered Givens decomposition
  `Σ_{i=1}^{min(Nc,Nr-1)}(Nr−i)`, subcarrier counts from Table 9-91, codebook
  widths from Table 9-92.

Honesty boundary encoded in the code and its tests:

- **VHT SU + MU** (both codebooks) are implemented. The angle bit widths
  (Table 9-92) are cross-checked against the Wi-BFI and WiPiCap reference
  decoders, which agree: SU (2,4)/(4,6), MU (5,7)/(7,9) as (psi,phi). The
  cross-check caught a real bug — an earlier draft used the MU widths for SU;
  `angle_bits_match_reference_decoders` now pins all four rows.
- **HE SU** (802.11ax) is decoded: 5-octet MIMO Control, SU widths as above,
  subcarrier count derived from payload length (WiPiCap approach, no RU table).
  HE MU/CQI feedback returns `Unsupported`.
- **EHT** returns `Unsupported`.
- Decoded angle *values* stay `CLAIMED` until a real captured frame decodes to
  a unitary steering matrix (WiPiCap's own check) — no over-the-air capture yet.

The decoder is allocation-light, `#![forbid(unsafe_code)]` (crate-wide), and
passes the crate's `-D warnings` clippy gate (pedantic + nursery).

### Offline ingest and motion features (software e2e)

Two further modules complete the software pipeline on `SYNTHETIC` data, so only
the physical capture remains:

- `capture` — parses a `LINKTYPE_IEEE802_11_RADIOTAP` pcap itself (no libpcap
  dependency), skips radiotap by its length field, extracts management
  Action / Action-No-Ack bodies, feeds each to the decoder, and returns a
  `ReportRateSummary` (reports/sec, sequence gaps, duplicates, decode
  failures) — the report-rate dashboard input the plan calls for.
- `features` — the "feedback angles" motion path: a bounded per-report summary
  (circular-mean `Phi`, mean `Psi`, mean SNR) and a temporal `motion_energy`
  primitive (circular `Phi` difference between consecutive reports), producing
  a motion time series from a capture. No steering matrix `V` is
  reconstructed, so no range/AoA is claimed.

Chain now runnable end to end in software: `pcap -> decode -> motion series`.
Not yet implemented: a trained sensing/tracking model, fusion with the CSI
boards, and — the gating item — a real capture. No motion or tracking accuracy
is claimed until a real capture with ground truth exists.

## Consequences

- The BFLD pipeline can be fed real beamforming angles once a capture path
  exists; until then the decoder is exercised on synthetic frames.
- MU codebook 1 / HE / EHT are explicit gaps, surfaced as typed errors so a
  capture of an unsupported format fails closed instead of producing garbage.
- The next deliverable is the one the plan calls for: a real capture on an
  AX210 or BE550-controlled link, a report-rate dashboard, and cross-decoding
  a captured frame against WiPiCap to promote angle values from CLAIMED to
  MEASURED.

## Validation

```bash
cd v2 && cargo test -p wifi-densepose-bfld --lib cbr::
cd v2 && cargo clippy -p wifi-densepose-bfld --features mqtt --all-targets -- -D warnings
```

6 cbr unit tests pass (angle-count table, MIMO Control round-trip, 2×2 SU
end-to-end round-trip, MU-codebook-1 refusal, non-beamforming rejection,
truncation). Full crate suite green.

## References

- IEEE 802.11-2020 §9.4.1.50 (VHT MIMO Control), §9.4.1.51 (VHT Compressed
  Beamforming Report), Tables 9-91/9-92.
- WiPiCap (github.com/sarulab-ou/WiPiCap) — ac/ax reference decoder, MIT.
- Wi-BFI (github.com/kfoysalhaque/Wi-BFI) — independent comparison (omits 2×2).
