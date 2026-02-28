//! Mathematical expression parsing and conversion module
//!
//! This module provides functionality for parsing, representing, and converting
//! mathematical expressions between various formats including LaTeX, MathML, and AsciiMath.
//!
//! # Modules
//!
//! - `ast`: Abstract Syntax Tree definitions for mathematical expressions
//! - `symbols`: Symbol mappings between Unicode and LaTeX
//! - `latex`: LaTeX generation from AST
//! - `mathml`: MathML generation from AST
//! - `asciimath`: AsciiMath generation from AST
//! - `parser`: Expression parsing from various formats
//!
//! # Examples
//!
//! ## Parsing and converting to LaTeX
//!
//! ```no_run
//! use ruvector_scipix::math::{parse_expression, to_latex};
//!
//! let expr = parse_expression("x^2 + 2x + 1").unwrap();
//! let latex = to_latex(&expr);
//! println!("LaTeX: {}", latex);
//! ```
//!
//! ## Building an expression manually
//!
//! ```no_run
//! use ruvector_scipix::math::ast::{MathExpr, MathNode, BinaryOp};
//!
//! let expr = MathExpr::new(
//!     MathNode::Binary {
//!         op: BinaryOp::Add,
//!         left: Box::new(MathNode::Number {
//!             value: "1".to_string(),
//!             is_decimal: false,
//!         }),
//!         right: Box::new(MathNode::Number {
//!             value: "2".to_string(),
//!             is_decimal: false,
//!         }),
//!     },
//!     1.0,
//! );
//! ```

pub mod asciimath;
pub mod ast;
pub mod latex;
pub mod mathml;
pub mod parser;
pub mod symbols;

// Re-export commonly used types
pub use asciimath::AsciiMathGenerator;
pub use ast::{BinaryOp, BracketType, LargeOpType, MathExpr, MathNode, MathVisitor, UnaryOp};
pub use latex::{LaTeXConfig, LaTeXGenerator};
pub use mathml::MathMLGenerator;
pub use parser::{parse_expression, Parser};
pub use symbols::{get_symbol, unicode_to_latex, MathSymbol, SymbolCategory};

/// Parse a mathematical expression from a string
///
/// # Arguments
///
/// * `input` - The input string to parse (LaTeX, Unicode, or mixed)
///
/// # Returns
///
/// A `Result` containing the parsed `MathExpr` or an error message
///
/// # Examples
///
/// ```no_run
/// use ruvector_scipix::math::parse_expression;
///
/// let expr = parse_expression("\\frac{1}{2}").unwrap();
/// ```
pub fn parse(input: &str) -> Result<MathExpr, String> {
    parse_expression(input)
}

/// Convert a mathematical expression to LaTeX format
///
/// # Arguments
///
/// * `expr` - The mathematical expression to convert
///
/// # Returns
///
/// A LaTeX string representation of the expression
///
/// # Examples
///
/// ```no_run
/// use ruvector_scipix::math::{parse_expression, to_latex};
///
/// let expr = parse_expression("x^2").unwrap();
/// let latex = to_latex(&expr);
/// assert!(latex.contains("^"));
/// ```
pub fn to_latex(expr: &MathExpr) -> String {
    LaTeXGenerator::new().generate(expr)
}

/// Convert a mathematical expression to LaTeX with custom configuration
///
/// # Arguments
///
/// * `expr` - The mathematical expression to convert
/// * `config` - LaTeX generation configuration
///
/// # Returns
///
/// A LaTeX string representation of the expression
pub fn to_latex_with_config(expr: &MathExpr, config: LaTeXConfig) -> String {
    LaTeXGenerator::with_config(config).generate(expr)
}

/// Convert a mathematical expression to MathML format
///
/// # Arguments
///
/// * `expr` - The mathematical expression to convert
///
/// # Returns
///
/// A MathML XML string representation of the expression
///
/// # Examples
///
/// ```no_run
/// use ruvector_scipix::math::{parse_expression, to_mathml};
///
/// let expr = parse_expression("x^2").unwrap();
/// let mathml = to_mathml(&expr);
/// assert!(mathml.contains("<msup>"));
/// ```
pub fn to_mathml(expr: &MathExpr) -> String {
    MathMLGenerator::new().generate(expr)
}

/// Convert a mathematical expression to AsciiMath format
///
/// # Arguments
///
/// * `expr` - The mathematical expression to convert
///
/// # Returns
///
/// An AsciiMath string representation of the expression
///
/// # Examples
///
/// ```no_run
/// use ruvector_scipix::math::{parse_expression, to_asciimath};
///
/// let expr = parse_expression("x^2").unwrap();
/// let asciimath = to_asciimath(&expr);
/// ```
pub fn to_asciimath(expr: &MathExpr) -> String {
    AsciiMathGenerator::new().generate(expr)
}

/// Convert a mathematical expression to ASCII-only AsciiMath format
///
/// # Arguments
///
/// * `expr` - The mathematical expression to convert
///
/// # Returns
///
/// An ASCII-only AsciiMath string representation of the expression
pub fn to_asciimath_ascii_only(expr: &MathExpr) -> String {
    AsciiMathGenerator::ascii_only().generate(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_convert() {
        let expr = parse("1 + 2").unwrap();
        let latex = to_latex(&expr);
        assert!(latex.contains("+"));
    }

    #[test]
    fn test_fraction_conversion() {
        let expr = parse("\\frac{1}{2}").unwrap();

        let latex = to_latex(&expr);
        assert!(latex.contains("\\frac"));

        let mathml = to_mathml(&expr);
        assert!(mathml.contains("<mfrac>"));

        let asciimath = to_asciimath(&expr);
        assert!(asciimath.contains("/"));
    }

    #[test]
    fn test_sqrt_conversion() {
        let expr = parse("\\sqrt{2}").unwrap();

        let latex = to_latex(&expr);
        assert!(latex.contains("\\sqrt"));

        let mathml = to_mathml(&expr);
        assert!(mathml.contains("<msqrt>"));

        let asciimath = to_asciimath(&expr);
        assert!(asciimath.contains("sqrt"));
    }

    #[test]
    fn test_complex_expression() {
        // Quadratic formula: (-b ± √(b² - 4ac)) / 2a
        let expr = parse("\\frac{-b + \\sqrt{b^2 - 4*a*c}}{2*a}").unwrap();

        let latex = to_latex(&expr);
        assert!(latex.contains("\\frac"));
        assert!(latex.contains("\\sqrt"));

        let mathml = to_mathml(&expr);
        assert!(mathml.contains("<mfrac>"));
        assert!(mathml.contains("<msqrt>"));
    }

    #[test]
    fn test_symbol_lookup() {
        assert!(unicode_to_latex('α').is_some());
        assert_eq!(unicode_to_latex('α'), Some("alpha"));
        assert_eq!(unicode_to_latex('π'), Some("pi"));
        assert_eq!(unicode_to_latex('∑'), Some("sum"));
    }

    #[test]
    fn test_get_symbol() {
        let sym = get_symbol('α').unwrap();
        assert_eq!(sym.latex, "alpha");
        assert_eq!(sym.category, SymbolCategory::Greek);
    }
}
