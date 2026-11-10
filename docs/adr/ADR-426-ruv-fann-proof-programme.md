# ADR-426: What the ruv-FANN presence head can be shown to do, and the tools to show it

- **Status:** Proposed
- **Date:** 2026-11-10
- **Relates to:** ADR-425 (default-off ruv-FANN presence head), ADR-423 (presence benchmark
  protocol, radar reference), ADR-424 (any-source fusion), ADR-420 (room learner, PR #2423, not on
  `main`)

## Context

ADR-425 landed a ruv-FANN presence head with engine evidence only: bit-exact parity with the
incumbent linear head and nanosecond timings. Five further claims were open. Each needs its own
tool and its own kind of evidence, and none of them may be allowed to read as a sensing-accuracy
result unless independent reference labels back it.

| # | Claim | Evidence it needs |
|---|---|---|
| 1 | A fit is reproducible across machines | The same digest on different targets |
| 2 | The head is cheap enough for the appliance | A timing on the appliance |
| 3 | The candidate beats the linear control on real rooms | Reference-labelled recordings, two rooms |
| 4 | How much labelled data a new room needs | A learning curve on those recordings |
| 5 | The candidate is safe to run next to the live decision | Disagreement counted on live data |

The room learner's frozen features (ADR-420) are not on `main`, so anything that runs on `main`
has to use what the sensing server already computes per node.

## Decision

### D1. Reproducibility probe

`ruview-fann-repro` fits the candidate and the linear control on a fixed data set generated from
integer arithmetic, then prints the head digests and a digest of the scores on 1,024 fixed probes.
With `--head` it scores an exported head instead, which separates "is fitting reproducible" from
"does one head score the same everywhere".

**Policy that follows from the result below:** a head is fitted once and distributed as its
exported document. A head's digest identifies the document, not the recipe; refitting on another
operating system is a different head and must be qualified again.

### D2. Appliance timing

The existing Criterion bench (`cargo bench -p ruview-fann-head --bench head`) is the instrument.
It is run on each machine the head is meant for and reported per machine.

### D3. Replay rows from real captures, with label provenance

- A fixed, stateless encoding of the server's per-node features, `ruview.node-features/1`
  (8 values: two RSSI figures, variance, three band powers, dominant frequency, change points).
  Nothing is fitted in the encoding, so a change to it must change the schema id.
- `scripts/build_replay_rows.py` turns a server feature capture (D5) into replay rows. Each row
  carries `label_source`:
  - `radar_gated`: the operator records a session under one declared condition and a radar
    (`radar_window_logger.py`) gates it. A row is kept only when a healthy radar window covers it
    and agrees with the declared occupancy. Everything else is dropped and counted.
  - `input_gated`: the same gate with `input_activity_logger.py` as the reference. Keyboard or
    mouse input within the last three seconds proves someone is at the workstation. It records
    idle time only. It is one-sided: no input says nothing, so it can never label a row empty.
  - `operator_declared`: the operator declares the session's condition and leaves. Any row a
    reference contradicts is dropped. This is the available way to label an empty room without
    a radar.
  - `incumbent`: the label is the server's own classification.
- Every replay and curve report states `label_sources` and `labels_are_reference`. Only
  `radar_gated`, `input_gated`, `operator_declared` or `reference` labels make a report an
  accuracy result, and the report lists which were used because they differ in strength. The CLI prints a warning
  otherwise.
- `ruview-fann-fit` fits and exports a head from a room's Train rows only.

### D4. Learning curve

`ruview-fann-replay --learning-curve` refits the candidate and the linear control on growing
prefixes of a room's Train rows in recording order (session boundaries with three or more Train
sessions, fixed fractions otherwise) and scores each on the full HeldOut set with the ADR-425
gates. A prefix with one class only is reported as unfittable, never skipped. The report gives the
smallest prefix from which a model passes the gates at every later point, and elapsed minutes when
rows carry timestamps.

### D5. Shadow scoring in the sensing server, default off

The server gains an optional cargo feature, `fann-shadow`. A default build contains no ruv-FANN.
With the feature, and only when configured:

- `RUVIEW_FANN_SHADOW_LOG=<file>` captures one JSON line per fresh per-node classification:
  encoded features, the server's decision and, when a head is loaded, the candidate's score.
- `RUVIEW_FANN_SHADOW_HEAD=<head.json>` loads an exported head and scores every fresh
  classification of the nodes named in the head's `sources` (`csi-node-<id>`).
- `GET /api/v1/shadow/fann` reports counters and the disagreement rate.

Rules: the task holds a read lock only and writes nothing to sensing state; stale, repeated and
non-finite observations are counted and skipped; a head for another schema is refused; the log
stops at 256 MiB and counts what it drops; a bad configuration disables the shadow and never
stops the server. The candidate has no path to a published result.

This shadows the server's existing per-node classifier, not the room learner's head. Shadowing the
room learner itself remains a follow-up for when PR #2423 is on `main`.

## Results on 2026-11-10

### 1. Reproducibility: MEASURED, with a limit

Same source, same pinned engine revision `d94b47d8`, release builds.

