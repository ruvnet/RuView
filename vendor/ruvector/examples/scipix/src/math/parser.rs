//! Mathematical expression parser
//!
//! This module parses mathematical expressions from various formats
//! including LaTeX, Unicode text, and symbolic notation.

use crate::math::ast::{BinaryOp, BracketType, LargeOpType, MathExpr, MathNode, UnaryOp};
use crate::math::symbols::get_symbol;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{alpha1, char, digit1, multispace0},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

/// Parser for mathematical expressions
pub struct Parser {
    /// Confidence score for parsed expression
    confidence: f32,
}

impl Parser {
    /// Create a new parser
    pub fn new() -> Self {
        Self { confidence: 1.0 }
    }

    /// Parse a mathematical expression from string
    pub fn parse(&mut self, input: &str) -> Result<MathExpr, String> {
        match self.parse_expression(input) {
            Ok((_, node)) => Ok(MathExpr::new(node, self.confidence)),
            Err(e) => Err(format!("Parse error: {:?}", e)),
        }
    }

    /// Parse top-level expression
    fn parse_expression<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        self.parse_relational(input)
    }

    /// Parse relational operators (=, <, >, etc.)
    fn parse_relational<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, left) = self.parse_additive(input)?;
        let (input, op_right) = opt(pair(
            delimited(
                multispace0,
                alt((
                    map(tag("=="), |_| BinaryOp::Equal),
                    map(tag("="), |_| BinaryOp::Equal),
                    map(tag("!="), |_| BinaryOp::NotEqual),
                    map(tag("≠"), |_| BinaryOp::NotEqual),
                    map(tag("<="), |_| BinaryOp::LessEqual),
                    map(tag("≤"), |_| BinaryOp::LessEqual),
                    map(tag(">="), |_| BinaryOp::GreaterEqual),
                    map(tag("≥"), |_| BinaryOp::GreaterEqual),
                    map(tag("<"), |_| BinaryOp::Less),
                    map(tag(">"), |_| BinaryOp::Greater),
                    map(tag("≈"), |_| BinaryOp::ApproxEqual),
                    map(tag("≡"), |_| BinaryOp::Equivalent),
                    map(tag("∼"), |_| BinaryOp::Similar),
                )),
                multispace0,
            ),
            |i| self.parse_additive(i),
        ))(input)?;

        Ok((
            input,
            if let Some((op, right)) = op_right {
                MathNode::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            } else {
                left
            },
        ))
    }

    /// Parse additive operators (+, -)
    fn parse_additive<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, left) = self.parse_multiplicative(input)?;
        let (input, ops) = many0(pair(
            delimited(
                multispace0,
                alt((
                    map(char('+'), |_| BinaryOp::Add),
                    map(char('-'), |_| BinaryOp::Subtract),
                )),
                multispace0,
            ),
            |i| self.parse_multiplicative(i),
        ))(input)?;

        Ok((
            input,
            ops.into_iter()
                .fold(left, |acc, (op, right)| MathNode::Binary {
                    op,
                    left: Box::new(acc),
                    right: Box::new(right),
                }),
        ))
    }

    /// Parse multiplicative operators (*, /, ×, ÷)
    fn parse_multiplicative<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, left) = self.parse_power(input)?;
        let (input, ops) = many0(pair(
            delimited(
                multispace0,
                alt((
                    map(char('*'), |_| BinaryOp::Multiply),
                    map(char('/'), |_| BinaryOp::Divide),
                    map(char('×'), |_| BinaryOp::Multiply),
                    map(char('÷'), |_| BinaryOp::Divide),
                    map(tag("\\times"), |_| BinaryOp::Multiply),
                    map(tag("\\div"), |_| BinaryOp::Divide),
                    map(tag("\\cdot"), |_| BinaryOp::Multiply),
                )),
                multispace0,
            ),
            |i| self.parse_power(i),
        ))(input)?;

        Ok((
            input,
            ops.into_iter()
                .fold(left, |acc, (op, right)| MathNode::Binary {
                    op,
                    left: Box::new(acc),
                    right: Box::new(right),
                }),
        ))
    }

    /// Parse power operator (^)
    fn parse_power<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, base) = self.parse_unary(input)?;
        let (input, exp) = opt(preceded(
            delimited(multispace0, char('^'), multispace0),
            |i| self.parse_unary(i),
        ))(input)?;

        Ok((
            input,
            if let Some(exponent) = exp {
                MathNode::Binary {
                    op: BinaryOp::Power,
                    left: Box::new(base),
                    right: Box::new(exponent),
                }
            } else {
                base
            },
        ))
    }

    /// Parse unary operators (+, -)
    fn parse_unary<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        alt((
            map(
                pair(
                    delimited(
                        multispace0,
                        alt((
                            map(char('+'), |_| UnaryOp::Plus),
                            map(char('-'), |_| UnaryOp::Minus),
                        )),
                        multispace0,
                    ),
                    |i| self.parse_script(i),
                ),
                |(op, operand)| MathNode::Unary {
                    op,
                    operand: Box::new(operand),
                },
            ),
            |i| self.parse_script(i),
        ))(input)
    }

    /// Parse subscript/superscript
    fn parse_script<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, base) = self.parse_primary(input)?;
        let (input, sub) = opt(preceded(char('_'), |i| self.parse_script_content(i)))(input)?;
        let (input, sup) = opt(preceded(char('^'), |i| self.parse_script_content(i)))(input)?;

        Ok((
            input,
            if sub.is_some() || sup.is_some() {
                MathNode::Script {
                    base: Box::new(base),
                    subscript: sub.map(Box::new),
                    superscript: sup.map(Box::new),
                }
            } else {
                base
            },
        ))
    }

    /// Parse script content (single char or braced expression)
    fn parse_script_content<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        alt((
            delimited(char('{'), |i| self.parse_expression(i), char('}')),
            map(recognize(alpha1), |s: &str| MathNode::Symbol {
                value: s.to_string(),
                unicode: s.chars().next(),
            }),
            map(digit1, |s: &str| MathNode::Number {
                value: s.to_string(),
                is_decimal: false,
            }),
        ))(input)
    }

    /// Parse primary expressions (atoms)
    fn parse_primary<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        delimited(
            multispace0,
            alt((
                |i| self.parse_function(i),
                |i| self.parse_fraction(i),
                |i| self.parse_radical(i),
                |i| self.parse_large_op(i),
                |i| self.parse_greek(i),
                |i| self.parse_number(i),
                |i| self.parse_symbol(i),
                |i| self.parse_grouped(i),
            )),
            multispace0,
        )(input)
    }

    /// Parse fraction (\frac{a}{b})
    fn parse_fraction<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, _) = tag("\\frac")(input)?;
        let (input, num) = delimited(char('{'), |i| self.parse_expression(i), char('}'))(input)?;
        let (input, den) = delimited(char('{'), |i| self.parse_expression(i), char('}'))(input)?;

        Ok((
            input,
            MathNode::Fraction {
                numerator: Box::new(num),
                denominator: Box::new(den),
            },
        ))
    }

    /// Parse radical (\sqrt[n]{x})
    fn parse_radical<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, _) = tag("\\sqrt")(input)?;
        let (input, index) = opt(delimited(
            char('['),
            |i| self.parse_expression(i),
            char(']'),
        ))(input)?;
        let (input, radicand) =
            delimited(char('{'), |i| self.parse_expression(i), char('}'))(input)?;

        Ok((
            input,
            MathNode::Radical {
                index: index.map(Box::new),
                radicand: Box::new(radicand),
            },
        ))
    }

    /// Parse large operators (sum, integral, etc.)
    fn parse_large_op<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, op_type) = alt((
            map(tag("\\sum"), |_| LargeOpType::Sum),
            map(tag("\\prod"), |_| LargeOpType::Product),
            map(tag("\\int"), |_| LargeOpType::Integral),
            map(tag("\\iint"), |_| LargeOpType::DoubleIntegral),
            map(tag("\\iiint"), |_| LargeOpType::TripleIntegral),
            map(tag("\\oint"), |_| LargeOpType::ContourIntegral),
            map(tag("∑"), |_| LargeOpType::Sum),
            map(tag("∏"), |_| LargeOpType::Product),
            map(tag("∫"), |_| LargeOpType::Integral),
        ))(input)?;

        let (input, lower) = opt(preceded(
            char('_'),
            alt((
                delimited(char('{'), |i| self.parse_expression(i), char('}')),
                |i| self.parse_primary(i),
            )),
        ))(input)?;

        let (input, upper) = opt(preceded(
            char('^'),
            alt((
                delimited(char('{'), |i| self.parse_expression(i), char('}')),
                |i| self.parse_primary(i),
            )),
        ))(input)?;

        let (input, content) = self.parse_primary(input)?;

        Ok((
            input,
            MathNode::LargeOp {
                op_type,
                lower: lower.map(Box::new),
                upper: upper.map(Box::new),
                content: Box::new(content),
            },
        ))
    }

    /// Parse function (sin, cos, etc.)
    fn parse_function<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, _) = char('\\')(input)?;
        let (input, name) = alpha1(input)?;
        let (input, _) = multispace0(input)?;
        let (input, arg) = self.parse_primary(input)?;

        Ok((
            input,
            MathNode::Function {
                name: name.to_string(),
                argument: Box::new(arg),
            },
        ))
    }

    /// Parse Greek letter
    fn parse_greek<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, _) = char('\\')(input)?;
        let (input, name) = alpha1(input)?;

        // Convert LaTeX name to Unicode if possible
        let unicode = match name {
            "alpha" => Some('α'),
            "beta" => Some('β'),
            "gamma" => Some('γ'),
            "delta" => Some('δ'),
            "epsilon" => Some('ε'),
            "pi" => Some('π'),
            "theta" => Some('θ'),
            "lambda" => Some('λ'),
            "mu" => Some('μ'),
            "sigma" => Some('σ'),
            _ => None,
        };

        Ok((
            input,
            MathNode::Symbol {
                value: name.to_string(),
                unicode,
            },
        ))
    }

    /// Parse number
    fn parse_number<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        let (input, num_str) = recognize(pair(digit1, opt(pair(char('.'), digit1))))(input)?;
        let is_decimal = num_str.contains('.');

        Ok((
            input,
            MathNode::Number {
                value: num_str.to_string(),
                is_decimal,
            },
        ))
    }

    /// Parse symbol (variable)
    fn parse_symbol<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        map(take_while1(|c: char| c.is_alphabetic()), |s: &str| {
            let c = s.chars().next();
            MathNode::Symbol {
                value: s.to_string(),
                unicode: c,
            }
        })(input)
    }

    /// Parse grouped expression (parentheses)
    fn parse_grouped<'a>(&self, input: &'a str) -> IResult<&'a str, MathNode> {
        delimited(char('('), |i| self.parse_expression(i), char(')'))(input)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a mathematical expression from string
