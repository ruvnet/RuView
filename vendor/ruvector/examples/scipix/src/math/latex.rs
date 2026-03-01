//! LaTeX generation from mathematical AST
//!
//! This module converts mathematical AST nodes to LaTeX strings with proper
//! formatting, precedence handling, and delimiter placement.

use crate::math::ast::{BinaryOp, BracketType, LargeOpType, MathExpr, MathNode, UnaryOp};
use crate::math::symbols::unicode_to_latex;

/// Configuration for LaTeX generation
#[derive(Debug, Clone)]
pub struct LaTeXConfig {
    /// Use display style (true) or inline style (false)
    pub display_style: bool,
    /// Use \left and \right for delimiters
    pub auto_size_delimiters: bool,
    /// Insert spaces around operators
    pub spacing: bool,
}

impl Default for LaTeXConfig {
    fn default() -> Self {
        Self {
            display_style: false,
            auto_size_delimiters: true,
            spacing: true,
        }
    }
}

/// LaTeX generator for mathematical expressions
pub struct LaTeXGenerator {
    config: LaTeXConfig,
}

impl LaTeXGenerator {
    /// Create a new LaTeX generator with default configuration
    pub fn new() -> Self {
        Self {
            config: LaTeXConfig::default(),
        }
    }

    /// Create a new LaTeX generator with custom configuration
    pub fn with_config(config: LaTeXConfig) -> Self {
        Self { config }
    }

    /// Generate LaTeX string from a mathematical expression
    pub fn generate(&self, expr: &MathExpr) -> String {
        self.generate_node(&expr.root, None)
    }

    /// Generate LaTeX for a single node
    fn generate_node(&self, node: &MathNode, parent_precedence: Option<u8>) -> String {
        match node {
            MathNode::Symbol { value, unicode } => {
                if let Some(c) = unicode {
                    if let Some(latex) = unicode_to_latex(*c) {
                        return format!("\\{}", latex);
                    }
                }
                value.clone()
            }

            MathNode::Number { value, .. } => value.clone(),

            MathNode::Binary { op, left, right } => {
                let precedence = op.precedence();
                let needs_parens = parent_precedence.map_or(false, |p| precedence < p);

                let left_str = self.generate_node(left, Some(precedence));
                let right_str = self.generate_node(
                    right,
                    Some(if op.is_left_associative() {
                        precedence
                    } else {
                        precedence + 1
                    }),
                );

                let op_str = self.binary_op_to_latex(op);
                let space = if self.config.spacing { " " } else { "" };

                let result = format!("{}{}{}{}{}", left_str, space, op_str, space, right_str);

                if needs_parens {
                    self.wrap_parens(&result)
                } else {
                    result
                }
            }

            MathNode::Unary { op, operand } => {
                let op_str = self.unary_op_to_latex(op);
                let operand_str = self.generate_node(operand, Some(70)); // High precedence
                format!("{}{}", op_str, operand_str)
            }

            MathNode::Fraction {
                numerator,
                denominator,
            } => {
                let num_str = self.generate_node(numerator, None);
                let den_str = self.generate_node(denominator, None);
                format!("\\frac{{{}}}{{{}}}", num_str, den_str)
            }

            MathNode::Radical { index, radicand } => {
                let rad_str = self.generate_node(radicand, None);
                if let Some(idx) = index {
                    let idx_str = self.generate_node(idx, None);
                    format!("\\sqrt[{}]{{{}}}", idx_str, rad_str)
                } else {
                    format!("\\sqrt{{{}}}", rad_str)
                }
            }

            MathNode::Script {
                base,
                subscript,
                superscript,
            } => {
                let base_str = self.generate_node(base, Some(65));
                let mut result = base_str;

                if let Some(sub) = subscript {
                    let sub_str = self.generate_node(sub, None);
                    result.push_str(&format!("_{{{}}}", sub_str));
                }

                if let Some(sup) = superscript {
                    let sup_str = self.generate_node(sup, None);
                    result.push_str(&format!("^{{{}}}", sup_str));
                }

                result
            }

            MathNode::Function { name, argument } => {
                let arg_str = self.generate_node(argument, None);
                // Check if it's a standard function
                if is_standard_function(name) {
                    format!("\\{} {}", name, arg_str)
                } else {
                    format!("\\text{{{}}}({})", name, arg_str)
                }
            }

            MathNode::Matrix { rows, bracket_type } => {
                let env = match bracket_type {
                    BracketType::Parentheses => "pmatrix",
                    BracketType::Brackets => "bmatrix",
                    BracketType::Braces => "Bmatrix",
                    BracketType::Vertical => "vmatrix",
                    BracketType::DoubleVertical => "Vmatrix",
                    _ => "matrix",
                };

                let mut content = String::new();
                for (i, row) in rows.iter().enumerate() {
                    if i > 0 {
                        content.push_str(" \\\\ ");
                    }
                    for (j, elem) in row.iter().enumerate() {
                        if j > 0 {
                            content.push_str(" & ");
                        }
                        content.push_str(&self.generate_node(elem, None));
                    }
                }

                format!("\\begin{{{}}} {} \\end{{{}}}", env, content, env)
            }

            MathNode::Group {
                content,
                bracket_type,
            } => {
                let content_str = self.generate_node(content, None);
                self.wrap_with_brackets(&content_str, *bracket_type)
            }

            MathNode::LargeOp {
                op_type,
                lower,
                upper,
                content,
            } => {
                let op_str = self.large_op_to_latex(op_type);
                let content_str = self.generate_node(content, None);

                let mut result = op_str;

                if let Some(low) = lower {
                    let low_str = self.generate_node(low, None);
                    result.push_str(&format!("_{{{}}}", low_str));
                }

                if let Some(up) = upper {
                    let up_str = self.generate_node(up, None);
                    result.push_str(&format!("^{{{}}}", up_str));
                }

                format!("{} {}", result, content_str)
            }

            MathNode::Sequence { elements } => elements
                .iter()
                .map(|e| self.generate_node(e, None))
                .collect::<Vec<_>>()
                .join(", "),

            MathNode::Text { content } => {
                format!("\\text{{{}}}", content)
            }

            MathNode::Empty => String::new(),
        }
    }

