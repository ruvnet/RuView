# Separate capture coverage from traffic-period coverage

Status: **MEASURED on two existing real captures; synthetic tests passed.** Existing full-capture measurements
in [OPTIMIZATION.md](../bfi/OPTIMIZATION.md) remain unchanged.

## Measured traffic-period findings

The two recorded higher-traffic trials both requested 1000 UDP packets/second
and averaged approximately 949 successful sender calls/second. Their saved
traffic receipts pass the analyzer's interval, profile and clock checks.

| Trial | Traffic interval | Reports during traffic | Reports/s during traffic | Duration-weighted occupied 1 s bins | Maximum silence during traffic | Post-traffic interval / reports |
|---|---:|---:|---:|---:|---:|---:|
| Higher traffic 1 | 30.000066569 s | 66 | 2.200 | 43.33324% | 8.392047 s | 14.899209 s / 0 |
| Higher traffic 2 | 30.000062819 s | 96 | 3.200 | 69.99985% | 6.437294 s | 14.937371 s / 0 |

All retained reports in both captures occurred inside the sender interval.
The planned post-traffic quiet period explains part of the original 23.291 s
and 16.393 s full-capture gaps. It does not explain the remaining traffic-period
blackouts: trial 1 ended with 8.392 s of traffic and no report, and trial 2 had
a 6.437 s gap between reports while traffic was active. These measurements
still do not establish uninterrupted sensing or uniform sampling.

The occupied fraction is weighted by each bin's duration. Both traffic intervals
extend slightly past 30 seconds, creating a very short 31st bin; treating that
bin as a whole second would instead produce 13/31 and 21/31, which are different
metrics. A bin containing a report does not imply continuous coverage within it.

Evidence was read from the private analysis receipts named
`ruview-bfi-opt-1000-1-traffic-coverage-v2.json` and
`ruview-bfi-opt-1000-2-traffic-coverage-v2.json`. Their input PCAP hashes are:

- Trial 1: `747bc01cd6689388ac03b88aeef8cea8b85147e72e628ca7899e721a2d8cf9bb`.
- Trial 2: `f6308f822d59875f1c3bf8a028280fddc6649ba045be7a2ef2a572389aa23031`.

The first receipt version inherited an incorrect `manifest_capture_start` label
for the additional intervals' bin origin. Its calculations already used each
interval's actual start, so the numerical findings above are unaffected. The
wrapper now labels these bins `interval_start` and records the exact integer
`bin_origin_unix_ns`. Both real captures were rerun into new v2 receipts; exact
integer origins and equality of the embedded full-capture objects were checked.
Original receipts remain retained.

## Why this second receipt exists

The bounded trial normally records about 45 seconds while sending UDP traffic
for 30 seconds. The full-capture leading/trailing silence correctly includes
time before and after that sender interval. A long full-capture gap can therefore
include intentionally quiet time. It cannot, by itself, establish a long gap
while traffic was being sent.

[traffic_coverage.py](traffic_coverage.py) reads the same PCAP, capture manifest,
and `traffic.json` and emits both views. It also reports the before/after
intervals separately. The calculation does not alter the original PCAP,
manifest, traffic receipt, router configuration or capture settings.

```bash
python tools/bfi_learning/traffic_coverage.py /private/trial/capture.pcap \
  --manifest /private/trial/manifest.json \
  --traffic /private/trial/traffic.json \
  --ap <selected-ap-bssid> --client <selected-client-mac> \
  --output /private/trial/traffic-coverage.json
```

The output must be new and outside Git. Preserve the relative directories
`tools/bfi` and `tools/bfi_learning` when copying the standalone tools. The
analyzer imports the existing decoder and quality reporter; it does not import
or execute the capture runner. It uses only the Python standard library.

## Source contract and validation

The source of the traffic fields is
[run_trial_linux.py:send_traffic](../bfi/run_trial_linux.py). It records integer
Unix nanoseconds before its socket/send loop and in its `finally` block, plus a
monotonic elapsed duration. These timestamps bound the sender's attempted
traffic interval. They are not exact first/last over-air transmission times and
successful UDP sends do not establish reception or sounding activity.

The analyzer requires:

