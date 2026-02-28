//! AsciiMath generation from mathematical AST
//!
//! This module converts mathematical AST nodes to AsciiMath notation,
//! a simplified plain-text format for mathematical expressions.

use crate::math::ast::{BinaryOp, BracketType, LargeOpType, MathExpr, MathNode, UnaryOp};

/// AsciiMath generator for mathematical expressions
pub struct AsciiMathGenerator {
    /// Use Unicode symbols (true) or ASCII approximations (false)
    unicode: bool,
}

impl AsciiMathGenerator {
    /// Create a new AsciiMath generator with Unicode support
    pub fn new() -> Self {
        Self { unicode: true }
    }

    /// Create an ASCII-only generator
    pub fn ascii_only() -> Self {
        Self { unicode: false }
    }

    /// Generate AsciiMath string from a mathematical expression
    pub fn generate(&self, expr: &MathExpr) -> String {
        self.generate_node(&expr.root, None)
    }

    /// Generate AsciiMath for a single node
    fn generate_node(&self, node: &MathNode, parent_precedence: Option<u8>) -> String {
        match node {
            MathNode::Symbol { value, .. } => value.clone(),

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

                let op_str = self.binary_op_to_asciimath(op);

                let result = format!("{} {} {}", left_str, op_str, right_str);

                if needs_parens {
                    format!("({})", result)
                } else {
                    result
                }
            }

            MathNode::Unary { op, operand } => {
                let op_str = self.unary_op_to_asciimath(op);
                let operand_str = self.generate_node(operand, Some(70));
                format!("{}{}", op_str, operand_str)
            }

            MathNode::Fraction {
                numerator,
                denominator,
            } => {
                let num_str = self.generate_node(numerator, None);
                let den_str = self.generate_node(denominator, None);
                format!("({})/({})", num_str, den_str)
            }

            MathNode::Radical { index, radicand } => {
                let rad_str = self.generate_node(radicand, None);
                if let Some(idx) = index {
                    let idx_str = self.generate_node(idx, None);
                    format!("root({})({} )", idx_str, rad_str)
                } else {
                    format!("sqrt({})", rad_str)
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
                format!("{}({})", name, arg_str)
            }

            MathNode::Matrix { rows, .. } => {
                let mut content = String::new();
                content.push('[');

                for (i, row) in rows.iter().enumerate() {
                    if i > 0 {
                        content.push_str("; ");
                    }
                    for (j, elem) in row.iter().enumerate() {
                        if j > 0 {
                            content.push_str(", ");
                        }
                        content.push_str(&self.generate_node(elem, None));
                    }
                }

                content.push(']');
                content
            }

            MathNode::Group {
                content,
                bracket_type,
            } => {
                let content_str = self.generate_node(content, None);
                let (open, close) = match bracket_type {
                    BracketType::Parentheses => ("(", ")"),
                    BracketType::Brackets => ("[", "]"),
                    BracketType::Braces => ("{", "}"),
                    BracketType::AngleBrackets => {
                        if self.unicode {
                            ("⟨", "⟩")
                        } else {
                            ("<", ">")
                        }
                    }
                    BracketType::Vertical => ("|", "|"),
                    BracketType::DoubleVertical => {
                        if self.unicode {
                            ("‖", "‖")
                        } else {
                            ("||", "||")
                        }
                    }
                    BracketType::Floor => {
                        if self.unicode {
                            ("⌊", "⌋")
                        } else {
                            ("|_", "_|")
                        }
                    }
                    BracketType::Ceiling => {
                        if self.unicode {
                            ("⌈", "⌉")
                        } else {
                            ("|^", "^|")
                        }
                    }
                    BracketType::None => ("", ""),
                };

                format!("{}{}{}", open, content_str, close)
            }

            MathNode::LargeOp {
                op_type,
                lower,
                upper,
                content,
            } => {
                let op_str = self.large_op_to_asciimath(op_type);
                let content_str = self.generate_node(content, None);

                let mut result = op_str.to_string();

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
                format!("\"{}\"", content)
            }

            MathNode::Empty => String::new(),
        }
    }