pub fn parse_expression(input: &str) -> Result<MathExpr, String> {
    Parser::new().parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number() {
        let expr = parse_expression("42").unwrap();
        match expr.root {
            MathNode::Number { value, .. } => assert_eq!(value, "42"),
            _ => panic!("Expected Number node"),
        }
    }

    #[test]
    fn test_parse_addition() {
        let expr = parse_expression("1 + 2").unwrap();
        match expr.root {
            MathNode::Binary { op, .. } => assert_eq!(op, BinaryOp::Add),
            _ => panic!("Expected Binary node"),
        }
    }

    #[test]
    fn test_parse_multiplication() {
        let expr = parse_expression("3 * 4").unwrap();
        match expr.root {
            MathNode::Binary { op, .. } => assert_eq!(op, BinaryOp::Multiply),
            _ => panic!("Expected Binary node"),
        }
    }

    #[test]
    fn test_parse_precedence() {
        let expr = parse_expression("1 + 2 * 3").unwrap();
        // Should parse as 1 + (2 * 3)
        match expr.root {
            MathNode::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                assert!(matches!(*left, MathNode::Number { .. }));
                assert!(matches!(
                    *right,
                    MathNode::Binary {
                        op: BinaryOp::Multiply,
                        ..
                    }
                ));
            }
            _ => panic!("Expected Add with Multiply on right"),
        }
    }

    #[test]
    fn test_parse_power() {
        let expr = parse_expression("x^2").unwrap();
        match expr.root {
            MathNode::Binary { op, .. } => assert_eq!(op, BinaryOp::Power),
            _ => panic!("Expected Binary node with power"),
        }
    }

    #[test]
    fn test_parse_fraction() {
        let expr = parse_expression("\\frac{1}{2}").unwrap();
        match expr.root {
            MathNode::Fraction { .. } => {}
            _ => panic!("Expected Fraction node"),
        }
    }

    #[test]
    fn test_parse_sqrt() {
        let expr = parse_expression("\\sqrt{2}").unwrap();
        match expr.root {
            MathNode::Radical { index, .. } => assert!(index.is_none()),
            _ => panic!("Expected Radical node"),
        }
    }

    #[test]
    fn test_parse_nth_root() {
        let expr = parse_expression("\\sqrt[3]{8}").unwrap();
        match expr.root {
            MathNode::Radical { index, .. } => assert!(index.is_some()),
            _ => panic!("Expected Radical node with index"),
        }
    }

    #[test]
    fn test_parse_subscript() {
        let expr = parse_expression("a_n").unwrap();
        match expr.root {
            MathNode::Script { subscript, .. } => assert!(subscript.is_some()),
            _ => panic!("Expected Script node"),
        }
    }

    #[test]
    fn test_parse_superscript() {
        let expr = parse_expression("x^2").unwrap();
        match expr.root {
            MathNode::Binary { op, .. } => assert_eq!(op, BinaryOp::Power),
            _ => panic!("Expected power operation"),
        }
    }

    #[test]
    fn test_parse_sum() {
        let expr = parse_expression("\\sum_{i=1}^{n} i").unwrap();
        match expr.root {
            MathNode::LargeOp { op_type, .. } => assert_eq!(op_type, LargeOpType::Sum),
            _ => panic!("Expected LargeOp node"),
        }
    }

    #[test]
    fn test_parse_complex() {
        let expr = parse_expression("\\frac{-b + \\sqrt{b^2 - 4ac}}{2a}").unwrap();
        match expr.root {
            MathNode::Fraction { .. } => {}
            _ => panic!("Expected Fraction node"),
        }
    }
}
