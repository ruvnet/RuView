//! JSON API response formatter matching Scipix API specification

use super::{FormatsData, LineData, OcrResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Complete API response matching Scipix format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Request identifier
    pub request_id: String,

    /// API version
    pub version: String,

    /// Image information
    pub image_width: u32,
    pub image_height: u32,

    /// Detection metadata
    pub is_printed: bool,
    pub is_handwritten: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_rotate_confidence: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_rotate_degrees: Option<i32>,

    /// Confidence metrics
    pub confidence: f32,
    pub confidence_rate: f32,

    /// Available output formats
    #[serde(flatten)]
    pub formats: FormatsData,

    /// Detailed line data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_data: Option<Vec<LineData>>,

    /// Error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_info: Option<ErrorInfo>,

    /// Processing metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

/// Error information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ApiResponse {
    /// Create response from OCR result
    pub fn from_ocr_result(result: OcrResult) -> Self {
        Self {
            request_id: result.request_id,
            version: result.version,
            image_width: result.image_width,
            image_height: result.image_height,
            is_printed: result.is_printed,
            is_handwritten: result.is_handwritten,
            auto_rotate_confidence: Some(result.auto_rotate_confidence),
            auto_rotate_degrees: Some(result.auto_rotate_degrees),
            confidence: result.confidence,
            confidence_rate: result.confidence_rate,
            formats: result.formats,
            line_data: result.line_data,
            error: result.error,
            error_info: None,
            metadata: if result.metadata.is_empty() {
                None
            } else {
                Some(result.metadata)
            },
        }
    }

    /// Create error response
    pub fn error(request_id: String, code: &str, message: &str) -> Self {
        Self {
            request_id,
            version: "3.0".to_string(),
            image_width: 0,
            image_height: 0,
            is_printed: false,
            is_handwritten: false,
            auto_rotate_confidence: None,
            auto_rotate_degrees: None,
            confidence: 0.0,
            confidence_rate: 0.0,
            formats: FormatsData::default(),
            line_data: None,
            error: Some(message.to_string()),
            error_info: Some(ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            }),
            metadata: None,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("JSON serialization error: {}", e))
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("JSON serialization error: {}", e))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("JSON parsing error: {}", e))
    }
}

/// Batch API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchApiResponse {
    pub batch_id: String,
    pub total: usize,
    pub completed: usize,
    pub results: Vec<ApiResponse>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<BatchError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchError {
    pub index: usize,
    pub error: ErrorInfo,
}

impl BatchApiResponse {
    pub fn new(batch_id: String, results: Vec<ApiResponse>) -> Self {
        let total = results.len();
        let completed = results.iter().filter(|r| r.error.is_none()).count();

        let errors: Vec<BatchError> = results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                r.error_info.as_ref().map(|e| BatchError {
                    index: i,
                    error: e.clone(),
                })
            })
            .collect();

        Self {
            batch_id,
            total,
            completed,
            results,
            errors: if errors.is_empty() {
                None
            } else {
                Some(errors)
            },
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("JSON serialization error: {}", e))
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("JSON serialization error: {}", e))
    }
}

/// API request format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// Image source (URL or base64)
    pub src: String,

    /// Requested output formats
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<String>>,

    /// OCR options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrOptions>,

    /// Additional metadata
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math_inline_delimiters: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub math_display_delimiters: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rm_spaces: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rm_fonts: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbers_default_to_math: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_result() -> OcrResult {
        OcrResult {
            request_id: "test_123".to_string(),
            version: "3.0".to_string(),
            image_width: 800,
            image_height: 600,
            is_printed: true,
            is_handwritten: false,
            auto_rotate_confidence: 0.95,
            auto_rotate_degrees: 0,
            confidence: 0.98,
            confidence_rate: 0.97,
            formats: FormatsData {
                text: Some("E = mc^2".to_string()),
                latex_normal: Some(r"E = mc^2".to_string()),
                ..Default::default()
            },
            line_data: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_api_response_from_result() {
        let result = create_test_result();
        let response = ApiResponse::from_ocr_result(result);

        assert_eq!(response.request_id, "test_123");
        assert_eq!(response.version, "3.0");
        assert_eq!(response.confidence, 0.98);
        assert!(response.formats.text.is_some());
    }

    #[test]
    fn test_api_response_to_json() {
        let result = create_test_result();
        let response = ApiResponse::from_ocr_result(result);
        let json = response.to_json().unwrap();

        assert!(json.contains("request_id"));
        assert!(json.contains("test_123"));
        assert!(json.contains("confidence"));
    }

    #[test]
    fn test_api_response_round_trip() {
        let result = create_test_result();
        let response = ApiResponse::from_ocr_result(result);
        let json = response.to_json().unwrap();
        let parsed = ApiResponse::from_json(&json).unwrap();

        assert_eq!(response.request_id, parsed.request_id);
        assert_eq!(response.confidence, parsed.confidence);
    }

    #[test]
    fn test_error_response() {
        let response = ApiResponse::error(
            "test_456".to_string(),
            "invalid_image",
            "Image format not supported",
        );

        assert_eq!(response.request_id, "test_456");
        assert!(response.error.is_some());
        assert!(response.error_info.is_some());

        let error_info = response.error_info.unwrap();
        assert_eq!(error_info.code, "invalid_image");
    }

    #[test]
    fn test_batch_response() {
        let result1 = create_test_result();
        let result2 = create_test_result();

        let responses = vec![
            ApiResponse::from_ocr_result(result1),
            ApiResponse::from_ocr_result(result2),
        ];

        let batch = BatchApiResponse::new("batch_789".to_string(), responses);

        assert_eq!(batch.batch_id, "batch_789");
        assert_eq!(batch.total, 2);
        assert_eq!(batch.completed, 2);
        assert!(batch.errors.is_none());
    }

    #[test]
    fn test_batch_with_errors() {
        let success = create_test_result();
        let error_response =
            ApiResponse::error("fail_1".to_string(), "timeout", "Processing timeout");

        let responses = vec![ApiResponse::from_ocr_result(success), error_response];

        let batch = BatchApiResponse::new("batch_error".to_string(), responses);

        assert_eq!(batch.total, 2);
        assert_eq!(batch.completed, 1);
        assert!(batch.errors.is_some());
        assert_eq!(batch.errors.unwrap().len(), 1);
    }

    #[test]
    fn test_api_request() {
        let request = ApiRequest {
            src: "https://example.com/image.png".to_string(),
            formats: Some(vec!["text".to_string(), "latex_styled".to_string()]),
            ocr: Some(OcrOptions {
                math_inline_delimiters: Some(vec!["$".to_string(), "$".to_string()]),
                math_display_delimiters: Some(vec!["$$".to_string(), "$$".to_string()]),
                rm_spaces: Some(true),
                rm_fonts: None,
                numbers_default_to_math: Some(false),
            }),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("src"));
        assert!(json.contains("formats"));
    }
}
