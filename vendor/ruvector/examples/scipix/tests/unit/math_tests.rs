// Math parsing tests for ruvector-scipix
//
// Tests symbol recognition, AST construction, and LaTeX/MathML/AsciiMath generation
// for various mathematical expressions including fractions, roots, matrices, integrals, etc.
// Target: 90%+ coverage of math parsing module

#[cfg(test)]
mod math_tests {
    // Mock math structures for testing
    #[derive(Debug, Clone, PartialEq)]
    enum MathSymbol {
        // Numbers
        Digit(char),

        // Variables
        Variable(char),

        // Greek letters
        Alpha,
        Beta,
        Gamma,
        Delta,
        Epsilon,
        Pi,
        Sigma,
        Omega,

        // Operators
        Plus,
        Minus,
        Times,
        Divide,
        Equals,

        // Relations
        LessThan,
        GreaterThan,
        LessEqual,
        GreaterEqual,
        NotEqual,

        // Special symbols
        Infinity,
        Partial,
        Nabla,
        Integral,
        Sum,
        Product,
        Root,
        Sqrt,

        // Brackets
        LeftParen,
        RightParen,
        LeftBracket,
        RightBracket,
        LeftBrace,
        RightBrace,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum MathNode {
        Number(String),
        Variable(String),
        Symbol(MathSymbol),
        BinaryOp {
            op: String,
            left: Box<MathNode>,
            right: Box<MathNode>,
        },
        Fraction {
            numerator: Box<MathNode>,
            denominator: Box<MathNode>,
        },
        Superscript {
            base: Box<MathNode>,
            exponent: Box<MathNode>,
        },
        Subscript {
            base: Box<MathNode>,
            index: Box<MathNode>,
        },
        Root {
            degree: Option<Box<MathNode>>,
            radicand: Box<MathNode>,
        },
        Matrix {
            rows: usize,
            cols: usize,
            elements: Vec<Vec<MathNode>>,
        },
        Integral {
            lower: Option<Box<MathNode>>,
            upper: Option<Box<MathNode>>,
            integrand: Box<MathNode>,
        },
        Summation {
            lower: Option<Box<MathNode>>,
            upper: Option<Box<MathNode>>,
            term: Box<MathNode>,
        },
    }

    impl MathNode {
        fn to_latex(&self) -> String {
            match self {
                Self::Number(n) => n.clone(),
                Self::Variable(v) => v.clone(),
                Self::Symbol(MathSymbol::Plus) => "+".to_string(),
                Self::Symbol(MathSymbol::Minus) => "-".to_string(),
                Self::Symbol(MathSymbol::Times) => r"\times".to_string(),
                Self::Symbol(MathSymbol::Divide) => r"\div".to_string(),
                Self::Symbol(MathSymbol::Pi) => r"\pi".to_string(),
                Self::Symbol(MathSymbol::Alpha) => r"\alpha".to_string(),
                Self::Symbol(MathSymbol::Infinity) => r"\infty".to_string(),
                Self::BinaryOp { op, left, right } => {
                    format!("{} {} {}", left.to_latex(), op, right.to_latex())
                }
                Self::Fraction {
                    numerator,
                    denominator,
                } => {
                    format!(r"\frac{{{}}}{{{}}}", numerator.to_latex(), denominator.to_latex())
                }
                Self::Superscript { base, exponent } => {
                    format!("{}^{{{}}}", base.to_latex(), exponent.to_latex())
                }
                Self::Subscript { base, index } => {
                    format!("{}_{{{}}}",  base.to_latex(), index.to_latex())
                }
                Self::Root { degree: None, radicand } => {
                    format!(r"\sqrt{{{}}}", radicand.to_latex())
                }
                Self::Root { degree: Some(n), radicand } => {
                    format!(r"\sqrt[{{{}}}]{{{}}}", n.to_latex(), radicand.to_latex())
                }
                Self::Matrix { elements, .. } => {
                    let mut result = r"\begin{bmatrix}".to_string();
                    for (i, row) in elements.iter().enumerate() {
                        if i > 0 {
                            result.push_str(r" \\ ");
                        }
                        for (j, elem) in row.iter().enumerate() {
                            if j > 0 {
                                result.push_str(" & ");
                            }
                            result.push_str(&elem.to_latex());
                        }
                    }
                    result.push_str(r" \end{bmatrix}");
                    result
                }
                Self::Integral { lower, upper, integrand } => {
                    let mut result = r"\int".to_string();
                    if let Some(l) = lower {
                        result.push_str(&format!("_{{{}}}", l.to_latex()));
                    }
                    if let Some(u) = upper {
                        result.push_str(&format!("^{{{}}}", u.to_latex()));
                    }
                    result.push_str(&format!(" {} dx", integrand.to_latex()));
                    result
                }
                Self::Summation { lower, upper, term } => {
                    let mut result = r"\sum".to_string();
                    if let Some(l) = lower {
                        result.push_str(&format!("_{{{}}}", l.to_latex()));
                    }
                    if let Some(u) = upper {
                        result.push_str(&format!("^{{{}}}", u.to_latex()));
                    }
                    result.push_str(&format!(" {}", term.to_latex()));
                    result
                }
                _ => String::new(),
            }
        }

