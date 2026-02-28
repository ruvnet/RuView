# exo-exotic

Exotic cognitive experiments for EXO-AI. A laboratory crate that
implements speculative and frontier cognitive phenomena, providing
building blocks for research into non-standard AI architectures.

## Features

- **Strange loops** -- self-referential feedback structures (Hofstadter).
- **Dream generation** -- offline generative replay for memory consolidation.
- **Free energy minimization** -- active inference (Friston) to reduce
  prediction error.
- **Morphogenesis** -- developmental growth rules for self-organisation.
- **Collective consciousness** -- shared awareness across substrates.
- **Temporal qualia** -- subjective time as a first-class object.
- **Multiple selves** -- parallel competing/cooperating identity models.
- **Cognitive thermodynamics** -- entropy production and efficiency tracking.
- **Emergence detection** -- phase transitions in cognitive networks.
- **Cognitive black holes** -- information-trapping attractor dynamics.
- **Domain transfer** -- cross-domain knowledge migration strategies.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-exotic = "0.1"
```

Basic usage:

```rust
use exo_exotic::{DreamEngine, StrangeLoop, ExoticExperiments};

// Run a dream consolidation cycle
let mut dreamer = DreamEngine::with_creativity(0.8);
dreamer.add_memory(vec![0.1, 0.2, 0.3, 0.4], 0.7, 0.9);
let report = dreamer.dream_cycle(100);
println!("Creativity: {:.2}", report.creativity_score);

// Create a strange loop
let mut sl = StrangeLoop::new(10);
sl.model_self();
println!("Self-model depth: {}", sl.measure_depth());

// Run all experiments at once
let mut suite = ExoticExperiments::new();
let results = suite.run_all();
println!("Overall score: {:.2}", results.overall_score());
```

## Crate Layout

| Module            | Purpose                                  |
|-------------------|------------------------------------------|
| `strange_loops`   | Self-referential feedback structures      |
| `dreams`          | Offline generative replay                 |
| `free_energy`     | Active inference engine                   |
| `morphogenesis`   | Developmental self-organisation           |
| `collective`      | Multi-substrate shared awareness          |
| `temporal_qualia` | Subjective time representation            |
| `multiple_selves` | Parallel identity models                  |
| `thermodynamics`  | Cognitive entropy and energy tracking     |
| `emergence`       | Phase transition detection                |
| `black_holes`     | Attractor dynamics and escape methods     |

## Requirements

- Rust 1.78+
- Depends on `exo-core`

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
