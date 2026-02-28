# sevensense-audio

[![Crate](https://img.shields.io/badge/crates.io-sevensense--audio-orange.svg)](https://crates.io/crates/sevensense-audio)
[![Docs](https://img.shields.io/badge/docs-sevensense--audio-blue.svg)](https://docs.rs/sevensense-audio)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Audio ingestion and preprocessing pipeline for bioacoustic analysis.

**sevensense-audio** handles all aspects of audio input—loading files, streaming from microphones, segmenting into fixed-length chunks, computing Mel spectrograms, and normalizing for neural network input. It's the gateway for raw audio into the 7sense platform.

## Features

- **Multi-Format Support**: WAV, MP3, FLAC, OGG via symphonia
- **Streaming Input**: Real-time microphone/line-in capture
- **Smart Segmentation**: Fixed-length or voice-activity-based splitting
- **Mel Spectrograms**: Configurable FFT, hop length, and mel bins
- **Audio Augmentation**: Time stretch, pitch shift, noise injection
- **Batch Processing**: Process multiple files in parallel

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| File Loading | Load audio from various formats | `AudioLoader::load()` |
| Segmentation | Split recordings into analysis chunks | `Segmenter::segment()` |
| Spectrogram | Convert audio to mel spectrogram | `MelSpectrogram::compute()` |
| Streaming | Real-time audio capture | `AudioStream::new()` |
| Augmentation | Data augmentation for training | `Augmenter::augment()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-audio = "0.1"
```

## Quick Start

```rust
use sevensense_audio::{AudioLoader, Segmenter, MelSpectrogram};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load audio file
    let audio = AudioLoader::load("birdsong.wav").await?;
    println!("Loaded {} seconds of audio", audio.duration_secs());

    // Segment into 5-second chunks
    let segmenter = Segmenter::new(5.0, 0.5);  // 5s windows, 0.5s overlap
    let segments = segmenter.segment(&audio);
    println!("Created {} segments", segments.len());

    // Compute mel spectrograms
    for segment in &segments {
        let mel = MelSpectrogram::compute(segment, Default::default())?;
        println!("Mel shape: {:?}", mel.shape());
    }

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: Loading Audio Files</b></summary>

### Basic File Loading

```rust
use sevensense_audio::{AudioLoader, AudioFormat};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Auto-detect format
    let audio = AudioLoader::load("recording.mp3").await?;

    // Get audio properties
    println!("Sample rate: {} Hz", audio.sample_rate());
    println!("Channels: {}", audio.channels());
    println!("Duration: {:.2}s", audio.duration_secs());
    println!("Samples: {}", audio.samples().len());

    Ok(())
}
```

### Loading with Options

```rust
use sevensense_audio::{AudioLoader, LoadOptions};

let options = LoadOptions {
    target_sample_rate: Some(32000),  // Resample to 32kHz
    mono: true,                        // Convert to mono
    normalize: true,                   // Normalize amplitude
};

let audio = AudioLoader::load_with_options("stereo.wav", options).await?;
assert_eq!(audio.channels(), 1);
assert_eq!(audio.sample_rate(), 32000);
```

### Batch Loading

```rust
use sevensense_audio::AudioLoader;

let paths = vec!["file1.wav", "file2.wav", "file3.wav"];
let audios = AudioLoader::load_batch(&paths).await?;

for (path, audio) in paths.iter().zip(audios.iter()) {
    println!("{}: {:.2}s", path, audio.duration_secs());
}
```

</details>

<details>
<summary><b>Tutorial: Audio Segmentation</b></summary>

### Fixed-Length Segmentation

```rust
use sevensense_audio::{AudioLoader, Segmenter};

let audio = AudioLoader::load("long_recording.wav").await?;

// 5-second segments with 50% overlap
let segmenter = Segmenter::new(5.0, 2.5);
let segments = segmenter.segment(&audio);

println!("Total segments: {}", segments.len());
for (i, seg) in segments.iter().enumerate() {
    println!("Segment {}: {:.2}s - {:.2}s",
        i, seg.start_time(), seg.end_time());
}
```

### Voice Activity Detection (VAD)

```rust
use sevensense_audio::{AudioLoader, VadSegmenter, VadConfig};

let audio = AudioLoader::load("recording.wav").await?;

let config = VadConfig {
    energy_threshold: 0.01,
    min_speech_duration: 0.3,
    min_silence_duration: 0.5,
};

let segmenter = VadSegmenter::new(config);
let segments = segmenter.segment(&audio);

println!("Found {} vocalizations", segments.len());
```

### Segment Iterator

```rust
use sevensense_audio::{AudioLoader, SegmentIterator};

let audio = AudioLoader::load("stream.wav").await?;

// Lazy iteration over segments
for segment in SegmentIterator::new(&audio, 5.0, 0.0) {
    // Process each segment
    println!("Processing segment at {:.2}s", segment.start_time());
}
```

</details>

<details>
<summary><b>Tutorial: Mel Spectrograms</b></summary>

### Basic Mel Computation

```rust
use sevensense_audio::{AudioLoader, MelSpectrogram, MelConfig};

let audio = AudioLoader::load("birdsong.wav").await?;

// Default configuration (128 mel bins, 2048 FFT, 512 hop)
let mel = MelSpectrogram::compute(&audio, Default::default())?;

println!("Mel spectrogram shape: {:?}", mel.shape());
// Shape: [n_frames, n_mels] e.g., [312, 128]
```

### Custom Configuration

```rust
use sevensense_audio::{MelSpectrogram, MelConfig};

let config = MelConfig {
    n_mels: 128,          // Number of mel frequency bins
    n_fft: 2048,          // FFT window size
    hop_length: 512,      // Hop between frames
    f_min: 50.0,          // Minimum frequency (Hz)
    f_max: 14000.0,       // Maximum frequency (Hz)
    power: 2.0,           // Power spectrogram exponent
    normalized: true,     // Normalize by max value
};

let mel = MelSpectrogram::compute(&audio, config)?;
```

### Log-Mel Spectrogram

```rust
use sevensense_audio::{MelSpectrogram, MelConfig};

let mel = MelSpectrogram::compute(&audio, Default::default())?;

// Convert to log scale (commonly used for neural networks)
let log_mel = mel.to_log_scale(1e-10);  // Add small constant to avoid log(0)
```

### Visualizing Spectrograms

```rust
use sevensense_audio::{MelSpectrogram, visualize};

let mel = MelSpectrogram::compute(&audio, Default::default())?;

// Save as PNG image
visualize::save_spectrogram(&mel, "spectrogram.png")?;

// Get as RGB buffer
let rgb_buffer = visualize::to_rgb(&mel, "viridis")?;
```

</details>

<details>
<summary><b>Tutorial: Audio Augmentation</b></summary>

### Basic Augmentation

```rust
use sevensense_audio::{AudioLoader, Augmenter, AugmentConfig};

let audio = AudioLoader::load("training_sample.wav").await?;

let config = AugmentConfig {
    time_stretch_range: (0.9, 1.1),  // ±10% speed
    pitch_shift_range: (-2, 2),       // ±2 semitones
    noise_level: 0.01,                // 1% noise
};

let augmenter = Augmenter::new(config);
let augmented = augmenter.augment(&audio)?;
```

### Specific Augmentations

```rust
use sevensense_audio::{Augmenter, TimeStretch, PitchShift, NoiseInjection};

// Time stretch (slow down by 10%)
let stretched = TimeStretch::apply(&audio, 0.9)?;

// Pitch shift (up by 2 semitones)
let shifted = PitchShift::apply(&audio, 2)?;

// Add background noise
let noisy = NoiseInjection::apply(&audio, 0.02)?;
```

### Augmentation Pipeline

```rust
use sevensense_audio::{AugmentPipeline, RandomCrop, Normalize};

let pipeline = AugmentPipeline::new()
    .add(TimeStretch::random(0.9, 1.1))
    .add(PitchShift::random(-2, 2))
    .add(NoiseInjection::gaussian(0.01))
    .add(RandomCrop::new(5.0))
    .add(Normalize::peak());

let augmented = pipeline.apply(&audio)?;
```

</details>

---

## Configuration

### Mel Spectrogram Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `n_mels` | 128 | Number of mel frequency bins |
| `n_fft` | 2048 | FFT window size |
| `hop_length` | 512 | Samples between frames |
| `f_min` | 50.0 | Minimum frequency (Hz) |
| `f_max` | 14000.0 | Maximum frequency (Hz) |

### Segmentation Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `window_size` | 5.0 | Segment duration in seconds |
| `overlap` | 0.5 | Overlap between segments in seconds |
| `min_duration` | 1.0 | Minimum segment duration |

## Performance

| Operation | Throughput | Notes |
|-----------|------------|-------|
| File Loading | ~500 MB/s | With SSD |
| Mel Spectrogram | ~1000 segments/s | 5s segments |
| Resampling | ~200 MB/s | Using libsamplerate |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-audio](https://crates.io/crates/sevensense-audio)
- **Documentation**: [docs.rs/sevensense-audio](https://docs.rs/sevensense-audio)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