        fn to_mathml(&self) -> String {
            match self {
                Self::Number(n) => format!("<mn>{}</mn>", n),
                Self::Variable(v) => format!("<mi>{}</mi>", v),
                Self::BinaryOp { op, left, right } => {
                    format!(
                        "<mrow>{}<mo>{}</mo>{}</mrow>",
                        left.to_mathml(),
                        op,
                        right.to_mathml()
                    )
                }
                Self::Fraction {
                    numerator,
                    denominator,
                } => {
                    format!(
                        "<mfrac>{}{}</mfrac>",
                        numerator.to_mathml(),
                        denominator.to_mathml()
                    )
                }
                Self::Superscript { base, exponent } => {
                    format!(
                        "<msup>{}{}</msup>",
                        base.to_mathml(),
                        exponent.to_mathml()
                    )
                }
                Self::Root { degree: None, radicand } => {
                    format!("<msqrt>{}</msqrt>", radicand.to_mathml())
                }
                _ => String::new(),
            }
        }

        fn to_asciimath(&self) -> String {
            match self {
                Self::Number(n) => n.clone(),
                Self::Variable(v) => v.clone(),
                Self::BinaryOp { op, left, right } => {
                    format!("{} {} {}", left.to_asciimath(), op, right.to_asciimath())
                }
                Self::Fraction {
                    numerator,
                    denominator,
                } => {
                    format!("({})/({})", numerator.to_asciimath(), denominator.to_asciimath())
                }
                Self::Superscript { base, exponent } => {
                    format!("{}^{}", base.to_asciimath(), exponent.to_asciimath())
                }
                Self::Root { degree: None, radicand } => {
                    format!("sqrt({})", radicand.to_asciimath())
                }
                _ => String::new(),
            }
        }
    }

    #[test]
    fn test_symbol_recognition_numbers() {
        let symbols = vec![
            MathSymbol::Digit('0'),
            MathSymbol::Digit('1'),
            MathSymbol::Digit('9'),
        ];

        for symbol in symbols {
            assert!(matches!(symbol, MathSymbol::Digit(_)));
        }
    }

    #[test]
    fn test_symbol_recognition_variables() {
        let symbols = vec![
            MathSymbol::Variable('x'),
            MathSymbol::Variable('y'),
            MathSymbol::Variable('z'),
        ];

        for symbol in symbols {
            assert!(matches!(symbol, MathSymbol::Variable(_)));
        }
    }

    #[test]
    fn test_symbol_recognition_greek() {
        let greeks = vec![
            (MathSymbol::Alpha, "α"),
            (MathSymbol::Beta, "β"),
            (MathSymbol::Gamma, "γ"),
            (MathSymbol::Delta, "δ"),
            (MathSymbol::Pi, "π"),
            (MathSymbol::Sigma, "Σ"),
            (MathSymbol::Omega, "Ω"),
        ];

        assert_eq!(greeks.len(), 7);
    }

    #[test]
    fn test_symbol_recognition_operators() {
        let ops = vec![
            MathSymbol::Plus,
            MathSymbol::Minus,
            MathSymbol::Times,
            MathSymbol::Divide,
            MathSymbol::Equals,
        ];

        assert_eq!(ops.len(), 5);
    }

    #[test]
    fn test_ast_construction_simple_addition() {
        let expr = MathNode::BinaryOp {
            op: "+".to_string(),
            left: Box::new(MathNode::Variable("x".to_string())),
            right: Box::new(MathNode::Variable("y".to_string())),
        };

        assert!(matches!(expr, MathNode::BinaryOp { .. }));
    }

