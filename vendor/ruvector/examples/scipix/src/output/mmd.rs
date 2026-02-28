//! Scipix Markdown (MMD) formatter
//!
//! MMD is an enhanced markdown format that supports:
//! - Inline and display math with LaTeX
//! - Tables with alignment
//! - Chemistry notation (SMILES)
//! - Image embedding
//! - Structured documents

use super::{LineData, MathDelimiters};

/// Scipix Markdown formatter
pub struct MmdFormatter {
    delimiters: MathDelimiters,
    include_metadata: bool,
    preserve_structure: bool,
}

impl MmdFormatter {
    pub fn new() -> Self {
        Self {
            delimiters: MathDelimiters::default(),
            include_metadata: false,
            preserve_structure: true,
        }
    }

    pub fn with_delimiters(delimiters: MathDelimiters) -> Self {
        Self {
            delimiters,
            include_metadata: false,
            preserve_structure: true,
        }
    }

    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    pub fn preserve_structure(mut self, preserve: bool) -> Self {
        self.preserve_structure = preserve;
        self
    }

    /// Format line data to MMD
    pub fn format(&self, lines: &[LineData]) -> String {
        let mut output = String::new();
        let mut in_table = false;
        let mut in_list = false;

        for line in lines {
            match line.line_type.as_str() {
                "text" => {
                    if in_table {
                        output.push_str("\n");
                        in_table = false;
                    }
                    if in_list && !line.text.trim_start().starts_with(&['-', '*', '1']) {
                        output.push_str("\n");
                        in_list = false;
                    }
                    output.push_str(&line.text);
                    output.push_str("\n");
                }
                "math" | "equation" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    let formatted = self.format_math(latex, true); // display mode
                    output.push_str(&formatted);
                    output.push_str("\n\n");
                }
                "inline_math" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    let formatted = self.format_math(latex, false); // inline mode
                    output.push_str(&formatted);
                }
                "table_row" => {
                    if !in_table {
                        in_table = true;
                    }
                    output.push_str(&self.format_table_row(&line.text));
                    output.push_str("\n");
                }
                "list_item" => {
                    if !in_list {
                        in_list = true;
                    }
                    output.push_str(&line.text);
                    output.push_str("\n");
                }
                "heading" => {
                    output.push_str(&format!("# {}\n\n", line.text));
                }
                "image" => {
                    output.push_str(&self.format_image(&line.text));
                    output.push_str("\n\n");
                }
                "chemistry" => {
                    let smiles = line.text.trim();
                    output.push_str(&format!("```smiles\n{}\n```\n\n", smiles));
                }
                _ => {
                    // Unknown type, output as text
                    output.push_str(&line.text);
                    output.push_str("\n");
                }
            }
        }

        output.trim().to_string()
    }

    /// Format LaTeX math expression
    pub fn format_math(&self, latex: &str, display: bool) -> String {
        if display {
            format!(
                "{}\n{}\n{}",
                self.delimiters.display_start,
                latex.trim(),
                self.delimiters.display_end
            )
        } else {
            format!(
                "{}{}{}",
                self.delimiters.inline_start,
                latex.trim(),
                self.delimiters.inline_end
            )
        }
    }

    /// Format table row
    fn format_table_row(&self, row: &str) -> String {
        // Basic table formatting - split by | and rejoin
        let cells: Vec<&str> = row.split('|').map(|s| s.trim()).collect();
        format!("| {} |", cells.join(" | "))
    }

    /// Format image reference
    fn format_image(&self, path: &str) -> String {
        // Extract alt text and path if available
        if path.contains('[') && path.contains(']') {
            path.to_string()
        } else {
            format!("![Image]({})", path)
        }
    }

    /// Convert plain text with embedded LaTeX to MMD
    pub fn from_mixed_text(&self, text: &str) -> String {
        let mut output = String::new();
        let mut current = String::new();
        let mut in_math = false;
        let mut display_math = false;

        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Check for display math $$
            if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '$' {
                if in_math && display_math {
                    // End display math
                    output.push_str(&self.format_math(&current, true));
                    current.clear();
                    in_math = false;
                    display_math = false;
                } else if !in_math {
                    // Start display math
                    if !current.is_empty() {
                        output.push_str(&current);
                        current.clear();
                    }
                    in_math = true;
                    display_math = true;
                }
                i += 2;
                continue;
            }

            // Check for inline math $
            if chars[i] == '$' && !display_math {
                if in_math {
                    // End inline math
                    output.push_str(&self.format_math(&current, false));
                    current.clear();
                    in_math = false;
                } else {
                    // Start inline math
                    if !current.is_empty() {
                        output.push_str(&current);
                        current.clear();
                    }
                    in_math = true;
                }
                i += 1;
                continue;
            }

            current.push(chars[i]);
            i += 1;
        }

        if !current.is_empty() {
            output.push_str(&current);
        }

        output
    }

    /// Format a complete document with frontmatter
    pub fn format_document(&self, title: &str, content: &str, metadata: Option<&str>) -> String {
        let mut doc = String::new();

        // Add frontmatter if metadata provided
        if let Some(meta) = metadata {
            doc.push_str("---\n");
            doc.push_str(meta);
            doc.push_str("\n---\n\n");
        }

        // Add title
        doc.push_str(&format!("# {}\n\n", title));

        // Add content
        doc.push_str(content);

        doc
    }
}

