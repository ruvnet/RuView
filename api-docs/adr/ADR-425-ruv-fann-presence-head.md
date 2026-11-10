# ADR-425: A default-off ruv-FANN presence head, in shadow, behind fixed gates

- **Status:** Proposed
- **Date:** 2026-11-09
- **Relates to:** ADR-420 (managed source-aware room learner, PR #2423, not yet on `main`),
  ADR-423 (HE-LTF validity gate, presence benchmark protocol), ADR-424 (any-source fusion),
  issue #2418 (room learner epic)
- **Source analysis:** *RuFANN and RuView Integration Analysis*, 2026-11-09

## Context

The room learner's presence decision (ADR-420) is a small head over frozen features: for each
source an AETHER embedding plus two skip features (baseline-relative DC and temporal energy), then
`sigmoid(sum(x_i * w_i) + bias)`, abstaining on `[0.45, 0.55]`. With two sources that is 10
features and 11 parameters.

The product problem is **false occupied readings when an adjacent room is active**, without losing
a stationary person in the target room. A linear head may not separate "adjacent activity" from
"still occupancy" on those features. ruv-FANN (merged PR 202, revision `d94b47d8`) now offers what
a bounded experiment needs: immutable prepared inference with a reusable workspace, reverse-mode
dense training with MSE and AdamW, and a versioned interchange document.

Constraints that shape the decision:

- **RF separability comes first.** On the ADR-423 bench, 77-90 % of HE-SU estimates were invalid
  and unstable. Firmware validity, PHY admission, gain and placement are first-order variables. If
  the admitted signal cannot separate the conditions, more head capacity will not help.
- **The room learner is not on `main`.** PR #2423 carries it. This ADR therefore lands the head as
  a standalone crate over a plain feature vector; wiring it into the learner is a follow-up.
- **No release of the needed engine.** crates.io has `ruv-fann` 0.2.1; the prepared and autograd
  APIs are in 0.2.2 source only. An exact source revision is the only honest pin.
- **Evidence.** Engine benchmarks and RF results are separate evidence. Nothing in this ADR is a
  sensing-accuracy result.

## Decision

### D1 — A standalone, default-off crate over the feature vector

`v2/crates/ruview-fann-head` depends on `ruv-fann` at the exact revision
`d94b47d8e3a5dcb92d6a1f0b966593233b40458c`, with `default-features = false` and only `std` and
`serde` (no rayon, bincode, flate2, wgpu or wasm in the helper). It is `publish = false`.
Nothing in the sensing server depends on it yet, so the default build and runtime are unchanged.

Its input is the feature vector the incumbent head already scores. It does not tokenise, encode,
admit or publish.

### D2 — Runtime parity before any new model

`FannHead::from_linear` expresses the incumbent as one dense sigmoid layer with the same weight
order, bias and steepness 1. Parity is a gate, not an assumption: probabilities must be
**bit-identical** to the incumbent expression and decisions identical, including at the `0.45`
and `0.55` boundaries. Only then is a different architecture compared.

### D3 — One fixed candidate, and a feature-matched control

- **Candidate:** `n -> 4 -> 1`, ReLU then sigmoid. For 10 features: 49 `f32` parameters, 196
  bytes of weights and biases.
- **Objective:** full-batch AdamW on MSE of the sigmoid probability against 0/1 targets, with
  decoupled weight decay, a fixed epoch count and a fixed seed. Fitting is deterministic.
- **Control:** a linear head refitted with the *same* loss, optimiser and rows. The incumbent was
  fitted with binary cross-entropy (plus InfoNCE on the encoder), so a gain over the incumbent
  alone may be the change of loss. A gain is attributed to capacity only when the candidate beats
  the control.
- With 4 to 64 Train cohorts per condition, 49 parameters can overfit. The architecture is fixed;
  any tuning uses Validation only and is frozen before HeldOut.

### D4 — Context binding, and scoring without authority

- A head carries its context: feature schema, **ordered** source ids, and room revision. Its
  digest (SHA-256 over engine revision, context and every parameter) is the key for any prepared
  model or cached score. A head is refused for any other context.
- An exported head records the engine revision. A document from another revision, with unknown
  fields, or whose digest does not match, is refused rather than reinterpreted.
- **Shadow scoring** runs the candidate on exactly the vector the incumbent scored and returns a
  diagnostic record. There is no path from a shadow score to a published result. A candidate
  failure is `Unavailable` and never changes the incumbent's outcome; an incumbent failure stays
  the caller's existing `Unavailable` path.
- Only the existing owner qualification and selection flow can make a model live. Selected
  publication keeps suppressing count, motion, pose and vitals authority.
- Preparation happens at load or selection, never per observation. Scoring stays off the capture
  critical path; a dropped or late shadow score is a diagnostic outcome.

### D5 — Replay with fixed gates

`ruview-fann-replay` reads one row per admitted cohort: features, operator condition
(`quiet_empty`, `adjacent_only`, `still_occupied`, `moving_occupied`), session, a split assigned
**per session**, and the incumbent's recorded probability for that cohort. A session in more than
one split is rejected. For each room it fits the candidate and the control on Train and scores
HeldOut next to the incumbent.

- **Abstentions stay in every denominator.** Sensitivity is occupied decisions over all occupied
  rows; a model cannot improve by going quiet.
- **Local gates** (the learner's qualification policy): at least 4 HeldOut cohorts per condition,
  sensitivity >= 90 % for each occupied condition, false positives <= 5 % for each negative
  condition, coverage >= 90 % for every condition.
- **Pilot target:** at least 25 % fewer adjacent-only false occupied rows than the incumbent, with
  a 95 % bootstrap interval of the paired difference above zero. **Sessions are resampled, not
  rows**: sliding windows within a session are not independent trials.
- Where the incumbent has no adjacent false occupied rows, the report says so and the simpler
  model is kept.
- Reports are `SYNTHETIC` if any row is synthetic.

A local pass is an installation check, not an accuracy claim. Zero false positives in four
independent negatives still allows a true rate up to 52.7 % (exact one-sided 95 % bound); a
product claim needs the larger, independent evidence of ADR-423's protocol: at least three rooms,
separate sessions and days per split, a consented time-aligned reference, and all rules frozen
before HeldOut.

### D6 — Performance budget and what is measured

Provisional budgets: <= 1 ms added p95 processing and <= 5 MiB added peak RSS in the helper.

MEASURED on the development workstation (Windows, x86-64, criterion, synthetic 10-feature
vector; reproducer `cargo bench -p ruview-fann-head --bench head`):

| Operation | Time |
|---|---|
| Incumbent linear head, hand-written | 6.5 ns |
| Same head through ruv-FANN (`10 -> 1`) | 33.6 ns |
| Candidate (`10 -> 4 -> 1`) | 61-70 ns across runs |
| Shadow: incumbent and candidate on one vector | 75 ns (157 ns before the score record stopped allocating) |
| Prepare a head from its exported document | 6.2 µs |

Every scoring figure is four orders of magnitude inside the 1 ms budget, and a prepared head with
its workspace is a few hundred bytes. Two things follow:

1. **Prepare once.** Preparation costs about 100 scores. A head is prepared at load or selection.
2. **The head is not the latency problem.** The incumbent's `infer` validates the model, builds
   tokens and reconstructs the encoder and projectors before this dot product. Profile that path
   (tokenisation, graph setup, scoring, serialisation; evaluations per observation) before
   attributing latency to the head. That work belongs with ADR-420, not here.

### D7 — Upstream

Integration found one engine defect worth fixing at the source, and it is fixed there rather than
worked around here: `PreparedNetwork::run_into` copied the caller's inputs into its workspace and
read them back one scalar at a time, which stalls store-to-load forwarding and dominated the cost
of small networks. Reading a dense first layer's inputs in place, and inlining activation
dispatch, takes the `10 -> 1` sigmoid from 35.4 ns to 19.5 ns in ruv-FANN's own benchmark
(MEASURED, same workstation), bit-identical; the `10 -> 4 -> 1` case is unchanged within noise.
Proposed as [ruv-FANN PR 203](https://github.com/ruvnet/ruv-FANN/pull/203), which also makes a
plain `cargo test` compile on default features. This crate keeps the reviewed pin until that
change is merged upstream; the pin is then moved deliberately, with parity re-run.

## Consequences

- A candidate can be fitted, exported, loaded, shadow-scored and replayed today on recorded
  feature vectors, with the gates fixed in code.
- No behaviour changes for users: nothing calls the crate from the server.
- A git-pinned dependency enters the workspace (precedent: `wifi-veil`). It is small, has no
  default features, and must be re-reviewed whenever the pin moves.
- The MSE objective is a known difference from the incumbent. The control makes that visible; it
  does not remove it.

## Not decided here

- Wiring into the room learner (needs PR #2423), the Mac helper rebuild and manifest, Windows
  learner parity, any phone runtime.
- Residual (conformal) intervals for drift, similarity search for calibration diagnostics.
- Any accuracy statement. If the signal cannot separate the conditions in the paired trial,
  improve capture validity or placement before adding model capacity.

## Validation

```bash
cd v2
cargo test  -p ruview-fann-head            # parity, fit, export/load, shadow, replay (SYNTHETIC)
cargo bench -p ruview-fann-head --bench head
cargo run   -p ruview-fann-head --bin ruview-fann-replay -- \
  --rows replay.jsonl --feature-schema aether-skip/1 --sources node-a,node-b
```
