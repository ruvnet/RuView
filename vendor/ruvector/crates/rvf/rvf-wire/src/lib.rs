//! RVF wire format reader/writer.
//!
//! This crate implements the binary encoding and decoding for the RuVector
//! Format (RVF): segment headers, varint encoding, delta coding, hash
//! computation, tail scanning, and per-segment-type codecs.

pub mod delta;
pub mod hash;
pub mod hot_seg_codec;
pub mod index_seg_codec;
pub mod manifest_codec;
pub mod reader;
pub mod tail_scan;
pub mod varint;
pub mod vec_seg_codec;
pub mod writer;

pub use reader::{read_segment, read_segment_header, validate_segment};
pub use tail_scan::find_latest_manifest;
pub use writer::{calculate_padded_size, write_segment};
