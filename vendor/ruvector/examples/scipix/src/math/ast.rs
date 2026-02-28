//! Abstract Syntax Tree definitions for mathematical expressions
//!
//! This module defines the complete AST structure for representing mathematical
//! expressions including symbols, operators, fractions, matrices, and more.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A complete mathematical expression with confidence score
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathExpr {
    /// Root node of the expression tree
    pub root: MathNode,
    /// Confidence score (0.0 to 1.0) from OCR recognition
    pub confidence: f32,
}

impl MathExpr {
    /// Create a new mathematical expression
    pub fn new(root: MathNode, confidence: f32) -> Self {
        Self { root, confidence }
    }

    /// Accept a visitor for tree traversal
    pub fn accept<V: MathVisitor>(&self, visitor: &mut V) {
        self.root.accept(visitor);
    }
}

/// Main AST node representing any mathematical construct
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathNode {
    /// A mathematical symbol (variable, Greek letter, operator)
    Symbol {
        value: String,
        unicode: Option<char>,
    },

    /// A numeric value
    Number {
        value: String,
        /// Whether this is part of a decimal number
        is_decimal: bool,
    },

    /// Binary operation (a op b)
    Binary {
        op: BinaryOp,
        left: Box<MathNode>,
        right: Box<MathNode>,
    },

    /// Unary operation (op a)
    Unary { op: UnaryOp, operand: Box<MathNode> },

    /// Fraction (numerator / denominator)
    Fraction {
        numerator: Box<MathNode>,
        denominator: Box<MathNode>,
    },

    /// Radical (√, ∛, etc.)
    Radical {
        /// Index of the radical (2 for square root, 3 for cube root, etc.)
        index: Option<Box<MathNode>>,
        radicand: Box<MathNode>,
    },

    /// Subscript or superscript
    Script {
        base: Box<MathNode>,
        subscript: Option<Box<MathNode>>,
        superscript: Option<Box<MathNode>>,
    },

    /// Function application (sin, cos, log, etc.)
    Function {
        name: String,
        argument: Box<MathNode>,
    },

    /// Matrix or vector
    Matrix {
        rows: Vec<Vec<MathNode>>,
        bracket_type: BracketType,
    },

    /// Grouped expression with delimiters
    Group {
        content: Box<MathNode>,
        bracket_type: BracketType,
    },

    /// Large operators (∑, ∫, ∏, etc.)
    LargeOp {
        op_type: LargeOpType,
        lower: Option<Box<MathNode>>,
        upper: Option<Box<MathNode>>,
        content: Box<MathNode>,
    },

    /// Sequence of expressions (e.g., function arguments)
    Sequence { elements: Vec<MathNode> },

    /// Text annotation in math mode
    Text { content: String },

    /// Empty/placeholder node
    Empty,
}

impl MathNode {
    /// Accept a visitor for tree traversal
    pub fn accept<V: MathVisitor>(&self, visitor: &mut V) {
        visitor.visit(self);
        match self {
            MathNode::Binary { left, right, .. } => {
                left.accept(visitor);
                right.accept(visitor);
            }
            MathNode::Unary { operand, .. } => {
                operand.accept(visitor);
            }
            MathNode::Fraction {
                numerator,
                denominator,
            } => {
                numerator.accept(visitor);
                denominator.accept(visitor);
            }
            MathNode::Radical { index, radicand } => {
                if let Some(idx) = index {
                    idx.accept(visitor);
                }
                radicand.accept(visitor);
            }
            MathNode::Script {
                base,
                subscript,
                superscript,
            } => {
                base.accept(visitor);
                if let Some(sub) = subscript {
                    sub.accept(visitor);
                }
                if let Some(sup) = superscript {
                    sup.accept(visitor);
                }
            }
            MathNode::Function { argument, .. } => {
                argument.accept(visitor);
            }
            MathNode::Matrix { rows, .. } => {
                for row in rows {
                    for elem in row {
                        elem.accept(visitor);
                    }
                }
            }
            MathNode::Group { content, .. } => {
                content.accept(visitor);
            }
            MathNode::LargeOp {
                lower,
                upper,
                content,
                ..
            } => {
                if let Some(l) = lower {
                    l.accept(visitor);
                }
                if let Some(u) = upper {
                    u.accept(visitor);
                }
                content.accept(visitor);
            }
            MathNode::Sequence { elements } => {
                for elem in elements {
                    elem.accept(visitor);
                }
            }
            _ => {}
        }
    }
}

/// Binary operators
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    ApproxEqual,
    Equivalent,
    Similar,
    Congruent,
    Proportional,
    /// Custom operator with LaTeX representation
    Custom(String),
}

impl BinaryOp {
    /// Get precedence level (higher = binds tighter)
    pub fn precedence(&self) -> u8 {
        match self {
            BinaryOp::Power => 60,
            BinaryOp::Multiply | BinaryOp::Divide => 50,
            BinaryOp::Add | BinaryOp::Subtract => 40,
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessEqual
            | BinaryOp::GreaterEqual
            | BinaryOp::ApproxEqual
            | BinaryOp::Equivalent
            | BinaryOp::Similar
            | BinaryOp::Congruent
            | BinaryOp::Proportional => 30,
            BinaryOp::Custom(_) => 35,
        }
    }

