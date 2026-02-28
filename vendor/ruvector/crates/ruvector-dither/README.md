# ruvector-dither

Deterministic, low-discrepancy **pre-quantization dithering** for low-bit
neural network inference on tiny devices (WASM, Seed, STM32).

## Why dither?

Quantizers at 3/5/7 bits can align with power-of-two boundaries, producing
idle tones, sticky activations, and periodic errors that degrade accuracy.
A sub-LSB pre-quantization offset:

- Decorrelates the signal from grid boundaries.
- Pushes quantization error toward high frequencies (blue-noise-like),
  which average out downstream.
- Uses **no RNG** -- outputs are deterministic, reproducible across
  platforms (WASM / x86 / ARM), and cache-friendly.

## Features

- **Golden-ratio sequence** -- best 1-D equidistribution, irrational period (never repeats).
- **Pi-digit table** -- 256-byte cyclic lookup, exact reproducibility from a tensor/layer ID.
- **Per-channel dither pools** -- structurally decorrelated channels without any randomness.
- **Scalar, slice, and integer-code quantization** helpers included.
- **`no_std`-compatible** -- zero runtime dependencies; enable with `features = ["no_std"]`.

## Quick start

```rust
use ruvector_dither::{GoldenRatioDither, PiDither, quantize_dithered};

// Golden-ratio dither, 8-bit, epsilon = 0.5 LSB
let mut gr = GoldenRatioDither::new(0.0);
let q = quantize_dithered(0.314, 8, 0.5, &mut gr);
assert!(q >= -1.0 && q <= 1.0);

// Pi-digit dither, 5-bit
let mut pi = PiDither::new(0);
let q2 = quantize_dithered(0.271, 5, 0.5, &mut pi);
assert!(q2 >= -1.0 && q2 <= 1.0);
```

### Per-channel batch quantization

```rust
use ruvector_dither::ChannelDither;

let mut cd = ChannelDither::new(/*layer_id=*/ 0, /*channels=*/ 8, /*bits=*/ 5, /*eps=*/ 0.5);
let mut activations = vec![0.5_f32; 64]; // shape [batch=8, channels=8]
cd.quantize_batch(&mut activations);
```

## Modules

| Module | Description |
|--------|-------------|
| `golden` | `GoldenRatioDither` -- additive golden-ratio quasi-random sequence |
| `pi` | `PiDither` -- cyclic 256-byte table derived from digits of pi |
| `quantize` | `quantize_dithered`, `quantize_slice_dithered`, `quantize_to_code` |
| `channel` | `ChannelDither` -- per-channel dither pool seeded from layer/channel IDs |

## Trait: `DitherSource`

Implement `DitherSource` to plug in your own deterministic sequence:

```rust
pub trait DitherSource {
    /// Return the next zero-mean offset in [-0.5, +0.5].
    fn next_unit(&mut self) -> f32;
}
```

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT License](http://opensource.org/licenses/MIT) at your option.
