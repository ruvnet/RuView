# thermorust

A minimal thermodynamic neural-motif engine for Rust. Treats computation as
**energy-driven state transitions** with Landauer-style dissipation tracking
and Langevin/Metropolis noise baked in.

## Features

- **Ising and soft-spin Hamiltonians** with configurable coupling matrices and local fields.
- **Metropolis-Hastings** (discrete) and **overdamped Langevin** (continuous) dynamics.
- **Landauer dissipation accounting** -- every accepted irreversible transition charges
  kT ln 2 of heat, giving a physical energy audit of your computation.
- **Langevin and Poisson spike noise** sources satisfying the fluctuation-dissipation theorem.
- **Thermodynamic observables** -- magnetisation, pattern overlap, binary entropy,
  free energy, and running energy/dissipation traces.
- **Pre-wired motif factories** -- ring, fully-connected, Hopfield memory, and
  random soft-spin networks ready to simulate out of the box.
- **Simulated annealing** helpers for both discrete and continuous models.

## Quick start

```rust
use thermorust::{motifs::IsingMotif, dynamics::{Params, anneal_discrete}};
use rand::SeedableRng;

let mut motif = IsingMotif::ring(16, 0.2);
let params    = Params::default_n(16);
let mut rng   = rand::rngs::StdRng::seed_from_u64(42);

let trace = anneal_discrete(
    &motif.model, &mut motif.state, &params, 10_000, 100, &mut rng,
);
println!("Mean energy: {:.3}", trace.mean_energy());
println!("Heat shed:   {:.3e} J", trace.total_dissipation());
```

### Continuous soft-spin simulation

```rust
use thermorust::{motifs::SoftSpinMotif, dynamics::{Params, anneal_continuous}};
use rand::SeedableRng;

let mut motif = SoftSpinMotif::random(32, 1.0, 0.5, 42);
let params    = Params::default_n(32);
let mut rng   = rand::rngs::StdRng::seed_from_u64(7);

let trace = anneal_continuous(
    &motif.model, &mut motif.state, &params, 5_000, 50, &mut rng,
);
```

## Modules

| Module | Description |
|--------|-------------|
| `state` | `State` -- activation vector with cumulative dissipation counter |
| `energy` | `EnergyModel` trait, `Ising`, `SoftSpin`, `Couplings` |
| `dynamics` | `step_discrete` (MH), `step_continuous` (Langevin), annealers |
| `noise` | Langevin Gaussian and Poisson spike noise sources |
| `metrics` | Magnetisation, overlap, entropy, free energy, `Trace` |
| `motifs` | Pre-wired ring, fully-connected, Hopfield, and soft-spin motifs |

## Dependencies

- `rand` 0.8 (with `small_rng`)
- `rand_distr` 0.4

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT License](http://opensource.org/licenses/MIT) at your option.
