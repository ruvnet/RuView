# ADR 364: CSI capture stall watchdog and send-gate alignment

## Status

Accepted. Validated on one ESP32-C6 (C6FH4 rev v0.2, 4 MB, firmware 0.8.12
base) with deliberately injected stalls. The natural stall from RuView#1941
has not yet been reproduced on this board, so recovery from that specific
fault is expected but not yet observed.

## Context

RuView#1941 measured an ESP32-C6 whose CSI callbacks stopped for 473 s while
Wi-Fi, ping, ESP-NOW, the console and `RUVIEW_HELLO` all stayed healthy. The
node looked alive to every check the fleet has.

The existing `UPLINK_WATCHDOG` cannot see this. It keys on any successful
`sendto()`, and the adaptive controller and mesh send priority packets every
tick whether or not CSI is flowing. Its 1200 s limit is also sized for a
different fault (the ~225 s self-healing uplink gaps).

While measuring a baseline for this work, a second defect showed up in the
same path. `csi_collector.c` rate-limited twice: once at callback accept
(mesh-aligned 20 ms buckets with a 10 ms floor) and again at send (20 ms since
the last send). The two gates are timed from different moments, so frames the
first gate accepted 10–20 ms apart were serialized, fed to the edge DSP and
sequence-numbered, then dropped before `sendto`.

## Decision

1. **Capture liveness is measured directly.** The CSI callback stamps a
   32-bit ms time on entry, before any gate, and again on each CSI frame that
   `sendto` accepts. The watchdog uses the older of the two, so a stall in
   either the radio or the send path counts.

2. **A pure, host-tested escalation policy** (`csi_stall_policy.c`) drives a
   ladder with one `CSI_STALL_TIMEOUT_S` step (default 15 s) per stage, reset
   by any fresh capture:
   1. re-arm: CSI config, callback, promiscuous mode with the filter in
      force, and the self-ping traffic floor;
   2. force a Wi-Fi reassociation (main.c's reconnect path rejoins);
   3. restart.

   The ladder arms only after the first captured and sent frame. It pauses
   while the link is down, and a returning link gets a full window. It is off
   when power duty-cycling is enabled. Stage 1 does not disturb the
   association or mesh time sync.

3. **The send gate becomes a burst guard** at half the process interval. The
   process gate already bounds the average to 50 Hz and `stream_sender`
   already backs off on ENOMEM.

4. **Test-only fault injection** (`CONFIG_CSI_STALL_INJECT_MODE`: 1 transient,
   2 persistent) exists so the ladder can be proven on hardware. It defaults
   to off and the boot log flags any injection build.

## Evidence

All results are MEASURED on ESP32-C6 node 42 on 2026-09-29, streaming to the
host over one AP on channel 9. The host is
`scripts/benchmark-esp32-csi.py --source-ip <node>` (new; the ESP32
counterpart of the RTL8721Dx benchmark requested in RuView#1943). Frame rate
depends on ambient traffic, so the delivered fraction of sequence numbers is
the comparable metric, not fps.

| Build | Window | CSI frames | Sequence loss | Longest CSI gap |
|---|---|---|---|---|
| stock 0.8.12 | 120 s | 1558 | 425 (21.4%), steps only +1/+2 | 1.42 s |
| stock 0.8.12 (raw capture) | 30 s | 658 | 255 of 912 (28%), steps only +1/+2 | — |
| this change | 180 s | 2656 | 2 (0.08%) | 1.17 s |
| this change, soak | 600 s | 12895 | 19 (0.15%), incl. one 2.2 s over-air gap | 2.25 s |

Steps of exactly +2 and never more mean the frames were discarded on the
device, not lost over the air.

Stall injection (serial log timestamps, device clock):

| Scenario | Detected | Action | Result |
|---|---|---|---|
| transient (`set_csi(false)`), 2 runs | +15.0 s | stage 1 re-arm | recovered within 1 s; host outage 15.29 s and 15.17 s; no reassociation, no reboot |
| persistent (callbacks discarded) | +15.0 s | re-arm at +15 s, reassociation at +30 s, link back after 4 s, restart 14 s later | node rebooted and resumed streaming; host outage 52.9 s |

The production build logged zero stall triggers across 855 s of streaming
(180 s + 600 s + 75 s windows).

With an ADR-060 source-MAC filter active, only the raw callback stamp counts,
so a filtered transmitter that is switched off cannot walk a healthy node to a
reboot. That path is reviewed but not hardware-tested; the test node has no
filter provisioned.
The first transient run recorded 55 lost frames in 2 gaps before per-gap
attribution was added to the benchmark; the repeat run with attribution
recorded 0.

Host tests: `make -C firmware/esp32-csi-node/test host_tests` includes the
seven-case ladder test (`run_csi_stall`), which is also clean under ASan and
UBSan.

## Consequences

- A capture-only stall costs at most about one timeout of data (15 s) when a
  re-arm clears it, and at most about 50 s plus reboot when it does not.
  Previously it was unbounded (473 s measured).
- A reboot at stage 3 re-runs ESP-NOW leader election and drops mesh time
  sync. That only happens after a re-arm and a reassociation have both
  failed to restore capture.
- On a channel where fewer than one CSI frame arrives every 15 s despite the
  self-ping floor, the node would re-arm repeatedly. Raise
  `CSI_STALL_TIMEOUT_S` there. The re-arm is non-disruptive, but a
  reassociation is not.
- The root cause of the natural C6 stall is still unknown. This bounds its
  cost; it does not explain it.

## Reproduction

```bash
# host policy tests
make -C firmware/esp32-csi-node/test run_csi_stall
# hardware: build with injection, flash, then from the host
#   CONFIG_CSI_STALL_INJECT_MODE=1 (or 2), CONFIG_CSI_STALL_INJECT_AFTER_S=45
python scripts/benchmark-esp32-csi.py --duration 150 --source-ip <node-ip>
```

Related: RuView#1941, RuView#1943, ADR-018, ADR-081, ADR-110.