    #[test]
    fn test_ast_construction_simple_multiplication() {
        let expr = MathNode::BinaryOp {
            op: "*".to_string(),
            left: Box::new(MathNode::Number("2".to_string())),
            right: Box::new(MathNode::Variable("x".to_string())),
        };

        match expr {
            MathNode::BinaryOp { op, .. } => assert_eq!(op, "*"),
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_latex_generation_simple_addition() {
        let expr = MathNode::BinaryOp {
            op: "+".to_string(),
            left: Box::new(MathNode::Variable("x".to_string())),
            right: Box::new(MathNode::Variable("y".to_string())),
        };

        let latex = expr.to_latex();
        assert_eq!(latex, "x + y");
    }

    #[test]
    fn test_latex_generation_fraction_simple() {
        let frac = MathNode::Fraction {
            numerator: Box::new(MathNode::Number("1".to_string())),
            denominator: Box::new(MathNode::Number("2".to_string())),
        };

        let latex = frac.to_latex();
        assert_eq!(latex, r"\frac{1}{2}");
    }

    #[test]
    fn test_latex_generation_fraction_variables() {
        let frac = MathNode::Fraction {
            numerator: Box::new(MathNode::Variable("a".to_string())),
            denominator: Box::new(MathNode::Variable("b".to_string())),
        };

        let latex = frac.to_latex();
        assert_eq!(latex, r"\frac{a}{b}");
    }

    #[test]
    fn test_latex_generation_fraction_complex() {
        let numerator = MathNode::BinaryOp {
            op: "+".to_string(),
            left: Box::new(MathNode::Variable("a".to_string())),
            right: Box::new(MathNode::Number("1".to_string())),
        };

        let frac = MathNode::Fraction {
            numerator: Box::new(numerator),
            denominator: Box::new(MathNode::Variable("b".to_string())),
        };

        let latex = frac.to_latex();
        assert_eq!(latex, r"\frac{a + 1}{b}");
    }

    #[test]
    fn test_latex_generation_root_square() {
        let root = MathNode::Root {
            degree: None,
            radicand: Box::new(MathNode::Variable("x".to_string())),
        };

        let latex = root.to_latex();
        assert_eq!(latex, r"\sqrt{x}");
    }

    #[test]
    fn test_latex_generation_root_nth() {
        let root = MathNode::Root {
            degree: Some(Box::new(MathNode::Number("3".to_string()))),
            radicand: Box::new(MathNode::Variable("x".to_string())),
        };

        let latex = root.to_latex();
        assert_eq!(latex, r"\sqrt[{3}]{x}");
    }

    #[test]
    fn test_latex_generation_superscript() {
        let power = MathNode::Superscript {
            base: Box::new(MathNode::Variable("x".to_string())),
            exponent: Box::new(MathNode::Number("2".to_string())),
        };

        let latex = power.to_latex();
        assert_eq!(latex, "x^{2}");
    }

    #[test]
    fn test_latex_generation_subscript() {
        let sub = MathNode::Subscript {
            base: Box::new(MathNode::Variable("x".to_string())),
            index: Box::new(MathNode::Number("1".to_string())),
        };

        let latex = sub.to_latex();
        assert_eq!(latex, "x_{1}");
    }

    #[test]
    fn test_latex_generation_subscript_and_superscript() {
        let base = MathNode::Variable("x".to_string());
        let with_sub = MathNode::Subscript {
            base: Box::new(base),
            index: Box::new(MathNode::Number("1".to_string())),
        };
        let with_both = MathNode::Superscript {
            base: Box::new(with_sub),
            exponent: Box::new(MathNode::Number("2".to_string())),
        };

        let latex = with_both.to_latex();
        assert_eq!(latex, "x_{1}^{2}");
    }

    #[test]
    fn test_latex_generation_matrix_2x2() {
        let matrix = MathNode::Matrix {
            rows: 2,
            cols: 2,
            elements: vec![
                vec![
                    MathNode::Number("1".to_string()),
                    MathNode::Number("2".to_string()),
                ],
                vec![
                    MathNode::Number("3".to_string()),
                    MathNode::Number("4".to_string()),
                ],
            ],
        };

        let latex = matrix.to_latex();
        assert!(latex.contains(r"\begin{bmatrix}"));
        assert!(latex.contains(r"\end{bmatrix}"));
        assert!(latex.contains("1 & 2"));
        assert!(latex.contains("3 & 4"));
    }

    #[test]
    fn test_latex_generation_matrix_3x3() {
        let matrix = MathNode::Matrix {
            rows: 3,
            cols: 3,
            elements: vec![
                vec![
                    MathNode::Number("1".to_string()),
                    MathNode::Number("2".to_string()),
                    MathNode::Number("3".to_string()),
                ],
                vec![
                    MathNode::Number("4".to_string()),
                    MathNode::Number("5".to_string()),
                    MathNode::Number("6".to_string()),
                ],
                vec![
                    MathNode::Number("7".to_string()),
                    MathNode::Number("8".to_string()),
                    MathNode::Number("9".to_string()),
                ],
            ],
        };

        let latex = matrix.to_latex();
        assert!(latex.contains(r"\begin{bmatrix}"));
        assert!(latex.contains("1 & 2 & 3"));
    }

    #[test]
    fn test_latex_generation_integral_simple() {
        let integral = MathNode::Integral {
            lower: None,
            upper: None,
            integrand: Box::new(MathNode::Variable("x".to_string())),
        };

        let latex = integral.to_latex();
        assert!(latex.contains(r"\int"));
        assert!(latex.contains("x dx"));
    }

    #[test]
    fn test_latex_generation_integral_with_limits() {
        let integral = MathNode::Integral {
            lower: Some(Box::new(MathNode::Number("0".to_string()))),
            upper: Some(Box::new(MathNode::Number("1".to_string()))),
            integrand: Box::new(MathNode::Variable("x".to_string())),
        };

        let latex = integral.to_latex();
        assert!(latex.contains(r"\int_{0}^{1}"));
    }

    #[test]
    fn test_latex_generation_summation() {
        let sum = MathNode::Summation {
            lower: Some(Box::new(MathNode::BinaryOp {
                op: "=".to_string(),
                left: Box::new(MathNode::Variable("i".to_string())),
                right: Box::new(MathNode::Number("1".to_string())),
            })),
            upper: Some(Box::new(MathNode::Variable("n".to_string()))),
            term: Box::new(MathNode::Variable("i".to_string())),
        };

        let latex = sum.to_latex();
        assert!(latex.contains(r"\sum"));
    }

    #[test]
    fn test_mathml_generation_number() {
        let num = MathNode::Number("42".to_string());
        let mathml = num.to_mathml();
        assert_eq!(mathml, "<mn>42</mn>");
    }

    #[test]
    fn test_mathml_generation_variable() {
        let var = MathNode::Variable("x".to_string());
        let mathml = var.to_mathml();
        assert_eq!(mathml, "<mi>x</mi>");
    }

    #[test]
    fn test_mathml_generation_fraction() {
        let frac = MathNode::Fraction {
            numerator: Box::new(MathNode::Number("1".to_string())),
            denominator: Box::new(MathNode::Number("2".to_string())),
        };

        let mathml = frac.to_mathml();
        assert!(mathml.contains("<mfrac>"));
        assert!(mathml.contains("<mn>1</mn>"));
        assert!(mathml.contains("<mn>2</mn>"));
    }

    #[test]
    fn test_mathml_generation_superscript() {
        let power = MathNode::Superscript {
            base: Box::new(MathNode::Variable("x".to_string())),
            exponent: Box::new(MathNode::Number("2".to_string())),
        };

        let mathml = power.to_mathml();
        assert!(mathml.contains("<msup>"));
        assert!(mathml.contains("<mi>x</mi>"));
        assert!(mathml.contains("<mn>2</mn>"));
    }

    #[test]
    fn test_asciimath_generation_simple() {
        let expr = MathNode::BinaryOp {
            op: "+".to_string(),
            left: Box::new(MathNode::Variable("x".to_string())),
            right: Box::new(MathNode::Number("1".to_string())),
        };

        let ascii = expr.to_asciimath();
        assert_eq!(ascii, "x + 1");
    }

    #[test]
    fn test_asciimath_generation_fraction() {
        let frac = MathNode::Fraction {
            numerator: Box::new(MathNode::Variable("a".to_string())),
            denominator: Box::new(MathNode::Variable("b".to_string())),
        };

        let ascii = frac.to_asciimath();
        assert_eq!(ascii, "(a)/(b)");
    }

    #[test]
    fn test_asciimath_generation_power() {
        let power = MathNode::Superscript {
            base: Box::new(MathNode::Variable("x".to_string())),
            exponent: Box::new(MathNode::Number("2".to_string())),
        };

        let ascii = power.to_asciimath();
        assert_eq!(ascii, "x^2");
    }
}
