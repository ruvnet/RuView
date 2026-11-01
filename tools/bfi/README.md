# Beamforming feedback capture acceptance

This directory provides a bounded Linux capture helper and a standard-library
Python decoder for local, offline acceptance tests. It is not connected to the
production BFLD sensing pipeline and does not infer motion or recover full CSI.

## Supported decoding

- Classic PCAP with radiotap headers, including explicit FCS validation when
  the header says an FCS is present.
- Complete VHT single-user feedback with two rows and one or two columns,
  20/40/80 MHz, grouping 1/2/4, and either SU codebook.
- Complete HE single-user feedback with two rows and one or two columns,
  grouping 4, and full 20/40/80 MHz allocations (RU ranges 0–8, 0–17, 0–36).
- Quantized angles, dequantized angles, and the corresponding steering matrix.

Other matrix shapes, MU reports, segmented reports, other HE configurations,
EHT, and PCAPNG are explicitly unsupported. The CLI prints aggregate results;
raw angles and matrices are available through the local Python API and are not
included in the default JSON output. Unsupported reports must not be reported
as a capture or decoding success.

## Prepare the link

Use an owned access point and a separate associated client. Start with a fixed,
permitted 5 GHz channel, 80 MHz, and a Wi-Fi 6 association. Record the actual
BSSID, primary frequency, center frequency, client association, and firmware.
An advertised beamformer capability does not prove readable feedback.

The capture computer must have independent wired management. Its Wi-Fi adapter
is temporarily dedicated to monitoring, then restored to its original
NetworkManager connection. The helper refuses to start without a managed,
associated Wi-Fi interface and an Ethernet route to the management address.
It never stops NetworkManager globally or modifies router settings.

Prerequisites on Linux: Python 3.11 or later, `iw`, `ip`, `nmcli`, `tcpdump`,
GNU `timeout`, and existing authorization to run the required commands using
`sudo -n`. `tshark` is useful for independent offline inspection. Do not weaken
host permissions just to satisfy this helper.

## Capture and inspect

Keep captures outside every Git checkout. Replace each placeholder with the
verified own-network value. Use a new output directory for every run.

```bash
python3 tools/bfi/capture_linux.py \
  --interface <wifi-interface> --ethernet <wired-interface> \
  --management-ip <operator-ip> --bssid <own-ap-bssid> \
  --frequency 5180 --width 80 --center 5210 \
  --seconds 10 --beacons-only \
  --output /var/tmp/bfi-beacon-control-1 --confirm-capture
```

First establish repeated reception of the target AP's beacons on the intended
frequency. Then omit `--beacons-only`, use a 30–60 second window, and generate
bounded ordinary downlink traffic to the verified associated client. Keep the
phone awake. Traffic encourages sounding; it does not guarantee feedback.

The private output contains `capture.pcap` and `manifest.json`. The manifest
records the exact command/filter, timestamps, channel state, capture duration,
packet/drop counters, capture SHA-256 and verified restoration. Capture is
bounded by both time and packet count. Stop if restoration fails and use the
recorded state to restore the original connection over Ethernet.

### Repeatable downlink trial

After the beacon check, use the wrapper below to coordinate capture and traffic.
Verify the client's current IP/MAC in the AP client list first. The wrapper checks
that the source address belongs to the specified Ethernet interface, the client
is on that directly connected subnet, and its neighbor entry matches the expected
MAC. It refuses a missing or conflicting entry; it does not scan for clients.

```bash
python3 tools/bfi/run_trial_linux.py \
  --interface <wifi-interface> --ethernet <wired-interface> \
  --management-ip <operator-ip> --bssid <own-ap-bssid> \
  --client-ip <verified-client-ip> --client-mac <verified-client-mac> \
  --source-ip <wired-computer-ip> \
  --frequency 5180 --width 80 --center 5210 \
  --output /var/tmp/bfi-downlink-trial-1 --confirm-capture
```

This starts a 45-second capture, waits for its PCAP header and active-capture
marker, and sends at most
30 seconds of 1,200-byte UDP datagrams to client port 9, capped at 250 packets
per second. `traffic.json` records actual timing and counts. No receiving app is
required, and successful sends do not prove delivery to an application. Scheduling
delays can reduce the number sent. The capture helper owns every radio change and
restoration; the wrapper signals it to clean up if traffic fails or is interrupted.
Datagrams select the verified Ethernet interface explicitly. The active marker is
removed before restoration, and its absence stops traffic.

For the measured higher-traffic profile, add `--pps 1000`. The default remains
250 packets/second. `--traffic-seconds` and `--capture-seconds` default to 30 and
45. Profiles require at least five seconds of capture beyond traffic, at most
120 capture seconds, 1,000 packets/second, 60,000 planned datagrams and 64 MiB of
payload. The packet-capture limit remains 2,000. Pacing records actual rate,
lateness and missed schedules; a delayed process does not send catch-up bursts.

