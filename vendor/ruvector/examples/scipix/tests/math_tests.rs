//! Comprehensive integration tests for mathematical expression parsing and conversion
//!
//! These tests cover complex mathematical expressions including:
//! - Fractions and nested fractions
//! - Radicals (square roots, nth roots)
//! - Powers and exponents
//! - Matrices and vectors
//! - Integrals and summations
//! - Trigonometric functions
//! - Greek letters and special symbols
//! - Complex nested expressions
//!
//! NOTE: These tests require the `math` feature to be enabled.
//! Run with: cargo test --features math

#![cfg(feature = "math")]

use ruvector_scipix::math::{
    parse_expression, to_asciimath, to_latex, to_mathml, AsciiMathGenerator, BinaryOp, BracketType,
    LaTeXConfig, LaTeXGenerator, LargeOpType, MathExpr, MathNode,
};

#[test]
fn test_quadratic_formula() {
    // The famous quadratic formula: x = (-b ± √(b² - 4ac)) / 2a
    let latex_input = r"\frac{-b + \sqrt{b^2 - 4*a*c}}{2*a}";
    let expr = parse_expression(latex_input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\frac"));
    assert!(latex.contains(r"\sqrt"));
    assert!(latex.contains("b"));
    assert!(latex.contains("a"));
    assert!(latex.contains("c"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mfrac>"));
    assert!(mathml.contains("<msqrt>"));

    let asciimath = to_asciimath(&expr);
    assert!(asciimath.contains("sqrt"));
    assert!(asciimath.contains("/"));
}

#[test]
fn test_pythagorean_theorem() {
    // a² + b² = c²
    let input = "a^2 + b^2 = c^2";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains("a^{2}"));
    assert!(latex.contains("b^{2}"));
    assert!(latex.contains("c^{2}"));
    assert!(latex.contains("="));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<msup>"));
    assert!(mathml.contains("<mo>=</mo>"));
}

#[test]
fn test_euler_identity() {
    // e^(iπ) + 1 = 0
    let input = "e^{i*\\pi} + 1 = 0";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains("e^"));
    assert!(latex.contains("\\pi"));
}

#[test]
fn test_nested_fractions() {
    // Complex continued fraction
    let input = r"\frac{1}{\frac{2}{\frac{3}{4}}}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    // Should contain multiple \frac commands
    assert!(latex.matches(r"\frac").count() >= 3);

    let mathml = to_mathml(&expr);
    assert!(mathml.matches("<mfrac>").count() >= 3);
}

#[test]
fn test_matrix_2x2() {
    // 2x2 matrix
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

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\begin{bmatrix}"));
    assert!(latex.contains(r"\end{bmatrix}"));
    assert!(latex.contains("&"));
    assert!(latex.contains(r"\\"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mtable>"));
    assert!(mathml.contains("<mtr>"));
    assert!(mathml.contains("<mtd>"));

    let asciimath = to_asciimath(&expr);
    assert!(asciimath.contains("["));
    assert!(asciimath.contains(";"));
}

#[test]
fn test_matrix_3x3() {
    // 3x3 identity matrix
    let expr = MathExpr::new(
        MathNode::Matrix {
            rows: vec![
                vec![
                    MathNode::Number {
                        value: "1".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                ],
                vec![
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "1".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                ],
                vec![
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "0".to_string(),
                        is_decimal: false,
                    },
                    MathNode::Number {
                        value: "1".to_string(),
                        is_decimal: false,
                    },
                ],
            ],
            bracket_type: BracketType::Parentheses,
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\begin{pmatrix}"));
    assert!(latex.matches(r"\\").count() >= 2);
}

#[test]
fn test_definite_integral() {
    // ∫₀¹ x² dx
    let expr = MathExpr::new(
        MathNode::LargeOp {
            op_type: LargeOpType::Integral,
            lower: Some(Box::new(MathNode::Number {
                value: "0".to_string(),
                is_decimal: false,
            })),
            upper: Some(Box::new(MathNode::Number {
                value: "1".to_string(),
                is_decimal: false,
            })),
            content: Box::new(MathNode::Binary {
                op: BinaryOp::Power,
                left: Box::new(MathNode::Symbol {
                    value: "x".to_string(),
                    unicode: Some('x'),
                }),
                right: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
            }),
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\int"));
    assert!(latex.contains("_{0}"));
    assert!(latex.contains("^{1}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<munderover>"));
    assert!(mathml.contains("∫"));
}

#[test]
fn test_summation() {
    // ∑_{i=1}^{n} i²
    let expr = MathExpr::new(
        MathNode::LargeOp {
            op_type: LargeOpType::Sum,
            lower: Some(Box::new(MathNode::Binary {
                op: BinaryOp::Equal,
                left: Box::new(MathNode::Symbol {
                    value: "i".to_string(),
                    unicode: Some('i'),
                }),
                right: Box::new(MathNode::Number {
                    value: "1".to_string(),
                    is_decimal: false,
                }),
            })),
            upper: Some(Box::new(MathNode::Symbol {
                value: "n".to_string(),
                unicode: Some('n'),
            })),
            content: Box::new(MathNode::Binary {
                op: BinaryOp::Power,
                left: Box::new(MathNode::Symbol {
                    value: "i".to_string(),
                    unicode: Some('i'),
                }),
                right: Box::new(MathNode::Number {
                    value: "2".to_string(),
                    is_decimal: false,
                }),
            }),
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\sum"));
    assert!(latex.contains("_{i"));
    assert!(latex.contains("^{n}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("∑"));
    assert!(mathml.contains("<munderover>"));
}

#[test]
fn test_product_notation() {
    // ∏_{k=1}^{n} k
    let expr = MathExpr::new(
        MathNode::LargeOp {
            op_type: LargeOpType::Product,
            lower: Some(Box::new(MathNode::Binary {
                op: BinaryOp::Equal,
                left: Box::new(MathNode::Symbol {
                    value: "k".to_string(),
                    unicode: Some('k'),
                }),
                right: Box::new(MathNode::Number {
                    value: "1".to_string(),
                    is_decimal: false,
                }),
            })),
            upper: Some(Box::new(MathNode::Symbol {
                value: "n".to_string(),
                unicode: Some('n'),
            })),
            content: Box::new(MathNode::Symbol {
                value: "k".to_string(),
                unicode: Some('k'),
            }),
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\prod"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("∏"));
}

#[test]
fn test_nth_root() {
    // ∛8 (cube root)
    let input = r"\sqrt[3]{8}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\sqrt[3]"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mroot>"));
}

#[test]
fn test_complex_radical() {
    // √(a² + b²)
    let input = r"\sqrt{a^2 + b^2}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\sqrt"));
    assert!(latex.contains("a^{2}"));
    assert!(latex.contains("b^{2}"));
}

#[test]
fn test_binomial_coefficient() {
    // (n choose k) represented as fraction
    let input = r"\frac{n}{k}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\frac{n}{k}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mfrac>"));
    assert!(mathml.contains("<mi>n</mi>"));
    assert!(mathml.contains("<mi>k</mi>"));
}

#[test]
fn test_trigonometric_functions() {
    // sin²(x) + cos²(x) = 1
    let input = r"\sin{x}^2 + \cos{x}^2 = 1";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\sin"));
    assert!(latex.contains(r"\cos"));
}

#[test]
fn test_limits() {
    // lim_{x→∞} (1 + 1/x)^x = e
    // Simplified: testing basic limit structure
    let input = r"\sum_{x=1}^{10} x";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\sum"));
}

#[test]
fn test_greek_letters() {
    // α + β + γ = δ
    let input = r"\alpha + \beta + \gamma = \delta";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\alpha"));
    assert!(latex.contains(r"\beta"));
    assert!(latex.contains(r"\gamma"));
    assert!(latex.contains(r"\delta"));
}

#[test]
fn test_subscript_and_superscript() {
    // a₁² + a₂² = a₃²
    let input = "a_1^2 + a_2^2 = a_3^2";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains("a_{1}^{2}"));
    assert!(latex.contains("a_{2}^{2}"));
    assert!(latex.contains("a_{3}^{2}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<msubsup>"));
}

#[test]
fn test_operator_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let input = "1 + 2 * 3";
    let expr = parse_expression(input).unwrap();

    match expr.root {
        MathNode::Binary {
            op: BinaryOp::Add,
            right,
            ..
        } => {
            assert!(matches!(
                *right,
                MathNode::Binary {
                    op: BinaryOp::Multiply,
                    ..
                }
            ));
        }
        _ => panic!("Expected addition with multiplication on right"),
    }

    let latex = to_latex(&expr);
    // Should not have unnecessary parentheses around 2 * 3
    assert!(!latex.contains("(2"));
}

#[test]
fn test_parentheses_grouping() {
    // (1 + 2) * 3 should parse as (1 + 2) * 3
    let input = "(1 + 2) * 3";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    // The addition should be grouped
    assert!(latex.contains("1 + 2"));
}

#[test]
fn test_complex_nested_expression() {
    // Complex expression with multiple levels of nesting
    let input = r"\frac{\sqrt{a + b}}{c^2 - d^2}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\frac"));
    assert!(latex.contains(r"\sqrt"));
    assert!(latex.contains("c^{2}"));
    assert!(latex.contains("d^{2}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mfrac>"));
    assert!(mathml.contains("<msqrt>"));
    assert!(mathml.contains("<msup>"));
}

#[test]
fn test_latex_config_display_style() {
    let expr = parse_expression(r"\frac{1}{2}").unwrap();

    let config_inline = LaTeXConfig {
        display_style: false,
        auto_size_delimiters: true,
        spacing: true,
    };

    let config_display = LaTeXConfig {
        display_style: true,
        auto_size_delimiters: true,
        spacing: true,
    };

    let gen_inline = LaTeXGenerator::with_config(config_inline);
    let gen_display = LaTeXGenerator::with_config(config_display);

    let latex_inline = gen_inline.generate(&expr);
    let latex_display = gen_display.generate(&expr);

    // Both should contain \frac
    assert!(latex_inline.contains(r"\frac"));
    assert!(latex_display.contains(r"\frac"));
}

#[test]
fn test_latex_config_no_auto_size() {
    let expr = MathExpr::new(
        MathNode::Group {
            content: Box::new(MathNode::Number {
                value: "42".to_string(),
                is_decimal: false,
            }),
            bracket_type: BracketType::Parentheses,
        },
        1.0,
    );

    let config_auto = LaTeXConfig {
        display_style: false,
        auto_size_delimiters: true,
        spacing: true,
    };

    let config_no_auto = LaTeXConfig {
        display_style: false,
        auto_size_delimiters: false,
        spacing: true,
    };

    let gen_auto = LaTeXGenerator::with_config(config_auto);
    let gen_no_auto = LaTeXGenerator::with_config(config_no_auto);

    let latex_auto = gen_auto.generate(&expr);
    let latex_no_auto = gen_no_auto.generate(&expr);

    assert!(latex_auto.contains(r"\left("));
    assert!(!latex_no_auto.contains(r"\left("));
}

#[test]
fn test_asciimath_unicode_vs_ascii() {
    let expr = parse_expression("2 * 3").unwrap();

    let gen_unicode = AsciiMathGenerator::new();
    let gen_ascii = AsciiMathGenerator::ascii_only();

    let unicode_output = gen_unicode.generate(&expr);
    let ascii_output = gen_ascii.generate(&expr);

    assert!(unicode_output.contains("×") || unicode_output.contains("*"));
    assert!(ascii_output.contains("*"));
}

#[test]
fn test_double_integral() {
    let expr = MathExpr::new(
        MathNode::LargeOp {
            op_type: LargeOpType::DoubleIntegral,
            lower: None,
            upper: None,
            content: Box::new(MathNode::Symbol {
                value: "f".to_string(),
                unicode: Some('f'),
            }),
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\iint"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("∬"));
}

#[test]
fn test_triple_integral() {
    let expr = MathExpr::new(
        MathNode::LargeOp {
            op_type: LargeOpType::TripleIntegral,
            lower: None,
            upper: None,
            content: Box::new(MathNode::Symbol {
                value: "f".to_string(),
                unicode: Some('f'),
            }),
        },
        1.0,
    );

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\iiint"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("∭"));
}

#[test]
fn test_decimal_numbers() {
    let input = "3.14159";
    let expr = parse_expression(input).unwrap();

    match expr.root {
        MathNode::Number { value, is_decimal } => {
            assert_eq!(value, "3.14159");
            assert!(is_decimal);
        }
        _ => panic!("Expected decimal number"),
    }

    let latex = to_latex(&expr);
    assert!(latex.contains("3.14159"));
}

#[test]
fn test_large_expression() {
    // Test a realistically complex expression
    let input = r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}";
    let expr = parse_expression(input).unwrap();

    let latex = to_latex(&expr);
    assert!(latex.contains(r"\frac"));
    assert!(latex.contains(r"\sqrt"));
    assert!(latex.contains("b^{2}"));

    let mathml = to_mathml(&expr);
    assert!(mathml.contains("<mfrac>"));
    assert!(mathml.contains("<msqrt>"));

    let asciimath = to_asciimath(&expr);
    assert!(asciimath.contains("sqrt"));
    assert!(asciimath.contains("/"));
}

#[test]
fn test_all_bracket_types() {
    for bracket_type in &[
        BracketType::Parentheses,
        BracketType::Brackets,
        BracketType::Braces,
        BracketType::Vertical,
    ] {
        let expr = MathExpr::new(
            MathNode::Group {
                content: Box::new(MathNode::Symbol {
                    value: "x".to_string(),
                    unicode: Some('x'),
                }),
                bracket_type: *bracket_type,
            },
            1.0,
        );

        let latex = to_latex(&expr);
        let mathml = to_mathml(&expr);

        // All should produce valid output
        assert!(!latex.is_empty());
        assert!(!mathml.is_empty());
    }
}
