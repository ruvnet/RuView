//! Lexical analyzer (tokenizer) for Cypher query language
//!
//! Hand-rolled lexer for WASM compatibility - no external dependencies.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

/// Token with kind and location information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub position: Position,
}

/// Source position for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

/// Token kinds
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    // Keywords
    Match,
    OptionalMatch,
    Where,
    Return,
    Create,
    Merge,
    Delete,
    DetachDelete,
    Set,
    Remove,
    With,
    OrderBy,
    Limit,
    Skip,
    Distinct,
    As,
    Asc,
    Desc,
    Case,
    When,
    Then,
    Else,
    End,
    And,
    Or,
    Xor,
    Not,
    In,
    Is,
    Null,
    True,
    False,
    OnCreate,
    OnMatch,

    // Identifiers and literals
    Identifier(String),
    Integer(i64),
    Float(f64),
    String(String),

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Arrow,     // ->
    LeftArrow, // <-
    Dash,      // -

    // Delimiters
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Pipe,

    // Special
    DotDot, // ..
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Identifier(s) => write!(f, "identifier '{}'", s),
            TokenKind::Integer(n) => write!(f, "integer {}", n),
            TokenKind::Float(n) => write!(f, "float {}", n),
            TokenKind::String(s) => write!(f, "string \"{}\"", s),
            _ => write!(f, "{:?}", self),
        }
    }
}

/// Lexer error
#[derive(Debug, Clone)]
pub struct LexerError {
    pub message: String,
    pub position: Position,
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lexer error at {}:{}: {}",
            self.position.line, self.position.column, self.message
        )
    }
}

impl std::error::Error for LexerError {}

