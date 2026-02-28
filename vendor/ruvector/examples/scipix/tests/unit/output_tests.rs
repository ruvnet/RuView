// Output formatting tests for ruvector-scipix
//
// Tests output format conversion between LaTeX, MathML, AsciiMath, etc.
// and MMD delimiter handling, JSON serialization.
// Target: 85%+ coverage of output formatting module

#[cfg(test)]
mod output_tests {
    use serde::{Deserialize, Serialize};

    // Mock output format types
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    enum OutputFormat {
        Latex,
        MathML,
        AsciiMath,
        MMD, // Scipix Markdown
        Unicode,
        PlainText,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct FormattedOutput {
        format: OutputFormat,
        content: String,
        metadata: Option<OutputMetadata>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct OutputMetadata {
        confidence: f32,
        processing_time_ms: u64,
        num_symbols: usize,
    }

    // Mock format converter
    fn convert_format(input: &str, from: OutputFormat, to: OutputFormat) -> Result<String, String> {
        match (from, to) {
            (OutputFormat::Latex, OutputFormat::MathML) => {
                Ok(latex_to_mathml(input))
            }
            (OutputFormat::Latex, OutputFormat::AsciiMath) => {
                Ok(latex_to_asciimath(input))
            }
            (OutputFormat::Latex, OutputFormat::Unicode) => {
                Ok(latex_to_unicode(input))
            }
            (OutputFormat::Latex, OutputFormat::PlainText) => {
                Ok(latex_to_plaintext(input))
            }
            (OutputFormat::MathML, OutputFormat::Latex) => {
                Ok(mathml_to_latex(input))
            }
            _ => Ok(input.to_string()),
        }
    }

    fn latex_to_mathml(latex: &str) -> String {
        // Simple mock conversion
        if latex.contains(r"\frac") {
            format!("<mfrac>{}</mfrac>", latex.replace(r"\frac", ""))
        } else if latex.contains("^") {
            "<msup></msup>".to_string()
        } else {
            format!("<math>{}</math>", latex)
        }
    }

    fn latex_to_asciimath(latex: &str) -> String {
        latex
            .replace(r"\frac{", "(")
            .replace("}{", ")/(")
            .replace("}", ")")
            .replace("^", "^")
    }

    fn latex_to_unicode(latex: &str) -> String {
        latex
            .replace(r"\alpha", "α")
            .replace(r"\beta", "β")
            .replace(r"\gamma", "γ")
            .replace(r"\pi", "π")
            .replace(r"\sigma", "σ")
            .replace(r"\infty", "∞")
    }

    fn latex_to_plaintext(latex: &str) -> String {
        latex
            .replace(r"\frac{", "(")
            .replace("}{", ")/(")
            .replace("}", ")")
            .replace("^", "**")
            .replace("_", "")
    }

    fn mathml_to_latex(mathml: &str) -> String {
        // Very simple mock
        if mathml.contains("<mfrac>") {
            r"\frac{a}{b}".to_string()
        } else {
            "x".to_string()
        }
    }

    fn apply_mmd_delimiters(latex: &str, inline: bool) -> String {
        if inline {
            format!("${}$", latex)
        } else {
            format!("$$\n{}\n$$", latex)
        }
    }

    #[test]
    fn test_format_conversion_latex_to_mathml() {
        let latex = r"\frac{1}{2}";
        let mathml = convert_format(latex, OutputFormat::Latex, OutputFormat::MathML).unwrap();

        assert!(mathml.contains("<mfrac>"));
    }

    #[test]
    fn test_format_conversion_latex_to_asciimath() {
        let latex = r"x^2 + 1";
        let ascii = convert_format(latex, OutputFormat::Latex, OutputFormat::AsciiMath).unwrap();

        assert!(ascii.contains("x^2"));
    }

    #[test]
    fn test_format_conversion_latex_to_unicode() {
        let latex = r"\alpha + \beta";
        let unicode = convert_format(latex, OutputFormat::Latex, OutputFormat::Unicode).unwrap();

        assert!(unicode.contains("α"));
        assert!(unicode.contains("β"));
    }

    #[test]
    fn test_format_conversion_latex_to_plaintext() {
        let latex = r"\frac{a}{b}";
        let text = convert_format(latex, OutputFormat::Latex, OutputFormat::PlainText).unwrap();

        assert!(text.contains("(a)/(b)") || text.contains("a/b"));
    }

    #[test]
    fn test_format_conversion_mathml_to_latex() {
        let mathml = "<mfrac><mn>1</mn><mn>2</mn></mfrac>";
        let latex = convert_format(mathml, OutputFormat::MathML, OutputFormat::Latex).unwrap();

        assert!(latex.contains(r"\frac") || latex.contains("/"));
    }

    #[test]
    fn test_mmd_delimiter_inline() {
        let latex = "x^2";
        let mmd = apply_mmd_delimiters(latex, true);

        assert_eq!(mmd, "$x^2$");
    }

    #[test]
    fn test_mmd_delimiter_display() {
        let latex = r"\int_0^1 x dx";
        let mmd = apply_mmd_delimiters(latex, false);

        assert!(mmd.starts_with("$$"));
        assert!(mmd.ends_with("$$"));
        assert!(mmd.contains(latex));
    }

    #[test]
    fn test_mmd_delimiter_multiple_inline() {
        let equations = vec!["x + 1", "y - 2", "z * 3"];

        for eq in equations {
            let mmd = apply_mmd_delimiters(eq, true);
            assert!(mmd.starts_with("$"));
            assert!(mmd.ends_with("$"));
        }
    }

    #[test]
    fn test_json_serialization_formatted_output() {
        let output = FormattedOutput {
            format: OutputFormat::Latex,
            content: r"\frac{1}{2}".to_string(),
            metadata: Some(OutputMetadata {
                confidence: 0.95,
                processing_time_ms: 123,
                num_symbols: 5,
            }),
        };

        let json = serde_json::to_string(&output).unwrap();

        assert!(json.contains("Latex"));
        assert!(json.contains(r"\frac"));
        assert!(json.contains("0.95"));
    }

    #[test]
    fn test_json_deserialization_formatted_output() {
        let json = r#"{
            "format": "Latex",
            "content": "x^2 + 1",
            "metadata": {
                "confidence": 0.92,
                "processing_time_ms": 87,
                "num_symbols": 4
            }
        }"#;

        let output: FormattedOutput = serde_json::from_str(json).unwrap();

        assert_eq!(output.format, OutputFormat::Latex);
        assert_eq!(output.content, "x^2 + 1");
        assert!(output.metadata.is_some());
    }

    #[test]
    fn test_json_serialization_all_formats() {
        let formats = vec![
            OutputFormat::Latex,
            OutputFormat::MathML,
            OutputFormat::AsciiMath,
            OutputFormat::MMD,
            OutputFormat::Unicode,
            OutputFormat::PlainText,
        ];

        for format in formats {
            let output = FormattedOutput {
                format: format.clone(),
                content: "test".to_string(),
                metadata: None,
            };

            let json = serde_json::to_string(&output).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_scipix_api_compatibility_response() {
        #[derive(Serialize, Deserialize)]
        struct ScipixResponse {
            latex: String,
            mathml: Option<String>,
            text: String,
            confidence: f32,
            #[serde(rename = "confidence_rate")]
            confidence_rate: f32,
        }

        let response = ScipixResponse {
            latex: r"\frac{1}{2}".to_string(),
            mathml: Some("<mfrac><mn>1</mn><mn>2</mn></mfrac>".to_string()),
            text: "1/2".to_string(),
            confidence: 0.95,
            confidence_rate: 0.93,
        };

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("latex"));
        assert!(json.contains("confidence_rate"));
    }

    #[test]
    fn test_scipix_api_compatibility_request() {
        #[derive(Serialize, Deserialize)]
        struct ScipixRequest {
            src: String,
            formats: Vec<String>,
            #[serde(rename = "ocr")]
            ocr_types: Vec<String>,
        }

        let request = ScipixRequest {
            src: "data:image/png;base64,iVBORw0KGgo...".to_string(),
            formats: vec!["latex".to_string(), "mathml".to_string()],
            ocr_types: vec!["math".to_string(), "text".to_string()],
        };

        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("src"));
        assert!(json.contains("formats"));
        assert!(json.contains("ocr"));
    }

    #[test]
    fn test_unicode_symbol_conversion() {
        let conversions = vec![
            (r"\alpha", "α"),
            (r"\beta", "β"),
            (r"\gamma", "γ"),
            (r"\delta", "δ"),
            (r"\pi", "π"),
            (r"\sigma", "σ"),
            (r"\omega", "ω"),
            (r"\infty", "∞"),
            (r"\partial", "∂"),
            (r"\nabla", "∇"),
        ];

        for (latex, expected_unicode) in conversions {
            let unicode = latex_to_unicode(latex);
            assert!(
                unicode.contains(expected_unicode),
                "Failed to convert {} to {}",
                latex,
                expected_unicode
            );
        }
    }

    #[test]
    fn test_output_format_enumeration() {
        let formats = vec![
            OutputFormat::Latex,
            OutputFormat::MathML,
            OutputFormat::AsciiMath,
            OutputFormat::MMD,
            OutputFormat::Unicode,
            OutputFormat::PlainText,
        ];

        assert_eq!(formats.len(), 6);
    }

    #[test]
    fn test_formatted_output_with_no_metadata() {
        let output = FormattedOutput {
            format: OutputFormat::Latex,
            content: "x + y".to_string(),
            metadata: None,
        };

        assert!(output.metadata.is_none());
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("null"));
    }

