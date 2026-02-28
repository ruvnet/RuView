//! HTML output formatter with math rendering support

use super::{HtmlEngine, LineData};

/// HTML formatter with math rendering
pub struct HtmlFormatter {
    engine: HtmlEngine,
    css_styling: bool,
    accessibility: bool,
    responsive: bool,
    theme: HtmlTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlTheme {
    Light,
    Dark,
    Auto,
}

impl HtmlFormatter {
    pub fn new() -> Self {
        Self {
            engine: HtmlEngine::MathJax,
            css_styling: true,
            accessibility: true,
            responsive: true,
            theme: HtmlTheme::Light,
        }
    }

    pub fn with_engine(mut self, engine: HtmlEngine) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_styling(mut self, styling: bool) -> Self {
        self.css_styling = styling;
        self
    }

    pub fn accessibility(mut self, enabled: bool) -> Self {
        self.accessibility = enabled;
        self
    }

    pub fn responsive(mut self, enabled: bool) -> Self {
        self.responsive = enabled;
        self
    }

    pub fn theme(mut self, theme: HtmlTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Format content to HTML
    pub fn format(&self, content: &str, lines: Option<&[LineData]>) -> String {
        let mut html = String::new();

        // HTML header with math rendering scripts
        html.push_str(&self.html_header());

        // Body start with theme class
        html.push_str("<body");
        if self.css_styling {
            html.push_str(&format!(r#" class="theme-{:?}""#, self.theme).to_lowercase());
        }
        html.push_str(">\n");

        // Main content container
        html.push_str(r#"<div class="content">"#);
        html.push_str("\n");

        // Format content
        if let Some(line_data) = lines {
            html.push_str(&self.format_lines(line_data));
        } else {
            html.push_str(&self.format_text(content));
        }

        html.push_str("</div>\n");
        html.push_str("</body>\n</html>");

        html
    }

    /// Generate HTML header with scripts and styles
    fn html_header(&self) -> String {
        let mut header = String::from("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        header.push_str(r#"    <meta charset="UTF-8">"#);
        header.push_str("\n");

        if self.responsive {
            header.push_str(
                r#"    <meta name="viewport" content="width=device-width, initial-scale=1.0">"#,
            );
            header.push_str("\n");
        }

        header.push_str("    <title>Mathematical Content</title>\n");

        // Math rendering scripts
        match self.engine {
            HtmlEngine::MathJax => {
                header.push_str(r#"    <script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>"#);
                header.push_str("\n");
                header.push_str(r#"    <script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>"#);
                header.push_str("\n");
                header.push_str("    <script>\n");
                header.push_str("    MathJax = {\n");
                header.push_str("        tex: {\n");
                header.push_str(r#"            inlineMath: [['$', '$'], ['\\(', '\\)']],"#);
                header.push_str("\n");
                header.push_str(r#"            displayMath: [['$$', '$$'], ['\\[', '\\]']]"#);
                header.push_str("\n");
                header.push_str("        }\n");
                header.push_str("    };\n");
                header.push_str("    </script>\n");
            }
            HtmlEngine::KaTeX => {
                header.push_str(r#"    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.css">"#);
                header.push_str("\n");
                header.push_str(r#"    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.js"></script>"#);
                header.push_str("\n");
                header.push_str(r#"    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/contrib/auto-render.min.js" onload="renderMathInElement(document.body);"></script>"#);
                header.push_str("\n");
            }
            HtmlEngine::Raw => {
                // No math rendering
            }
        }

        // CSS styling
        if self.css_styling {
            header.push_str("    <style>\n");
            header.push_str(&self.generate_css());
            header.push_str("    </style>\n");
        }

        header.push_str("</head>\n");
        header
    }

    /// Generate CSS styles
    fn generate_css(&self) -> String {
        let mut css = String::new();

        css.push_str("        body {\n");
        css.push_str("            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;\n");
        css.push_str("            line-height: 1.6;\n");
        css.push_str("            max-width: 800px;\n");
        css.push_str("            margin: 0 auto;\n");
        css.push_str("            padding: 20px;\n");
        css.push_str("        }\n");

        // Theme colors
        match self.theme {
            HtmlTheme::Light => {
                css.push_str("        body.theme-light {\n");
                css.push_str("            background-color: #ffffff;\n");
                css.push_str("            color: #333333;\n");
                css.push_str("        }\n");
            }
            HtmlTheme::Dark => {
                css.push_str("        body.theme-dark {\n");
                css.push_str("            background-color: #1e1e1e;\n");
                css.push_str("            color: #d4d4d4;\n");
                css.push_str("        }\n");
            }
            HtmlTheme::Auto => {
                css.push_str("        @media (prefers-color-scheme: dark) {\n");
                css.push_str("            body { background-color: #1e1e1e; color: #d4d4d4; }\n");
                css.push_str("        }\n");
            }
        }

        css.push_str("        .content { padding: 20px; }\n");
        css.push_str("        .math-display { text-align: center; margin: 20px 0; }\n");
        css.push_str("        .math-inline { display: inline; }\n");
        css.push_str("        .equation-block { margin: 15px 0; padding: 10px; background: #f5f5f5; border-radius: 4px; }\n");
        css.push_str("        table { border-collapse: collapse; width: 100%; margin: 20px 0; }\n");
        css.push_str(
            "        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n",
        );
        css.push_str("        th { background-color: #f2f2f2; }\n");

        if self.accessibility {
            css.push_str("        .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); border: 0; }\n");
        }

        css
    }

    /// Format plain text to HTML
    fn format_text(&self, text: &str) -> String {
        let escaped = self.escape_html(text);

        // Convert math delimiters if present
        let mut html = escaped;

        // Display math $$...$$
        html = html.replace("$$", "<div class=\"math-display\">$$");
        html = html.replace("$$", "$$</div>");

        // Inline math $...$
        // This is simplistic - a real implementation would need proper parsing

        format!("<p>{}</p>", html)
    }

    /// Format line data to HTML
    fn format_lines(&self, lines: &[LineData]) -> String {
        let mut html = String::new();

        for line in lines {
            match line.line_type.as_str() {
                "text" => {
                    html.push_str("<p>");
                    html.push_str(&self.escape_html(&line.text));
                    html.push_str("</p>\n");
                }
                "math" | "equation" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    html.push_str(r#"<div class="math-display">"#);
                    if self.accessibility {
                        html.push_str(&format!(
                            r#"<span class="sr-only">Equation: {}</span>"#,
                            self.escape_html(&line.text)
                        ));
                    }
                    html.push_str(&format!("$${}$$", latex));
                    html.push_str("</div>\n");
                }
                "inline_math" => {
                    let latex = line.latex.as_ref().unwrap_or(&line.text);
                    html.push_str(&format!(r#"<span class="math-inline">${}$</span>"#, latex));
                }
                "heading" => {
                    html.push_str(&format!("<h2>{}</h2>\n", self.escape_html(&line.text)));
                }
                "table" => {
                    html.push_str(&self.format_table(&line.text));
                }
                "image" => {
                    html.push_str(&format!(
                        r#"<img src="{}" alt="Image" loading="lazy">"#,
                        self.escape_html(&line.text)
                    ));
                    html.push_str("\n");
                }
                _ => {
                    html.push_str("<p>");
                    html.push_str(&self.escape_html(&line.text));
                    html.push_str("</p>\n");
                }
            }
        }

        html
    }

    /// Format table to HTML
    fn format_table(&self, table: &str) -> String {
        let mut html = String::from("<table>\n");

        let rows: Vec<&str> = table.lines().collect();
        for (i, row) in rows.iter().enumerate() {
            html.push_str("  <tr>\n");

            let cells: Vec<&str> = row
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            let tag = if i == 0 { "th" } else { "td" };

            for cell in cells {
                html.push_str(&format!(
                    "    <{}>{}</{}>\n",
                    tag,
                    self.escape_html(cell),
                    tag
                ));
            }

            html.push_str("  </tr>\n");
        }

        html.push_str("</table>\n");
        html
    }

    /// Escape HTML special characters
    fn escape_html(&self, text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
}

impl Default for HtmlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BoundingBox;

    #[test]
    fn test_html_header() {
        let formatter = HtmlFormatter::new().with_engine(HtmlEngine::MathJax);
        let header = formatter.html_header();

        assert!(header.contains("<!DOCTYPE html>"));
        assert!(header.contains("MathJax"));
    }

    #[test]
    fn test_katex_header() {
        let formatter = HtmlFormatter::new().with_engine(HtmlEngine::KaTeX);
        let header = formatter.html_header();

        assert!(header.contains("katex"));
    }

    #[test]
    fn test_escape_html() {
        let formatter = HtmlFormatter::new();
        let result = formatter.escape_html("<script>alert('test')</script>");

        assert!(result.contains("&lt;"));
        assert!(result.contains("&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn test_format_lines() {
        let formatter = HtmlFormatter::new();
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
        assert!(result.contains("<p>Introduction</p>"));
        assert!(result.contains("math-display"));
        assert!(result.contains("$$"));
    }

    #[test]
    fn test_dark_theme() {
        let formatter = HtmlFormatter::new().theme(HtmlTheme::Dark);
        let css = formatter.generate_css();

        assert!(css.contains("theme-dark"));
        assert!(css.contains("#1e1e1e"));
    }

    #[test]
    fn test_accessibility() {
        let formatter = HtmlFormatter::new().accessibility(true);
        let lines = vec![LineData {
            line_type: "equation".to_string(),
            text: "x squared".to_string(),
            latex: Some("x^2".to_string()),
            bbox: BoundingBox::new(0.0, 0.0, 100.0, 20.0),
            confidence: 0.98,
            words: None,
        }];

        let result = formatter.format_lines(&lines);
        assert!(result.contains("sr-only"));
        assert!(result.contains("Equation:"));
    }
}
