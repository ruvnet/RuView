use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark simple LaTeX expression generation
fn bench_simple_expressions(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_expressions");
    group.measurement_time(Duration::from_secs(5));

    let test_cases = vec![
        (
            "fraction",
            Expression::Fraction(
                Box::new(Expression::Number(1)),
                Box::new(Expression::Number(2)),
            ),
        ),
        (
            "power",
            Expression::Power(
                Box::new(Expression::Variable("x".to_string())),
                Box::new(Expression::Number(2)),
            ),
        ),
        (
            "sum",
            Expression::Sum(
                Box::new(Expression::Number(1)),
                Box::new(Expression::Number(2)),
            ),
        ),
        (
            "product",
            Expression::Product(
                Box::new(Expression::Variable("a".to_string())),
                Box::new(Expression::Variable("b".to_string())),
            ),
        ),
    ];

    for (name, expr) in test_cases {
        group.bench_with_input(BenchmarkId::new("to_latex", name), &expr, |b, expr| {
            b.iter(|| black_box(expr.to_latex()));
        });
    }

    group.finish();
}

/// Benchmark complex LaTeX expression generation
fn bench_complex_expressions(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_expressions");
    group.measurement_time(Duration::from_secs(8));

    // Create complex nested expressions
    let test_cases = vec![
        ("matrix_2x2", create_matrix(2, 2)),
        ("matrix_3x3", create_matrix(3, 3)),
        ("matrix_4x4", create_matrix(4, 4)),
        ("integral", create_integral()),
        ("summation", create_summation()),
        ("nested_fraction", create_nested_fraction(3)),
        ("polynomial", create_polynomial(5)),
    ];

    for (name, expr) in test_cases {
        group.bench_with_input(BenchmarkId::new("to_latex", name), &expr, |b, expr| {
            b.iter(|| black_box(expr.to_latex()));
        });
    }

    group.finish();
}

/// Benchmark AST traversal performance
fn bench_ast_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_traversal");
    group.measurement_time(Duration::from_secs(5));

    let depths = [3, 5, 7, 10];

    for depth in depths {
        let expr = create_nested_expression(depth);

        group.bench_with_input(BenchmarkId::new("depth", depth), &expr, |b, expr| {
            b.iter(|| black_box(count_nodes(black_box(expr))));
        });
    }

    group.finish();
}

/// Benchmark string building and concatenation
fn bench_string_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_building");
    group.measurement_time(Duration::from_secs(5));

    let expr = create_polynomial(20);

    // Compare different string building strategies
    group.bench_function("to_latex_default", |b| {
        b.iter(|| black_box(expr.to_latex()));
    });

    group.bench_function("to_latex_with_capacity", |b| {
        b.iter(|| black_box(expr.to_latex_with_capacity()));
    });

    group.finish();
}

/// Benchmark LaTeX escaping and special characters
fn bench_latex_escaping(c: &mut Criterion) {
    let mut group = c.benchmark_group("latex_escaping");
    group.measurement_time(Duration::from_secs(5));

    let test_strings = vec![
        ("no_special", "simple text"),
        ("underscores", "var_1 + var_2"),
        ("braces", "{x} + {y}"),
        ("mixed", "α + β_1^2 ∫ dx"),
    ];

    for (name, text) in test_strings {
        group.bench_with_input(BenchmarkId::new("escape", name), &text, |b, text| {
            b.iter(|| black_box(escape_latex(black_box(text))));
        });
    }

    group.finish();
}

/// Benchmark target: LaTeX generation should complete in <5ms
fn bench_latency_target(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_target_5ms");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Typical complex expression from OCR
    let expr = create_typical_ocr_expression();

    group.bench_function("typical_ocr_expression", |b| {
        b.iter(|| black_box(expr.to_latex()));
    });

    group.finish();
}