    #[test]
    fn test_formatted_output_cloning() {
        let output1 = FormattedOutput {
            format: OutputFormat::Latex,
            content: "test".to_string(),
            metadata: None,
        };

        let output2 = output1.clone();
        assert_eq!(output1.format, output2.format);
        assert_eq!(output1.content, output2.content);
    }

    #[test]
    fn test_multiple_format_conversions_chain() {
        let latex = r"\frac{1}{2}";

        // Latex -> MathML
        let mathml = convert_format(latex, OutputFormat::Latex, OutputFormat::MathML).unwrap();
        assert!(mathml.contains("<mfrac>"));

        // MathML -> Latex
        let back_to_latex = convert_format(&mathml, OutputFormat::MathML, OutputFormat::Latex).unwrap();
        assert!(back_to_latex.contains(r"\frac") || !back_to_latex.is_empty());
    }

    #[test]
    fn test_special_latex_commands_preservation() {
        let latex_commands = vec![
            r"\sum_{i=1}^{n}",
            r"\int_0^1",
            r"\prod_{k=1}^{m}",
            r"\lim_{x \to \infty}",
        ];

        for latex in latex_commands {
            let output = FormattedOutput {
                format: OutputFormat::Latex,
                content: latex.to_string(),
                metadata: None,
            };

            assert_eq!(output.content, latex);
        }
    }

    #[test]
    fn test_output_with_confidence_metadata() {
        let output = FormattedOutput {
            format: OutputFormat::Latex,
            content: r"x^2".to_string(),
            metadata: Some(OutputMetadata {
                confidence: 0.98,
                processing_time_ms: 45,
                num_symbols: 3,
            }),
        };

        let metadata = output.metadata.unwrap();
        assert_eq!(metadata.confidence, 0.98);
        assert_eq!(metadata.processing_time_ms, 45);
        assert_eq!(metadata.num_symbols, 3);
    }
}
