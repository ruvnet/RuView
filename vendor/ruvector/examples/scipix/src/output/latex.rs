//! LaTeX output formatter with styling and package management

use super::LineData;

/// LaTeX document formatter
#[derive(Clone)]
pub struct LaTeXFormatter {
    packages: Vec<String>,
    document_class: String,
    preamble: String,
    numbered_equations: bool,
    custom_delimiters: Option<(String, String)>,
}

impl LaTeXFormatter {
    pub fn new() -> Self {
        Self {
            packages: vec!["amsmath".to_string(), "amssymb".to_string()],
            document_class: "article".to_string(),
            preamble: String::new(),
            numbered_equations: false,
            custom_delimiters: None,
        }
    }

    pub fn with_packages(mut self, packages: Vec<String>) -> Self {
        self.packages = packages;
        self
    }

    pub fn add_package(mut self, package: String) -> Self {
        if !self.packages.contains(&package) {
            self.packages.push(package);
        }
        self
    }

    pub fn document_class(mut self, class: String) -> Self {
        self.document_class = class;
        self
    }

    pub fn preamble(mut self, preamble: String) -> Self {
        self.preamble = preamble;
        self
    }

    pub fn numbered_equations(mut self, numbered: bool) -> Self {
        self.numbered_equations = numbered;
        self
    }

    pub fn custom_delimiters(mut self, start: String, end: String) -> Self {
        self.custom_delimiters = Some((start, end));
        self
    }

    /// Format plain LaTeX content
    pub fn format(&self, latex: &str) -> String {
        // Clean up LaTeX if needed
        let cleaned = self.clean_latex(latex);

        // Apply custom delimiters if specified
        if let Some((start, end)) = &self.custom_delimiters {
            format!("{}{}{}", start, cleaned, end)
        } else {
            cleaned
        }
    }