    /// Check if operator is left-associative
    pub fn is_left_associative(&self) -> bool {
        !matches!(self, BinaryOp::Power)
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Subtract => write!(f, "-"),
            BinaryOp::Multiply => write!(f, "×"),
            BinaryOp::Divide => write!(f, "÷"),
            BinaryOp::Power => write!(f, "^"),
            BinaryOp::Equal => write!(f, "="),
            BinaryOp::NotEqual => write!(f, "≠"),
            BinaryOp::Less => write!(f, "<"),
            BinaryOp::Greater => write!(f, ">"),
            BinaryOp::LessEqual => write!(f, "≤"),
            BinaryOp::GreaterEqual => write!(f, "≥"),
            BinaryOp::ApproxEqual => write!(f, "≈"),
            BinaryOp::Equivalent => write!(f, "≡"),
            BinaryOp::Similar => write!(f, "∼"),
            BinaryOp::Congruent => write!(f, "≅"),
            BinaryOp::Proportional => write!(f, "∝"),
            BinaryOp::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Unary operators
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    /// Custom unary operator
    Custom(String),
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Plus => write!(f, "+"),
            UnaryOp::Minus => write!(f, "-"),
            UnaryOp::Not => write!(f, "¬"),
            UnaryOp::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Large operator types (∑, ∫, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LargeOpType {
    Sum,             // ∑
    Product,         // ∏
    Integral,        // ∫
    DoubleIntegral,  // ∬
    TripleIntegral,  // ∭
    ContourIntegral, // ∮
    Union,           // ⋃
    Intersection,    // ⋂
    Coproduct,       // ∐
    DirectSum,       // ⊕
    Custom(String),
}

impl fmt::Display for LargeOpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LargeOpType::Sum => write!(f, "∑"),
            LargeOpType::Product => write!(f, "∏"),
            LargeOpType::Integral => write!(f, "∫"),
            LargeOpType::DoubleIntegral => write!(f, "∬"),
            LargeOpType::TripleIntegral => write!(f, "∭"),
            LargeOpType::ContourIntegral => write!(f, "∮"),
            LargeOpType::Union => write!(f, "⋃"),
            LargeOpType::Intersection => write!(f, "⋂"),
            LargeOpType::Coproduct => write!(f, "∐"),
            LargeOpType::DirectSum => write!(f, "⊕"),
            LargeOpType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Bracket types for grouping and matrices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BracketType {
    Parentheses,    // ( )
    Brackets,       // [ ]
    Braces,         // { }
    AngleBrackets,  // ⟨ ⟩
    Vertical,       // | |
    DoubleVertical, // ‖ ‖
    Floor,          // ⌊ ⌋
    Ceiling,        // ⌈ ⌉
    None,           // No brackets
}

impl BracketType {
    /// Get opening delimiter
    pub fn opening(&self) -> &str {
        match self {
            BracketType::Parentheses => "(",
            BracketType::Brackets => "[",
            BracketType::Braces => "{",
            BracketType::AngleBrackets => "⟨",
            BracketType::Vertical => "|",
            BracketType::DoubleVertical => "‖",
            BracketType::Floor => "⌊",
            BracketType::Ceiling => "⌈",
            BracketType::None => "",
        }
    }

    /// Get closing delimiter
    pub fn closing(&self) -> &str {
        match self {
            BracketType::Parentheses => ")",
            BracketType::Brackets => "]",
            BracketType::Braces => "}",
            BracketType::AngleBrackets => "⟩",
            BracketType::Vertical => "|",
            BracketType::DoubleVertical => "‖",
            BracketType::Floor => "⌋",
            BracketType::Ceiling => "⌉",
            BracketType::None => "",
        }
    }
}

/// Visitor pattern for traversing the AST
pub trait MathVisitor {
    fn visit(&mut self, node: &MathNode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_op_precedence() {
        assert!(BinaryOp::Power.precedence() > BinaryOp::Multiply.precedence());
        assert!(BinaryOp::Multiply.precedence() > BinaryOp::Add.precedence());
        assert!(BinaryOp::Add.precedence() > BinaryOp::Equal.precedence());
    }

    #[test]
    fn test_binary_op_associativity() {
        assert!(BinaryOp::Add.is_left_associative());
        assert!(BinaryOp::Multiply.is_left_associative());
        assert!(!BinaryOp::Power.is_left_associative());
    }

    #[test]
    fn test_bracket_delimiters() {
        assert_eq!(BracketType::Parentheses.opening(), "(");
        assert_eq!(BracketType::Parentheses.closing(), ")");
        assert_eq!(BracketType::Brackets.opening(), "[");
        assert_eq!(BracketType::Braces.closing(), "}");
    }

    #[test]
    fn test_math_expr_creation() {
        let expr = MathExpr::new(
            MathNode::Number {
                value: "42".to_string(),
                is_decimal: false,
            },
            0.95,
        );
        assert_eq!(expr.confidence, 0.95);
    }

    #[test]
    fn test_visitor_pattern() {
        struct CountVisitor {
            count: usize,
        }

        impl MathVisitor for CountVisitor {
            fn visit(&mut self, _node: &MathNode) {
                self.count += 1;
            }
        }

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

        let mut visitor = CountVisitor { count: 0 };
        expr.accept(&mut visitor);
        assert_eq!(visitor.count, 3); // Binary + 2 numbers
    }
}