- `schema_version=1`, a successful capture child exit and verified restoration,
  no recorded trial/cleanup error, and successful recorded preflight/link checks.
- Integer `armed_at_unix_ns`, `traffic_started_unix_ns` and
  `traffic_ended_unix_ns`, in that order and inside the verified capture interval.
  Timestamp integers are never converted to Unix float seconds for comparison.
- The supported runner profile: 1–1000 requested packets/second, 1200-byte
  payloads, destination port 9, at most 120 capture seconds, at least five
  requested capture seconds beyond the traffic duration, at most 60,000 planned
  datagrams and 64 MiB of payload. The manifest's requested capture duration,
  traffic profile string and planned count must agree.
- Positive sent counts no greater than planned, an exact sent-byte product,
  and an actual send rate matching sent count divided by recorded elapsed time.
- A complete traffic duration: no more than 1 millisecond shorter than requested
  and no more than 2 seconds longer. The latter allows bounded sender scheduling
  and socket completion overhead; it is not a rate guarantee. Wall-clock and
  monotonic elapsed durations must agree within 1 millisecond. A clock jump or
  partial interrupted interval fails closed.
- A valid [quality_report.py](../bfi/quality_report.py) manifest and matching
  PCAP SHA256. Strict decoder bounds, FCS checks, supported-format checks,
  packet time ordering and capture-window validation remain authoritative.

Missing historical fields are not guessed. A legacy traffic script without the
required timing/profile receipt cannot use this analyzer as evidence of an
actual traffic-period denominator.

`traffic.json` currently does **not** contain the PCAP hash or explicit AP/client
identifiers. Consequently the analyzer cannot cryptographically prove that an
arbitrary traffic receipt belongs to an arbitrary PCAP. The caller selects the
paired files and addresses; containment, profile consistency and recorded checks
are validated. The new receipt hashes the exact PCAP, manifest and traffic JSON
bytes and all three analysis source files so later review can reproduce this
particular association. Those hashes establish content integrity, not source
authentication. The output records this limitation and omits raw MAC addresses
and source paths.

## Counting and time boundaries

First run the original full-capture quality analysis unchanged. A second pass
uses the same strict report decoder and direction rule, applies the same bounded
retry-deduplication rule over the **whole capture**, and checks that its retained
count matches the quality reporter. This matters for a retry appearing just
inside the traffic interval whose original report was captured just before it.

Partition retained report timestamps exactly:

- Before: earlier than traffic start.
- Traffic: start through end, both included.
- After: later than traffic end.

These disjoint report sets sum to the full retained count. Each nonzero interval
uses the existing `quality.coverage` calculation with its own exact integer
endpoints and duration. Report rates, occupied one-second bins, partial final
bins, leading/trailing silence, and maximum no-report gap use that interval's
full denominator. A zero-duration before/after interval has no coverage object.
An interval with no reports has a no-report gap equal to its whole duration.

The receipt leaves full-capture unsupported-format counts and quality data
intact. Its traffic-period report rate is an additional measurement; do not
replace the prior full-capture rate or compare the two as if they shared a
denominator. Neither sequence jumps nor gaps are converted into RF loss.

## How to use the result

Compare three facts per trial: traffic-period report count, traffic-period
maximum silence, and post-traffic duration. If most full-capture silence falls
after the sender stops and the traffic interval has short gaps, the next question
is whether a later bounded capture can maintain those observations. If long
traffic-period gaps remain, continuous sensing is still blocked even when the
full-capture average improves. UDP traffic, AP sounding decisions, RF reception
and report-format changes remain distinct possible causes; this receipt alone
does not identify which caused a gap.

Any future active trial remains a separate authorized hardware action. This tool
only analyzes existing files and cannot create traffic or modify an interface.

## Focused tests

```bash
python -m unittest discover -s tools/bfi_learning -p test_traffic_coverage.py -v
```

[test_traffic_coverage.py](test_traffic_coverage.py) checks full versus traffic
windows, endpoint partitioning, a retry across the boundary, empty captures,
short/out-of-window/clock-inconsistent intervals, profile and byte-count bounds,
failed link evidence, hash mismatches, duplicate/nonfinite JSON, private output
and input hashes. All packets and receipts in these tests are synthetic.
