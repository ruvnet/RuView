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

### D6. Windowed, drift-resistant features

`ruview.node-features/2` replaces levels with variability. A per-node `WindowEncoder` keeps the
last 30 seconds of base vectors and emits, for each of the eight base features, its standard
deviation over the window and its mean absolute step between consecutive observations (16
values). Neither depends on the level, which is what drifts. The encoder has no fitted state,
waits for 20 observations over 15 seconds before emitting, and restarts after a 5 second gap.
`ruview-fann-window` re-encodes a `/1` capture with the same code, so offline and live features
are identical. Captures stay in `/1`, the lossless base.

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

### 7. First two-class reference-labelled comparison

The operator then left the room for 32 minutes while the capture and the input logger ran. With
30 seconds trimmed from each end that is 31.0 minutes of declared-empty recording (6,104
observations, none contradicted by the input reference).

**The existing per-node classifier is effectively always on.** It reported present on 6,093 of
the 6,104 empty-room observations: a false-positive rate of 99.8 %. Its thresholds assume a motion
score scale this link does not have. On reference labels, variance, motion-band and breathing-band
power are *higher* in the empty room than with a person seated at the desk (single-feature AUC
0.75 to 0.81 in that inverted direction), so the information is present and the fixed thresholds
point the wrong way.

Replay, Train on one occupied session (429 input-gated rows) plus the first half of the empty
recording (2,871 rows); HeldOut on a different occupied session (759 rows) plus the second half of
the empty recording (3,069 rows). MEASURED, labels `input_gated` and `operator_declared`:

| Model | False positives on empty | Sensitivity on occupied | Coverage (empty / occupied) |
|---|---|---|---|
| Existing per-node classifier | 99.6 % | 93.7 % | 100 % / 100 % |
| Linear control, balanced fit | 11.9 % | 55.3 % | 80.8 % / 76.0 % |
| ruv-FANN candidate 8 → 4 → 1, balanced fit | 2.6 % | 87.2 % | 99.5 % / 97.5 % |

- **Class balance matters.** The Train set is 87 % empty. Fitted unbalanced, the candidate fell to
  24.8 % sensitivity and the linear control to 0.1 %. `FitConfig::balance` (CLI `--balance`) thins
  the larger class evenly to the size of the smaller, and is what the table uses.
- **The candidate beats its linear control here**, on both error rates. This is the first result
  in which the extra capacity is worth something.
- **It does not pass the gates.** Sensitivity is 87.2 % against a 90 % floor, and only two of the
  four conditions were recorded, so the gate verdict is "not judged".
- **Learning curve:** the candidate reaches this level with the occupied session plus about 400
  empty rows (roughly eight minutes of recording in total) and does not improve with more empty
  rows.

Limits, each of which could change the numbers:

- One node, one room, one afternoon.
- The two empty halves are one continuous recording split in time, not independent sessions.
- "Occupied" means seated and typing. A motionless person, a person elsewhere in the room, and
  activity in an adjacent room were not recorded.
- "Empty" is the operator's declaration. Nothing independent confirms nobody else was present.
- A drift shortcut is possible in principle (the model learning the time of day rather than
  occupancy). The HeldOut occupied rows were recorded immediately before and after the empty
  block, closer in time to the empty rows than to the occupied Train session, which works against
  that explanation but does not exclude it.

### 8. Live shadow of the reference-fitted head: it degrades on a later session

The balanced candidate (fitted on the Train rows of result 7, digest `b91d2d59…`) then shadowed
the live server for about seven minutes with the operator back at the desk: 1,439 observations
scored, none unavailable. Only 153 of them fell in input-confirmed windows.

- Candidate on those 153: occupied 110, empty 40, abstain 3. Sensitivity 71.9 %, down from
  87.2 % on the HeldOut session an hour earlier.
- Existing classifier on the same 153: present on 143 (93.5 %), which is what an always-on
  detector scores.
- Over all 1,439 observations the candidate said occupied 59 % of the time. Without a reference
  for the other windows that figure cannot be scored.

MEASURED, 153 observations, one session. The drop suggests the features drift over tens of
minutes (absolute RSSI and band powers are in the feature vector) and that a head fitted on one
occupied session does not hold. The fix to try is more occupied sessions spread over time, and
features expressed relative to a slow baseline.

### 9. Windowed features remove the drift, and the linear control catches up

Looking for what stays stable across the three occupied sessions and the empty block: the
variability of RSSI over the window separates them with a single-feature AUC of 1.00 in every
session, and a threshold chosen on the first session carries to the later ones. A body near the
link perturbs signal strength; an empty room is steady.

Same recordings as result 7, re-encoded with D6, balanced fits, Train on the first occupied
session and the first empty half. MEASURED, same label sources:

| Model | False positives, empty (3,069) | Sensitivity, HeldOut session (769) | Sensitivity, later session (153) |
|---|---|---|---|
| Linear control, 17 parameters | 0.0 % | 91.3 % | 100 % |
| ruv-FANN candidate 16 → 4 → 1, 73 parameters | 0.0 % | 91.2 % | 100 % |

- Both clear the 90 % sensitivity and 5 % false-positive thresholds on the two recorded
  conditions. The gate verdict is still "not judged": adjacent-room and motionless-person
  sessions do not exist.
- The later session is the one on which the level-based head fell to 71.9 %.
- **The candidate no longer beats the linear control.** With features that carry the signal
  directly, a line is enough. The advantage in result 7 came from the candidate compensating for
  poor features.
- Sensitivity falls from 100 % to 91 % as more empty rows are added to Train (learning curve),
  which suggests the balanced thinning is still not ideal.

The limits listed under result 7 all still apply. One more: variability detects a person who is
moving, even slightly. A motionless person may look like an empty room, and that session has not
been recorded.

### 10. Live shadow of the windowed head

The server shadow scores `/2` heads with one `WindowEncoder` per node, the same code the
offline tool uses. The balanced windowed candidate (73 parameters, digest `d132a06d…`) shadowed
the live server for seven minutes with the operator at the desk: 1,621 observations scored after
a 60-observation warm-up, none unavailable.

- On the 210 input-confirmed occupied observations it said occupied 210 times. Sensitivity 100 %.
- Over all 1,621 it said occupied 1,599 times, abstained 15 and said empty 7.

MEASURED, 210 observations, one session, occupied only. No empty-room time was recorded live with
this head; its false-positive figure is the replay one in result 9.

## Consequences

- Fitting must be treated as platform-specific until the engine uses a portable `exp`. Reported
  upstream. RuView ships head documents, never recipes.
- The earlier reading in results 3 and 4, that the per-node features are not enough for a
  presence head, was wrong: those runs imitated a classifier that is itself always on. With
  reference labels the features carry usable signal (result 7).
- The existing per-node classifier's thresholds need replacing or calibrating per link. That is a
  defect in what RuView publishes today, independent of ruv-FANN. Filed as issue #2522.
- Claim 3 has a first positive answer in one room and is not settled. It needs repeat sessions on
  other days, a motionless-person session, an adjacent-room session, a second room, and a radar
  so that "empty" and "still" are confirmed by something other than the operator.
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