Two higher-traffic runs returned reports, but still had substantial silent
periods. Use the quality report below before selecting a sensing sample rate;
see [measured optimization results](OPTIMIZATION.md).

### Inspect capture quality

If a management capture has beacons but no feedback, use
`--include-link-traffic` for one bounded diagnostic. It enables control-frame
reception and retains control/data frames addressed to or from the own AP as
well as management frames. Check downlink data, client acknowledgments and
sounding announcements independently. This broader private capture can contain
encrypted data payloads; retain it locally. `--beacons-only` and
`--include-link-traffic` are mutually exclusive.

```bash
python3 tools/bfi/bfi_decode.py /var/tmp/bfi-feedback-1/capture.pcap \
  --ap <own-ap-bssid> --duration <manifest-capture-seconds> \
  --kernel-drops <observed-kernel-drops>
```

Omit `--kernel-drops` when unknown. Omitting `--duration` uses the packet
timestamp span, which is not the full observation window and can overstate
rates. The decoder's successful exit means supported reports were decoded;
independent validation is still required. Identical payloads are not discarded
unless supported by retry/sequence evidence.

Independently compare a bounded sample of supported reports using the tested
`tshark` 4.2.2 dissector (no live interface is opened):

```bash
python3 tools/bfi/compare_tshark.py --self-test
python3 tools/bfi/compare_tshark.py /var/tmp/bfi-feedback-1/capture.pcap \
  --ap <own-ap-bssid> --max-reports 8
```

The first command uses synthetic literal fixtures. The second checks every
integer angle, selected MIMO control fields and signed SNR in the selected real
reports. An empty report set cannot pass. The oracle is version-pinned because
its field representation changes across versions. VHT frequency-axis labels
are excluded from this comparison; the tested dissector labels those
sequentially. Matrix correctness is tested separately against a closed-form
2×2 oracle, not inferred from an angle-comparison success.

### Measure coverage over the entire capture

```bash
python3 tools/bfi/quality_report.py /var/tmp/bfi-feedback-1/capture.pcap \
  --manifest /var/tmp/bfi-feedback-1/manifest.json \
  --ap <own-ap-bssid> --client <verified-client-mac>
```

This verifies the capture hash and time window, then counts reports from the
specified client to the AP. It includes leading/trailing silent periods,
one-second bins, partial final bins, format counts, and available beacon signal
and frequency metadata. A valid zero-report analysis exits successfully with
`NO_EXPECTED_CLIENT_REPORTS`; this is a diagnostic result, not acceptance.
Different 20/40/80 MHz tone grids must be handled separately by future features.

### Reproduce CPU measurements

```bash
python3 tools/bfi/benchmark_bfi.py --baseline /private/trusted-baseline.py \
  --pcap /private/capture-supported-by-both-versions.pcap \
  --ap <own-ap-bssid> --repeats 9 --iterations 5
```

The benchmark imports and executes the supplied baseline source: use a trusted
saved version. It checks exact outputs before timing, alternates measurement
order, and reports cold and warm results separately. Use formats supported by
both versions for parity comparisons. Cache memory is bounded; returned matrix
rows are independent copies. Faster decoding does not increase RF report rate.

## Acceptance gates

1. **Reception:** repeated target beacons, correct recorded frequency, complete
   packets, capture bounds respected, and Wi-Fi restored.
2. **Reports:** complete VHT/HE feedback from the intended client–AP link.
   Beacons, action frames, and sounding announcements alone do not pass.
3. **Decoding:** exact control fields, tone count, bit budget and integer angles
   checked against an independent implementation; matrix elements checked
   against a separate closed-form 2×2 oracle. Orthonormality alone is not proof.
4. **Rate:** record complete unique reports per second, gaps, duplicates,
   malformed/unsupported frames and available capture-drop counters over stated
   windows. Kernel drops do not measure all radio/firmware losses.

A zero-report run is inconclusive about hardware BFI compatibility. Movement
validation is a later experiment requiring stationary controls, independent
labels and held-out repetitions. This harness makes no movement-accuracy claim.

## Validation

```bash
python -B -m unittest discover -s tools/bfi -p 'test_*.py' -v
```

Tests are **SYNTHETIC** and include a hand-derived cross-byte angle vector,
matrix elements, format bounds, truncation/FCS errors, unsupported variants,
retry accounting, and mocked capture interruption/restoration races.
No raw hardware captures, credentials, private addresses, or client identifiers
belong in the repository.

For the MT7927 driver trial and rollback procedure, see [driver/README.md](driver/README.md).