    /// Convert binary operator to LaTeX
    fn binary_op_to_latex(&self, op: &BinaryOp) -> String {
        match op {
            BinaryOp::Add => "+".to_string(),
            BinaryOp::Subtract => "-".to_string(),
            BinaryOp::Multiply => "\\times".to_string(),
            BinaryOp::Divide => "\\div".to_string(),
            BinaryOp::Power => "^".to_string(),
            BinaryOp::Equal => "=".to_string(),
            BinaryOp::NotEqual => "\\neq".to_string(),
            BinaryOp::Less => "<".to_string(),
            BinaryOp::Greater => ">".to_string(),
            BinaryOp::LessEqual => "\\leq".to_string(),
            BinaryOp::GreaterEqual => "\\geq".to_string(),
            BinaryOp::ApproxEqual => "\\approx".to_string(),
            BinaryOp::Equivalent => "\\equiv".to_string(),
            BinaryOp::Similar => "\\sim".to_string(),
            BinaryOp::Congruent => "\\cong".to_string(),
            BinaryOp::Proportional => "\\propto".to_string(),
            BinaryOp::Custom(s) => s.to_string(),
        }
    }

    /// Convert unary operator to LaTeX
    fn unary_op_to_latex(&self, op: &UnaryOp) -> String {
        match op {
            UnaryOp::Plus => "+".to_string(),
            UnaryOp::Minus => "-".to_string(),
            UnaryOp::Not => "\\neg".to_string(),
            UnaryOp::Custom(s) => s.to_string(),
        }
    }

