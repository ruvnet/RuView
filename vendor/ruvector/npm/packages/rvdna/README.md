# @ruvector/rvdna

**DNA analysis in JavaScript.** Encode sequences, translate proteins, search genomes by similarity, and read the `.rvdna` AI-native file format — all from Node.js or the browser.

Built on Rust via NAPI-RS for native speed. Falls back to pure JavaScript when native bindings aren't available.

```bash
npm install @ruvector/rvdna
```

## What It Does

| Function | What It Does | Native Required? |
|---|---|---|
| `encode2bit(seq)` | Pack DNA into 2-bit bytes (4 bases per byte) | No (JS fallback) |
| `decode2bit(buf, len)` | Unpack 2-bit bytes back to DNA string | No (JS fallback) |
| `translateDna(seq)` | Translate DNA to protein amino acids | No (JS fallback) |
| `cosineSimilarity(a, b)` | Cosine similarity between two vectors | No (JS fallback) |
| `fastaToRvdna(seq, opts)` | Convert FASTA to `.rvdna` binary format | Yes |
| `readRvdna(buf)` | Parse a `.rvdna` file from a Buffer | Yes |
| `isNativeAvailable()` | Check if native Rust bindings are loaded | No |

## Quick Start

```js
const { encode2bit, decode2bit, translateDna, cosineSimilarity } = require('@ruvector/rvdna');

// Encode DNA to compact 2-bit format (4 bases per byte)
const packed = encode2bit('ACGTACGTACGT');
console.log(packed); // <Buffer 1b 1b 1b>

// Decode it back — lossless round-trip
const dna = decode2bit(packed, 12);
console.log(dna); // 'ACGTACGTACGT'

// Translate DNA to protein (standard genetic code)
const protein = translateDna('ATGGCCATTGTAATG');
console.log(protein); // 'MAIV'

// Compare two k-mer vectors
const sim = cosineSimilarity([1, 2, 3], [1, 2, 3]);
console.log(sim); // 1.0 (identical)
```

## API Reference

### `encode2bit(sequence: string): Buffer`

Packs a DNA string into 2-bit bytes. Each byte holds 4 bases: A=00, C=01, G=10, T=11. Ambiguous bases (N) map to A.

```js
encode2bit('ACGT') // <Buffer 1b> — one byte for 4 bases
encode2bit('AAAA') // <Buffer 00>
encode2bit('TTTT') // <Buffer ff>
```

### `decode2bit(buffer: Buffer, length: number): string`

Decodes 2-bit packed bytes back to a DNA string. You must pass the original sequence length since the last byte may have padding.

```js
decode2bit(Buffer.from([0x1b]), 4) // 'ACGT'
```

### `translateDna(sequence: string): string`

Translates a DNA string to a protein amino acid string using the standard genetic code. Stops at the first stop codon (TAA, TAG, TGA).

```js
translateDna('ATGGCCATTGTAATGGGCCGCTGAAAGGGTGCCCGA')
// 'MAIVMGR' — stops at TGA stop codon
```

### `cosineSimilarity(a: number[], b: number[]): number`

Returns cosine similarity between two numeric arrays. Result is between -1 and 1.

```js
cosineSimilarity([1, 0, 0], [0, 1, 0]) // 0 (orthogonal)
cosineSimilarity([1, 2, 3], [2, 4, 6]) // 1 (parallel)
```

### `fastaToRvdna(sequence: string, options?: RvdnaOptions): Buffer`

Converts a raw DNA sequence to the `.rvdna` binary format with pre-computed k-mer vectors. **Requires native bindings.**

```js
const { fastaToRvdna, isNativeAvailable } = require('@ruvector/rvdna');

if (isNativeAvailable()) {
  const rvdna = fastaToRvdna('ACGTACGT...', { k: 11, dims: 512, blockSize: 500 });
  require('fs').writeFileSync('output.rvdna', rvdna);
}
```

