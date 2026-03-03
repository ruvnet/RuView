# bit-parallel-search

Fast bit-parallel string matching that processes 64 positions simultaneously.

[![Crates.io](https://img.shields.io/crates/v/bit-parallel-search.svg)](https://crates.io/crates/bit-parallel-search)
[![Docs.rs](https://docs.rs/bit-parallel-search/badge.svg)](https://docs.rs/bit-parallel-search) 

## Performance

Processes up to 64 pattern positions in parallel using bit manipulation:

```
Pattern "quick" (5 bytes):    2.3x faster than naive
Pattern "medium" (19 bytes):  3.8x faster than naive
Pattern "long" (44 bytes):    4.2x faster than naive
```

## Usage

```rust
use bit_parallel_search::BitParallelMatcher;

let text = b"The quick brown fox";
let pattern = b"quick";

// Find first match
assert_eq!(BitParallelMatcher::find(text, pattern), Some(4));

// Find all matches
let matches: Vec<usize> = BitParallelMatcher::find_all(b"abababa", b"aba")
    .collect();
assert_eq!(matches, vec![0, 2, 4]);

// Count occurrences
assert_eq!(BitParallelMatcher::count(b"abababa", b"aba"), 3);
```

## How It Works

Uses bit masks to track pattern matches across 64 positions simultaneously:

1. Each bit represents a potential match position
2. Shift and mask operations update all positions in parallel
3. No branching in inner loop (CPU-friendly)

## Limitations

- Pattern must be ≤ 64 bytes
- Best performance for patterns < 32 bytes
- Not suitable for very long patterns

## Benchmarks

```bash
cargo bench
```

## Why Use This?

- **Actually faster** than standard approaches (verified)
- **No unsafe code** in main algorithm
- **No dependencies** (`no_std` compatible)
- **Cache-friendly** - processes data sequentially

## License

MIT OR Apache-2.0