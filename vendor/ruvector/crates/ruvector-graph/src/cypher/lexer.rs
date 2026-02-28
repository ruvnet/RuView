//! Lexical analyzer (tokenizer) for Cypher query language
//!
//! Converts raw Cypher text into a stream of tokens for parsing.

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{char, multispace0, multispace1, one_of},
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Token with kind and location information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub position: Position,
}

/// Source position for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Tokenize a Cypher query string
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError> {
    let mut tokens = Vec::new();
    let mut remaining = input;
    let mut position = Position {
        line: 1,
        column: 1,
        offset: 0,
    };

    while !remaining.is_empty() {
        // Skip whitespace
        if let Ok((rest, _)) = multispace1::<_, nom::error::Error<_>>(remaining) {
            let consumed = remaining.len() - rest.len();
            update_position(&mut position, &remaining[..consumed]);
            remaining = rest;
            continue;
        }

        // Try to parse a token
        match parse_token(remaining) {
            Ok((rest, (kind, lexeme))) => {
                tokens.push(Token {
                    kind,
                    lexeme: lexeme.to_string(),
                    position,
                });
                update_position(&mut position, lexeme);
                remaining = rest;
            }
            Err(_) => {
                return Err(LexerError::UnexpectedCharacter {
                    character: remaining.chars().next().unwrap(),
                    position,
                });
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        lexeme: String::new(),
        position,
    });

    Ok(tokens)
}

fn update_position(pos: &mut Position, text: &str) {
    for ch in text.chars() {
        pos.offset += ch.len_utf8();
        if ch == '\n' {
            pos.line += 1;
            pos.column = 1;
        } else {
            pos.column += 1;
        }
    }
}

fn parse_token(input: &str) -> IResult<&str, (TokenKind, &str)> {
    alt((
        parse_keyword,
        parse_number,
        parse_string,
        parse_identifier,
        parse_operator,
        parse_delimiter,
    ))(input)
}

fn parse_keyword(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    // Split into nested alt() calls since nom's alt() supports max 21 alternatives
    alt((
        alt((
            map(tag_no_case("OPTIONAL MATCH"), |s: &str| {
                (TokenKind::OptionalMatch, s)
            }),
            map(tag_no_case("DETACH DELETE"), |s: &str| {
                (TokenKind::DetachDelete, s)
            }),
            map(tag_no_case("ORDER BY"), |s: &str| (TokenKind::OrderBy, s)),
            map(tag_no_case("ON CREATE"), |s: &str| (TokenKind::OnCreate, s)),
            map(tag_no_case("ON MATCH"), |s: &str| (TokenKind::OnMatch, s)),
            map(tag_no_case("MATCH"), |s: &str| (TokenKind::Match, s)),
            map(tag_no_case("WHERE"), |s: &str| (TokenKind::Where, s)),
            map(tag_no_case("RETURN"), |s: &str| (TokenKind::Return, s)),
            map(tag_no_case("CREATE"), |s: &str| (TokenKind::Create, s)),
            map(tag_no_case("MERGE"), |s: &str| (TokenKind::Merge, s)),
            map(tag_no_case("DELETE"), |s: &str| (TokenKind::Delete, s)),
            map(tag_no_case("SET"), |s: &str| (TokenKind::Set, s)),
            map(tag_no_case("REMOVE"), |s: &str| (TokenKind::Remove, s)),
            map(tag_no_case("WITH"), |s: &str| (TokenKind::With, s)),
            map(tag_no_case("LIMIT"), |s: &str| (TokenKind::Limit, s)),
            map(tag_no_case("SKIP"), |s: &str| (TokenKind::Skip, s)),
            map(tag_no_case("DISTINCT"), |s: &str| (TokenKind::Distinct, s)),
        )),
        alt((
            map(tag_no_case("ASC"), |s: &str| (TokenKind::Asc, s)),
            map(tag_no_case("DESC"), |s: &str| (TokenKind::Desc, s)),
            map(tag_no_case("CASE"), |s: &str| (TokenKind::Case, s)),
            map(tag_no_case("WHEN"), |s: &str| (TokenKind::When, s)),
            map(tag_no_case("THEN"), |s: &str| (TokenKind::Then, s)),
            map(tag_no_case("ELSE"), |s: &str| (TokenKind::Else, s)),
            map(tag_no_case("END"), |s: &str| (TokenKind::End, s)),
            map(tag_no_case("AND"), |s: &str| (TokenKind::And, s)),
            map(tag_no_case("OR"), |s: &str| (TokenKind::Or, s)),
            map(tag_no_case("XOR"), |s: &str| (TokenKind::Xor, s)),
            map(tag_no_case("NOT"), |s: &str| (TokenKind::Not, s)),
            map(tag_no_case("IN"), |s: &str| (TokenKind::In, s)),
            map(tag_no_case("IS"), |s: &str| (TokenKind::Is, s)),
            map(tag_no_case("NULL"), |s: &str| (TokenKind::Null, s)),
            map(tag_no_case("TRUE"), |s: &str| (TokenKind::True, s)),
            map(tag_no_case("FALSE"), |s: &str| (TokenKind::False, s)),
            map(tag_no_case("AS"), |s: &str| (TokenKind::As, s)),
        )),
    ))(input)
}

fn parse_number(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    // Try to parse float first
    if let Ok((rest, num_str)) = recognize::<_, _, nom::error::Error<_>, _>(tuple((
        opt(char('-')),
        take_while1(|c: char| c.is_ascii_digit()),
        char('.'),
        take_while1(|c: char| c.is_ascii_digit()),
        opt(tuple((
            one_of("eE"),
            opt(one_of("+-")),
            take_while1(|c: char| c.is_ascii_digit()),
        ))),
    )))(input)
    {
        if let Ok(n) = num_str.parse::<f64>() {
            return Ok((rest, (TokenKind::Float(n), num_str)));
        }
    }

    // Parse integer
    let (rest, num_str) = recognize(tuple((
        opt(char('-')),
        take_while1(|c: char| c.is_ascii_digit()),
    )))(input)?;

    let n = num_str.parse::<i64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;

    Ok((rest, (TokenKind::Integer(n), num_str)))
}

fn parse_string(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    let (rest, s) = alt((
        delimited(
            char('\''),
            recognize(many0(alt((
                tag("\\'"),
                tag("\\\\"),
                take_while1(|c| c != '\'' && c != '\\'),
            )))),
            char('\''),
        ),
        delimited(
            char('"'),
            recognize(many0(alt((
                tag("\\\""),
                tag("\\\\"),
                take_while1(|c| c != '"' && c != '\\'),
            )))),
            char('"'),
        ),
    ))(input)?;

    // Unescape string
    let unescaped = s
        .replace("\\'", "'")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");

    Ok((rest, (TokenKind::String(unescaped), s)))
}

fn parse_identifier(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    // Backtick-quoted identifier
    let backtick_result: IResult<&str, &str> =
        delimited(char('`'), take_while1(|c| c != '`'), char('`'))(input);
    if let Ok((rest, id)) = backtick_result {
        return Ok((rest, (TokenKind::Identifier(id.to_string()), id)));
    }

    // Regular identifier
    let (rest, id) = recognize(pair(
        alt((
            take_while1(|c: char| c.is_ascii_alphabetic() || c == '_'),
            tag("$"),
        )),
        take_while(|c: char| c.is_ascii_alphanumeric() || c == '_'),
    ))(input)?;

    Ok((rest, (TokenKind::Identifier(id.to_string()), id)))
}

fn parse_operator(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    alt((
        map(tag("<="), |s| (TokenKind::LessThanOrEqual, s)),
        map(tag(">="), |s| (TokenKind::GreaterThanOrEqual, s)),
        map(tag("<>"), |s| (TokenKind::NotEqual, s)),
        map(tag("!="), |s| (TokenKind::NotEqual, s)),
        map(tag("->"), |s| (TokenKind::Arrow, s)),
        map(tag("<-"), |s| (TokenKind::LeftArrow, s)),
        map(tag(".."), |s| (TokenKind::DotDot, s)),
        map(char('+'), |_| (TokenKind::Plus, "+")),
        map(char('-'), |_| (TokenKind::Dash, "-")),
        map(char('*'), |_| (TokenKind::Star, "*")),
        map(char('/'), |_| (TokenKind::Slash, "/")),
        map(char('%'), |_| (TokenKind::Percent, "%")),
        map(char('^'), |_| (TokenKind::Caret, "^")),
        map(char('='), |_| (TokenKind::Equal, "=")),
        map(char('<'), |_| (TokenKind::LessThan, "<")),
        map(char('>'), |_| (TokenKind::GreaterThan, ">")),
    ))(input)
}

fn parse_delimiter(input: &str) -> IResult<&str, (TokenKind, &str)> {
    let (input, _) = multispace0(input)?;

    alt((
        map(char('('), |_| (TokenKind::LeftParen, "(")),
        map(char(')'), |_| (TokenKind::RightParen, ")")),
        map(char('['), |_| (TokenKind::LeftBracket, "[")),
        map(char(']'), |_| (TokenKind::RightBracket, "]")),
        map(char('{'), |_| (TokenKind::LeftBrace, "{")),
        map(char('}'), |_| (TokenKind::RightBrace, "}")),
        map(char(','), |_| (TokenKind::Comma, ",")),
        map(char('.'), |_| (TokenKind::Dot, ".")),
        map(char(':'), |_| (TokenKind::Colon, ":")),
        map(char(';'), |_| (TokenKind::Semicolon, ";")),
        map(char('|'), |_| (TokenKind::Pipe, "|")),
    ))(input)
}

#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unexpected character '{character}' at line {}, column {}", position.line, position.column)]
    UnexpectedCharacter { character: char, position: Position },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_match() {
        let input = "MATCH (n:Person) RETURN n";
        let tokens = tokenize(input).unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Match);
        assert_eq!(tokens[1].kind, TokenKind::LeftParen);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("n".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Colon);
        assert_eq!(tokens[4].kind, TokenKind::Identifier("Person".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::RightParen);
        assert_eq!(tokens[6].kind, TokenKind::Return);
        assert_eq!(tokens[7].kind, TokenKind::Identifier("n".to_string()));
    }

    #[test]
    fn test_tokenize_numbers() {
        let tokens = tokenize("123 45.67 -89 3.14e-2").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Integer(123));
        assert_eq!(tokens[1].kind, TokenKind::Float(45.67));
        assert_eq!(tokens[2].kind, TokenKind::Integer(-89));
        assert!(matches!(tokens[3].kind, TokenKind::Float(_)));
    }

    #[test]
    fn test_tokenize_strings() {
        let tokens = tokenize(r#"'Alice' "Bob's friend""#).unwrap();
        assert_eq!(tokens[0].kind, TokenKind::String("Alice".to_string()));
        assert_eq!(
            tokens[1].kind,
            TokenKind::String("Bob's friend".to_string())
        );
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = tokenize("-> <- = <> >= <=").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Arrow);
        assert_eq!(tokens[1].kind, TokenKind::LeftArrow);
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::NotEqual);
        assert_eq!(tokens[4].kind, TokenKind::GreaterThanOrEqual);
        assert_eq!(tokens[5].kind, TokenKind::LessThanOrEqual);
    }
}