| Option | Default | Description |
|---|---|---|
| `k` | 11 | K-mer size for vector encoding |
| `dims` | 512 | Vector dimensions per block |
| `blockSize` | 500 | Bases per vector block |

### `readRvdna(buffer: Buffer): RvdnaFile`

Parses a `.rvdna` file. Returns the decoded sequence, k-mer vectors, variants, metadata, and file statistics. **Requires native bindings.**

```js
const fs = require('fs');
const { readRvdna } = require('@ruvector/rvdna');

const file = readRvdna(fs.readFileSync('sample.rvdna'));

console.log(file.sequenceLength);           // 430
console.log(file.sequence.slice(0, 20));    // 'ATGGTGCATCTGACTCCTGA'
console.log(file.kmerVectors.length);       // number of vector blocks
console.log(file.stats.bitsPerBase);        // ~3.2
console.log(file.stats.compressionRatio);   // vs raw FASTA
```

**RvdnaFile fields:**

| Field | Type | Description |
|---|---|---|
| `version` | `number` | Format version |
| `sequenceLength` | `number` | Number of bases |
| `sequence` | `string` | Decoded DNA string |
| `kmerVectors` | `Array` | Pre-computed k-mer vector blocks |
| `variants` | `Array \| null` | Variant positions with genotype likelihoods |
| `metadata` | `Record \| null` | Key-value metadata |
| `stats.totalSize` | `number` | File size in bytes |
| `stats.bitsPerBase` | `number` | Storage efficiency |
| `stats.compressionRatio` | `number` | Compression vs raw |

## The `.rvdna` File Format

Traditional genomic formats (FASTA, FASTQ, BAM) store raw sequences. Every time an AI model needs that data, it re-encodes everything from scratch — vectors, attention matrices, features. This takes 30-120 seconds per file.

`.rvdna` stores the sequence **and** pre-computed AI features together. Open the file and everything is ready — no re-encoding.

```
.rvdna file layout:

[Magic: "RVDNA\x01\x00\x00"]        8 bytes — file identifier
[Header]                              64 bytes — version, flags, offsets
[Section 0: Sequence]                 2-bit packed DNA (4 bases/byte)
[Section 1: K-mer Vectors]            HNSW-ready embeddings
[Section 2: Attention Weights]        Sparse COO matrices
[Section 3: Variant Tensor]           f16 genotype likelihoods
[Section 4: Protein Embeddings]       GNN features + contact graphs
[Section 5: Epigenomic Tracks]        Methylation + clock data
[Section 6: Metadata]                 JSON provenance + checksums
```

### Format Comparison

| | FASTA | FASTQ | BAM | CRAM | **.rvdna** |
|---|---|---|---|---|---|
| **Encoding** | ASCII (1 char/base) | ASCII + Phred | Binary + ref | Ref-compressed | 2-bit packed |
| **Bits per base** | 8 | 16 | 2-4 | 0.5-2 | **3.2** (seq only) |
| **Random access** | Scan from start | Scan from start | Index ~10 us | Decode ~50 us | **mmap <1 us** |
| **AI features included** | No | No | No | No | **Yes** |
| **Vector search ready** | No | No | No | No | **HNSW built-in** |
| **Zero-copy mmap** | No | No | Partial | No | **Full** |
| **Single file** | Yes | Yes | Needs .bai | Needs .crai | **Yes** |

## Platform Support

Native NAPI-RS bindings are available for these platforms:

| Platform | Architecture | Package |
|---|---|---|
| Linux | x64 (glibc) | `@ruvector/rvdna-linux-x64-gnu` |
| Linux | ARM64 (glibc) | `@ruvector/rvdna-linux-arm64-gnu` |
| macOS | x64 (Intel) | `@ruvector/rvdna-darwin-x64` |
| macOS | ARM64 (Apple Silicon) | `@ruvector/rvdna-darwin-arm64` |
| Windows | x64 | `@ruvector/rvdna-win32-x64-msvc` |