| Target | Candidate digest | Linear control digest |
|---|---|---|
| `x86_64-unknown-linux-gnu` (Ryzen 9 9950X, rustc 1.98.1) | `7753d819…` | `1c369039…` |
| `x86_64-unknown-linux-musl` (same host) | `7753d819…` | `1c369039…` |
| `aarch64-unknown-linux-gnu` (same host, `qemu-aarch64-static`) | `7753d819…` | `1c369039…` |
| `x86_64-pc-windows-msvc` (rustc 1.91.1) | `2d83dbfa…` | `d5552b11…` |

- The three Linux targets produce bit-identical heads and bit-identical scores.
- Windows produces a different head: all 49 parameters differ, by at most 0.0156.
- Cause: `f32::exp` is not the same function on the two platforms. A hash of `exp(x)` over
  4,000,001 inputs differs between Windows and Linux, while `sqrt` matches. The sigmoid uses
  `exp`, and 400 optimiser steps amplify last-bit differences.
- **Scoring one exported head is identical everywhere tested:** the Linux-fitted head gives the
  same 1,024 score bit patterns on Windows, Linux and emulated aarch64. That is 1,024 probes, not
  a guarantee for every input, since `exp` does differ somewhere.
- The Windows and Linux compilers also differ in version; the `exp` difference alone is sufficient
  to explain the result, but the compiler was not isolated as a variable.

### 2. Appliance timing: not done

The Pi 5 appliance (`cognitum-v0`) and all cluster Pis were offline. The aarch64 build is proven
to run and to score correctly under emulation, which says nothing about its speed. A second
workstation was measured instead:

| Operation | Windows dev PC (ADR-425) | Linux, Ryzen 9 9950X |
|---|---|---|
| Incumbent linear head | 6.5 ns | 5.4 ns |
| Same head through ruv-FANN | 33.6 ns | 17.3 ns |
| Candidate 10 → 4 → 1 | 61–70 ns | 52.6 ns |
| Shadow, both heads | 75 ns | 67.2 ns |
| Prepare a head from its document | 6.2 µs | 3.8 µs |

MEASURED, synthetic 10-feature vector, engine time only.

### 3 and 4. Real rooms and learning curve: tooling done, no accuracy result

No radar was attached and only one node in one room was live, so there are no reference labels
and no second room. What was run is the whole pipeline on real hardware with **incumbent labels**:

- One ESP32-C6 (node 6, firmware 0.8.17, channel 10), streaming real CSI to the server.
- Session 1: 585 observations in 3.1 minutes (Train). Session 2: 1,014 observations (HeldOut).
- Labels are the server's own classification, so this measures imitation of the server.

Outcome, MEASURED features, incumbent labels, **not accuracy**:

- Both the 41-parameter candidate and the linear control learned "always occupied". On HeldOut
  they call all 54 of the server's "absent" rows occupied and agree with it on all 960 present
  rows.
- The learning curve shows both models settling on that answer from about 290 rows (1.6 minutes).
- The gates return no verdict: no session was recorded under the adjacent-room condition.

Reading: the server's "absent" state is not recoverable from the instantaneous per-node features.
The session was 94 % "present" by the server's own account, and the server's decision carries
temporal state the feature vector does not. A head on these 8 features cannot replace the
per-node classifier, and extra capacity did not help.

### 5. Live shadow: MEASURED on one node for five minutes

The head fitted on session 1 shadowed the live server during session 2: 998 observations scored,
0 unavailable, 0 abstentions, 944 agree, 54 disagree (5.4 %). Every disagreement is the case
above: server says absent, candidate says occupied. The shadow had no effect on published state.
Five minutes on one node is a functional check, not the multi-day run the claim needs.

### 6. First reference-backed figure: occupied only

A third session recorded the C6 beside the input-activity reference for 7.3 minutes: 1,506
observations, of which 429 fall in windows where typing proved the desk was occupied.

- The server's per-node classifier reported present on all 429. Sensitivity 100 % on
  input-confirmed occupied observations. MEASURED, one node, one room, one session.
- It reported present on 1,505 of the 1,506 observations overall, so this figure cannot tell a
  working detector from one that is always on. The false-positive rate needs an empty room.
- The short "absent" bursts seen in sessions 1 and 2 (28 runs of 0.3 to 1.2 s in five minutes)
  did not recur. Nothing was changed in the classifier on the strength of one session.

## Consequences

- Fitting must be treated as platform-specific until the engine uses a portable `exp`. Reported
  upstream. RuView ships head documents, never recipes.
- The per-node feature vector is not sufficient for a presence head. The room-learner features
  (ADR-420) remain the intended input, which makes PR #2423 the blocker for a meaningful
  candidate.
- The accuracy question (claim 3) is still open. The next step needs no new hardware: an
  operator-declared empty session, to get a false-positive rate and the first two-class
  reference-labelled fit. The full protocol still needs a radar, two rooms, and sessions under
  each of the four conditions, including adjacent-room activity.
- Claim 2 needs the Pi 5 back online.
- The server gains one optional dependency behind a feature that is off by default, and one
  read-only status route.

## Validate

```bash
cd v2
cargo test -p ruview-fann-head
cargo test -p wifi-densepose-sensing-server --features fann-shadow --bin sensing-server -- adr426
cargo run --release -p ruview-fann-head --bin ruview-fann-repro
cd ../scripts && python -m unittest tests/test_build_replay_rows.py
```