    /// Convert binary operator to AsciiMath
    fn binary_op_to_asciimath<'a>(&self, op: &'a BinaryOp) -> &'a str {
        if self.unicode {
            match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "×",
                BinaryOp::Divide => "÷",
                BinaryOp::Power => "^",
                BinaryOp::Equal => "=",
                BinaryOp::NotEqual => "≠",
                BinaryOp::Less => "<",
                BinaryOp::Greater => ">",
                BinaryOp::LessEqual => "≤",
                BinaryOp::GreaterEqual => "≥",
                BinaryOp::ApproxEqual => "≈",
                BinaryOp::Equivalent => "≡",
                BinaryOp::Similar => "∼",
                BinaryOp::Congruent => "≅",
                BinaryOp::Proportional => "∝",
                BinaryOp::Custom(s) => s,
            }
        } else {
            match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Power => "^",
                BinaryOp::Equal => "=",
                BinaryOp::NotEqual => "!=",
                BinaryOp::Less => "<",
                BinaryOp::Greater => ">",
                BinaryOp::LessEqual => "<=",
                BinaryOp::GreaterEqual => ">=",
                BinaryOp::ApproxEqual => "~~",
                BinaryOp::Equivalent => "-=",
                BinaryOp::Similar => "~",
                BinaryOp::Congruent => "~=",
                BinaryOp::Proportional => "prop",
                BinaryOp::Custom(s) => s.as_str(),
            }
        }
    }

    /// Convert unary operator to AsciiMath
    fn unary_op_to_asciimath<'a>(&self, op: &'a UnaryOp) -> &'a str {
        match op {
            UnaryOp::Plus => "+",
            UnaryOp::Minus => "-",
            UnaryOp::Not => {
                if self.unicode {
                    "¬"
                } else {
                    "not "
                }
            }
            UnaryOp::Custom(s) => s.as_str(),
        }
    }

    /// Convert large operator to AsciiMath
    fn large_op_to_asciimath(&self, op: &LargeOpType) -> &str {
        if self.unicode {
            match op {
                LargeOpType::Sum => "∑",
                LargeOpType::Product => "∏",
                LargeOpType::Integral => "∫",
                LargeOpType::DoubleIntegral => "∬",
                LargeOpType::TripleIntegral => "∭",
                LargeOpType::ContourIntegral => "∮",
                LargeOpType::Union => "⋃",
                LargeOpType::Intersection => "⋂",
                LargeOpType::Coproduct => "∐",
                LargeOpType::DirectSum => "⊕",
                LargeOpType::Custom(_) => "sum",
            }
        } else {
            match op {
                LargeOpType::Sum => "sum",
                LargeOpType::Product => "prod",
                LargeOpType::Integral => "int",
                LargeOpType::DoubleIntegral => "iint",
                LargeOpType::TripleIntegral => "iiint",
                LargeOpType::ContourIntegral => "oint",
                LargeOpType::Union => "cup",
                LargeOpType::Intersection => "cap",
                LargeOpType::Coproduct => "coprod",
                LargeOpType::DirectSum => "oplus",
                LargeOpType::Custom(_) => "sum",
            }
        }
    }
}

impl Default for AsciiMathGenerator {
    fn default() -> Self {
        Self::new()
    }
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
        let gen = AsciiMathGenerator::new();
        assert_eq!(gen.generate(&expr), "42");
    }

    #[test]
    fn test_addition() {
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
        let gen = AsciiMathGenerator::new();
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
        let gen = AsciiMathGenerator::new();
        assert_eq!(gen.generate(&expr), "(1)/(2)");
    }

    #[test]
    fn test_sqrt() {
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
        let gen = AsciiMathGenerator::new();
        assert_eq!(gen.generate(&expr), "sqrt(2)");
    }

    #[test]
    fn test_superscript() {
        let expr = MathExpr::new(
            MathNode::Script {
                base: Box::new(MathNode::Symbol {
                    value: "x".to_string(),
                    unicode: Some('x'),
                }),
                subscript: None,
                superscript: Some(Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                })),
            },
            1.0,
        );
        let gen = AsciiMathGenerator::new();
        assert_eq!(gen.generate(&expr), "x^{2}");
    }

    #[test]
    fn test_unicode_vs_ascii() {
        let expr = MathExpr::new(
            MathNode::Binary {
                op: BinaryOp::Multiply,
                left: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
                right: Box::new(MathNode::Number {
                    value: "3".to_string(),
                    is_decimal: false,
                }),
            },
            1.0,
        );

        let gen_unicode = AsciiMathGenerator::new();
        assert_eq!(gen_unicode.generate(&expr), "2 × 3");

        let gen_ascii = AsciiMathGenerator::ascii_only();
        assert_eq!(gen_ascii.generate(&expr), "2 * 3");
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
        let gen = AsciiMathGenerator::new();
        assert_eq!(gen.generate(&expr), "[1, 2; 3, 4]");
    }
}
