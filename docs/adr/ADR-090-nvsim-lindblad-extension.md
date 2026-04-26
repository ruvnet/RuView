# ADR-090: nvsim — Full Hamiltonian / Lindblad Solver Extension

| Field          | Value                                                                                   |
|----------------|-----------------------------------------------------------------------------------------|
| **Status**     | Proposed — conditional. Only built if a pulsed-protocol use case emerges. Default-off, opt-in feature gate.                |
| **Date**       | 2026-04-26                                                                              |
| **Authors**    | ruv                                                                                     |
| **Refines**    | ADR-089 (nvsim simulator)                                                                |
| **Companion**  | `docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md` §3.1, `docs/research/quantum-sensing/15-nvsim-implementation-plan.md` §6 |

## Context

[ADR-089](ADR-089-nvsim-nv-diamond-simulator.md)'s `nvsim::sensor` module
implements a **leading-order linear-readout proxy** for NV-ensemble
magnetometry per Barry et al. *Rev. Mod. Phys.* 92, 015004 (2020) §III.A.
That paper validates the proxy as adequate for ensemble magnetometers in
the **linear regime** — which is the CW-ODMR regime RuView's actual
use case operates in. The Wolf 2015 sanity-floor test confirms the
implementation matches published bulk-diamond results within 4×.

What the proxy does *not* model:

- **Pulsed protocols**: Rabi nutation, Hahn echo, CPMG / XY-N dynamical
  decoupling sequences.
- **Microwave-power saturation**: line-broadening at high CW MW power.
- **Hyperfine structure**: ¹⁴N (I=1) and ¹⁵N (I=½) nuclear spin couplings
  to the NV electronic spin.
- **Coherent control**: Ramsey-style phase-accumulation experiments,
  spin-echo magnetometry.

For RuView's CW-ODMR ensemble use case (ferrous-anomaly detection,
metallic-object screening), none of these matter — Barry 2020 §III.A is
explicit that the linear-readout proxy is adequate. For *future* use cases
that involve pulsed protocols (e.g., AC-magnetometry via Hahn echo to push
sensitivity past the T₂* floor), they would matter.

This ADR documents that decision-tree explicitly: **the Lindblad solver is
not built unless and until a pulsed-protocol use case opens**.

## Decision

Defer the full Hamiltonian + Lindblad solver to a **conditional, opt-in
feature gate** named `lindblad` on the `nvsim` crate. Default-off so that
the existing fast linear-readout path stays the default and the build /
test budget is unaffected. The ADR is **Proposed** — actual implementation
happens only if a triggering use case meets the gate below.

### Trigger conditions for promoting to Accepted

This ADR transitions from Proposed → Accepted when **any one** of the
following is true:

1. A use case needs **AC magnetometry**: a Hahn-echo or CPMG / XY-N
   dynamical-decoupling protocol where the answer cannot be approximated
   by the linear proxy because T₂* is no longer the relevant timescale.
2. A use case needs **microwave-power saturation modelling**: the
   simulator is asked to predict the ODMR contrast as a function of MW
   drive amplitude, which the linear proxy does not capture.
3. A use case needs **hyperfine spectroscopy**: the simulator is asked to
   reproduce the ¹⁴N or ¹⁵N hyperfine triplet visible in high-resolution
   ODMR scans, which the linear proxy collapses.
4. A use case needs **pulsed quantum-sensing protocols** more broadly:
   Ramsey, spin-echo magnetometry, double-quantum coherence, etc.

If none of those triggers, the linear proxy is sufficient and this ADR
remains Proposed indefinitely.

### Why the deferral is the right call today

- **Adequacy validated by primary source.** Barry 2020 §III.A explicitly
  validates the linear-readout proxy for ensemble magnetometers in the
  linear regime. nvsim's existing `sensor.rs` matches Wolf 2015 within 4×.
  We're not under-modelling — we're correctly-modelling.
- **3–7 days of focused work.** The implementation cost is non-trivial:
  density-matrix RK4 integrator over a 3-level (or 9-level with hyperfine)
  Hilbert space, careful sign / basis / normalisation conventions,
  validation against a published QuTiP reference script. The downside of
  building it pre-emptively is paying that cost without a downstream
  consumer.
- **No current downstream consumer.** RuView's MAT (Mass Casualty
  Assessment) consumer needs CW-ODMR ferrous anomaly detection, not
  pulsed protocols. ADR-066 swarm-bridge (proposed) is similarly
  CW-amplitude-only.
- **Not blocked.** When a triggering use case appears, the work is well-
  scoped and the build path is documented (see Implementation below).
  Deferral is reversible at any time.

### Why we don't just delegate to QuTiP

QuTiP is the obvious off-the-shelf option and is what `15.md` §6 originally
proposed deferring to. Two reasons we'd prefer an in-tree Rust
implementation if we ever build it:

1. **Determinism**. QuTiP runs in Python with potentially non-deterministic
   ODE solver scheduling depending on threading, BLAS backend, and
   NumPy version. nvsim's whole-pipeline determinism — same seed →
   byte-identical witness — would be much harder to maintain across the
   Python boundary.
