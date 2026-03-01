//! Output formatting module for Scipix OCR results
//!
//! Supports multiple output formats:
//! - Text: Plain text extraction
//! - LaTeX: Mathematical notation
//! - Scipix Markdown (mmd): Enhanced markdown with math
//! - MathML: XML-based mathematical markup
//! - HTML: Web-ready output with math rendering
//! - SMILES: Chemical structure notation
//! - DOCX: Microsoft Word format (Office Math ML)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod docx;
pub mod formatter;
pub mod html;
pub mod json;
pub mod latex;
pub mod mmd;
pub mod smiles;

pub use formatter::{HtmlEngine, MathDelimiters, OutputFormatter};
pub use json::ApiResponse;

/// Output format types supported by Scipix OCR
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Plain text output
    Text,
    /// LaTeX mathematical notation
    #[serde(rename = "latex_normal")]
    LaTeX,
    /// Styled LaTeX with custom packages
    #[serde(rename = "latex_styled")]
    LaTeXStyled,
    /// Mathematical Markup Language
    #[serde(rename = "mathml")]
    MathML,
    /// Scipix Markdown (enhanced markdown)
    #[serde(rename = "mmd")]
    Mmd,
    /// ASCII Math notation
    #[serde(rename = "asciimath")]
    AsciiMath,
    /// HTML with embedded math
    Html,
    /// Chemical structure notation
    #[serde(rename = "smiles")]
    Smiles,
    /// Microsoft Word format
    Docx,
}

impl OutputFormat {
    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Text => "txt",
            OutputFormat::LaTeX | OutputFormat::LaTeXStyled => "tex",
            OutputFormat::MathML => "xml",
            OutputFormat::Mmd => "mmd",
            OutputFormat::AsciiMath => "txt",
            OutputFormat::Html => "html",
            OutputFormat::Smiles => "smi",
            OutputFormat::Docx => "docx",
        }
    }

    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            OutputFormat::Text | OutputFormat::AsciiMath => "text/plain",
            OutputFormat::LaTeX | OutputFormat::LaTeXStyled => "application/x-latex",
            OutputFormat::MathML => "application/mathml+xml",
            OutputFormat::Mmd => "text/markdown",
            OutputFormat::Html => "text/html",
            OutputFormat::Smiles => "chemical/x-daylight-smiles",
            OutputFormat::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
        }
    }
}

/// Complete OCR result with all possible output formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Request identifier
    pub request_id: String,

    /// Version of the OCR engine
    pub version: String,

    /// Image dimensions
    pub image_width: u32,
    pub image_height: u32,

    /// Processing status
    pub is_printed: bool,
    pub is_handwritten: bool,
    pub auto_rotate_confidence: f32,
    pub auto_rotate_degrees: i32,

    /// Confidence scores
    pub confidence: f32,
    pub confidence_rate: f32,

    /// Available output formats
    pub formats: FormatsData,

    /// Detailed line and word data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_data: Option<Vec<LineData>>,

    /// Error information if processing failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Processing metadata
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// All available output format data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatsData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex_normal: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex_styled: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex_simplified: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mathml: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub asciimath: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mmd: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub smiles: Option<String>,
}

/// Line-level OCR data with positioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineData {
    /// Line type: text, math, table, image, etc.
    #[serde(rename = "type")]
    pub line_type: String,

    /// Content in various formats
    pub text: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<String>,

    /// Bounding box coordinates
    pub bbox: BoundingBox,

    /// Confidence score
    pub confidence: f32,

    /// Word-level data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<WordData>>,
}

/// Word-level OCR data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordData {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<String>,
}

/// Bounding box coordinates (x, y, width, height)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Convert between output formats
pub fn convert_format(
    content: &str,
    from: OutputFormat,
    to: OutputFormat,
) -> Result<String, String> {
    // Simple pass-through for same format
    if from == to {
        return Ok(content.to_string());
    }

    // Format-specific conversions
    match (from, to) {
        (OutputFormat::LaTeX, OutputFormat::Text) => {
            // Strip LaTeX commands for plain text
            Ok(strip_latex(content))
        }
        (OutputFormat::Mmd, OutputFormat::LaTeX) => {
            // Extract LaTeX from markdown
            Ok(extract_latex_from_mmd(content))
        }
        (OutputFormat::LaTeX, OutputFormat::Html) => {
            // Wrap LaTeX in HTML with MathJax
            Ok(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>
    <script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
</head>
<body>
    <p>\({}\)</p>
</body>
</html>"#,
                content
            ))
        }
        _ => Err(format!(
            "Conversion from {:?} to {:?} not supported",
            from, to
        )),
    }
}

fn strip_latex(content: &str) -> String {
    // Remove common LaTeX commands
    let mut result = content.to_string();

    // Remove math delimiters
    result = result.replace("\\(", "").replace("\\)", "");
    result = result.replace("\\[", "").replace("\\]", "");
    result = result.replace("$$", "");

    // Remove common commands but keep their content
    for cmd in &["\\text", "\\mathrm", "\\mathbf", "\\mathit"] {
        result = result.replace(&format!("{}{}", cmd, "{"), "");
    }
    result = result.replace("}", "");

    // Remove standalone commands
    for cmd in &["\\\\", "\\,", "\\;", "\\:", "\\!", "\\quad", "\\qquad"] {
        result = result.replace(cmd, " ");
    }

    result.trim().to_string()
}

fn extract_latex_from_mmd(content: &str) -> String {
    let mut latex_parts = Vec::new();
    let mut in_math = false;
    let mut current = String::new();

    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '$' {
            if in_math {
                latex_parts.push(current.clone());
                current.clear();
                in_math = false;
            } else {
                in_math = true;
            }
            i += 2;
        } else if chars[i] == '$' {
            in_math = !in_math;
            i += 1;
        } else if in_math {
            current.push(chars[i]);
            i += 1;
        } else {
            i += 1;
        }
    }

    latex_parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Text.extension(), "txt");
        assert_eq!(OutputFormat::LaTeX.extension(), "tex");
        assert_eq!(OutputFormat::Html.extension(), "html");
        assert_eq!(OutputFormat::Mmd.extension(), "mmd");
    }

    #[test]
    fn test_output_format_mime_type() {
        assert_eq!(OutputFormat::Text.mime_type(), "text/plain");
        assert_eq!(OutputFormat::LaTeX.mime_type(), "application/x-latex");
        assert_eq!(OutputFormat::Html.mime_type(), "text/html");
    }

    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(bbox.area(), 5000.0);
        assert_eq!(bbox.center(), (60.0, 45.0));
    }

    #[test]
    fn test_strip_latex() {
        let input = r"\text{Hello } \mathbf{World}";
        let output = strip_latex(input);
        assert!(output.contains("Hello"));
        assert!(output.contains("World"));
    }

    #[test]
    fn test_convert_same_format() {
        let content = "test content";
        let result = convert_format(content, OutputFormat::Text, OutputFormat::Text).unwrap();
        assert_eq!(result, content);
    }
}