/// Benchmark batch LaTeX generation
fn bench_batch_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_generation");
    group.measurement_time(Duration::from_secs(10));

    let batch_sizes = [10, 50, 100];

    for size in batch_sizes {
        let expressions: Vec<_> = (0..size).map(|i| create_polynomial(i % 10 + 1)).collect();

        group.bench_with_input(
            BenchmarkId::new("batch_size", size),
            &expressions,
            |b, exprs| {
                b.iter(|| {
                    let results: Vec<_> = exprs.iter().map(|expr| expr.to_latex()).collect();
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

// Mock AST and Expression types

#[derive(Clone)]
enum Expression {
    Number(i32),
    Variable(String),
    Fraction(Box<Expression>, Box<Expression>),
    Power(Box<Expression>, Box<Expression>),
    Sum(Box<Expression>, Box<Expression>),
    Product(Box<Expression>, Box<Expression>),
    Matrix(Vec<Vec<Expression>>),
    Integral(Box<Expression>, String, String, String),
    Summation(Box<Expression>, String, String, String),
}

impl Expression {
    fn to_latex(&self) -> String {
        match self {
            Expression::Number(n) => n.to_string(),
            Expression::Variable(v) => v.clone(),
            Expression::Fraction(num, den) => {
                format!("\\frac{{{}}}{{{}}}", num.to_latex(), den.to_latex())
            }
            Expression::Power(base, exp) => {
                format!("{{{}}}^{{{}}}", base.to_latex(), exp.to_latex())
            }
            Expression::Sum(a, b) => {
                format!("{} + {}", a.to_latex(), b.to_latex())
            }
            Expression::Product(a, b) => {
                format!("{} \\cdot {}", a.to_latex(), b.to_latex())
            }
            Expression::Matrix(rows) => {
                let mut result = String::from("\\begin{bmatrix}");
                for (i, row) in rows.iter().enumerate() {
                    for (j, cell) in row.iter().enumerate() {
                        result.push_str(&cell.to_latex());
                        if j < row.len() - 1 {
                            result.push_str(" & ");
                        }
                    }
                    if i < rows.len() - 1 {
                        result.push_str(" \\\\ ");
                    }
                }
                result.push_str("\\end{bmatrix}");
                result
            }
            Expression::Integral(expr, var, lower, upper) => {
                format!(
                    "\\int_{{{}}}^{{{}}} {} \\, d{}",
                    lower,
                    upper,
                    expr.to_latex(),
                    var
                )
            }
            Expression::Summation(expr, var, lower, upper) => {
                format!(
                    "\\sum_{{{}={}}}^{{{}}} {}",
                    var,
                    lower,
                    upper,
                    expr.to_latex()
                )
            }
        }
    }

    fn to_latex_with_capacity(&self) -> String {
        let mut result = String::with_capacity(256);
        self.append_latex(&mut result);
        result
    }

    fn append_latex(&self, buffer: &mut String) {
        buffer.push_str(&self.to_latex());
    }
}

fn create_matrix(rows: usize, cols: usize) -> Expression {
    let matrix = (0..rows)
        .map(|i| {
            (0..cols)
                .map(|j| Expression::Number((i * cols + j) as i32))
                .collect()
        })
        .collect();
    Expression::Matrix(matrix)
}

fn create_integral() -> Expression {
    Expression::Integral(
        Box::new(Expression::Power(
            Box::new(Expression::Variable("x".to_string())),
            Box::new(Expression::Number(2)),
        )),
        "x".to_string(),
        "0".to_string(),
        "1".to_string(),
    )
}

fn create_summation() -> Expression {
    Expression::Summation(
        Box::new(Expression::Power(
            Box::new(Expression::Variable("i".to_string())),
            Box::new(Expression::Number(2)),
        )),
        "i".to_string(),
        "1".to_string(),
        "n".to_string(),
    )
}

fn create_nested_fraction(depth: usize) -> Expression {
    if depth == 0 {
        Expression::Number(1)
    } else {
        Expression::Fraction(
            Box::new(Expression::Number(1)),
            Box::new(create_nested_fraction(depth - 1)),
        )
    }
}

fn create_polynomial(degree: usize) -> Expression {
    let mut expr = Expression::Number(0);
    for i in 0..=degree {
        let term = Expression::Product(
            Box::new(Expression::Number(i as i32 + 1)),
            Box::new(Expression::Power(
                Box::new(Expression::Variable("x".to_string())),
                Box::new(Expression::Number(i as i32)),
            )),
        );
        expr = Expression::Sum(Box::new(expr), Box::new(term));
    }
    expr
}

fn create_nested_expression(depth: usize) -> Expression {
    if depth == 0 {
        Expression::Variable("x".to_string())
    } else {
        Expression::Sum(
            Box::new(create_nested_expression(depth - 1)),
            Box::new(Expression::Number(depth as i32)),
        )
    }
}

fn create_typical_ocr_expression() -> Expression {
    // Typical expression: (a + b)^2 = a^2 + 2ab + b^2
    Expression::Sum(
        Box::new(Expression::Sum(
            Box::new(Expression::Power(
                Box::new(Expression::Variable("a".to_string())),
                Box::new(Expression::Number(2)),
            )),
            Box::new(Expression::Product(
                Box::new(Expression::Product(
                    Box::new(Expression::Number(2)),
                    Box::new(Expression::Variable("a".to_string())),
                )),
                Box::new(Expression::Variable("b".to_string())),
            )),
        )),
        Box::new(Expression::Power(
            Box::new(Expression::Variable("b".to_string())),
            Box::new(Expression::Number(2)),
        )),
    )
}

fn count_nodes(expr: &Expression) -> usize {
    match expr {
        Expression::Number(_) | Expression::Variable(_) => 1,
        Expression::Fraction(a, b)
        | Expression::Power(a, b)
        | Expression::Sum(a, b)
        | Expression::Product(a, b) => 1 + count_nodes(a) + count_nodes(b),
        Expression::Matrix(rows) => {
            1 + rows
                .iter()
                .map(|row| row.iter().map(|e| count_nodes(e)).sum::<usize>())
                .sum::<usize>()
        }
        Expression::Integral(expr, _, _, _) | Expression::Summation(expr, _, _, _) => {
            1 + count_nodes(expr)
        }
    }
}

fn escape_latex(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '_' => "\\_".to_string(),
            '{' => "\\{".to_string(),
            '}' => "\\}".to_string(),
            '&' => "\\&".to_string(),
            '%' => "\\%".to_string(),
            '$' => "\\$".to_string(),
            '#' => "\\#".to_string(),
            '^' => "\\^{}".to_string(),
            '~' => "\\~{}".to_string(),
            '\\' => "\\textbackslash{}".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

criterion_group!(
    benches,
    bench_simple_expressions,
    bench_complex_expressions,
    bench_ast_traversal,
    bench_string_building,
    bench_latex_escaping,
    bench_latency_target,
    bench_batch_generation
);
criterion_main!(benches);
