# rvf-quant

Temperature-tiered vector quantization for RuVector Format.

## Overview

`rvf-quant` provides quantization codecs that reduce vector storage size based on access temperature:

- **f32** -- full precision for hot vectors
- **f16** -- half precision for warm vectors
- **u8** -- scalar quantization for cool vectors
- **binary** -- 1-bit quantization for cold/archive vectors
- **Automatic tiering** -- promote/demote vectors based on access patterns

## Usage

```toml
[dependencies]
rvf-quant = "0.1"
```

## Features

- `std` (default) -- enable `std` support
- `simd` -- enable SIMD-accelerated quantization

## License

MIT OR Apache-2.0
