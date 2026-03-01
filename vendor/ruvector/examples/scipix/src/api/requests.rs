use serde::{Deserialize, Serialize};
use validator::Validate;

/// Text/Image OCR request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TextRequest {
    /// Image source (base64, URL, or multipart)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,

    /// Base64 encoded image data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,

    /// Image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(url)]
    pub url: Option<String>,

    /// Request metadata
    #[serde(default)]
    pub metadata: RequestMetadata,
}

impl TextRequest {
    /// Get image data from request
    pub async fn get_image_data(&self) -> anyhow::Result<Vec<u8>> {
        if let Some(base64_data) = &self.base64 {
            // Decode base64
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD.decode(base64_data)?;
            Ok(decoded)
        } else if let Some(url) = &self.url {
            // Download from URL
            let response = reqwest::get(url).await?;
            let bytes = response.bytes().await?;
            Ok(bytes.to_vec())
        } else {
            anyhow::bail!("No image data provided")
        }
    }
}

/// Digital ink strokes request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct StrokesRequest {
    /// Array of stroke data
    #[validate(length(min = 1))]
    pub strokes: Vec<Stroke>,

    /// Request metadata
    #[serde(default)]
    pub metadata: RequestMetadata,
}

/// Single stroke in digital ink
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    /// X coordinates
    pub x: Vec<f64>,

    /// Y coordinates
    pub y: Vec<f64>,

    /// Optional timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<Vec<f64>>,
}

/// Legacy LaTeX equation request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LatexRequest {
    /// Image source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,

    /// Base64 encoded image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,

    /// Image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(url)]
    pub url: Option<String>,

    /// Request metadata
    #[serde(default)]
    pub metadata: RequestMetadata,
}

/// PDF processing request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PdfRequest {
    /// PDF file URL
    #[validate(url)]
    pub url: String,

    /// Conversion options
    #[serde(default)]
    pub options: PdfOptions,

    /// Webhook URL for completion notification
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(url)]
    pub webhook_url: Option<String>,

    /// Request metadata
    #[serde(default)]
    pub metadata: RequestMetadata,
}

/// PDF processing options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfOptions {
    /// Output format
    #[serde(default = "default_format")]
    pub format: String,

    /// Enable OCR
    #[serde(default)]
    pub enable_ocr: bool,

    /// Include images
    #[serde(default = "default_true")]
    pub include_images: bool,

    /// Page range (e.g., "1-5")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_range: Option<String>,
}

fn default_format() -> String {
    "mmd".to_string()
}

fn default_true() -> bool {
    true
}

/// Request metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestMetadata {
    /// Output formats
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,

    /// Include confidence scores
    #[serde(default)]
    pub include_confidence: bool,

    /// Enable math mode
    #[serde(default = "default_true")]
    pub enable_math: bool,

    /// Language hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

fn default_formats() -> Vec<String> {
    vec!["text".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_request_validation() {
        let request = TextRequest {
            src: None,
            base64: Some("SGVsbG8gV29ybGQ=".to_string()),
            url: None,
            metadata: RequestMetadata::default(),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_strokes_request_validation() {
        let request = StrokesRequest {
            strokes: vec![Stroke {
                x: vec![0.0, 1.0, 2.0],
                y: vec![0.0, 1.0, 0.0],
                t: None,
            }],
            metadata: RequestMetadata::default(),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_empty_strokes_validation() {
        let request = StrokesRequest {
            strokes: vec![],
            metadata: RequestMetadata::default(),
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_pdf_request_validation() {
        let request = PdfRequest {
            url: "https://example.com/document.pdf".to_string(),
            options: PdfOptions::default(),
            webhook_url: None,
            metadata: RequestMetadata::default(),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_invalid_url() {
        let request = PdfRequest {
            url: "not-a-url".to_string(),
            options: PdfOptions::default(),
            webhook_url: None,
            metadata: RequestMetadata::default(),
        };

        assert!(request.validate().is_err());
    }
}
