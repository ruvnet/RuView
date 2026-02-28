//! DOCX (Microsoft Word) formatter with Office Math ML support
//!
//! This is a stub implementation. Full DOCX generation requires:
//! - ZIP file creation for .docx format
//! - XML generation for document.xml, styles.xml, etc.
//! - Office Math ML for equations
//! - Image embedding support
//!
//! Consider using libraries like `docx-rs` for production implementation.

use super::{LineData, OcrResult};
use std::io::Write;

/// DOCX formatter (stub implementation)
#[allow(dead_code)]
pub struct DocxFormatter {
    include_styles: bool,
    page_size: PageSize,
    margins: Margins,
}

#[derive(Debug, Clone, Copy)]
pub struct PageSize {
    pub width: u32, // in twips (1/1440 inch)
    pub height: u32,
}

impl PageSize {
    pub fn letter() -> Self {
        Self {
            width: 12240,  // 8.5 inches
            height: 15840, // 11 inches
        }
    }

    pub fn a4() -> Self {
        Self {
            width: 11906,  // 210mm
            height: 16838, // 297mm
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl Margins {
    pub fn normal() -> Self {
        Self {
            top: 1440, // 1 inch
            right: 1440,
            bottom: 1440,
            left: 1440,
        }
    }
}

impl DocxFormatter {
    pub fn new() -> Self {
        Self {
            include_styles: true,
            page_size: PageSize::letter(),
            margins: Margins::normal(),
        }
    }

    pub fn with_page_size(mut self, page_size: PageSize) -> Self {
        self.page_size = page_size;
        self
    }

    pub fn with_margins(mut self, margins: Margins) -> Self {
        self.margins = margins;
        self
    }

    /// Generate Office Math ML from LaTeX
    /// This is a simplified placeholder - real implementation needs proper conversion
    pub fn latex_to_mathml(&self, latex: &str) -> String {
        // This is a very simplified stub
        // Real implementation would parse LaTeX and generate proper Office Math ML
        format!(
            r#"<m:oMathPara>
  <m:oMath>
    <m:r>
      <m:t>{}</m:t>
    </m:r>
  </m:oMath>
</m:oMathPara>"#,
            self.escape_xml(latex)
        )
    }

    /// Generate document.xml content
    pub fn generate_document_xml(&self, lines: &[LineData]) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:body>
"#,
        );

        for line in lines {
            xml.push_str(&self.format_line(line));
        }

        xml.push_str("  </w:body>\n</w:document>");
        xml
    }

    fn format_line(&self, line: &LineData) -> String {
        match line.line_type.as_str() {
            "text" => self.format_paragraph(&line.text),
            "math" | "equation" => {
                let latex = line.latex.as_ref().unwrap_or(&line.text);
                self.format_math(latex)
            }
            "heading" => self.format_heading(&line.text, 1),
            _ => self.format_paragraph(&line.text),
        }
    }

    fn format_paragraph(&self, text: &str) -> String {
        format!(
            r#"    <w:p>
      <w:r>
        <w:t>{}</w:t>
      </w:r>
    </w:p>
"#,
            self.escape_xml(text)
        )
    }

    fn format_heading(&self, text: &str, level: u32) -> String {
        format!(
            r#"    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading{}"/>
      </w:pPr>
      <w:r>
        <w:t>{}</w:t>
      </w:r>
    </w:p>
"#,
            level,
            self.escape_xml(text)
        )
    }

    fn format_math(&self, latex: &str) -> String {
        let mathml = self.latex_to_mathml(latex);
        format!(
            r#"    <w:p>
      <w:r>
        {}
      </w:r>
    </w:p>
"#,
            mathml
        )
    }

    fn escape_xml(&self, text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Save DOCX to file (stub - needs ZIP implementation)
    pub fn save_to_file<W: Write>(
        &self,
        _writer: &mut W,
        _result: &OcrResult,
    ) -> Result<(), String> {
        Err("DOCX binary format generation not implemented. Use docx-rs library for full implementation.".to_string())
    }

    /// Generate styles.xml content
    pub fn generate_styles_xml(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr>
      <w:keepNext/>
      <w:keepLines/>
    </w:pPr>
    <w:rPr>
      <w:b/>
      <w:sz w:val="32"/>
    </w:rPr>
  </w:style>
</w:styles>"#
            .to_string()
    }
}

impl Default for DocxFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BoundingBox;

    #[test]
    fn test_page_sizes() {
        let letter = PageSize::letter();
        assert_eq!(letter.width, 12240);

        let a4 = PageSize::a4();
        assert!(a4.width < letter.width);
    }

    #[test]
    fn test_escape_xml() {
        let formatter = DocxFormatter::new();
        let result = formatter.escape_xml("Test <tag> & \"quote\"");

        assert!(result.contains("&lt;"));
        assert!(result.contains("&gt;"));
        assert!(result.contains("&amp;"));
        assert!(result.contains("&quot;"));
    }

    #[test]
    fn test_format_paragraph() {
        let formatter = DocxFormatter::new();
        let result = formatter.format_paragraph("Hello World");

        assert!(result.contains("<w:p>"));
        assert!(result.contains("<w:t>Hello World</w:t>"));
    }

    #[test]
    fn test_format_heading() {
        let formatter = DocxFormatter::new();
        let result = formatter.format_heading("Chapter 1", 1);

        assert!(result.contains("Heading1"));
        assert!(result.contains("Chapter 1"));
    }

    #[test]
    fn test_latex_to_mathml() {
        let formatter = DocxFormatter::new();
        let result = formatter.latex_to_mathml("E = mc^2");

        assert!(result.contains("<m:oMath>"));
        assert!(result.contains("mc^2"));
    }

    #[test]
    fn test_generate_document_xml() {
        let formatter = DocxFormatter::new();
        let lines = vec![LineData {
            line_type: "text".to_string(),
            text: "Hello".to_string(),
            latex: None,
            bbox: BoundingBox::new(0.0, 0.0, 100.0, 20.0),
            confidence: 0.95,
            words: None,
        }];

        let xml = formatter.generate_document_xml(&lines);
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("<w:document"));
        assert!(xml.contains("Hello"));
    }

    #[test]
    fn test_generate_styles_xml() {
        let formatter = DocxFormatter::new();
        let xml = formatter.generate_styles_xml();

        assert!(xml.contains("<w:styles"));
        assert!(xml.contains("Normal"));
        assert!(xml.contains("Heading 1"));
    }
}