    /// Format line data to LaTeX
    pub fn format_lines(&self, lines: &[LineData]) -> String {
        let mut output = String::new();
        let mut in_align = false;

        for line in lines {
            match line.line_type.as_str() {
                "text" => {
                    if in_align {
                        output.push_str("\\end{align*}\n\n");
                        in_align = false;
                    }
                    output.push_str(&self.escape_text(&line.text));
                    output.push_str("\n\n");
                }
                "math" | "equation" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);

                    if self.numbered_equations {
                        output.push_str("\\begin{equation}\n");
                        output.push_str(latex.trim());
                        output.push_str("\n\\end{equation}\n\n");
                    } else {
                        output.push_str("\\[\n");
                        output.push_str(latex.trim());
                        output.push_str("\n\\]\n\n");
                    }
                }
                "inline_math" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    output.push_str(&format!("${}$", latex.trim()));
                }
                "align" => {
                    if !in_align {
                        output.push_str("\\begin{align*}\n");
                        in_align = true;
                    }
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    output.push_str(latex.trim());
                    output.push_str(" \\\\\n");
                }
                "table" => {
                    output.push_str(&self.format_table(&line.text));
                    output.push_str("\n\n");
                }
                _ => {
                    output.push_str(&line.text);
                    output.push_str("\n");
                }
            }
        }

        if in_align {
            output.push_str("\\end{align*}\n");
        }

        output.trim().to_string()
    }

    /// Format complete LaTeX document
    pub fn format_document(&self, content: &str) -> String {
        let mut doc = String::new();

        // Document class
        doc.push_str(&format!("\\documentclass{{{}}}\n\n", self.document_class));

        // Packages
        for package in &self.packages {
            doc.push_str(&format!("\\usepackage{{{}}}\n", package));
        }
        doc.push_str("\n");

        // Custom preamble
        if !self.preamble.is_empty() {
            doc.push_str(&self.preamble);
            doc.push_str("\n\n");
        }

        // Begin document
        doc.push_str("\\begin{document}\n\n");

        // Content
        doc.push_str(content);
        doc.push_str("\n\n");

        // End document
        doc.push_str("\\end{document}\n");

        doc
    }

    /// Clean and normalize LaTeX
    fn clean_latex(&self, latex: &str) -> String {
        let mut cleaned = latex.to_string();

        // Remove excessive whitespace
        while cleaned.contains("  ") {
            cleaned = cleaned.replace("  ", " ");
        }

        // Normalize line breaks
        cleaned = cleaned.replace("\r\n", "\n");

        // Ensure proper spacing around operators
        for op in &["=", "+", "-", r"\times", r"\div"] {
            let spaced = format!(" {} ", op);
            cleaned = cleaned.replace(op, &spaced);
        }

        // Remove duplicate spaces again
        while cleaned.contains("  ") {
            cleaned = cleaned.replace("  ", " ");
        }

        cleaned.trim().to_string()
    }

    /// Escape special LaTeX characters in text
    fn escape_text(&self, text: &str) -> String {
        text.replace('\\', r"\\")
            .replace('{', r"\{")
            .replace('}', r"\}")
            .replace('$', r"\$")
            .replace('%', r"\%")
            .replace('_', r"\_")
            .replace('&', r"\&")
            .replace('#', r"\#")
            .replace('^', r"\^")
            .replace('~', r"\~")
    }

    /// Format table to LaTeX tabular environment
    fn format_table(&self, table: &str) -> String {
        let rows: Vec<&str> = table.lines().collect();
        if rows.is_empty() {
            return String::new();
        }

        // Determine number of columns from first row
        let num_cols = rows[0].split('|').filter(|s| !s.is_empty()).count();
        let col_spec = "c".repeat(num_cols);

        let mut output = format!("\\begin{{tabular}}{{{}}}\n", col_spec);
        output.push_str("\\hline\n");

        for (i, row) in rows.iter().enumerate() {
            let cells: Vec<&str> = row
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            output.push_str(&cells.join(" & "));
            output.push_str(" \\\\\n");

            if i == 0 {
                output.push_str("\\hline\n");
            }
        }

        output.push_str("\\hline\n");
        output.push_str("\\end{tabular}");

        output
    }

    /// Convert inline LaTeX to display math
    pub fn inline_to_display(&self, latex: &str) -> String {
        if self.numbered_equations {
            format!("\\begin{{equation}}\n{}\n\\end{{equation}}", latex.trim())
        } else {
            format!("\\[\n{}\n\\]", latex.trim())
        }
    }

    /// Add equation label
    pub fn add_label(&self, latex: &str, label: &str) -> String {
        format!("{}\n\\label{{{}}}", latex.trim(), label)
    }
}

impl Default for LaTeXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Styled LaTeX formatter with predefined templates
#[allow(dead_code)]
pub struct StyledLaTeXFormatter {
    base: LaTeXFormatter,
    style: LaTeXStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaTeXStyle {
    Article,
    Report,
    Book,
    Beamer,
    Minimal,
}

impl StyledLaTeXFormatter {
    pub fn new(style: LaTeXStyle) -> Self {
        let base = match style {
            LaTeXStyle::Article => LaTeXFormatter::new()
                .document_class("article".to_string())
                .with_packages(vec![
                    "amsmath".to_string(),
                    "amssymb".to_string(),
                    "graphicx".to_string(),
                    "hyperref".to_string(),
                ]),
            LaTeXStyle::Report => LaTeXFormatter::new()
                .document_class("report".to_string())
                .with_packages(vec![
                    "amsmath".to_string(),
                    "amssymb".to_string(),
                    "graphicx".to_string(),
                    "hyperref".to_string(),
                    "geometry".to_string(),
                ]),
            LaTeXStyle::Book => LaTeXFormatter::new()
                .document_class("book".to_string())
                .with_packages(vec![
                    "amsmath".to_string(),
                    "amssymb".to_string(),
                    "graphicx".to_string(),
                    "hyperref".to_string(),
                    "geometry".to_string(),
                    "fancyhdr".to_string(),
                ]),
            LaTeXStyle::Beamer => LaTeXFormatter::new()
                .document_class("beamer".to_string())
                .with_packages(vec![
                    "amsmath".to_string(),
                    "amssymb".to_string(),
                    "graphicx".to_string(),
                ]),
            LaTeXStyle::Minimal => LaTeXFormatter::new()
                .document_class("article".to_string())
                .with_packages(vec!["amsmath".to_string()]),
        };

        Self { base, style }
    }

