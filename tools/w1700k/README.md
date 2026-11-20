# W1700K RF diagnostics bridge

Status: **SYNTHETIC tested**, hardware validation pending. This read-only Linux
consumer produces advisory RF quality JSONL for RuView and RuField. It does not
extract CSI, classify presence, estimate pose, or output calibrated RF power.

## Build and use

Requires a C11 compiler, make, Python 3 for tests, and upstream w1700k-stream
for a live sensor. No network access or runtime dependencies.

```sh
make -C tools/w1700k
make -C tools/w1700k check
tools/w1700k/build/w1700k-diagnostics --once
tools/w1700k/build/w1700k-diagnostics --follow --timeout-seconds 60 --max-records 4096
```

The default input is /dev/shm/w1700k-stream. The input is never written and radio
controls are never changed. --source-kind defaults to live; synthetic and
captured_replay are explicit caller assertions, not cryptographic provenance.
Optional --center-frequency-hz accepts 2 through 8 GHz, but remains unverified.

Generate an ABI-complete **synthetic** fixture and compact diagnostics:

```sh
tools/w1700k/build/w1700k-fixture /tmp/w1700k-synthetic.shm normal
tools/w1700k/build/w1700k-diagnostics --input /tmp/w1700k-synthetic.shm --source-kind synthetic --once > /tmp/w1700k-synthetic.jsonl
```

Other fixture modes: gap, event, flat, tag, magic, ring, offset, overflow,
seqlock, truncated, stale, idle, run_overflow. Fixtures are test-only generators;
do not overwrite a running service's shared memory. Arbitrary ci16 recorder
outputs are not accepted: their missing event positions cannot recover timing.

## Evidence contract

One JSON object per slot, schema ruview.rf_diagnostics.v1. Fields:

| Field | Meaning |
|---|---|
| source_kind, source_id | Caller assertion live, synthetic or captured_replay; w1700k |
| observed_monotonic_ns | Local host CLOCK_MONOTONIC read time, never UTC or cross-host comparable |
| run, slot_index | Upstream run and DMA completion identity, not a sample timestamp |
| sample_start | Always null in v1; prior event history is not available |
| sample_rate_hz | Control-word claim, 0 unknown or 40/80/160/320 MHz |
| center_frequency_hz | Null or caller-supplied claim; not observed from hardware |
| config_verified, valid | Always false: ABI lacks actual LO, gain, band and configuration generation |
| invalid_reasons | configuration_unverified plus lost/event/overrun/stale/tag_mismatch/run_change |
| sample_count | 2048 for a stable sample slot, otherwise 0 |
| rms_dbfs | 10 log10(mean(I²+Q²)/8192²), null at zero power or no samples |
| rail_fraction | Fraction of int16 components on either storage rail, null without samples |
| missing_samples | 2048 for one LOST slot; 0 for EVENT and unknown overrun composition |
| event_count, overrun_slices | Per-record events or skipped overwritten completions |
| ttl_ms | 1000; receiving systems must start their own local expiry timer |

14-bit full scale is 8192. Filter headroom can legitimately exceed that value;
positive dBFS is not clipping. Rail fraction is an indicator, not proof of ADC
clipping. Complex RMS is normalized per complex sample, not per component.
Metrics are advisory even when numeric. Nothing here may promote human presence
or override RuField's missing calibration/configuration gates.

## Safety and operating limits

Header magic/version, fixed geometry, power-of-two bounded ring, aligned
nonoverlapping offsets, arithmetic overflow, exact file size and tag identity
ranges are checked. Position reads have a 64-attempt bound. Input is opened
read-only with no symlink following and must be a regular file (including SHM).

The bridge uses Linux pread snapshots and acquire fences, then tag/copy/tag
validation, rather than exposing mmap memory to untrusted concurrent truncation.
Short reads fail with exit 2 instead of SIGBUS. This is intended for the trusted
local upstream producer on a native ABI host, not an adversarial writer capable
of forging consistent tags. It does not claim that fences convert kernel copies
into cross-process C atomic loads; hardware qualification must validate this
snapshot strategy on target architectures.

Live mode rejects stale or future heartbeats with an invalid stale record and
exit 3. Captured replay and synthetic inputs deliberately do not compare saved
heartbeats with the current host. A producer restart may leave the old inode
open; heartbeat expiry requires restart/reopen of this consumer. No automatic
reconnect or permission expansion.

--once takes the newest available bounded tail (64 records by default).
--follow starts at that same tail and follows until timeout, record limit,
interrupt or failure. Polling is 10 ms. This is a bounded diagnostic sampler,
not a promise of lossless full-bandwidth capture. Detected overruns are invalid
records; their unknown mix of samples/events is never converted to invented
missing sample time. Existing backlog intentionally outside the initial tail is
not treated as a newly observed overrun. Hardware throughput is unmeasured.

No raw IQ is exported. JSONL contains no decoded payloads, identifiers, MAC
addresses or presence claims. Do not commit live recordings. Shipping evidence
must distinguish fixtures from real device captures.

## Cross-repository acceptance

Build the RuField adapter from the companion ruvnet/rufield checkout:

```sh
cd ../rufield
cargo build -p rufield-adapters --bin rf-diagnostics-replay
cd ../RuView
python3 tools/w1700k/verify_integration.py --rufield-bin ../rufield/target/debug/rf-diagnostics-replay
```

This checks six synthetic ABI scenarios through the C bridge, browser validator and RuField adapter. It verifies gap/event semantics and that diagnostic evidence remains unsigned, synthetic and zero-confidence. It does not establish real receiver performance.

For interactive inspection, open [the local viewer](../../docs/rf-diagnostics/index.html) or [the hosted viewer](https://ruvnet.github.io/RuView/rf-diagnostics/) and import the bridge JSONL. Files remain in your browser. An imported live-labelled file is replay, not a live connection.

Convert diagnostic JSONL to advisory FieldEvents:

```sh
../rufield/target/debug/rf-diagnostics-replay < diagnostics.jsonl > advisory-events.jsonl
```

These events cannot contribute spatial/occupancy fusion. Automatic suppression of other sensors needs a future association and timing policy. CSI extraction, spectrum bins and hardware accuracy benchmarks remain future work.
