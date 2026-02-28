//! Multi-format output formatter with batch processing and streaming support

use super::*;
use crate::output::{html, latex, mmd, smiles};
use std::io::Write;

/// Configuration for output formatting
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Target output formats
    pub formats: Vec<OutputFormat>,

    /// Enable pretty printing (where applicable)
    pub pretty: bool,

    /// Include confidence scores in output
    pub include_confidence: bool,

    /// Include bounding box data
    pub include_bbox: bool,

    /// Math delimiter style for LaTeX/MMD
    pub math_delimiters: MathDelimiters,

    /// HTML rendering engine
    pub html_engine: HtmlEngine,

    /// Enable streaming for large documents
    pub streaming: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            formats: vec![OutputFormat::Text],
            pretty: true,
            include_confidence: false,
            include_bbox: false,
            math_delimiters: MathDelimiters::default(),
            html_engine: HtmlEngine::MathJax,
            streaming: false,
        }
    }
}

/// Math delimiter configuration
#[derive(Debug, Clone)]
pub struct MathDelimiters {
    pub inline_start: String,
    pub inline_end: String,
    pub display_start: String,
    pub display_end: String,
}

impl Default for MathDelimiters {
    fn default() -> Self {
        Self {
            inline_start: "$".to_string(),
            inline_end: "$".to_string(),
            display_start: "$$".to_string(),
            display_end: "$$".to_string(),
        }
    }
}

/// HTML rendering engine options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlEngine {
    MathJax,
    KaTeX,
    Raw,
}

/// Main output formatter
pub struct OutputFormatter {
    config: FormatterConfig,
}

impl OutputFormatter {
    /// Create a new formatter with default configuration
    pub fn new() -> Self {
        Self {
            config: FormatterConfig::default(),
        }
    }

    /// Create a formatter with custom configuration
    pub fn with_config(config: FormatterConfig) -> Self {
        Self { config }
    }

    /// Format a single OCR result
    pub fn format_result(&self, result: &OcrResult) -> Result<FormatsData, String> {
        let mut formats = FormatsData::default();

        for format in &self.config.formats {
            let output = self.format_single(result, *format)?;
            self.set_format_output(&mut formats, *format, output);
        }

        Ok(formats)
    }

    /// Format multiple results in batch
    pub fn format_batch(&self, results: &[OcrResult]) -> Result<Vec<FormatsData>, String> {
        results
            .iter()
            .map(|result| self.format_result(result))
            .collect()
    }

    /// Stream format results to a writer
    pub fn format_stream<W: Write>(
        &self,
        results: &[OcrResult],
        writer: &mut W,
        format: OutputFormat,
    ) -> Result<(), String> {
        for (i, result) in results.iter().enumerate() {
            let output = self.format_single(result, format)?;
            writer
                .write_all(output.as_bytes())
                .map_err(|e| format!("Write error: {}", e))?;

            // Add separator between results
            if i < results.len() - 1 {
                writer
                    .write_all(b"\n\n---\n\n")
                    .map_err(|e| format!("Write error: {}", e))?;
            }
        }

        Ok(())
    }

    /// Format a single result to a specific format
    fn format_single(&self, result: &OcrResult, format: OutputFormat) -> Result<String, String> {
        match format {
            OutputFormat::Text => self.format_text(result),
            OutputFormat::LaTeX => self.format_latex(result, false),
            OutputFormat::LaTeXStyled => self.format_latex(result, true),
            OutputFormat::Mmd => self.format_mmd(result),
            OutputFormat::Html => self.format_html(result),
            OutputFormat::Smiles => self.format_smiles(result),
            OutputFormat::Docx => self.format_docx(result),
            OutputFormat::MathML => self.format_mathml(result),
            OutputFormat::AsciiMath => self.format_asciimath(result),
        }
    }

    fn format_text(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(text) = &result.formats.text {
            return Ok(text.clone());
        }

        // Fallback: extract text from line data
        if let Some(line_data) = &result.line_data {
            let text = line_data
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(text);
        }

        Err("No text content available".to_string())
    }

    fn format_latex(&self, result: &OcrResult, styled: bool) -> Result<String, String> {
        let latex_content = if styled {
            result
                .formats
                .latex_styled
                .as_ref()
                .or(result.formats.latex_normal.as_ref())
        } else {
            result.formats.latex_normal.as_ref()
        };

        if let Some(latex) = latex_content {
            if styled {
                // Wrap in document with packages
                Ok(latex::LaTeXFormatter::new()
                    .with_packages(vec![
                        "amsmath".to_string(),
                        "amssymb".to_string(),
                        "graphicx".to_string(),
                    ])
                    .format_document(latex))
            } else {
                Ok(latex.clone())
            }
        } else {
            Err("No LaTeX content available".to_string())
        }
    }

