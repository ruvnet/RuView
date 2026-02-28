//! MathML generation from mathematical AST
//!
//! This module converts mathematical AST nodes to MathML (Mathematical Markup Language)
//! XML format for rendering in web browsers and applications.

use crate::math::ast::{BinaryOp, BracketType, LargeOpType, MathExpr, MathNode, UnaryOp};

/// MathML generator for mathematical expressions
pub struct MathMLGenerator {
    /// Use presentation MathML (true) or content MathML (false)
    presentation: bool,
}

impl MathMLGenerator {
    /// Create a new MathML generator (presentation mode)
    pub fn new() -> Self {
        Self { presentation: true }
    }

    /// Create a content MathML generator
    pub fn content() -> Self {
        Self {
            presentation: false,
        }
    }

    /// Generate MathML string from a mathematical expression
    pub fn generate(&self, expr: &MathExpr) -> String {
        let content = self.generate_node(&expr.root);
        format!(
            r#"<math xmlns="http://www.w3.org/1998/Math/MathML">{}</math>"#,
            content
        )
    }

    /// Generate MathML for a single node
    fn generate_node(&self, node: &MathNode) -> String {
        match node {
            MathNode::Symbol { value, .. } => {
                format!("<mi>{}</mi>", escape_xml(value))
            }

            MathNode::Number { value, .. } => {
                format!("<mn>{}</mn>", escape_xml(value))
            }

            MathNode::Binary { op, left, right } => {
                let left_ml = self.generate_node(left);
                let right_ml = self.generate_node(right);
                let op_ml = self.binary_op_to_mathml(op);

                format!("<mrow>{}<mo>{}</mo>{}</mrow>", left_ml, op_ml, right_ml)
            }

            MathNode::Unary { op, operand } => {
                let op_ml = self.unary_op_to_mathml(op);
                let operand_ml = self.generate_node(operand);

                format!("<mrow><mo>{}</mo>{}</mrow>", op_ml, operand_ml)
            }

            MathNode::Fraction {
                numerator,
                denominator,
            } => {
                let num_ml = self.generate_node(numerator);
                let den_ml = self.generate_node(denominator);

                format!("<mfrac>{}{}</mfrac>", num_ml, den_ml)
            }

            MathNode::Radical { index, radicand } => {
                let rad_ml = self.generate_node(radicand);

                if let Some(idx) = index {
                    let idx_ml = self.generate_node(idx);
                    format!("<mroot>{}{}</mroot>", rad_ml, idx_ml)
                } else {
                    format!("<msqrt>{}</msqrt>", rad_ml)
                }
            }

            MathNode::Script {
                base,
                subscript,
                superscript,
            } => {
                let base_ml = self.generate_node(base);

                match (subscript, superscript) {
                    (Some(sub), Some(sup)) => {
                        let sub_ml = self.generate_node(sub);
                        let sup_ml = self.generate_node(sup);
                        format!("<msubsup>{}{}{}</msubsup>", base_ml, sub_ml, sup_ml)
                    }
                    (Some(sub), None) => {
                        let sub_ml = self.generate_node(sub);
                        format!("<msub>{}{}</msub>", base_ml, sub_ml)
                    }
                    (None, Some(sup)) => {
                        let sup_ml = self.generate_node(sup);
                        format!("<msup>{}{}</msup>", base_ml, sup_ml)
                    }
                    (None, None) => base_ml,
                }
            }

            MathNode::Function { name, argument } => {
                let name_ml = format!("<mi>{}</mi>", escape_xml(name));
                let arg_ml = self.generate_node(argument);

                format!("<mrow>{}<mo>&ApplyFunction;</mo>{}</mrow>", name_ml, arg_ml)
            }

            MathNode::Matrix { rows, bracket_type } => {
                let mut content = String::new();

                for row in rows {
                    content.push_str("<mtr>");
                    for elem in row {
                        content.push_str("<mtd>");
                        content.push_str(&self.generate_node(elem));
                        content.push_str("</mtd>");
                    }
                    content.push_str("</mtr>");
                }

                let (open, close) = self.bracket_to_mathml(*bracket_type);

                format!(
                    "<mrow><mo>{}</mo><mtable>{}</mtable><mo>{}</mo></mrow>",
                    open, content, close
                )
            }

            MathNode::Group {
                content,
                bracket_type,
            } => {
                let content_ml = self.generate_node(content);
                let (open, close) = self.bracket_to_mathml(*bracket_type);

                if *bracket_type == BracketType::None {
                    content_ml
                } else {
                    format!(
                        "<mrow><mo>{}</mo>{}<mo>{}</mo></mrow>",
                        open, content_ml, close
                    )
                }
            }

            MathNode::LargeOp {
                op_type,
                lower,
                upper,
                content,
            } => {
                let op_ml = self.large_op_to_mathml(op_type);
                let content_ml = self.generate_node(content);

                match (lower, upper) {
                    (Some(low), Some(up)) => {
                        let low_ml = self.generate_node(low);
                        let up_ml = self.generate_node(up);
                        format!(
                            "<mrow><munderover><mo>{}</mo>{}{}</munderover>{}</mrow>",
                            op_ml, low_ml, up_ml, content_ml
                        )
                    }
                    (Some(low), None) => {
                        let low_ml = self.generate_node(low);
                        format!(
                            "<mrow><munder><mo>{}</mo>{}</munder>{}</mrow>",
                            op_ml, low_ml, content_ml
                        )
                    }
                    (None, Some(up)) => {
                        let up_ml = self.generate_node(up);
                        format!(
                            "<mrow><mover><mo>{}</mo>{}</mover>{}</mrow>",
                            op_ml, up_ml, content_ml
                        )
                    }
                    (None, None) => {
                        format!("<mrow><mo>{}</mo>{}</mrow>", op_ml, content_ml)
                    }
                }
            }

            MathNode::Sequence { elements } => {
                let mut content = String::new();
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        content.push_str("<mo>,</mo>");
                    }
                    content.push_str(&self.generate_node(elem));
                }
                format!("<mrow>{}</mrow>", content)
            }