    /// Convert large operator to LaTeX
    fn large_op_to_latex(&self, op: &LargeOpType) -> String {
        match op {
            LargeOpType::Sum => "\\sum".to_string(),
            LargeOpType::Product => "\\prod".to_string(),
            LargeOpType::Integral => "\\int".to_string(),
            LargeOpType::DoubleIntegral => "\\iint".to_string(),
            LargeOpType::TripleIntegral => "\\iiint".to_string(),
            LargeOpType::ContourIntegral => "\\oint".to_string(),
            LargeOpType::Union => "\\bigcup".to_string(),
            LargeOpType::Intersection => "\\bigcap".to_string(),
            LargeOpType::Coproduct => "\\coprod".to_string(),
            LargeOpType::DirectSum => "\\bigoplus".to_string(),
            LargeOpType::Custom(s) => s.clone(),
        }
    }

    /// Wrap content with brackets
    fn wrap_with_brackets(&self, content: &str, bracket_type: BracketType) -> String {
        let (left, right) = if self.config.auto_size_delimiters {
            match bracket_type {
                BracketType::Parentheses => ("\\left(", "\\right)"),
                BracketType::Brackets => ("\\left[", "\\right]"),
                BracketType::Braces => ("\\left\\{", "\\right\\}"),
                BracketType::AngleBrackets => ("\\left\\langle", "\\right\\rangle"),
                BracketType::Vertical => ("\\left|", "\\right|"),
                BracketType::DoubleVertical => ("\\left\\|", "\\right\\|"),
                BracketType::Floor => ("\\left\\lfloor", "\\right\\rfloor"),
                BracketType::Ceiling => ("\\left\\lceil", "\\right\\rceil"),
                BracketType::None => ("", ""),
            }
        } else {
            match bracket_type {
                BracketType::Parentheses => ("(", ")"),
                BracketType::Brackets => ("[", "]"),
                BracketType::Braces => ("\\{", "\\}"),
                BracketType::AngleBrackets => ("\\langle", "\\rangle"),
                BracketType::Vertical => ("|", "|"),
                BracketType::DoubleVertical => ("\\|", "\\|"),
                BracketType::Floor => ("\\lfloor", "\\rfloor"),
                BracketType::Ceiling => ("\\lceil", "\\rceil"),
                BracketType::None => ("", ""),
            }
        };

        format!("{}{}{}", left, content, right)
    }

    /// Wrap content in parentheses
    fn wrap_parens(&self, content: &str) -> String {
        self.wrap_with_brackets(content, BracketType::Parentheses)
    }
}