impl Default for MmdFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse MMD back to structured data
pub struct MmdParser;

impl MmdParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse MMD content and extract LaTeX expressions
    pub fn extract_latex(&self, content: &str) -> Vec<(String, bool)> {
        let mut expressions = Vec::new();
        let mut current = String::new();
        let mut in_math = false;
        let mut display_math = false;

        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '$' {
                if in_math && display_math {
                    expressions.push((current.trim().to_string(), true));
                    current.clear();
                    in_math = false;
                    display_math = false;
                } else if !in_math {
                    in_math = true;
                    display_math = true;
                }
                i += 2;
            } else if chars[i] == '$' && !display_math {
                if in_math {
                    expressions.push((current.trim().to_string(), false));
                    current.clear();
                    in_math = false;
                } else {
                    in_math = true;
                }
                i += 1;
            } else if in_math {
                current.push(chars[i]);
                i += 1;
            } else {
                i += 1;
            }
        }

        expressions
    }
}

impl Default for MmdParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BoundingBox;

    #[test]
    fn test_format_inline_math() {
        let formatter = MmdFormatter::new();
        let result = formatter.format_math("E = mc^2", false);
        assert_eq!(result, "$E = mc^2$");
    }

    #[test]
    fn test_format_display_math() {
        let formatter = MmdFormatter::new();
        let result = formatter.format_math(r"\int_0^1 x^2 dx", true);
        assert!(result.contains("$$"));
        assert!(result.contains(r"\int_0^1 x^2 dx"));
    }

    #[test]
    fn test_format_lines() {
        let formatter = MmdFormatter::new();
        let lines = vec![
            LineData {
                line_type: "text".to_string(),
                text: "The equation".to_string(),
                latex: None,
                bbox: BoundingBox::new(0.0, 0.0, 100.0, 20.0),
                confidence: 0.95,
                words: None,
            },
            LineData {
                line_type: "math".to_string(),
                text: "E = mc^2".to_string(),
                latex: Some(r"E = mc^2".to_string()),
                bbox: BoundingBox::new(0.0, 25.0, 100.0, 30.0),
                confidence: 0.98,
                words: None,
            },
        ];

        let result = formatter.format(&lines);
        assert!(result.contains("The equation"));
        assert!(result.contains("$$"));
        assert!(result.contains("mc^2"));
    }

    #[test]
    fn test_from_mixed_text() {
        let formatter = MmdFormatter::new();
        let text = "The formula $E = mc^2$ is famous.";
        let result = formatter.from_mixed_text(text);
        assert!(result.contains("$E = mc^2$"));
        assert!(result.contains("famous"));
    }

    #[test]
    fn test_extract_latex() {
        let parser = MmdParser::new();
        let content = "Text with $inline$ and $$display$$ math.";
        let expressions = parser.extract_latex(content);

        assert_eq!(expressions.len(), 2);
        assert_eq!(expressions[0].0, "inline");
        assert!(!expressions[0].1); // inline
        assert_eq!(expressions[1].0, "display");
        assert!(expressions[1].1); // display
    }

    #[test]
    fn test_format_document() {
        let formatter = MmdFormatter::new();
        let doc = formatter.format_document(
            "My Document",
            "Content here",
            Some("author: Test\ndate: 2025-01-01"),
        );

        assert!(doc.contains("---"));
        assert!(doc.contains("author: Test"));
        assert!(doc.contains("# My Document"));
        assert!(doc.contains("Content here"));
    }
}
