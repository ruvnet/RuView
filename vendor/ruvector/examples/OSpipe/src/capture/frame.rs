//! Captured frame data structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single captured frame from any Screenpipe source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedFrame {
    /// Unique identifier for this frame.
    pub id: Uuid,
    /// When this frame was captured.
    pub timestamp: DateTime<Utc>,
    /// The source that produced this frame.
    pub source: CaptureSource,
    /// The actual content of the frame.
    pub content: FrameContent,
    /// Additional metadata about the frame.
    pub metadata: FrameMetadata,
}

/// The source that produced a captured frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
    /// Screen capture with OCR.
    Screen {
        /// Monitor index.
        monitor: u32,
        /// Foreground application name.
        app: String,
        /// Window title.
        window: String,
    },
    /// Audio capture with transcription.
    Audio {
        /// Audio device name.
        device: String,
        /// Detected speaker (if diarization is available).
        speaker: Option<String>,
    },
    /// UI accessibility event.
    Ui {
        /// Type of UI event (e.g., "click", "focus", "scroll").
        event_type: String,
    },
}

/// The actual content extracted from a captured frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrameContent {
    /// OCR text extracted from a screen capture.
    OcrText(String),
    /// Transcribed text from an audio capture.
    Transcription(String),
    /// A UI accessibility event description.
    UiEvent(String),
}

/// Metadata associated with a captured frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetadata {
    /// Name of the foreground application, if known.
    pub app_name: Option<String>,
    /// Title of the active window, if known.
    pub window_title: Option<String>,
    /// Monitor index, if applicable.
    pub monitor_id: Option<u32>,
    /// Confidence score for the extracted content (0.0 to 1.0).
    pub confidence: f32,
    /// Detected language code (e.g., "en", "es"), if known.
    pub language: Option<String>,
}

impl CapturedFrame {
    /// Create a new frame from a screen capture with OCR text.
    pub fn new_screen(app: &str, window: &str, ocr_text: &str, monitor: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: CaptureSource::Screen {
                monitor,
                app: app.to_string(),
                window: window.to_string(),
            },
            content: FrameContent::OcrText(ocr_text.to_string()),
            metadata: FrameMetadata {
                app_name: Some(app.to_string()),
                window_title: Some(window.to_string()),
                monitor_id: Some(monitor),
                confidence: 0.9,
                language: None,
            },
        }
    }

    /// Create a new frame from an audio transcription.
    pub fn new_audio(device: &str, transcription: &str, speaker: Option<&str>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: CaptureSource::Audio {
                device: device.to_string(),
                speaker: speaker.map(|s| s.to_string()),
            },
            content: FrameContent::Transcription(transcription.to_string()),
            metadata: FrameMetadata {
                app_name: None,
                window_title: None,
                monitor_id: None,
                confidence: 0.85,
                language: None,
            },
        }
    }

    /// Create a new frame from a UI accessibility event.
    pub fn new_ui_event(event_type: &str, description: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: CaptureSource::Ui {
                event_type: event_type.to_string(),
            },
            content: FrameContent::UiEvent(description.to_string()),
            metadata: FrameMetadata {
                app_name: None,
                window_title: None,
                monitor_id: None,
                confidence: 1.0,
                language: None,
            },
        }
    }

    /// Extract the text content from this frame regardless of source type.
    pub fn text_content(&self) -> &str {
        match &self.content {
            FrameContent::OcrText(text) => text,
            FrameContent::Transcription(text) => text,
            FrameContent::UiEvent(text) => text,
        }
    }

    /// Return the content type as a string label.
    pub fn content_type(&self) -> &str {
        match &self.content {
            FrameContent::OcrText(_) => "ocr",
            FrameContent::Transcription(_) => "transcription",
            FrameContent::UiEvent(_) => "ui_event",
        }
    }
}

impl Default for FrameMetadata {
    fn default() -> Self {
        Self {
            app_name: None,
            window_title: None,
            monitor_id: None,
            confidence: 0.0,
            language: None,
        }
    }
}
