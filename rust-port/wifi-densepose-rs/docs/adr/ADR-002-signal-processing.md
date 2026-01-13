# ADR-002: Signal Processing Library Selection

## Status
Accepted

## Context
CSI signal processing requires FFT operations, complex number handling, and matrix operations. We need to select appropriate Rust libraries that provide Python/NumPy equivalent functionality.

## Decision
We will use the following libraries:

| Library | Purpose | Python Equivalent |
|---------|---------|-------------------|
| `ndarray` | N-dimensional arrays | NumPy |
| `rustfft` | FFT operations | numpy.fft |
| `num-complex` | Complex numbers | complex |
| `num-traits` | Numeric traits | - |

### Key Implementations

1. **Phase Sanitization**: Multiple unwrapping methods (Standard, Custom, Itoh, Quality-Guided)
2. **CSI Processing**: Amplitude/phase extraction, temporal smoothing, Hamming windowing
3. **Feature Extraction**: Doppler, PSD, amplitude, phase, correlation features
4. **Motion Detection**: Variance-based with adaptive thresholds

## Consequences

### Positive
- Pure Rust implementation (no FFI overhead)
- WASM compatible (rustfft is pure Rust)
- NumPy-like API with ndarray
- High performance with SIMD optimizations

### Negative
- ndarray-linalg requires BLAS backend for advanced operations
- Learning curve for ndarray patterns

## References
- [ndarray documentation](https://docs.rs/ndarray)
- [rustfft documentation](https://docs.rs/rustfft)
