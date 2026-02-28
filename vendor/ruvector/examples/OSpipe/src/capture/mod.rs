//! Capture module for processing screen, audio, and UI event data.
//!
//! This module defines the data structures that represent captured frames
//! from Screenpipe sources: OCR text from screen recordings, audio
//! transcriptions, and UI accessibility events.

pub mod frame;

pub use frame::{CaptureSource, CapturedFrame, FrameContent, FrameMetadata};