    pub fn format_document(
        &self,
        content: &str,
        title: Option<&str>,
        author: Option<&str>,
    ) -> String {
        let mut preamble = String::new();

        if let Some(t) = title {
            preamble.push_str(&format!("\\title{{{}}}\n", t));
        }
        if let Some(a) = author {
            preamble.push_str(&format!("\\author{{{}}}\n", a));
        }
        if title.is_some() || author.is_some() {
            preamble.push_str("\\date{\\today}\n");
        }

        let formatter = self.base.clone().preamble(preamble);
        let mut doc = formatter.format_document(content);

        // Add maketitle after \begin{document} if we have title/author
        if title.is_some() || author.is_some() {
            doc = doc.replace(
                "\\begin{document}\n\n",
                "\\begin{document}\n\n\\maketitle\n\n",
            );
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BoundingBox;

    #[test]
    fn test_format_simple() {
        let formatter = LaTeXFormatter::new();
        let result = formatter.format("E = mc^2");
        assert!(result.contains("mc^2"));
    }

    #[test]
    fn test_format_document() {
        let formatter = LaTeXFormatter::new();
        let doc = formatter.format_document("E = mc^2");

        assert!(doc.contains(r"\documentclass{article}"));
        assert!(doc.contains(r"\usepackage{amsmath}"));
        assert!(doc.contains(r"\begin{document}"));
        assert!(doc.contains("mc^2"));
        assert!(doc.contains(r"\end{document}"));
    }

    #[test]
    fn test_escape_text() {
        let formatter = LaTeXFormatter::new();
        let result = formatter.escape_text("Price: $100 & 50%");
        assert!(result.contains(r"\$100"));
        assert!(result.contains(r"\&"));
        assert!(result.contains(r"\%"));
    }

    #[test]
    fn test_inline_to_display() {
        let formatter = LaTeXFormatter::new();
        let result = formatter.inline_to_display("x^2 + y^2 = r^2");
        assert!(result.contains(r"\["));
        assert!(result.contains(r"\]"));
    }

    #[test]
    fn test_styled_formatter() {
        let formatter = StyledLaTeXFormatter::new(LaTeXStyle::Article);
        let doc = formatter.format_document("Content", Some("My Title"), Some("Author Name"));

        assert!(doc.contains(r"\title{My Title}"));
        assert!(doc.contains(r"\author{Author Name}"));
        assert!(doc.contains(r"\maketitle"));
    }

    #[test]
    fn test_format_lines() {
        let formatter = LaTeXFormatter::new();
        let lines = vec![
            LineData {
                line_type: "text".to_string(),
                text: "Introduction".to_string(),
                latex: None,
                bbox: BoundingBox::new(0.0, 0.0, 100.0, 20.0),
                confidence: 0.95,
                words: None,
            },
            LineData {
                line_type: "equation".to_string(),
                text: "E = mc^2".to_string(),
                latex: Some(r"E = mc^2".to_string()),
                bbox: BoundingBox::new(0.0, 25.0, 100.0, 30.0),
                confidence: 0.98,
                words: None,
            },
        ];

        let result = formatter.format_lines(&lines);
        assert!(result.contains("Introduction"));
        assert!(result.contains(r"\[") || result.contains(r"\begin{equation}"));
        assert!(result.contains("mc^2"));
    }
}
