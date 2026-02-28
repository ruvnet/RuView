// Hand-rolled SQL parser for WASM compatibility
// Implements recursive descent parsing for vector-specific SQL

use super::ast::*;
use std::fmt;

/// Parse error type
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Token types
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Create,
    Table,
    Drop,
    OrderBy,
    Limit,
    And,
    Or,
    Not,
    As,

    // Data types
    Text,
    Integer,
    Real,
    Vector,

    // Operators
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    Like,

    // Distance operators
    L2Distance,     // <->
    CosineDistance, // <=>
    DotProduct,     // <#>

    // Delimiters
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Asterisk,

    // Values
    Identifier(String),
    StringLiteral(String),
    NumberLiteral(String),

    // End
    Eof,
}

/// Tokenizer (lexer)
struct Tokenizer {
    input: Vec<char>,
    position: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Tokenizer {
            input: input.chars().collect(),
            position: 0,
        }
    }

    fn current(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_identifier(&mut self) -> String {
        let mut result = String::new();
        while let Some(ch) = self.current() {
            if ch.is_alphanumeric() || ch == '_' {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        result
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        let mut result = String::new();
        self.advance(); // Skip opening quote

        while let Some(ch) = self.current() {
            if ch == '\'' {
                self.advance();
                return Ok(result);
            } else {
                result.push(ch);
                self.advance();
            }
        }

        Err(ParseError {
            message: "Unterminated string literal".to_string(),
            position: self.position,
        })
    }

    fn read_number(&mut self) -> String {
        let mut result = String::new();
        let mut has_dot = false;

        while let Some(ch) = self.current() {
            if ch.is_numeric() {
                result.push(ch);
                self.advance();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        result
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        let ch = match self.current() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        match ch {
            '(' => {
                self.advance();
                Ok(Token::LeftParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RightParen)
            }
            '[' => {
                self.advance();
                Ok(Token::LeftBracket)
            }
            ']' => {
                self.advance();
                Ok(Token::RightBracket)
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            ';' => {
                self.advance();
                Ok(Token::Semicolon)
            }
            '*' => {
                self.advance();
                Ok(Token::Asterisk)
            }
            '=' => {
                self.advance();
                Ok(Token::Eq)
            }
            '!' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    Ok(Token::NotEq)
                } else {
                    Err(ParseError {
                        message: "Expected '=' after '!'".to_string(),
                        position: self.position,
                    })
                }
            }
            '>' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    Ok(Token::GtEq)
                } else {
                    Ok(Token::Gt)
                }
            }
            '<' => {
                self.advance();
                match self.current() {
                    Some('=') => {
                        self.advance();
                        if self.current() == Some('>') {
                            self.advance();
                            Ok(Token::CosineDistance)
                        } else {
                            Ok(Token::LtEq)
                        }
                    }
                    Some('-') => {
                        self.advance();
                        if self.current() == Some('>') {
                            self.advance();
                            Ok(Token::L2Distance)
                        } else {
                            Err(ParseError {
                                message: "Expected '>' after '<-'".to_string(),
                                position: self.position,
                            })
                        }
                    }
                    Some('#') => {
                        self.advance();
                        if self.current() == Some('>') {
                            self.advance();
                            Ok(Token::DotProduct)
                        } else {
                            Err(ParseError {
                                message: "Expected '>' after '<#'".to_string(),
                                position: self.position,
                            })
                        }
                    }
                    _ => Ok(Token::Lt),
                }
            }
            '\'' => Ok(Token::StringLiteral(self.read_string()?)),
            _ if ch.is_numeric() => Ok(Token::NumberLiteral(self.read_number())),
            _ if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_identifier();
                Ok(match ident.to_uppercase().as_str() {
                    "SELECT" => Token::Select,
                    "FROM" => Token::From,
                    "WHERE" => Token::Where,
                    "INSERT" => Token::Insert,
                    "INTO" => Token::Into,
                    "VALUES" => Token::Values,
                    "CREATE" => Token::Create,
                    "TABLE" => Token::Table,
                    "DROP" => Token::Drop,
                    "ORDER" => {
                        self.skip_whitespace();
                        if self.read_identifier().to_uppercase() == "BY" {
                            Token::OrderBy
                        } else {
                            Token::Identifier(ident)
                        }
                    }
                    "LIMIT" => Token::Limit,
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "NOT" => Token::Not,
                    "AS" => Token::As,
                    "TEXT" => Token::Text,
                    "INTEGER" => Token::Integer,
                    "REAL" => Token::Real,
                    "VECTOR" => Token::Vector,
                    "LIKE" => Token::Like,
                    _ => Token::Identifier(ident),
                })
            }
            _ => Err(ParseError {
                message: format!("Unexpected character: {}", ch),
                position: self.position,
            }),
        }
    }
}