/// Hand-rolled Cypher lexer
pub struct Lexer<'a> {
    input: &'a str,
    chars: Peekable<Chars<'a>>,
    position: Position,
    current_offset: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().peekable(),
            position: Position {
                line: 1,
                column: 1,
                offset: 0,
            },
            current_offset: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.current_offset += ch.len_utf8();
        if ch == '\n' {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
        self.position.offset = self.current_offset;
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '/' && self.lookahead(1) == Some('/') {
                // Skip line comments
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn lookahead(&self, n: usize) -> Option<char> {
        self.input[self.current_offset..].chars().nth(n)
    }

    fn make_token(&self, kind: TokenKind, lexeme: &str, start_pos: Position) -> Token {
        Token {
            kind,
            lexeme: lexeme.to_string(),
            position: start_pos,
        }
    }

    fn scan_string(&mut self, quote: char) -> Result<Token, LexerError> {
        let start = self.position;
        self.advance(); // consume opening quote
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch == quote {
                self.advance(); // consume closing quote
                return Ok(self.make_token(TokenKind::String(value.clone()), &value, start));
            } else if ch == '\\' {
                self.advance();
                match self.peek() {
                    Some('n') => {
                        value.push('\n');
                        self.advance();
                    }
                    Some('t') => {
                        value.push('\t');
                        self.advance();
                    }
                    Some('r') => {
                        value.push('\r');
                        self.advance();
                    }
                    Some('\\') => {
                        value.push('\\');
                        self.advance();
                    }
                    Some(c) if c == quote => {
                        value.push(c);
                        self.advance();
                    }
                    _ => value.push('\\'),
                }
            } else {
                value.push(ch);
                self.advance();
            }
        }

        Err(LexerError {
            message: "Unterminated string".to_string(),
            position: start,
        })
    }

    fn scan_number(&mut self) -> Token {
        let start = self.position;
        let start_offset = self.current_offset;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal
        if self.peek() == Some('.')
            && self
                .lookahead(1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            self.advance(); // consume '.'
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
            let lexeme = &self.input[start_offset..self.current_offset];
            let value: f64 = lexeme.parse().unwrap_or(0.0);
            return self.make_token(TokenKind::Float(value), lexeme, start);
        }

        // Check for exponent
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.advance();
            }
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
            let lexeme = &self.input[start_offset..self.current_offset];
            let value: f64 = lexeme.parse().unwrap_or(0.0);
            return self.make_token(TokenKind::Float(value), lexeme, start);
        }

        let lexeme = &self.input[start_offset..self.current_offset];
        let value: i64 = lexeme.parse().unwrap_or(0);
        self.make_token(TokenKind::Integer(value), lexeme, start)
    }

    fn scan_identifier(&mut self) -> Token {
        let start = self.position;
        let start_offset = self.current_offset;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let lexeme = &self.input[start_offset..self.current_offset];
        let kind = match lexeme.to_uppercase().as_str() {
            "MATCH" => TokenKind::Match,
            "OPTIONAL" if self.peek_keyword("MATCH") => {
                self.skip_whitespace();
                self.scan_keyword("MATCH");
                TokenKind::OptionalMatch
            }
            "WHERE" => TokenKind::Where,
            "RETURN" => TokenKind::Return,
            "CREATE" => TokenKind::Create,
            "MERGE" => TokenKind::Merge,
            "DELETE" => TokenKind::Delete,
            "DETACH" if self.peek_keyword("DELETE") => {
                self.skip_whitespace();
                self.scan_keyword("DELETE");
                TokenKind::DetachDelete
            }
            "SET" => TokenKind::Set,
            "REMOVE" => TokenKind::Remove,
            "WITH" => TokenKind::With,
            "ORDER" if self.peek_keyword("BY") => {
                self.skip_whitespace();
                self.scan_keyword("BY");
                TokenKind::OrderBy
            }
            "LIMIT" => TokenKind::Limit,
            "SKIP" => TokenKind::Skip,
            "DISTINCT" => TokenKind::Distinct,
            "AS" => TokenKind::As,
            "ASC" => TokenKind::Asc,
            "DESC" => TokenKind::Desc,
            "CASE" => TokenKind::Case,
            "WHEN" => TokenKind::When,
            "THEN" => TokenKind::Then,
            "ELSE" => TokenKind::Else,
            "END" => TokenKind::End,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "XOR" => TokenKind::Xor,
            "NOT" => TokenKind::Not,
            "IN" => TokenKind::In,
            "IS" => TokenKind::Is,
            "NULL" => TokenKind::Null,
            "TRUE" => TokenKind::True,
            "FALSE" => TokenKind::False,
            "ON" if self.peek_keyword("CREATE") => {
                self.skip_whitespace();
                self.scan_keyword("CREATE");
                TokenKind::OnCreate
            }
            _ if lexeme.to_uppercase() == "ON" && self.peek_keyword("MATCH") => {
                self.skip_whitespace();
                self.scan_keyword("MATCH");
                TokenKind::OnMatch
            }
            _ => TokenKind::Identifier(lexeme.to_string()),
        };

        self.make_token(kind, lexeme, start)
    }

    fn peek_keyword(&mut self, keyword: &str) -> bool {
        let saved_offset = self.current_offset;
        self.skip_whitespace();
        let remaining = &self.input[self.current_offset..];
        let matches = remaining.to_uppercase().starts_with(keyword)
            && remaining
                .chars()
                .nth(keyword.len())
                .map(|c| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(true);
        // Reset position if not consuming
        if !matches {
            self.current_offset = saved_offset;
            self.chars = self.input[saved_offset..].chars().peekable();
        }
        matches
    }

    fn scan_keyword(&mut self, keyword: &str) {
        for _ in 0..keyword.len() {
            self.advance();
        }
    }

    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        self.skip_whitespace();

        let start = self.position;

        match self.peek() {
            None => Ok(self.make_token(TokenKind::Eof, "", start)),
            Some(ch) => {
                match ch {
                    // Strings
                    '"' | '\'' => self.scan_string(ch),

                    // Numbers
                    '0'..='9' => Ok(self.scan_number()),

                    // Identifiers
                    'a'..='z' | 'A'..='Z' | '_' | '$' => Ok(self.scan_identifier()),

                    // Backtick-quoted identifiers
                    '`' => {
                        self.advance();
                        let id_start = self.current_offset;
                        while let Some(c) = self.peek() {
                            if c == '`' {
                                break;
                            }
                            self.advance();
                        }
                        let id = self.input[id_start..self.current_offset].to_string();
                        self.advance(); // consume closing backtick
                        Ok(self.make_token(TokenKind::Identifier(id.clone()), &id, start))
                    }

                    // Two-character operators
                    '<' => {
                        self.advance();
                        match self.peek() {
                            Some('=') => {
                                self.advance();
                                Ok(self.make_token(TokenKind::LessThanOrEqual, "<=", start))
                            }
                            Some('>') => {
                                self.advance();
                                Ok(self.make_token(TokenKind::NotEqual, "<>", start))
                            }
                            Some('-') => {
                                self.advance();
                                Ok(self.make_token(TokenKind::LeftArrow, "<-", start))
                            }
                            _ => Ok(self.make_token(TokenKind::LessThan, "<", start)),
                        }
                    }
                    '>' => {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            Ok(self.make_token(TokenKind::GreaterThanOrEqual, ">=", start))
                        } else {
                            Ok(self.make_token(TokenKind::GreaterThan, ">", start))
                        }
                    }
                    '-' => {
                        self.advance();
                        if self.peek() == Some('>') {
                            self.advance();
                            Ok(self.make_token(TokenKind::Arrow, "->", start))
                        } else {
                            Ok(self.make_token(TokenKind::Dash, "-", start))
                        }
                    }
                    '.' => {
                        self.advance();
                        if self.peek() == Some('.') {
                            self.advance();
                            Ok(self.make_token(TokenKind::DotDot, "..", start))
                        } else {
                            Ok(self.make_token(TokenKind::Dot, ".", start))
                        }
                    }
                    '=' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Equal, "=", start))
                    }

                    // Single-character tokens
                    '(' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::LeftParen, "(", start))
                    }
                    ')' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::RightParen, ")", start))
                    }
                    '[' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::LeftBracket, "[", start))
                    }
                    ']' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::RightBracket, "]", start))
                    }
                    '{' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::LeftBrace, "{", start))
                    }
                    '}' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::RightBrace, "}", start))
                    }
                    ',' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Comma, ",", start))
                    }
                    ':' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Colon, ":", start))
                    }
                    ';' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Semicolon, ";", start))
                    }
                    '|' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Pipe, "|", start))
                    }
                    '+' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Plus, "+", start))
                    }
                    '*' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Star, "*", start))
                    }
                    '/' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Slash, "/", start))
                    }
                    '%' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Percent, "%", start))
                    }
                    '^' => {
                        self.advance();
                        Ok(self.make_token(TokenKind::Caret, "^", start))
                    }

                    _ => Err(LexerError {
                        message: format!("Unexpected character: '{}'", ch),
                        position: start,
                    }),
                }
            }
        }
    }
}

/// Tokenize a Cypher query string
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token()?;
        let is_eof = token.kind == TokenKind::Eof;
        tokens.push(token);
        if is_eof {
            break;
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let tokens = tokenize("MATCH (n) RETURN n").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Match);
        assert_eq!(tokens[1].kind, TokenKind::LeftParen);
    }

    #[test]
    fn test_string() {
        let tokens = tokenize("'hello world'").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::String("hello world".to_string()));
    }

    #[test]
    fn test_number() {
        let tokens = tokenize("42 3.14").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Integer(42));
        assert_eq!(tokens[1].kind, TokenKind::Float(3.14));
    }

    #[test]
    fn test_relationship() {
        let tokens = tokenize("(a)-[:KNOWS]->(b)").unwrap();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Arrow));
    }
}