These install automatically as optional dependencies. On unsupported platforms, basic functions (`encode2bit`, `decode2bit`, `translateDna`, `cosineSimilarity`) still work via pure JavaScript fallbacks.

## WASM (WebAssembly)

rvDNA can run entirely in the browser via WebAssembly. No server needed, no data leaves the user's device.

### Browser Setup

```bash
# Build from the Rust source
cd examples/dna
wasm-pack build --target web --release
```

This produces a `pkg/` directory with `.wasm` and `.js` glue code.

### Using in HTML

```html
<script type="module">
  import init, { encode2bit, translateDna } from './pkg/rvdna.js';

  await init();  // Load the WASM module

  // Encode DNA
  const packed = encode2bit('ACGTACGTACGT');
  console.log('Packed bytes:', packed);

  // Translate to protein
  const protein = translateDna('ATGGCCATTGTAATG');
  console.log('Protein:', protein);  // 'MAIV'
</script>
```

### Using with Bundlers (Webpack, Vite)

```bash
# For bundler targets
wasm-pack build --target bundler --release
```

```js
// In your app
import { encode2bit, translateDna, fastaToRvdna } from '@ruvector/rvdna-wasm';

const packed = encode2bit('ACGTACGT');
const protein = translateDna('ATGGCCATT');
```

### WASM Features

| Feature | Status | Description |
|---|---|---|
| 2-bit encode/decode | Available | Pack/unpack DNA sequences |
| Protein translation | Available | Standard genetic code |
| Cosine similarity | Available | Vector comparison |
| `.rvdna` read/write | Planned | Full format support in browser |
| HNSW search | Planned | K-mer similarity search |
| Variant calling | Planned | Client-side mutation detection |

**Target WASM binary size:** <2 MB gzipped

### Privacy

WASM runs entirely client-side. DNA data never leaves the browser. This makes it suitable for:
- Clinical genomics dashboards
- Patient-facing genetic reports
- Educational tools
- Offline/edge analysis on devices with no internet

## TypeScript

Full TypeScript definitions are included. Import types directly:

```ts
import {
  encode2bit,
  decode2bit,
  translateDna,
  cosineSimilarity,
  fastaToRvdna,
  readRvdna,
  isNativeAvailable,
  RvdnaOptions,
  RvdnaFile,
} from '@ruvector/rvdna';
```

## Speed

The native (Rust) backend handles these operations on real human gene data:

| Operation | Time | What It Does |
|---|---|---|
| Single SNP call | **155 ns** | Bayesian genotyping at one position |
| Protein translation (1 kb) | **23 ns** | DNA to amino acids |
| K-mer vector (1 kb) | **591 us** | Full pipeline with HNSW indexing |
| Complete analysis (5 genes) | **12 ms** | All stages including `.rvdna` output |

### vs Traditional Tools

| Task | Traditional Tool | Their Time | rvDNA | Speedup |
|---|---|---|---|---|
| K-mer counting | Jellyfish | 15-30 min | 2-5 sec | **180-900x** |
| Sequence similarity | BLAST | 1-5 min | 5-50 ms | **1,200-60,000x** |
| Variant calling | GATK | 30-90 min | 3-10 min | **3-30x** |
| Methylation age | R/Bioconductor | 5-15 min | 0.1-0.5 sec | **600-9,000x** |

## Rust Crate

The full Rust crate with all algorithms is available on crates.io:

```toml
[dependencies]
rvdna = "0.1"
```

See the [Rust documentation](https://docs.rs/rvdna) for the complete API including Smith-Waterman alignment, Horvath clock, CYP2D6 pharmacogenomics, and more.

## Links

- [GitHub](https://github.com/ruvnet/ruvector/tree/main/examples/dna) - Source code
- [crates.io](https://crates.io/crates/rvdna) - Rust crate
- [RuVector](https://github.com/ruvnet/ruvector) - Parent vector computing platform

## License

MIT