            MathNode::Text { content } => {
                format!("<mtext>{}</mtext>", escape_xml(content))
            }

            MathNode::Empty => String::new(),
        }
    }

    /// Convert binary operator to MathML
    fn binary_op_to_mathml(&self, op: &BinaryOp) -> String {
        match op {
            BinaryOp::Add => "+".to_string(),
            BinaryOp::Subtract => "−".to_string(),
            BinaryOp::Multiply => "×".to_string(),
            BinaryOp::Divide => "÷".to_string(),
            BinaryOp::Power => "^".to_string(),
            BinaryOp::Equal => "=".to_string(),
            BinaryOp::NotEqual => "≠".to_string(),
            BinaryOp::Less => "&lt;".to_string(),
            BinaryOp::Greater => "&gt;".to_string(),
            BinaryOp::LessEqual => "≤".to_string(),
            BinaryOp::GreaterEqual => "≥".to_string(),
            BinaryOp::ApproxEqual => "≈".to_string(),
            BinaryOp::Equivalent => "≡".to_string(),
            BinaryOp::Similar => "∼".to_string(),
            BinaryOp::Congruent => "≅".to_string(),
            BinaryOp::Proportional => "∝".to_string(),
            BinaryOp::Custom(s) => s.clone(),
        }
    }

    /// Convert unary operator to MathML
    fn unary_op_to_mathml(&self, op: &UnaryOp) -> String {
        match op {
            UnaryOp::Plus => "+".to_string(),
            UnaryOp::Minus => "−".to_string(),
            UnaryOp::Not => "¬".to_string(),
            UnaryOp::Custom(s) => s.clone(),
        }
    }

    /// Convert large operator to MathML
    fn large_op_to_mathml(&self, op: &LargeOpType) -> &'static str {
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
            LargeOpType::Custom(_) => "∑", // Default fallback
        }
    }

    /// Convert bracket type to MathML delimiters
    fn bracket_to_mathml(&self, bracket_type: BracketType) -> (&'static str, &'static str) {
        match bracket_type {
            BracketType::Parentheses => ("(", ")"),
            BracketType::Brackets => ("[", "]"),
            BracketType::Braces => ("{", "}"),
            BracketType::AngleBrackets => ("⟨", "⟩"),
            BracketType::Vertical => ("|", "|"),
            BracketType::DoubleVertical => ("‖", "‖"),
            BracketType::Floor => ("⌊", "⌋"),
            BracketType::Ceiling => ("⌈", "⌉"),
            BracketType::None => ("", ""),
        }
    }
}

impl Default for MathMLGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number() {
        let expr = MathExpr::new(
            MathNode::Number {
                value: "42".to_string(),
                is_decimal: false,
            },
            1.0,
        );
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<mn>42</mn>"));
    }

    #[test]
    fn test_symbol() {
        let expr = MathExpr::new(
            MathNode::Symbol {
                value: "x".to_string(),
                unicode: Some('x'),
            },
            1.0,
        );
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<mi>x</mi>"));
    }

    #[test]
    fn test_binary_add() {
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
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<mrow>"));
        assert!(result.contains("<mo>+</mo>"));
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
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<mfrac>"));
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
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<msqrt>"));
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
        let gen = MathMLGenerator::new();
        let result = gen.generate(&expr);
        assert!(result.contains("<msup>"));
    }

    #[test]
    fn test_xml_escaping() {
        assert_eq!(escape_xml("a < b"), "a &lt; b");
        assert_eq!(escape_xml("x & y"), "x &amp; y");
    }
}
