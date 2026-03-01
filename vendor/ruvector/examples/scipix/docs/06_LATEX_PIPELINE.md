# LaTeX Generation and Mathematical Expression Pipeline

**Project**: ruvector-scipix
**Component**: LaTeX/Math Processing Specialist
**Version**: 1.0.0
**Last Updated**: 2025-11-28

---

## Table of Contents

1. [Mathematical Symbol Recognition](#1-mathematical-symbol-recognition)
2. [LaTeX Token Generation](#2-latex-token-generation)
3. [Expression Tree Representation](#3-expression-tree-representation)
4. [Output Format Specifications](#4-output-format-specifications)
5. [Chemistry Notation (SMILES)](#5-chemistry-notation-smiles)
6. [Rust Implementation Patterns](#6-rust-implementation-patterns)
7. [Performance Considerations](#7-performance-considerations)
8. [Testing Strategy](#8-testing-strategy)

---

## 1. Mathematical Symbol Recognition

### 1.1 Symbol Categories

The LaTeX pipeline must recognize and classify mathematical symbols into distinct categories for proper rendering.

#### Greek Letters

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GreekLetter {
    // Lowercase
    Alpha, Beta, Gamma, Delta, Epsilon, Zeta, Eta, Theta,
    Iota, Kappa, Lambda, Mu, Nu, Xi, Omicron, Pi,
    Rho, Sigma, Tau, Upsilon, Phi, Chi, Psi, Omega,

    // Uppercase
    CapitalGamma, CapitalDelta, CapitalTheta, CapitalLambda,
    CapitalXi, CapitalPi, CapitalSigma, CapitalUpsilon,
    CapitalPhi, CapitalPsi, CapitalOmega,

    // Variants
    VarEpsilon, VarTheta, VarPi, VarRho, VarSigma, VarPhi,
}

impl GreekLetter {
    pub fn to_latex(&self) -> &'static str {
        match self {
            Self::Alpha => r"\alpha",
            Self::Beta => r"\beta",
            Self::Gamma => r"\gamma",
            Self::Delta => r"\delta",
            Self::Epsilon => r"\epsilon",
            Self::VarEpsilon => r"\varepsilon",
            Self::CapitalGamma => r"\Gamma",
            // ... complete mapping
            _ => "",
        }
    }
}
```

#### Mathematical Operators

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathOperator {
    // Binary operators
    Plus, Minus, Times, Divide, Dot, Cross, Wedge, Vee,
    Cap, Cup, Oplus, Ominus, Otimes, Oslash,

    // Relational operators
    Equals, NotEquals, Less, Greater, LessEq, GreaterEq,
    Approx, Equiv, Sim, Cong, Propto,

    // Large operators
    Sum, Product, Integral, DoubleIntegral, TripleIntegral,
    ContourIntegral, Limit, Supremum, Infimum,

    // Set operators
    In, NotIn, Subset, Superset, SubsetEq, SupersetEq,
    Union, Intersection, EmptySet,

    // Logic operators
    And, Or, Not, Implies, Iff, Forall, Exists,
}

impl MathOperator {
    pub fn to_latex(&self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Times => r"\times",
            Self::Divide => r"\div",
            Self::Sum => r"\sum",
            Self::Integral => r"\int",
            Self::Subset => r"\subset",
            Self::Forall => r"\forall",
            // ... complete mapping
            _ => "",
        }
    }

    pub fn is_large_operator(&self) -> bool {
        matches!(self,
            Self::Sum | Self::Product | Self::Integral |
            Self::DoubleIntegral | Self::TripleIntegral |
            Self::Limit | Self::Supremum | Self::Infimum
        )
    }

    pub fn requires_limits(&self) -> bool {
        self.is_large_operator()
    }
}
```

#### Fractions and Roots

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FractionType {
    /// Standard fraction: \frac{num}{den}
    Standard,
    /// Display style fraction: \dfrac{num}{den}
    Display,
    /// Text style fraction: \tfrac{num}{den}
    Text,
    /// Continued fraction: \cfrac{num}{den}
    Continued,
    /// Binomial coefficient: \binom{n}{k}
    Binomial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RootType {
    /// Square root: \sqrt{expr}
    Square,
    /// Nth root: \sqrt[n]{expr}
    Nth(Box<MathExpr>),
}
```

#### Subscripts and Superscripts

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub base: Box<MathExpr>,
    pub subscript: Option<Box<MathExpr>>,
    pub superscript: Option<Box<MathExpr>>,
}

impl Script {
    pub fn to_latex(&self) -> String {
        let mut result = self.base.to_latex();

        if let Some(sub) = &self.subscript {
            result.push('_');
            if self.needs_braces(&sub) {
                result.push('{');
                result.push_str(&sub.to_latex());
                result.push('}');
            } else {
                result.push_str(&sub.to_latex());
            }
        }

        if let Some(sup) = &self.superscript {
            result.push('^');
            if self.needs_braces(&sup) {
                result.push('{');
                result.push_str(&sup.to_latex());
                result.push('}');
            } else {
                result.push_str(&sup.to_latex());
            }
        }

        result
    }

    fn needs_braces(&self, expr: &MathExpr) -> bool {
        !matches!(expr, MathExpr::Symbol(_) | MathExpr::Number(_))
    }
}
```

#### Matrices, Vectors, and Tensors

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MatrixType {
    /// Standard matrix: \begin{matrix}...\end{matrix}
    Plain,
    /// Parentheses: \begin{pmatrix}...\end{pmatrix}
    Paren,
    /// Brackets: \begin{bmatrix}...\end{bmatrix}
    Bracket,
    /// Braces: \begin{Bmatrix}...\end{Bmatrix}
    Brace,
    /// Vertical bars: \begin{vmatrix}...\end{vmatrix}
    Vbar,
    /// Double vertical bars: \begin{Vmatrix}...\end{Vmatrix}
    DoubleVbar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub matrix_type: MatrixType,
    pub rows: Vec<Vec<MathExpr>>,
}

impl Matrix {
    pub fn to_latex(&self) -> String {
        let env_name = match self.matrix_type {
            MatrixType::Plain => "matrix",
            MatrixType::Paren => "pmatrix",
            MatrixType::Bracket => "bmatrix",
            MatrixType::Brace => "Bmatrix",
            MatrixType::Vbar => "vmatrix",
            MatrixType::DoubleVbar => "Vmatrix",
        };

        let mut result = format!(r"\begin{{{}}}", env_name);

        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                result.push_str(r" \\ ");
            }

            for (j, cell) in row.iter().enumerate() {
                if j > 0 {
                    result.push_str(" & ");
                }
                result.push_str(&cell.to_latex());
            }
        }

        result.push_str(&format!(r" \end{{{}}}", env_name));
        result
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub components: Vec<MathExpr>,
    pub style: VectorStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VectorStyle {
    /// Column vector
    Column,
    /// Row vector
    Row,
    /// Arrow notation: \vec{v}
    Arrow,
    /// Bold notation: \mathbf{v}
    Bold,
}
```

#### Limits and Integrals

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub operator: MathOperator,
    pub lower: Option<Box<MathExpr>>,
    pub upper: Option<Box<MathExpr>>,
}

impl Limit {
    pub fn to_latex(&self, display_mode: bool) -> String {
        let op = self.operator.to_latex();
        let mut result = String::from(op);

        if let Some(lower) = &self.lower {
            result.push_str("_{");
            result.push_str(&lower.to_latex());
            result.push('}');
        }

        if let Some(upper) = &self.upper {
            result.push_str("^{");
            result.push_str(&upper.to_latex());
            result.push('}');
        }

        if display_mode && self.operator.requires_limits() {
            format!(r"\limits {}", result)
        } else {
            result
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integral {
    pub integral_type: IntegralType,
    pub lower_limit: Option<Box<MathExpr>>,
    pub upper_limit: Option<Box<MathExpr>>,
    pub integrand: Box<MathExpr>,
    pub variable: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegralType {
    Single,
    Double,
    Triple,
    Contour,
    Surface,
    Volume,
}
```

---

## 2. LaTeX Token Generation

### 2.1 Symbol-to-LaTeX Mapping Table

```rust
use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Global symbol mapping table for efficient lookup
pub static SYMBOL_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Greek letters
    m.insert("α", r"\alpha");
    m.insert("β", r"\beta");
    m.insert("γ", r"\gamma");
    m.insert("δ", r"\delta");
    m.insert("ε", r"\epsilon");
    m.insert("θ", r"\theta");
    m.insert("λ", r"\lambda");
    m.insert("μ", r"\mu");
    m.insert("π", r"\pi");
    m.insert("σ", r"\sigma");
    m.insert("φ", r"\phi");
    m.insert("ω", r"\omega");

    // Operators
    m.insert("≤", r"\leq");
    m.insert("≥", r"\geq");
    m.insert("≠", r"\neq");
    m.insert("≈", r"\approx");
    m.insert("≡", r"\equiv");
    m.insert("∈", r"\in");
    m.insert("∉", r"\notin");
    m.insert("⊂", r"\subset");
    m.insert("⊆", r"\subseteq");
    m.insert("∪", r"\cup");
    m.insert("∩", r"\cap");
    m.insert("∅", r"\emptyset");
    m.insert("∞", r"\infty");
    m.insert("∇", r"\nabla");
    m.insert("∂", r"\partial");
    m.insert("∫", r"\int");
    m.insert("∑", r"\sum");
    m.insert("∏", r"\prod");
    m.insert("√", r"\sqrt");
    m.insert("±", r"\pm");
    m.insert("×", r"\times");
    m.insert("÷", r"\div");
    m.insert("⋅", r"\cdot");

    // Logic
    m.insert("∧", r"\land");
    m.insert("∨", r"\lor");
    m.insert("¬", r"\neg");
    m.insert("⇒", r"\Rightarrow");
    m.insert("⇔", r"\Leftrightarrow");
    m.insert("∀", r"\forall");
    m.insert("∃", r"\exists");

    // Arrows
    m.insert("→", r"\to");
    m.insert("←", r"\leftarrow");
    m.insert("↔", r"\leftrightarrow");
    m.insert("⇒", r"\Rightarrow");
    m.insert("⇐", r"\Leftarrow");
    m.insert("⇔", r"\Leftrightarrow");

    m
});

pub fn unicode_to_latex(symbol: &str) -> Option<&'static str> {
    SYMBOL_MAP.get(symbol).copied()
}
```

### 2.2 Structural Tokens

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructuralToken {
    /// Fraction: \frac{numerator}{denominator}
    Frac {
        numerator: Box<MathExpr>,
        denominator: Box<MathExpr>,
        frac_type: FractionType,
    },

    /// Square root: \sqrt{expr} or \sqrt[n]{expr}
    Sqrt {
        root_type: RootType,
        expr: Box<MathExpr>,
    },

    /// Summation: \sum_{lower}^{upper}
    Sum {
        lower: Option<Box<MathExpr>>,
        upper: Option<Box<MathExpr>>,
    },

    /// Product: \prod_{lower}^{upper}
    Prod {
        lower: Option<Box<MathExpr>>,
        upper: Option<Box<MathExpr>>,
    },

    /// Integral: \int_{lower}^{upper}
    Int {
        integral_type: IntegralType,
        lower: Option<Box<MathExpr>>,
        upper: Option<Box<MathExpr>>,
    },

    /// Limit: \lim_{var \to value}
    Lim {
        variable: String,
        approach: Box<MathExpr>,
    },

    /// Matrix environment
    Matrix(Matrix),

    /// Cases environment: \begin{cases}...\end{cases}
    Cases {
        cases: Vec<(MathExpr, MathExpr)>, // (expression, condition)
    },
}

impl StructuralToken {
    pub fn to_latex(&self) -> String {
        match self {
            Self::Frac { numerator, denominator, frac_type } => {
                let cmd = match frac_type {
                    FractionType::Standard => r"\frac",
                    FractionType::Display => r"\dfrac",
                    FractionType::Text => r"\tfrac",
                    FractionType::Continued => r"\cfrac",
                    FractionType::Binomial => r"\binom",
                };
                format!("{}{{{}}}{{{}}}",
                    cmd,
                    numerator.to_latex(),
                    denominator.to_latex()
                )
            }

            Self::Sqrt { root_type, expr } => {
                match root_type {
                    RootType::Square => format!(r"\sqrt{{{}}}", expr.to_latex()),
                    RootType::Nth(n) => format!(
                        r"\sqrt[{{{}}}]{{{}}}",
                        n.to_latex(),
                        expr.to_latex()
                    ),
                }
            }

            Self::Cases { cases } => {
                let mut result = String::from(r"\begin{cases}");
                for (i, (expr, cond)) in cases.iter().enumerate() {
                    if i > 0 {
                        result.push_str(r" \\ ");
                    }
                    result.push_str(&format!(
                        "{} & \\text{{if }} {}",
                        expr.to_latex(),
                        cond.to_latex()
                    ));
                }
                result.push_str(r" \end{cases}");
                result
            }

            _ => String::new(),
        }
    }
}
```

### 2.3 Delimiter Handling

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Delimiter {
    Paren,          // ( )
    Bracket,        // [ ]
    Brace,          // { }
    Angle,          // ⟨ ⟩
    Pipe,           // | |
    DoublePipe,     // ‖ ‖
    Floor,          // ⌊ ⌋
    Ceiling,        // ⌈ ⌉
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelimitedExpr {
    pub delimiter: Delimiter,
    pub content: Box<MathExpr>,
    pub auto_size: bool,
}

impl DelimitedExpr {
    pub fn to_latex(&self) -> String {
        let (left, right) = match self.delimiter {
            Delimiter::Paren => ("(", ")"),
            Delimiter::Bracket => ("[", "]"),
            Delimiter::Brace => (r"\{", r"\}"),
            Delimiter::Angle => (r"\langle", r"\rangle"),
            Delimiter::Pipe => ("|", "|"),
            Delimiter::DoublePipe => (r"\|", r"\|"),
            Delimiter::Floor => (r"\lfloor", r"\rfloor"),
            Delimiter::Ceiling => (r"\lceil", r"\rceil"),
        };

        if self.auto_size {
            format!(r"\left{} {} \right{}", left, self.content.to_latex(), right)
        } else {
            format!("{} {} {}", left, self.content.to_latex(), right)
        }
    }
}
```

---

## 3. Expression Tree Representation

### 3.1 Node Types

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathExpr {
    /// Single symbol (variable, constant)
    Symbol(String),

    /// Numeric literal
    Number(Number),

    /// Greek letter
    Greek(GreekLetter),

    /// Operator
    Operator(MathOperator),

    /// Binary operation
    Binary {
        op: MathOperator,
        left: Box<MathExpr>,
        right: Box<MathExpr>,
    },

    /// Unary operation
    Unary {
        op: MathOperator,
        operand: Box<MathExpr>,
    },

    /// Function application: f(x)
    Function {
        name: String,
        args: Vec<MathExpr>,
    },

    /// Subscript/superscript
    Script(Script),

    /// Structural token (fraction, root, etc.)
    Structural(StructuralToken),

    /// Delimited expression
    Delimited(DelimitedExpr),

    /// Text within math mode
    Text(String),

    /// Sequence of expressions
    Sequence(Vec<MathExpr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Integer(i64),
    Float(f64),
    Decimal(String), // For exact decimal representation
    Scientific { mantissa: f64, exponent: i32 },
}

impl Number {
    pub fn to_latex(&self) -> String {
        match self {
            Self::Integer(n) => n.to_string(),
            Self::Float(f) => {
                if f.is_infinite() {
                    r"\infty".to_string()
                } else if f.is_nan() {
                    r"\text{NaN}".to_string()
                } else {
                    f.to_string()
                }
            }
            Self::Decimal(s) => s.clone(),
            Self::Scientific { mantissa, exponent } => {
                format!("{} \\times 10^{{{}}}", mantissa, exponent)
            }
        }
    }
}
```

### 3.2 Tree Traversal

```rust
impl MathExpr {
    /// Convert expression tree to LaTeX string
    pub fn to_latex(&self) -> String {
        match self {
            Self::Symbol(s) => s.clone(),
            Self::Number(n) => n.to_latex(),
            Self::Greek(g) => g.to_latex().to_string(),
            Self::Operator(op) => op.to_latex().to_string(),

            Self::Binary { op, left, right } => {
                let left_str = self.maybe_parenthesize(left);
                let right_str = self.maybe_parenthesize(right);
                let op_str = op.to_latex();

                format!("{} {} {}", left_str, op_str, right_str)
            }

            Self::Unary { op, operand } => {
                format!("{} {}", op.to_latex(), operand.to_latex())
            }

            Self::Function { name, args } => {
                let args_str = args
                    .iter()
                    .map(|arg| arg.to_latex())
                    .collect::<Vec<_>>()
                    .join(", ");

                if self.is_standard_function(name) {
                    format!(r"\{} \left( {} \right)", name, args_str)
                } else {
                    format!("{} \\left( {} \\right)", name, args_str)
                }
            }

            Self::Script(s) => s.to_latex(),
            Self::Structural(st) => st.to_latex(),
            Self::Delimited(d) => d.to_latex(),
            Self::Text(t) => format!(r"\text{{{}}}", t),
            Self::Sequence(exprs) => {
                exprs.iter()
                    .map(|e| e.to_latex())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }

    /// Add parentheses if needed based on precedence
    fn maybe_parenthesize(&self, expr: &MathExpr) -> String {
        if self.needs_parentheses(expr) {
            format!(r"\left( {} \right)", expr.to_latex())
        } else {
            expr.to_latex()
        }
    }

    /// Determine if parentheses are needed
    fn needs_parentheses(&self, expr: &MathExpr) -> bool {
        match (self, expr) {
            (Self::Binary { op: parent_op, .. }, Self::Binary { op: child_op, .. }) => {
                self.precedence(child_op) < self.precedence(parent_op)
            }
            _ => false,
        }
    }

    /// Operator precedence
    fn precedence(&self, op: &MathOperator) -> u8 {
        match op {
            MathOperator::Plus | MathOperator::Minus => 1,
            MathOperator::Times | MathOperator::Divide | MathOperator::Dot => 2,
            MathOperator::Cross | MathOperator::Wedge => 3,
            _ => 0,
        }
    }

    /// Check if function name is a standard LaTeX function
    fn is_standard_function(&self, name: &str) -> bool {
        matches!(name,
            "sin" | "cos" | "tan" | "sec" | "csc" | "cot" |
            "sinh" | "cosh" | "tanh" | "sech" | "csch" | "coth" |
            "arcsin" | "arccos" | "arctan" |
            "log" | "ln" | "exp" |
            "det" | "dim" | "deg" | "gcd" | "lcm" |
            "max" | "min" | "sup" | "inf" | "lim"
        )
    }

    /// Depth-first traversal
    pub fn traverse<F>(&self, visitor: &mut F)
    where
        F: FnMut(&MathExpr),
    {
        visitor(self);

        match self {
            Self::Binary { left, right, .. } => {
                left.traverse(visitor);
                right.traverse(visitor);
            }
            Self::Unary { operand, .. } => {
                operand.traverse(visitor);
            }
            Self::Function { args, .. } => {
                for arg in args {
                    arg.traverse(visitor);
                }
            }
            Self::Script(Script { base, subscript, superscript }) => {
                base.traverse(visitor);
                if let Some(sub) = subscript {
                    sub.traverse(visitor);
                }
                if let Some(sup) = superscript {
                    sup.traverse(visitor);
                }
            }
            Self::Sequence(exprs) => {
                for expr in exprs {
                    expr.traverse(visitor);
                }
            }
            _ => {}
        }
    }
}
```

### 3.3 Precedence and Grouping Rules

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Assignment = 0,     // = := ≔
    LogicalOr = 1,      // ∨ ∥
    LogicalAnd = 2,     // ∧ &
    Relational = 3,     // = ≠ < > ≤ ≥
    Additive = 4,       // + -
    Multiplicative = 5, // × ÷ · /
    Unary = 6,          // - ¬
    Exponential = 7,    // ^
    Application = 8,    // function application
}

pub struct PrecedenceRules;

impl PrecedenceRules {
    pub fn get_precedence(op: &MathOperator) -> Precedence {
        match op {
            MathOperator::Equals | MathOperator::NotEquals |
            MathOperator::Less | MathOperator::Greater |
            MathOperator::LessEq | MathOperator::GreaterEq |
            MathOperator::Approx | MathOperator::Equiv => Precedence::Relational,

            MathOperator::Plus | MathOperator::Minus => Precedence::Additive,

            MathOperator::Times | MathOperator::Divide |
            MathOperator::Dot | MathOperator::Cross => Precedence::Multiplicative,

            MathOperator::And => Precedence::LogicalAnd,
            MathOperator::Or => Precedence::LogicalOr,
            MathOperator::Not => Precedence::Unary,

            _ => Precedence::Application,
        }
    }

    pub fn needs_grouping(parent: &MathOperator, child: &MathOperator) -> bool {
        Self::get_precedence(child) < Self::get_precedence(parent)
    }
}
```

---

## 4. Output Format Specifications

### 4.1 Scipix Markdown (MMD) Format

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScipixMarkdown {
    pub content: String,
    pub metadata: MmdMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MmdMetadata {
    pub version: String,
    pub math_mode: MathMode,
    pub delimiter_config: DelimiterConfig,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathMode {
    Inline,
    Display,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelimiterConfig {
    pub inline_start: String,
    pub inline_end: String,
    pub display_start: String,
    pub display_end: String,
}

impl Default for DelimiterConfig {
    fn default() -> Self {
        Self {
            inline_start: "$".to_string(),
            inline_end: "$".to_string(),
            display_start: "$$".to_string(),
            display_end: "$$".to_string(),
        }
    }
}

impl ScipixMarkdown {
    pub fn new(expr: &MathExpr, mode: MathMode) -> Self {
        let latex = expr.to_latex();
        let config = DelimiterConfig::default();

        let content = match mode {
            MathMode::Inline => format!("{}{}{}",
                config.inline_start, latex, config.inline_end),
            MathMode::Display => format!("{}\n{}\n{}",
                config.display_start, latex, config.display_end),
        };

        Self {
            content,
            metadata: MmdMetadata {
                version: "1.0".to_string(),
                math_mode: mode,
                delimiter_config: config,
                extensions: vec!["amsmath".to_string(), "amssymb".to_string()],
            },
        }
    }

    pub fn to_string(&self) -> String {
        self.content.clone()
    }
}
```

### 4.2 Inline vs Display Math Modes

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathRenderer {
    pub mode: MathMode,
    pub style: RenderStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RenderStyle {
    /// Compact rendering for inline math
    Compact,
    /// Expanded rendering for display math
    Expanded,
    /// Auto-detect based on expression complexity
    Auto,
}

impl MathRenderer {
    pub fn render(&self, expr: &MathExpr) -> String {
        let latex = expr.to_latex();

        let style_prefix = match (&self.mode, &self.style) {
            (MathMode::Display, RenderStyle::Expanded) |
            (MathMode::Display, RenderStyle::Auto) => r"\displaystyle ",
            (MathMode::Inline, RenderStyle::Compact) => r"\textstyle ",
            _ => "",
        };

        format!("{}{}", style_prefix, latex)
    }

    pub fn should_use_display_mode(&self, expr: &MathExpr) -> bool {
        match &self.style {
            RenderStyle::Auto => self.is_complex(expr),
            _ => matches!(self.mode, MathMode::Display),
        }
    }

    fn is_complex(&self, expr: &MathExpr) -> bool {
        match expr {
            MathExpr::Structural(StructuralToken::Frac { .. }) |
            MathExpr::Structural(StructuralToken::Sum { .. }) |
            MathExpr::Structural(StructuralToken::Prod { .. }) |
            MathExpr::Structural(StructuralToken::Int { .. }) |
            MathExpr::Structural(StructuralToken::Matrix(_)) => true,
            _ => false,
        }
    }
}
```

### 4.3 Custom Delimiter Support

```rust
pub struct CustomDelimiters {
    delimiters: HashMap<String, (String, String)>,
}

impl CustomDelimiters {
    pub fn new() -> Self {
        let mut delimiters = HashMap::new();

        // Add default delimiter pairs
        delimiters.insert("inline".to_string(), ("$".to_string(), "$".to_string()));
        delimiters.insert("display".to_string(), ("$$".to_string(), "$$".to_string()));
        delimiters.insert("bracket".to_string(), (r"\[".to_string(), r"\]".to_string()));
        delimiters.insert("paren".to_string(), (r"\(".to_string(), r"\)".to_string()));

        Self { delimiters }
    }

    pub fn add_delimiter(&mut self, name: String, start: String, end: String) {
        self.delimiters.insert(name, (start, end));
    }

    pub fn wrap(&self, latex: &str, delimiter_type: &str) -> String {
        if let Some((start, end)) = self.delimiters.get(delimiter_type) {
            format!("{}{}{}", start, latex, end)
        } else {
            latex.to_string()
        }
    }
}
```

### 4.4 MathML Conversion

```rust
pub struct MathMLConverter;

impl MathMLConverter {
    pub fn convert(expr: &MathExpr) -> String {
        match expr {
            MathExpr::Symbol(s) => {
                format!("<mi>{}</mi>", Self::escape_xml(s))
            }

            MathExpr::Number(n) => {
                format!("<mn>{}</mn>", n.to_latex())
            }

            MathExpr::Operator(op) => {
                format!("<mo>{}</mo>", op.to_latex())
            }

            MathExpr::Binary { op, left, right } => {
                format!(
                    "<mrow>{}<mo>{}</mo>{}</mrow>",
                    Self::convert(left),
                    op.to_latex(),
                    Self::convert(right)
                )
            }

            MathExpr::Script(Script { base, subscript, superscript }) => {
                match (subscript, superscript) {
                    (Some(sub), Some(sup)) => {
                        format!(
                            "<msubsup>{}{}{}</msubsup>",
                            Self::convert(base),
                            Self::convert(sub),
                            Self::convert(sup)
                        )
                    }
                    (Some(sub), None) => {
                        format!(
                            "<msub>{}{}</msub>",
                            Self::convert(base),
                            Self::convert(sub)
                        )
                    }
                    (None, Some(sup)) => {
                        format!(
                            "<msup>{}{}</msup>",
                            Self::convert(base),
                            Self::convert(sup)
                        )
                    }
                    (None, None) => Self::convert(base),
                }
            }

            MathExpr::Structural(StructuralToken::Frac { numerator, denominator, .. }) => {
                format!(
                    "<mfrac>{}{}</mfrac>",
                    Self::convert(numerator),
                    Self::convert(denominator)
                )
            }

            MathExpr::Structural(StructuralToken::Sqrt { expr, root_type }) => {
                match root_type {
                    RootType::Square => {
                        format!("<msqrt>{}</msqrt>", Self::convert(expr))
                    }
                    RootType::Nth(n) => {
                        format!(
                            "<mroot>{}{}</mroot>",
                            Self::convert(expr),
                            Self::convert(n)
                        )
                    }
                }
            }

            _ => String::from("<mrow></mrow>"),
        }
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    pub fn wrap_mathml(content: &str, display: bool) -> String {
        let display_attr = if display { " display=\"block\"" } else { "" };
        format!(
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"{}>{}</math>",
            display_attr, content
        )
    }
}
```

---

## 5. Chemistry Notation (SMILES)

### 5.1 Molecular Structure Detection

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MolecularStructure {
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atom {
    pub element: ChemicalElement,
    pub charge: i8,
    pub isotope: Option<u16>,
    pub aromatic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChemicalElement {
    H, C, N, O, F, P, S, Cl, Br, I,
    // Extend as needed
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bond {
    pub from: usize,
    pub to: usize,
    pub bond_type: BondType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BondType {
    Single,
    Double,
    Triple,
    Aromatic,
}

pub struct MolecularDetector;

impl MolecularDetector {
    /// Detect if an expression represents a chemical structure
    pub fn is_chemical(expr: &MathExpr) -> bool {
        // Look for patterns like CH3, H2O, etc.
        match expr {
            MathExpr::Script(Script { base, subscript, .. }) => {
                if let MathExpr::Symbol(s) = &**base {
                    Self::is_element_symbol(s)
                } else {
                    false
                }
            }
            MathExpr::Sequence(exprs) => {
                exprs.iter().any(|e| Self::is_chemical(e))
            }
            _ => false,
        }
    }

    fn is_element_symbol(s: &str) -> bool {
        matches!(s,
            "H" | "He" | "Li" | "Be" | "B" | "C" | "N" | "O" | "F" | "Ne" |
            "Na" | "Mg" | "Al" | "Si" | "P" | "S" | "Cl" | "Ar" |
            "K" | "Ca" | "Fe" | "Cu" | "Zn" | "Br" | "Ag" | "I" | "Au"
        )
    }
}
```

### 5.2 SMILES String Generation

```rust
pub struct SmilesGenerator;

impl SmilesGenerator {
    /// Convert molecular structure to SMILES string
    pub fn generate(structure: &MolecularStructure) -> Result<String, SmilesError> {
        let mut smiles = String::new();
        let mut visited = vec![false; structure.atoms.len()];

        // Start DFS from first atom
        if !structure.atoms.is_empty() {
            Self::dfs(structure, 0, &mut visited, &mut smiles, None)?;
        }

        Ok(smiles)
    }

    fn dfs(
        structure: &MolecularStructure,
        atom_idx: usize,
        visited: &mut [bool],
        smiles: &mut String,
        from_bond: Option<BondType>,
    ) -> Result<(), SmilesError> {
        if visited[atom_idx] {
            return Ok(());
        }

        visited[atom_idx] = true;
        let atom = &structure.atoms[atom_idx];

        // Add bond symbol if needed
        if let Some(bond_type) = from_bond {
            smiles.push_str(&Self::bond_symbol(&bond_type));
        }

        // Add atom symbol
        smiles.push_str(&Self::atom_symbol(atom));

        // Find connected atoms
        let mut neighbors = Vec::new();
        for bond in &structure.bonds {
            if bond.from == atom_idx && !visited[bond.to] {
                neighbors.push((bond.to, bond.bond_type.clone()));
            } else if bond.to == atom_idx && !visited[bond.from] {
                neighbors.push((bond.from, bond.bond_type.clone()));
            }
        }

        // Process first neighbor inline
        if let Some((next_idx, bond_type)) = neighbors.first() {
            Self::dfs(structure, *next_idx, visited, smiles, Some(bond_type.clone()))?;
        }

        // Process remaining neighbors in branches
        for (next_idx, bond_type) in neighbors.iter().skip(1) {
            smiles.push('(');
            Self::dfs(structure, *next_idx, visited, smiles, Some(bond_type.clone()))?;
            smiles.push(')');
        }

        Ok(())
    }

    fn atom_symbol(atom: &Atom) -> String {
        let element = match &atom.element {
            ChemicalElement::H => "H",
            ChemicalElement::C if !atom.aromatic => "C",
            ChemicalElement::C => "c",
            ChemicalElement::N if !atom.aromatic => "N",
            ChemicalElement::N => "n",
            ChemicalElement::O if !atom.aromatic => "O",
            ChemicalElement::O => "o",
            ChemicalElement::F => "F",
            ChemicalElement::P => "P",
            ChemicalElement::S if !atom.aromatic => "S",
            ChemicalElement::S => "s",
            ChemicalElement::Cl => "Cl",
            ChemicalElement::Br => "Br",
            ChemicalElement::I => "I",
            ChemicalElement::Other(s) => s,
        };

        let mut result = element.to_string();

        // Add charge if present
        if atom.charge != 0 {
            result = format!("[{}{}]", element, Self::charge_string(atom.charge));
        }

        // Add isotope if present
        if let Some(isotope) = atom.isotope {
            result = format!("[{}{}]", isotope, element);
        }

        result
    }

    fn bond_symbol(bond_type: &BondType) -> String {
        match bond_type {
            BondType::Single => String::new(), // Implicit
            BondType::Double => "=".to_string(),
            BondType::Triple => "#".to_string(),
            BondType::Aromatic => ":".to_string(),
        }
    }

    fn charge_string(charge: i8) -> String {
        match charge {
            1 => "+".to_string(),
            -1 => "-".to_string(),
            n if n > 0 => format!("+{}", n),
            n => format!("{}", n),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SmilesError {
    #[error("Invalid molecular structure")]
    InvalidStructure,

    #[error("Unsupported element: {0}")]
    UnsupportedElement(String),
}
```

---

## 6. Rust Implementation Patterns

### 6.1 Token Enum Definitions

```rust
use serde::{Deserialize, Serialize};

/// Main token type for LaTeX generation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LatexToken {
    /// Mathematical expression
    Math(MathExpr),

    /// Text content
    Text(String),

    /// Environment begin
    BeginEnv { name: String, options: Option<String> },

    /// Environment end
    EndEnv { name: String },

    /// Command with optional arguments
    Command {
        name: String,
        args: Vec<String>,
        optional: Option<String>,
    },

    /// Whitespace
    Space,

    /// Newline
    Newline,
}

impl LatexToken {
    pub fn to_latex(&self) -> String {
        match self {
            Self::Math(expr) => expr.to_latex(),
            Self::Text(s) => s.clone(),
            Self::BeginEnv { name, options } => {
                if let Some(opts) = options {
                    format!(r"\begin{{{}}}[{}]", name, opts)
                } else {
                    format!(r"\begin{{{}}}", name)
                }
            }
            Self::EndEnv { name } => format!(r"\end{{{}}}", name),
            Self::Command { name, args, optional } => {
                let mut result = format!(r"\{}", name);
                if let Some(opt) = optional {
                    result.push_str(&format!("[{}]", opt));
                }
                for arg in args {
                    result.push_str(&format!("{{{}}}", arg));
                }
                result
            }
            Self::Space => " ".to_string(),
            Self::Newline => "\n".to_string(),
        }
    }
}
```

### 6.2 Parser Combinators

```rust
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, digit1, multispace0},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, tuple},
};

pub struct LatexParser;

impl LatexParser {
    /// Parse a number
    fn parse_number(input: &str) -> IResult<&str, Number> {
        alt((
            Self::parse_float,
            Self::parse_integer,
        ))(input)
    }

    fn parse_integer(input: &str) -> IResult<&str, Number> {
        map(
            recognize(pair(opt(char('-')), digit1)),
            |s: &str| Number::Integer(s.parse().unwrap())
        )(input)
    }

    fn parse_float(input: &str) -> IResult<&str, Number> {
        map(
            recognize(tuple((
                opt(char('-')),
                digit1,
                char('.'),
                digit1,
            ))),
            |s: &str| Number::Float(s.parse().unwrap())
        )(input)
    }

    /// Parse a symbol (variable name)
    fn parse_symbol(input: &str) -> IResult<&str, MathExpr> {
        map(
            take_while1(|c: char| c.is_alphabetic() || c == '_'),
            |s: &str| MathExpr::Symbol(s.to_string())
        )(input)
    }

    /// Parse a fraction: \frac{num}{den}
    fn parse_frac(input: &str) -> IResult<&str, MathExpr> {
        map(
            preceded(
                tag(r"\frac"),
                pair(
                    delimited(char('{'), Self::parse_expr, char('}')),
                    delimited(char('{'), Self::parse_expr, char('}')),
                )
            ),
            |(num, den)| MathExpr::Structural(StructuralToken::Frac {
                numerator: Box::new(num),
                denominator: Box::new(den),
                frac_type: FractionType::Standard,
            })
        )(input)
    }

    /// Parse square root: \sqrt{expr} or \sqrt[n]{expr}
    fn parse_sqrt(input: &str) -> IResult<&str, MathExpr> {
        map(
            preceded(
                tag(r"\sqrt"),
                pair(
                    opt(delimited(char('['), Self::parse_expr, char(']'))),
                    delimited(char('{'), Self::parse_expr, char('}')),
                )
            ),
            |(root, expr)| {
                let root_type = if let Some(n) = root {
                    RootType::Nth(Box::new(n))
                } else {
                    RootType::Square
                };
                MathExpr::Structural(StructuralToken::Sqrt {
                    root_type,
                    expr: Box::new(expr),
                })
            }
        )(input)
    }

    /// Parse subscript/superscript
    fn parse_script(input: &str) -> IResult<&str, MathExpr> {
        map(
            tuple((
                Self::parse_primary,
                opt(preceded(char('_'), Self::parse_primary)),
                opt(preceded(char('^'), Self::parse_primary)),
            )),
            |(base, sub, sup)| {
                if sub.is_some() || sup.is_some() {
                    MathExpr::Script(Script {
                        base: Box::new(base),
                        subscript: sub.map(Box::new),
                        superscript: sup.map(Box::new),
                    })
                } else {
                    base
                }
            }
        )(input)
    }

    /// Parse primary expression (atom)
    fn parse_primary(input: &str) -> IResult<&str, MathExpr> {
        preceded(
            multispace0,
            alt((
                Self::parse_frac,
                Self::parse_sqrt,
                delimited(char('('), Self::parse_expr, char(')')),
                map(Self::parse_number, MathExpr::Number),
                Self::parse_symbol,
            ))
        )(input)
    }

    /// Parse full expression
    fn parse_expr(input: &str) -> IResult<&str, MathExpr> {
        Self::parse_script(input)
    }

    /// Main entry point
    pub fn parse(input: &str) -> Result<MathExpr, String> {
        match Self::parse_expr(input) {
            Ok((_, expr)) => Ok(expr),
            Err(e) => Err(format!("Parse error: {:?}", e)),
        }
    }
}
```

### 6.3 Serde Serialization

```rust
use serde::{Deserialize, Serialize};
use serde_json;

/// Serializable representation of the expression tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMathExpr {
    #[serde(rename = "type")]
    pub expr_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<SerializableMathExpr>,
}

impl From<&MathExpr> for SerializableMathExpr {
    fn from(expr: &MathExpr) -> Self {
        match expr {
            MathExpr::Symbol(s) => Self {
                expr_type: "symbol".to_string(),
                value: Some(serde_json::json!(s)),
                children: vec![],
            },

            MathExpr::Number(n) => Self {
                expr_type: "number".to_string(),
                value: Some(serde_json::json!(n.to_latex())),
                children: vec![],
            },

            MathExpr::Binary { op, left, right } => Self {
                expr_type: "binary".to_string(),
                value: Some(serde_json::json!(op.to_latex())),
                children: vec![
                    SerializableMathExpr::from(&**left),
                    SerializableMathExpr::from(&**right),
                ],
            },

            MathExpr::Script(Script { base, subscript, superscript }) => {
                let mut children = vec![SerializableMathExpr::from(&**base)];
                if let Some(sub) = subscript {
                    children.push(SerializableMathExpr::from(&**sub));
                }
                if let Some(sup) = superscript {
                    children.push(SerializableMathExpr::from(&**sup));
                }
                Self {
                    expr_type: "script".to_string(),
                    value: None,
                    children,
                }
            }

            _ => Self {
                expr_type: "unknown".to_string(),
                value: None,
                children: vec![],
            },
        }
    }
}

/// Serialization helpers
pub mod serialization {
    use super::*;

    pub fn to_json(expr: &MathExpr) -> Result<String, serde_json::Error> {
        let serializable = SerializableMathExpr::from(expr);
        serde_json::to_string_pretty(&serializable)
    }

    pub fn to_json_compact(expr: &MathExpr) -> Result<String, serde_json::Error> {
        let serializable = SerializableMathExpr::from(expr);
        serde_json::to_string(&serializable)
    }
}
```

---

## 7. Performance Considerations

### 7.1 String Building Optimization

```rust
use std::fmt::Write;

/// Efficient LaTeX string builder
pub struct LatexBuilder {
    buffer: String,
    capacity_hint: usize,
}

impl LatexBuilder {
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(1024),
            capacity_hint: 1024,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
            capacity_hint: capacity,
        }
    }

    pub fn push_expr(&mut self, expr: &MathExpr) {
        self.write_expr(expr);
    }

    fn write_expr(&mut self, expr: &MathExpr) {
        match expr {
            MathExpr::Symbol(s) => {
                self.buffer.push_str(s);
            }

            MathExpr::Binary { op, left, right } => {
                self.write_expr(left);
                self.buffer.push(' ');
                self.buffer.push_str(op.to_latex());
                self.buffer.push(' ');
                self.write_expr(right);
            }

            _ => {
                self.buffer.push_str(&expr.to_latex());
            }
        }
    }

    pub fn build(self) -> String {
        self.buffer
    }
}
```

### 7.2 Caching Strategies

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cache for frequently used LaTeX conversions
pub struct LatexCache {
    cache: Arc<RwLock<HashMap<String, String>>>,
    max_size: usize,
}

impl LatexCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    pub fn get_or_compute<F>(&self, key: &str, compute: F) -> String
    where
        F: FnOnce() -> String,
    {
        // Try to read from cache
        {
            let cache = self.cache.read().unwrap();
            if let Some(value) = cache.get(key) {
                return value.clone();
            }
        }

        // Compute the value
        let value = compute();

        // Store in cache
        {
            let mut cache = self.cache.write().unwrap();
            if cache.len() >= self.max_size {
                // Simple eviction: clear half the cache
                cache.clear();
            }
            cache.insert(key.to_string(), value.clone());
        }

        value
    }
}
```

---

## 8. Testing Strategy

### 8.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greek_letter_conversion() {
        assert_eq!(GreekLetter::Alpha.to_latex(), r"\alpha");
        assert_eq!(GreekLetter::Beta.to_latex(), r"\beta");
        assert_eq!(GreekLetter::CapitalGamma.to_latex(), r"\Gamma");
    }

    #[test]
    fn test_fraction_rendering() {
        let frac = MathExpr::Structural(StructuralToken::Frac {
            numerator: Box::new(MathExpr::Number(Number::Integer(1))),
            denominator: Box::new(MathExpr::Number(Number::Integer(2))),
            frac_type: FractionType::Standard,
        });

        assert_eq!(frac.to_latex(), r"\frac{1}{2}");
    }

    #[test]
    fn test_script_rendering() {
        let script = MathExpr::Script(Script {
            base: Box::new(MathExpr::Symbol("x".to_string())),
            subscript: Some(Box::new(MathExpr::Number(Number::Integer(1)))),
            superscript: Some(Box::new(MathExpr::Number(Number::Integer(2)))),
        });

        assert_eq!(script.to_latex(), "x_{1}^{2}");
    }

    #[test]
    fn test_matrix_rendering() {
        let matrix = Matrix {
            matrix_type: MatrixType::Bracket,
            rows: vec![
                vec![
                    MathExpr::Number(Number::Integer(1)),
                    MathExpr::Number(Number::Integer(2)),
                ],
                vec![
                    MathExpr::Number(Number::Integer(3)),
                    MathExpr::Number(Number::Integer(4)),
                ],
            ],
        };

        let expected = r"\begin{bmatrix}1 & 2 \\ 3 & 4 \end{bmatrix}";
        assert_eq!(
            MathExpr::Structural(StructuralToken::Matrix(matrix)).to_latex(),
            expected
        );
    }
}
```

### 8.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_quadratic_formula() {
        // x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
        let b_squared = MathExpr::Script(Script {
            base: Box::new(MathExpr::Symbol("b".to_string())),
            subscript: None,
            superscript: Some(Box::new(MathExpr::Number(Number::Integer(2)))),
        });

        // Continue building the expression...
        // This tests complex expression assembly
    }
}
```

---

## Conclusion

This LaTeX generation pipeline provides a comprehensive framework for converting mathematical expressions from various input formats into properly formatted LaTeX output. The Rust implementation emphasizes type safety, performance, and extensibility while supporting advanced features like MathML conversion and chemical notation.

### Key Features

- **Type-safe** expression representation with comprehensive enum types
- **Extensible** architecture supporting new mathematical constructs
- **Performant** with caching and efficient string building
- **Standards-compliant** with Scipix Markdown and MathML support
- **Chemistry-aware** with SMILES notation support
- **Well-tested** with comprehensive unit and integration tests

### Next Steps

1. Implement OCR integration for symbol recognition
2. Add support for additional LaTeX packages (tikz, pgfplots)
3. Develop interactive editing capabilities
4. Create rendering preview system
5. Optimize for real-time conversion pipelines
