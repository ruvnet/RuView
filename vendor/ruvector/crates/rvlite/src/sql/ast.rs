// AST types for SQL statements
use serde::{Deserialize, Serialize};

/// SQL statement types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlStatement {
    /// CREATE TABLE name (columns)
    CreateTable { name: String, columns: Vec<Column> },
    /// INSERT INTO table (columns) VALUES (values)
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Value>,
    },
    /// SELECT columns FROM table WHERE condition ORDER BY ... LIMIT k
    Select {
        columns: Vec<SelectColumn>,
        from: String,
        where_clause: Option<Expression>,
        order_by: Option<OrderBy>,
        limit: Option<usize>,
    },
    /// DROP TABLE name
    Drop { table: String },
}

/// Column definition for CREATE TABLE
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
}

/// Data types supported in SQL
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    /// TEXT type for strings
    Text,
    /// INTEGER type
    Integer,
    /// REAL/FLOAT type
    Real,
    /// VECTOR(dimensions) type for vector data
    Vector(usize),
}

/// Column selector in SELECT
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectColumn {
    /// SELECT *
    Wildcard,
    /// SELECT column_name
    Name(String),
    /// SELECT expression AS alias
    Expression {
        expr: Expression,
        alias: Option<String>,
    },
}

/// SQL expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// Column reference
    Column(String),
    /// Literal value
    Literal(Value),
    /// Binary operation (e.g., a = b, a > b)
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    /// Logical AND
    And(Box<Expression>, Box<Expression>),
    /// Logical OR
    Or(Box<Expression>, Box<Expression>),
    /// NOT expression
    Not(Box<Expression>),
    /// Function call
    Function { name: String, args: Vec<Expression> },
    /// Vector literal [1.0, 2.0, 3.0]
    VectorLiteral(Vec<f32>),
    /// Distance operation: column <-> vector
    /// Used for ORDER BY embedding <-> $vector
    Distance {
        column: String,
        metric: DistanceMetric,
        vector: Vec<f32>,
    },
}

/// Binary operators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    /// =
    Eq,
    /// !=
    NotEq,
    /// >
    Gt,
    /// >=
    GtEq,
    /// <
    Lt,
    /// <=
    LtEq,
    /// LIKE
    Like,
}

/// Distance metrics for vector similarity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// L2 distance: <->
    L2,
    /// Cosine distance: <=>
    Cosine,
    /// Dot product: <#>
    DotProduct,
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBy {
    pub expression: Expression,
    pub direction: OrderDirection,
}

/// Sort direction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// SQL values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Text(String),
    Integer(i64),
    Real(f64),
    Vector(Vec<f32>),
    Boolean(bool),
}

impl Value {
    /// Convert to JSON value for metadata storage
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Text(s) => serde_json::Value::String(s.clone()),
            Value::Integer(i) => serde_json::Value::Number((*i).into()),
            Value::Real(f) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap_or(0.into()))
            }
            Value::Vector(v) => serde_json::Value::Array(
                v.iter()
                    .map(|f| {
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(*f as f64).unwrap_or(0.into()),
                        )
                    })
                    .collect(),
            ),
            Value::Boolean(b) => serde_json::Value::Bool(*b),
        }
    }

    /// Parse from JSON value
    pub fn from_json(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Real(f)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => Value::Text(s.clone()),
            serde_json::Value::Array(arr) => {
                // Try to parse as vector
                let floats: Option<Vec<f32>> =
                    arr.iter().map(|v| v.as_f64().map(|f| f as f32)).collect();

                if let Some(vec) = floats {
                    Value::Vector(vec)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::Object(_) => Value::Null,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Text(s) => write!(f, "'{}'", s),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Real(r) => write!(f, "{}", r),
            Value::Vector(v) => write!(
                f,
                "[{}]",
                v.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Boolean(b) => write!(f, "{}", b),
        }
    }
}