/// SQL Parser
pub struct SqlParser {
    tokens: Vec<Token>,
    position: usize,
}

impl SqlParser {
    /// Create a new parser from SQL string
    pub fn new(input: &str) -> Result<Self, ParseError> {
        let mut tokenizer = Tokenizer::new(input);
        let mut tokens = Vec::new();

        loop {
            let token = tokenizer.next_token()?;
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }

        Ok(SqlParser {
            tokens,
            position: 0,
        })
    }

    /// Parse SQL statement
    pub fn parse(&mut self) -> Result<SqlStatement, ParseError> {
        let token = self.current().clone();

        match token {
            Token::Select => self.parse_select(),
            Token::Insert => self.parse_insert(),
            Token::Create => self.parse_create(),
            Token::Drop => self.parse_drop(),
            _ => Err(ParseError {
                message: format!("Expected SELECT, INSERT, CREATE, or DROP, got {:?}", token),
                position: self.position,
            }),
        }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.position < self.tokens.len() {
            self.position += 1;
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let current = self.current().clone();
        if current == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: format!("Expected {:?}, got {:?}", expected, current),
                position: self.position,
            })
        }
    }

    fn parse_select(&mut self) -> Result<SqlStatement, ParseError> {
        self.expect(Token::Select)?;

        let columns = self.parse_select_columns()?;

        self.expect(Token::From)?;
        let from = self.parse_identifier()?;

        let where_clause = if matches!(self.current(), Token::Where) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };

        let order_by = if matches!(self.current(), Token::OrderBy) {
            self.advance();
            Some(self.parse_order_by()?)
        } else {
            None
        };

        let limit = if matches!(self.current(), Token::Limit) {
            self.advance();
            Some(self.parse_number()? as usize)
        } else {
            None
        };

        Ok(SqlStatement::Select {
            columns,
            from,
            where_clause,
            order_by,
            limit,
        })
    }

    fn parse_select_columns(&mut self) -> Result<Vec<SelectColumn>, ParseError> {
        if matches!(self.current(), Token::Asterisk) {
            self.advance();
            return Ok(vec![SelectColumn::Wildcard]);
        }

        let mut columns = Vec::new();
        loop {
            let name = self.parse_identifier()?;
            columns.push(SelectColumn::Name(name));

            if !matches!(self.current(), Token::Comma) {
                break;
            }
            self.advance();
        }

        Ok(columns)
    }

    fn parse_insert(&mut self) -> Result<SqlStatement, ParseError> {
        self.expect(Token::Insert)?;
        self.expect(Token::Into)?;

        let table = self.parse_identifier()?;

        self.expect(Token::LeftParen)?;
        let columns = self.parse_identifier_list()?;
        self.expect(Token::RightParen)?;

        self.expect(Token::Values)?;
        self.expect(Token::LeftParen)?;
        let values = self.parse_value_list()?;
        self.expect(Token::RightParen)?;

        Ok(SqlStatement::Insert {
            table,
            columns,
            values,
        })
    }

    fn parse_create(&mut self) -> Result<SqlStatement, ParseError> {
        self.expect(Token::Create)?;
        self.expect(Token::Table)?;

        let name = self.parse_identifier()?;

        self.expect(Token::LeftParen)?;
        let columns = self.parse_column_definitions()?;
        self.expect(Token::RightParen)?;

        Ok(SqlStatement::CreateTable { name, columns })
    }

    fn parse_drop(&mut self) -> Result<SqlStatement, ParseError> {
        self.expect(Token::Drop)?;
        self.expect(Token::Table)?;

        let table = self.parse_identifier()?;

        Ok(SqlStatement::Drop { table })
    }

    fn parse_column_definitions(&mut self) -> Result<Vec<Column>, ParseError> {
        let mut columns = Vec::new();

        loop {
            let name = self.parse_identifier()?;
            let data_type = self.parse_data_type()?;

            columns.push(Column { name, data_type });

            if !matches!(self.current(), Token::Comma) {
                break;
            }
            self.advance();
        }

        Ok(columns)
    }

    fn parse_data_type(&mut self) -> Result<DataType, ParseError> {
        match self.current().clone() {
            Token::Text => {
                self.advance();
                Ok(DataType::Text)
            }
            Token::Integer => {
                self.advance();
                Ok(DataType::Integer)
            }
            Token::Real => {
                self.advance();
                Ok(DataType::Real)
            }
            Token::Vector => {
                self.advance();
                self.expect(Token::LeftParen)?;
                let dims = self.parse_number()? as usize;
                self.expect(Token::RightParen)?;
                Ok(DataType::Vector(dims))
            }
            _ => Err(ParseError {
                message: "Expected data type (TEXT, INTEGER, REAL, or VECTOR)".to_string(),
                position: self.position,
            }),
        }
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_and_expression()?;

        while matches!(self.current(), Token::Or) {
            self.advance();
            let right = self.parse_and_expression()?;
            left = Expression::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_comparison_expression()?;

        while matches!(self.current(), Token::And) {
            self.advance();
            let right = self.parse_comparison_expression()?;
            left = Expression::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_comparison_expression(&mut self) -> Result<Expression, ParseError> {
        let left = self.parse_primary_expression()?;

        let op = match self.current() {
            Token::Eq => BinaryOperator::Eq,
            Token::NotEq => BinaryOperator::NotEq,
            Token::Gt => BinaryOperator::Gt,
            Token::GtEq => BinaryOperator::GtEq,
            Token::Lt => BinaryOperator::Lt,
            Token::LtEq => BinaryOperator::LtEq,
            Token::Like => BinaryOperator::Like,
            _ => return Ok(left),
        };

        self.advance();
        let right = self.parse_primary_expression()?;

        Ok(Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ParseError> {
        match self.current().clone() {
            Token::Identifier(name) => {
                self.advance();
                Ok(Expression::Column(name))
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Expression::Literal(Value::Text(s)))
            }
            Token::NumberLiteral(n) => {
                self.advance();
                let value = if n.contains('.') {
                    Value::Real(n.parse().unwrap())
                } else {
                    Value::Integer(n.parse().unwrap())
                };
                Ok(Expression::Literal(value))
            }
            Token::LeftBracket => {
                self.advance();
                let vec = self.parse_vector_literal()?;
                self.expect(Token::RightBracket)?;
                Ok(Expression::VectorLiteral(vec))
            }
            Token::Not => {
                self.advance();
                let expr = self.parse_primary_expression()?;
                Ok(Expression::Not(Box::new(expr)))
            }
            _ => Err(ParseError {
                message: format!("Unexpected token in expression: {:?}", self.current()),
                position: self.position,
            }),
        }
    }

    fn parse_order_by(&mut self) -> Result<OrderBy, ParseError> {
        // Parse column <-> vector or column <=> vector
        let column = self.parse_identifier()?;

        let metric = match self.current() {
            Token::L2Distance => {
                self.advance();
                DistanceMetric::L2
            }
            Token::CosineDistance => {
                self.advance();
                DistanceMetric::Cosine
            }
            Token::DotProduct => {
                self.advance();
                DistanceMetric::DotProduct
            }
            _ => {
                return Err(ParseError {
                    message: "Expected distance operator (<->, <=>, or <#>)".to_string(),
                    position: self.position,
                });
            }
        };

        let vector = if matches!(self.current(), Token::LeftBracket) {
            self.advance();
            let vec = self.parse_vector_literal()?;
            self.expect(Token::RightBracket)?;
            vec
        } else {
            return Err(ParseError {
                message: "Expected vector literal after distance operator".to_string(),
                position: self.position,
            });
        };

        Ok(OrderBy {
            expression: Expression::Distance {
                column,
                metric,
                vector,
            },
            direction: OrderDirection::Asc,
        })
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        match self.current().clone() {
            Token::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError {
                message: "Expected identifier".to_string(),
                position: self.position,
            }),
        }
    }

    fn parse_identifier_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut identifiers = Vec::new();

        loop {
            identifiers.push(self.parse_identifier()?);

            if !matches!(self.current(), Token::Comma) {
                break;
            }
            self.advance();
        }

        Ok(identifiers)
    }

    fn parse_value_list(&mut self) -> Result<Vec<Value>, ParseError> {
        let mut values = Vec::new();

        loop {
            values.push(self.parse_value()?);

            if !matches!(self.current(), Token::Comma) {
                break;
            }
            self.advance();
        }

        Ok(values)
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.current().clone() {
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Value::Text(s))
            }
            Token::NumberLiteral(n) => {
                self.advance();
                if n.contains('.') {
                    Ok(Value::Real(n.parse().unwrap()))
                } else {
                    Ok(Value::Integer(n.parse().unwrap()))
                }
            }
            Token::LeftBracket => {
                self.advance();
                let vec = self.parse_vector_literal()?;
                self.expect(Token::RightBracket)?;
                Ok(Value::Vector(vec))
            }
            _ => Err(ParseError {
                message: format!("Expected value, got {:?}", self.current()),
                position: self.position,
            }),
        }
    }

    fn parse_vector_literal(&mut self) -> Result<Vec<f32>, ParseError> {
        let mut values = Vec::new();

        loop {
            let n = self.parse_number()?;
            values.push(n as f32);

            if !matches!(self.current(), Token::Comma) {
                break;
            }
            self.advance();
        }

        Ok(values)
    }

    fn parse_number(&mut self) -> Result<f64, ParseError> {
        match self.current().clone() {
            Token::NumberLiteral(n) => {
                self.advance();
                n.parse().map_err(|_| ParseError {
                    message: format!("Invalid number: {}", n),
                    position: self.position,
                })
            }
            _ => Err(ParseError {
                message: "Expected number".to_string(),
                position: self.position,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table() {
        let sql = "CREATE TABLE documents (id TEXT, content TEXT, embedding VECTOR(384))";
        let mut parser = SqlParser::new(sql).unwrap();
        let stmt = parser.parse().unwrap();

        match stmt {
            SqlStatement::CreateTable { name, columns } => {
                assert_eq!(name, "documents");
                assert_eq!(columns.len(), 3);
                assert_eq!(columns[2].data_type, DataType::Vector(384));
            }
            _ => panic!("Expected CreateTable"),
        }
    }

    #[test]
    fn test_parse_insert() {
        let sql =
            "INSERT INTO documents (id, content, embedding) VALUES ('1', 'hello', [1.0, 2.0, 3.0])";
        let mut parser = SqlParser::new(sql).unwrap();
        let stmt = parser.parse().unwrap();

        match stmt {
            SqlStatement::Insert {
                table,
                columns,
                values,
            } => {
                assert_eq!(table, "documents");
                assert_eq!(columns.len(), 3);
                assert_eq!(values.len(), 3);
            }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_parse_select_with_vector_search() {
        let sql = "SELECT * FROM documents ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 5";
        let mut parser = SqlParser::new(sql).unwrap();
        let stmt = parser.parse().unwrap();

        match stmt {
            SqlStatement::Select {
                order_by, limit, ..
            } => {
                assert!(order_by.is_some());
                assert_eq!(limit, Some(5));
            }
            _ => panic!("Expected Select"),
        }
    }
}