impl Default for LaTeXGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a function name is a standard LaTeX function
fn is_standard_function(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "cot"
            | "sec"
            | "csc"
            | "sinh"
            | "cosh"
            | "tanh"
            | "coth"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "ln"
            | "log"
            | "exp"
            | "lim"
            | "sup"
            | "inf"
            | "max"
            | "min"
            | "det"
            | "dim"
            | "ker"
            | "deg"
            | "gcd"
            | "lcm"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_number() {
        let expr = MathExpr::new(
            MathNode::Number {
                value: "42".to_string(),
                is_decimal: false,
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "42");
    }

    #[test]
    fn test_simple_binary() {
        let expr = MathExpr::new(
            MathNode::Binary {
                op: BinaryOp::Add,
                left: Box::new(MathNode::Number {
                    value: "1".to_string(),
                    is_decimal: false,
                }),
                right: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "1 + 2");
    }

    #[test]
    fn test_fraction() {
        let expr = MathExpr::new(
            MathNode::Fraction {
                numerator: Box::new(MathNode::Number {
                    value: "1".to_string(),
                    is_decimal: false,
                }),
                denominator: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\frac{1}{2}");
    }

    #[test]
    fn test_square_root() {
        let expr = MathExpr::new(
            MathNode::Radical {
                index: None,
                radicand: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\sqrt{2}");
    }

    #[test]
    fn test_nth_root() {
        let expr = MathExpr::new(
            MathNode::Radical {
                index: Some(Box::new(MathNode::Number {
                    value: "3".to_string(),
                    is_decimal: false,
                })),
                radicand: Box::new(MathNode::Number {
                    value: "8".to_string(),
                    is_decimal: false,
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\sqrt[3]{8}");
    }

    #[test]
    fn test_superscript() {
        let expr = MathExpr::new(
            MathNode::Script {
                base: Box::new(MathNode::Symbol {
                    value: "x".to_string(),
                    unicode: None,
                }),
                subscript: None,
                superscript: Some(Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                })),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "x^{2}");
    }

    #[test]
    fn test_subscript() {
        let expr = MathExpr::new(
            MathNode::Script {
                base: Box::new(MathNode::Symbol {
                    value: "a".to_string(),
                    unicode: None,
                }),
                subscript: Some(Box::new(MathNode::Number {
                    value: "n".to_string(),
                    is_decimal: false,
                })),
                superscript: None,
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "a_{n}");
    }

    #[test]
    fn test_complex_fraction() {
        // (a + b) / (c - d)
        let expr = MathExpr::new(
            MathNode::Fraction {
                numerator: Box::new(MathNode::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(MathNode::Symbol {
                        value: "a".to_string(),
                        unicode: None,
                    }),
                    right: Box::new(MathNode::Symbol {
                        value: "b".to_string(),
                        unicode: None,
                    }),
                }),
                denominator: Box::new(MathNode::Binary {
                    op: BinaryOp::Subtract,
                    left: Box::new(MathNode::Symbol {
                        value: "c".to_string(),
                        unicode: None,
                    }),
                    right: Box::new(MathNode::Symbol {
                        value: "d".to_string(),
                        unicode: None,
                    }),
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\frac{a + b}{c - d}");
    }

    #[test]
    fn test_summation() {
        // ∑_{i=1}^{n} i
        let expr = MathExpr::new(
            MathNode::LargeOp {
                op_type: LargeOpType::Sum,
                lower: Some(Box::new(MathNode::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(MathNode::Symbol {
                        value: "i".to_string(),
                        unicode: None,
                    }),
                    right: Box::new(MathNode::Number {
                        value: "1".to_string(),
                        is_decimal: false,
                    }),
                })),
                upper: Some(Box::new(MathNode::Symbol {
                    value: "n".to_string(),
                    unicode: None,
                })),
                content: Box::new(MathNode::Symbol {
                    value: "i".to_string(),
                    unicode: None,
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\sum_{i = 1}^{n} i");
    }

    #[test]
    fn test_integral() {
        // ∫ x dx
        let expr = MathExpr::new(
            MathNode::LargeOp {
                op_type: LargeOpType::Integral,
                lower: None,
                upper: None,
                content: Box::new(MathNode::Sequence {
                    elements: vec![
                        MathNode::Symbol {
                            value: "x".to_string(),
                            unicode: None,
                        },
                        MathNode::Symbol {
                            value: "dx".to_string(),
                            unicode: None,
                        },
                    ],
                }),
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(gen.generate(&expr), "\\int x, dx");
    }

    #[test]
    fn test_matrix() {
        let expr = MathExpr::new(
            MathNode::Matrix {
                rows: vec![
                    vec![
                        MathNode::Number {
                            value: "1".to_string(),
                            is_decimal: false,
                        },
                        MathNode::Number {
                            value: "2".to_string(),
                            is_decimal: false,
                        },
                    ],
                    vec![
                        MathNode::Number {
                            value: "3".to_string(),
                            is_decimal: false,
                        },
                        MathNode::Number {
                            value: "4".to_string(),
                            is_decimal: false,
                        },
                    ],
                ],
                bracket_type: BracketType::Brackets,
            },
            1.0,
        );
        let gen = LaTeXGenerator::new();
        assert_eq!(
            gen.generate(&expr),
            "\\begin{bmatrix} 1 & 2 \\\\ 3 & 4 \\end{bmatrix}"
        );
    }
}