    fn format_mmd(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(mmd) = &result.formats.mmd {
            return Ok(mmd.clone());
        }

        // Generate MMD from line data
        if let Some(line_data) = &result.line_data {
            let formatter = mmd::MmdFormatter::with_delimiters(self.config.math_delimiters.clone());
            return Ok(formatter.format(line_data));
        }

        Err("No MMD content available".to_string())
    }

    fn format_html(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(html) = &result.formats.html {
            return Ok(html.clone());
        }

        // Generate HTML with math rendering
        let content = self.format_text(result)?;
        let formatter = html::HtmlFormatter::new()
            .with_engine(self.config.html_engine)
            .with_styling(self.config.pretty);

        Ok(formatter.format(&content, result.line_data.as_deref()))
    }

    fn format_smiles(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(smiles) = &result.formats.smiles {
            return Ok(smiles.clone());
        }

        // Generate SMILES if we have chemical structure data
        let generator = smiles::SmilesGenerator::new();
        generator.generate_from_result(result)
    }

    fn format_docx(&self, _result: &OcrResult) -> Result<String, String> {
        // DOCX requires binary format, return placeholder
        Err("DOCX format requires binary output - use save_docx() instead".to_string())
    }

    fn format_mathml(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(mathml) = &result.formats.mathml {
            return Ok(mathml.clone());
        }

        Err("MathML generation not yet implemented".to_string())
    }

    fn format_asciimath(&self, result: &OcrResult) -> Result<String, String> {
        if let Some(asciimath) = &result.formats.asciimath {
            return Ok(asciimath.clone());
        }

        Err("AsciiMath conversion not yet implemented".to_string())
    }

    fn set_format_output(&self, formats: &mut FormatsData, format: OutputFormat, output: String) {
        match format {
            OutputFormat::Text => formats.text = Some(output),
            OutputFormat::LaTeX => formats.latex_normal = Some(output),
            OutputFormat::LaTeXStyled => formats.latex_styled = Some(output),
            OutputFormat::Mmd => formats.mmd = Some(output),
            OutputFormat::Html => formats.html = Some(output),
            OutputFormat::Smiles => formats.smiles = Some(output),
            OutputFormat::MathML => formats.mathml = Some(output),
            OutputFormat::AsciiMath => formats.asciimath = Some(output),
            OutputFormat::Docx => {} // Binary format, handled separately
        }
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for OutputFormatter configuration
pub struct FormatterBuilder {
    config: FormatterConfig,
}

impl FormatterBuilder {
    pub fn new() -> Self {
        Self {
            config: FormatterConfig::default(),
        }
    }

    pub fn formats(mut self, formats: Vec<OutputFormat>) -> Self {
        self.config.formats = formats;
        self
    }

    pub fn add_format(mut self, format: OutputFormat) -> Self {
        self.config.formats.push(format);
        self
    }

    pub fn pretty(mut self, pretty: bool) -> Self {
        self.config.pretty = pretty;
        self
    }

    pub fn include_confidence(mut self, include: bool) -> Self {
        self.config.include_confidence = include;
        self
    }

    pub fn include_bbox(mut self, include: bool) -> Self {
        self.config.include_bbox = include;
        self
    }

    pub fn math_delimiters(mut self, delimiters: MathDelimiters) -> Self {
        self.config.math_delimiters = delimiters;
        self
    }

    pub fn html_engine(mut self, engine: HtmlEngine) -> Self {
        self.config.html_engine = engine;
        self
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.config.streaming = streaming;
        self
    }

    pub fn build(self) -> OutputFormatter {
        OutputFormatter::with_config(self.config)
    }
}

impl Default for FormatterBuilder {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_format_text() {
        let formatter = OutputFormatter::new();
        let result = create_test_result();

        let output = formatter
            .format_single(&result, OutputFormat::Text)
            .unwrap();
        assert_eq!(output, "E = mc^2");
    }

    #[test]
    fn test_format_latex() {
        let formatter = OutputFormatter::new();
        let result = create_test_result();

        let output = formatter
            .format_single(&result, OutputFormat::LaTeX)
            .unwrap();
        assert!(output.contains("mc^2"));
    }

    #[test]
    fn test_builder() {
        let formatter = FormatterBuilder::new()
            .add_format(OutputFormat::Text)
            .add_format(OutputFormat::LaTeX)
            .pretty(true)
            .include_confidence(true)
            .build();

        assert_eq!(formatter.config.formats.len(), 2);
        assert!(formatter.config.pretty);
        assert!(formatter.config.include_confidence);
    }

    #[test]
    fn test_batch_format() {
        let formatter = OutputFormatter::new();
        let results = vec![create_test_result(), create_test_result()];

        let outputs = formatter.format_batch(&results).unwrap();
        assert_eq!(outputs.len(), 2);
    }
}