2. **CI integration**. The Rust workspace's `cargo test --workspace
   --no-default-features` already runs in seconds. Adding QuTiP would
   pull a Python dependency into CI and slow the gate.

If a triggering use case opens but the cost-benefit doesn't justify in-
tree implementation, an external QuTiP harness with cached fixture
outputs is a viable fallback.

## Consequences

### Positive

- **No premature engineering.** 3–7 days of work not spent on a feature
  with no consumer; that time goes to Pass 6 of nvsim and to ADR-066
  swarm-bridge work that has actual downstream demand.
- **Honest scope.** ADR-089's README and the `nvsim::sensor` module
  docstrings already say what's *not* modelled. ADR-090 is the
  formal accountability for that boundary.
- **Reversible.** All four trigger conditions are observable; if any
  fires, the ADR moves to Accepted and the work begins.

### Negative / risks

- **Risk of premature commitment if triggers fire.** If pulsed-protocol
  use cases emerge late in the project (e.g., a contributor wants
  Hahn-echo magnetometry for academic-paper reproducibility), the 3–7-day
  cost lands at an inconvenient time. Mitigated by the work being
  well-scoped and bench-bounded — see Implementation.
- **Documentation debt.** Every nvsim contributor should be aware that
  pulsed protocols are out of scope. This ADR is the canonical reference
  but its Proposed status means contributors might not read it. Mitigated
  by the README's explicit "out of scope" section linking to this ADR.

### Neutral

- The existing linear-readout proxy is already feature-flag-free and
  always-on; no API changes when ADR-090 lands. The Lindblad path is
  additive.

## Implementation (when triggered)

If this ADR transitions to Accepted, the implementation is:

1. **Add `lindblad` feature to `nvsim/Cargo.toml`** — opt-in, default-off.
   Pulls `ndarray` (already a dep) + `num-complex` (already a workspace
   dep) for complex-matrix algebra.
2. **`src/lindblad.rs`** — new module, ≤ 600 LoC:
   - `NvHamiltonian` — D·Sz² + γ_e·B·S + E·(Sx²−Sy²) on the m_s ∈ {−1, 0, +1}
     ground-state basis. Optional ¹⁴N or ¹⁵N hyperfine extension.
   - `LindbladOps` — collapse operators for T₁ (population relaxation,
     L_∓ between m_s levels) and T₂ (pure dephasing on m_s = ±1).
   - `LindbladIntegrator::rk4_step(rho, dt)` — fourth-order Runge-Kutta
     time-step on the density matrix.
   - `Pulse` enum — supports CW, square, Gaussian-shaped MW pulses.
3. **`src/lindblad_protocols.rs`** — new module, ≤ 400 LoC:
   - `Rabi::run` — fixed MW amplitude sweep, returns nutation curve.
   - `HahnEcho::run` — π/2 — τ — π — τ — π/2 detection sequence.
   - `Cpmg::run` — repeated π pulses for dynamical decoupling.
4. **Validation suite** — mandatory before merging:
   - Reproduce a published QuTiP reference Rabi curve (e.g., from a
     Doherty 2013 supplementary script) within 1% per-bin error.
   - Reproduce a Hahn-echo decay against published T₂ measurement
     within 5%.
   - Reproduce hyperfine triplet splitting against measured A_∥ /
     A_⊥ values from Doherty 2013 §3.4.
5. **Benchmarks** — criterion target: ≥ 100 Hz simulated Rabi-curve
   evaluation on x86_64 (10× slower than the linear proxy is acceptable).
6. **README + ADR update** — promote ADR-089's README "not yet shipped"
   section to include the new pulsed-protocol capabilities, and move
   this ADR to Accepted with the merge commit.

Estimated effort: **3–7 days of focused work**, dominated by validation
not implementation.

## Validation (Proposed → Accepted)

This ADR is **Proposed** until any of the four trigger conditions in §"
Trigger conditions" fires. When that happens:

1. Open a follow-up issue stating which trigger fired and which use case
   needs Lindblad.
2. The implementation §1–6 above defines the build.
3. Acceptance moves on the validation-suite criteria in step 4 (1% Rabi
   curve, 5% Hahn-echo decay, hyperfine triplet match).
4. Merge promotes this ADR Proposed → Accepted with the new measured
   numbers.

## Open questions

- **Which Rust complex-matrix library is the right substrate?** Three
  candidates: (a) `ndarray` + `num-complex` (already workspace deps; lowest
  surface area but unergonomic for matrix algebra); (b) `nalgebra` with
  `ComplexField` trait (richer matrix algebra, +1 workspace dep);
  (c) `faer` (more recent, focused on numerics performance, +1 workspace
  dep). Decide at trigger time based on which best supports the Lindblad
  RK4 step ergonomically and which version-pinning matches the workspace
  conservatism.
- **Is hyperfine modelling in v1 or v2?** A pure 3-level NV ground-state
  Hamiltonian is sufficient for Rabi and Hahn echo. ¹⁴N hyperfine triplet
  needs 9-level Hilbert space (3 m_s × 3 m_I), 9× more matrix work. v1
  could ship with hyperfine off behind a sub-feature; v2 enables it.
- **Should the Lindblad solver back-validate the linear proxy?** Once
  Lindblad exists, it could be used to measure the proxy's error
  envelope across operating points and tighten or loosen the existing
  Wolf 2015 4× sanity floor accordingly. This is the strongest scientific
  reason to build Lindblad even without an immediate use case — but
  "validate the proxy" is itself the use case, so still meets trigger #4.

## Related

- **ADR-089** — nvsim NV-diamond simulator. The crate this extension
  attaches to.
- **ADR-018** — CSI binary frame format. Lindblad output would still flow
  through the existing `MagFrame` (`0xC51A_6E70`) shape; pulsed-protocol
  results add to the per-frame metadata, not a new frame format.
- **ADR-028** — ESP32 capability audit. Lindblad is host-side only; ESP32
  firmware untouched.
- **ADR-066** — Swarm bridge. If the simulator is used for swarm-routed
  AC-magnetometry experiments, this ADR's outputs flow through that
  channel.
