# BFI acceptance evidence

For the later decoder speedup, additional validated HE widths and higher-traffic
trials, see [optimization evidence](OPTIMIZATION.md). The runs below document the
initial acceptance and its negative repeats.

## Result: first capture/decode/rate acceptance passed

**MEASURED, September 29, 2026 local time (September 30 UTC):** an Archer BE550
and an associated iPhone 17 produced 21 complete HE beamforming reports captured
by an MT7927. Every quantized angle, selected MIMO control field, and signed SNR
matched the independent tshark 4.2.2 dissector. A second reviewer checked the
capture independently. The coordinator reproduced the comparison through the CLI.

This establishes the first hardware acceptance test for the format below.
**Sustained sampling, production BFLD ingestion, and movement sensing remain
unvalidated.** The next equal-duration capture and the final runner validation
both produced no reports.

## Validated real report format and rate

| Measurement | Result |
|---|---|
| Feedback | HE single-user, 2 rows × 2 columns, 80 MHz, Ng=4, codebook 1 |
| Frequency | Primary 5180 MHz; center 5210 MHz; verified before and after |
| Complete reports | 21, all from the intended client to the AP |
| Independent comparison | 21/21 matched; 250 angle pairs/report; 10,500 angle values total |
| Capture duration | 45.033401777 seconds |
| Captured unique report rate | 0.466320535 reports/second over the full capture |
| Between-report gaps within the burst | Median 0.2347655 s; p95 1.330627 s; maximum 1.559962 s |
| Report burst span | 7.516041 seconds, entirely within the traffic interval |
| Retry/deduplication | Distinct sequence numbers, no Retry bits, no discarded duplicates |
| Median report signal | −82 dBm in the first reported antenna sample |
| Capture integrity | No truncation, timestamp regressions, or indicated bad FCS |
| Kernel drops | 0; this does not measure RF or firmware losses |
| Restoration | Original Wi-Fi association and wired management verified |

The first report arrived 4.039 seconds into the capture and the last at 11.555
seconds. The remaining 33.479 seconds contained no reports. The gap statistics
above exclude these leading and trailing empty intervals.

The reports were unprotected Action No Ack frames with complete, unsegmented
payloads. Captured and original packet lengths both equaled 422 bytes. FCS bytes
were absent, so no independent CRC verification is claimed. The oracle checks
angles, selected controls and SNR; frequency-axis labels and matrix correctness
are separate checks. Matrix elements have a synthetic closed-form 2×2 test.

Traffic was bounded ordinary UDP downlink to the verified own-network client:
1,200-byte datagrams at 250 packets/second for 30 seconds (7,500 packets, 9 MB),
starting after the PCAP header appeared. No acknowledgment from a UDP application
is claimed. A preceding link diagnostic independently received AP-to-client HE
data and client acknowledgments.

## Environment and driver scope

- Linux `6.17.0-20-generic`, matching headers, GCC 13.3.0.
- MT7927 with `mediatek-mt7927/2.10` source, the two adapted upstream monitor
  fixes, and the separate experimental aggregate RX-filter band mirror.
- Existing firmware build `20250606201037`; no firmware upgrade was applied.
- BE550 v1 firmware `1.0.12 Build 20240902 rel.22993(4341)`, AP mode,
  fixed 5 GHz channel 36, 80 MHz, Wi-Fi 6 association, OFDMA/MU-MIMO enabled.
- Installed driver files and DKMS registration were preserved. Trial modules
  were loaded in memory; module reload or reboot returns to the installed driver.

The [patches, build procedure and rollback](driver/README.md) are included.
Received signal conditions varied between runs, so these observations do not
isolate the experimental patch's causal benefit. No persistent driver deployment
or claim of general MT7927 monitor compatibility is made.

## Trial history and repeatability

