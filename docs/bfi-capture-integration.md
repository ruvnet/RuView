# BFI capture → decode integration (for the cluster)

The decoder side (ADR-365, PR #2053) is done and reference-validated. Any node
that can capture beamforming feedback hands the decoder a **radiotap pcap**;
nothing else is coupled. This is the seam.

## Capture contract (what a capturing node must produce)

- A pcap (classic `.pcap`, microsecond or nanosecond) with link type
  **`LINKTYPE_IEEE802_11_RADIOTAP` (127)** — i.e. 802.11 frames with a radiotap
  header. `tcpdump -y IEEE802_11_RADIO -w out.pcap` on a monitor interface.
- It must contain management **Action / Action No Ack** frames carrying:
  - **VHT** compressed beamforming: category `21`, action `0`, or
  - **HE** compressed beamforming: category `30`, action `0`.
- Beamforming feedback exists only on **5/6 GHz** (VHT/HE). A 2.4 GHz capture
  has none. The client being sensed must be associated on 5/6 GHz and actively
  sounded (active downlink triggers sounding).

Capture-host options (any one is enough):
- **Nexmon on a BCM Pi** (cognitum-v0 / Pi cluster) — ADR-123's intended host;
  the CBFR filter is the same code path as its CSI capture.
- **Intel AX210** in monitor mode (the card KIT used).
- A co-bound monitor vif while associated (works where dedicated monitor RX is
  weak, e.g. the MediaTek MT7927 on ruvultra).

## Decode + validate (what any node runs)

```sh
# branch feat/bfld-cbr-parser (PR #2053)
cargo run -p wifi-densepose-bfld --example bfi_decode -- capture.pcap
```

Prints a report-rate dashboard (decoded/sec, sequence gaps, dupes, failures),
the first report's dimensions, and a motion-energy series. Library entry points
for programmatic use:

- `wifi_densepose_bfld::capture::decode_pcap(&bytes)` → VHT **and HE** reports
  (`Report::Vht` / `Report::He`) + `ReportRateSummary`
- `wifi_densepose_bfld::cbr::{parse_vht_action, parse_he_action}` → structured angles
- `wifi_densepose_bfld::features::motion_series(&reports)` → motion signal
- `wifi_densepose_bfld::steering::{reconstruct_v, unitarity_error}` → V + the
  correctness gate

## The validation gate (CLAIMED → MEASURED)

Reconstruct V from a captured frame and check `unitarity_error(&v)`. A real
capture whose V is unitary (< ~1e-6) confirms the decode end to end and is what
promotes the pipeline from CLAIMED to MEASURED — no code change needed, just a
real pcap. Decode math is already cross-checked against Wi-BFI and WiPiCap
(bit widths, angle ordering, dequantization) and unitary on synthetic angles.

## Not yet

- HE MU/CQI and EHT (802.11be) return `Unsupported` — need a reference/capture.
- Motion pairs are only formed between reports of matching format and
  dimensions (a VHT→HE switch is a reconfiguration, not motion).