| Trial | Duration | Packets | Complete BFI reports | Interpretation |
|---|---:|---:|---:|---|
| Two upstream fixes, beacon control | 10.028991 s | 90 | 0 | Target beacons received |
| Two upstream fixes, management | 45.017814 s | 428 | 0 | Phone later found on another band |
| Confirmed 5 GHz client, management | 45.015680 s | 425 | 0 | Beacons only |
| Two upstream fixes, link diagnostic | 20.022820 s | 1,798 | 0 | Client BlockAck received; no target downlink data |
| Additional RX-filter experiment, link diagnostic | 20.027742 s | 1,635 | 0 | 450 target downlink HE QoS frames received |
| Validated management capture | 45.033402 s | 473 | 21 | All reports independently matched |
| Same settings and traffic, repeat | 45.029207 s | 451 | 0 | Sustained sampling not established |
| Reproducible runner validation | 45.027231 s | 446 | 0 | Traffic timing and cleanup verified |

All listed runs reported zero kernel drops and restored Wi-Fi. The 22 HE triggers
in the second link diagnostic were Basic or Buffer Status Report Poll triggers;
none requested beamforming feedback. Advertised beamformer capability and such
triggers are not counted as valid reports.

The final hardware run used `run_trial_linux.py`, including its expected-client
checks, explicit Ethernet selection and active-capture marker. It sent 7,380
datagrams (8,856,000 bytes) over 30.002116 seconds, averaging 245.982650 packets
per second. The complete traffic interval fell inside capture. The marker was
removed and Wi-Fi restoration verified. This validates the runner's observed
operation; its empty report set is another negative BFI observation.

Earlier original-driver captures produced no target packets. One separate broad
diagnostic briefly received packets without retaining them, and one setup attempt
failed with `EBUSY` without creating a capture. Neither counts as BFI evidence.
The setup failure was followed by a correction that verifies NetworkManager
release before changing interface type.

## Reproduce and audit

Follow [README.md](README.md) and the driver build/rollback procedure. Use the
verified own-network addresses in private commands. Retain the capture manifest,
traffic interval, original connection state, loaded module source versions and
source hashes with the raw PCAP outside Git.
The README includes the bounded synchronized traffic runner. Capture manifests
now also record the original connection UUID for recovery; this metadata field
was added after the measured runs, which verified that UUID internally.

```bash
python3 tools/bfi/bfi_decode.py /private/path/capture.pcap \
  --ap <own-ap-bssid> --duration <manifest-capture-seconds> --kernel-drops 0
python3 tools/bfi/compare_tshark.py /private/path/capture.pcap \
  --ap <own-ap-bssid> --max-reports 64
```

The successful real capture returns `DECODED_REPORTS` and `MATCH`, with 21 reports
compared. The repeat returns `NO_FEEDBACK_REPORTS` and `NO_SUPPORTED_REPORTS`;
an empty report set does not pass. JSON comparison results and receipts were
retained beside both private captures. Source-level decoding success always
retains `requires_independent_capture_verification`; a checked artifact does not
validate unrelated future input.

Capture SHA-256:

- Validated: `461035ba62b8f0d1de18abb694240c5bd074eeeb522c02e866f47b95569e6344`
- Repeat: `f9fda1665dbd503c4d92b4758a140b520f64da37ab5f08b56fe82ce58cb9b402`
- Runner validation: `f5b6579d4ae1c756fbd9d39479f2659c2f79f8fa613da1a1377f32b6bc179f43`
- Beacon control: `586f6990c6c44426452a00275a0d0ef623a29fb908615265f2f22af0672f1427`
- First management capture: `1cce229395e7c8e40007d48bbd9247cf484a763b45b5a27397644cd2c848d1cf`

## Software validation

The unit tests use **SYNTHETIC** packet fixtures and mocked hardware commands.
Run the command in [README.md](README.md#validation) for the current test count.
Coverage includes a separately derived nonzero cross-byte angle vector, matrix
elements, FCS/truncation rejection, unsupported formats, rate accounting,
interruption cleanup, and restoration races.

`compare_tshark.py --self-test` also matched four **SYNTHETIC** literal fixtures
against Ubuntu tshark 4.2.2: VHT and HE with both SU codebooks, every integer angle,
selected MIMO controls, and signed SNR endpoints. VHT fixtures contained 62 angle
pairs and HE fixtures 250. These synthetic checks do not extend the real-radio
claim beyond the captured HE format above.
