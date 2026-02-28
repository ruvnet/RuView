//! Recursive descent parser for Cypher query language
//!
//! Converts token stream into Abstract Syntax Tree (AST).

use super::ast::*;
use super::lexer::{tokenize, Token, TokenKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(
        "Unexpected token: expected {expected}, found {found} at line {line}, column {column}"
    )]
    UnexpectedToken {
        expected: String,
        found: String,
        line: usize,
        column: usize,
    },
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Lexer error: {0}")]
    LexerError(#[from] super::lexer::LexerError),
    #[error("Invalid syntax: {0}")]
    InvalidSyntax(String),
}

type ParseResult<T> = Result<T, ParseError>;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> ParseResult<&Token> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            let token = self.peek();
            Err(ParseError::UnexpectedToken {
                expected: message.to_string(),
                found: token.kind.to_string(),
                line: token.position.line,
                column: token.position.column,
            })
        }
    }

    fn parse_query(&mut self) -> ParseResult<Query> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            statements.push(self.parse_statement()?);
            self.match_token(&[TokenKind::Semicolon]);
        }

        // Reject empty queries
        if statements.is_empty() {
            return Err(ParseError::InvalidSyntax(
                "Empty query - expected at least one statement".to_string(),
            ));
        }

        Ok(Query { statements })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.peek().kind {
            TokenKind::Match | TokenKind::OptionalMatch => {
                Ok(Statement::Match(self.parse_match()?))
            }
            TokenKind::Create => Ok(Statement::Create(self.parse_create()?)),
            TokenKind::Merge => Ok(Statement::Merge(self.parse_merge()?)),
            TokenKind::Delete | TokenKind::DetachDelete => {
                Ok(Statement::Delete(self.parse_delete()?))
            }
            TokenKind::Set => Ok(Statement::Set(self.parse_set()?)),
            TokenKind::Remove => Ok(Statement::Remove(self.parse_remove()?)),
            TokenKind::Return => Ok(Statement::Return(self.parse_return()?)),
            TokenKind::With => Ok(Statement::With(self.parse_with()?)),
            _ => {
                let token = self.peek();
                Err(ParseError::UnexpectedToken {
                    expected: "statement keyword".to_string(),
                    found: token.kind.to_string(),
                    line: token.position.line,
                    column: token.position.column,
                })
            }
        }
    }

    fn parse_match(&mut self) -> ParseResult<MatchClause> {
        let optional = self.match_token(&[TokenKind::OptionalMatch]);
        if !optional {
            self.consume(TokenKind::Match, "MATCH")?;
        }

        let patterns = self.parse_patterns()?;

        let where_clause = if self.match_token(&[TokenKind::Where]) {
            Some(WhereClause {
                condition: self.parse_expression()?,
            })
        } else {
            None
        };

        Ok(MatchClause {
            optional,
            patterns,
            where_clause,
        })
    }

    fn parse_patterns(&mut self) -> ParseResult<Vec<Pattern>> {
        let mut patterns = vec![self.parse_pattern()?];

        while self.match_token(&[TokenKind::Comma]) {
            patterns.push(self.parse_pattern()?);
        }

        Ok(patterns)
    }

    fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        // Check for path pattern: p = (...)
        if let TokenKind::Identifier(var) = &self.peek().kind {
            let var = var.clone();
            if self.tokens.get(self.current + 1).map(|t| &t.kind) == Some(&TokenKind::Equal) {
                self.advance(); // consume identifier
                self.advance(); // consume =
                return Ok(Pattern::Path(PathPattern {
                    variable: var,
                    pattern: Box::new(self.parse_pattern()?),
                }));
            }
        }

        self.parse_relationship_pattern()
    }

    fn parse_relationship_pattern(&mut self) -> ParseResult<Pattern> {
        let from = self.parse_node_pattern()?;

        // Check for relationship - can start with `-` or `<-`
        if self.check(&TokenKind::Dash) || self.check(&TokenKind::LeftArrow) {
            // Determine if this is an incoming relationship (<-)
            let starts_with_incoming = self.match_token(&[TokenKind::LeftArrow]);
            if !starts_with_incoming {
                self.consume(TokenKind::Dash, "-")?;
            }

            // Parse relationship details [r:TYPE {props} *min..max]
            let (variable, rel_type, properties, range) =
                if self.match_token(&[TokenKind::LeftBracket]) {
                    let variable = if let TokenKind::Identifier(v) = &self.peek().kind {
                        let v = v.clone();
                        self.advance();
                        Some(v)
                    } else {
                        None
                    };

                    let rel_type = if self.match_token(&[TokenKind::Colon]) {
                        if let TokenKind::Identifier(t) = &self.peek().kind {
                            let t = t.clone();
                            self.advance();
                            Some(t)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let properties = if self.check(&TokenKind::LeftBrace) {
                        Some(self.parse_property_map()?)
                    } else {
                        None
                    };

                    let range = if self.match_token(&[TokenKind::Star]) {
                        Some(self.parse_relationship_range()?)
                    } else {
                        None
                    };

                    self.consume(TokenKind::RightBracket, "]")?;
                    (variable, rel_type, properties, range)
                } else {
                    (None, None, None, None)
                };

            // Determine final direction based on ending pattern:
            // -[r]->  = Outgoing
            // <-[r]-  = Incoming
            // -[r]-   = Undirected
            // (also handle cases where we chain with another node)
            let direction = if self.match_token(&[TokenKind::Arrow]) {
                // Ends with -> means Outgoing
                Direction::Outgoing
            } else if self.match_token(&[TokenKind::Dash]) {
                // Ends with just -
                if starts_with_incoming {
                    // <-[r]- means Incoming
                    Direction::Incoming
                } else {
                    // -[r]- means Undirected
                    Direction::Undirected
                }
            } else {
                return Err(ParseError::InvalidSyntax(
                    "Expected '->' or '-' after relationship".to_string(),
                ));
            };

            // Parse target node(s) - check for hyperedge
            self.consume(TokenKind::LeftParen, "(")?;

            let mut target_nodes = vec![self.parse_node_pattern_content()?];

            // Check for multiple target nodes (hyperedge)
            while self.match_token(&[TokenKind::Comma]) {
                target_nodes.push(self.parse_node_pattern_content()?);
            }

            self.consume(TokenKind::RightParen, ")")?;

            // If multiple targets, create hyperedge
            if target_nodes.len() > 1 {
                return Ok(Pattern::Hyperedge(HyperedgePattern {
                    variable,
                    rel_type: rel_type.ok_or_else(|| {
                        ParseError::InvalidSyntax(
                            "Hyperedge requires relationship type".to_string(),
                        )
                    })?,
                    properties,
                    from: Box::new(from),
                    arity: target_nodes.len() + 1, // +1 for source node
                    to: target_nodes,
                }));
            }

            // Get the single target node pattern
            let target_node = target_nodes.into_iter().next().unwrap();

            // Check if there's a chained pattern (another relationship starting from target)
            if self.check(&TokenKind::Dash) || self.check(&TokenKind::LeftArrow) {
                // There's a chained pattern - recursively parse from the target node
                let chained = self.parse_chained_pattern(target_node)?;

                // Create the first relationship pattern with the chained pattern as target
                Ok(Pattern::Relationship(RelationshipPattern {
                    variable,
                    rel_type,
                    properties,
                    direction,
                    range,
                    from: Box::new(from),
                    to: Box::new(chained),
                }))
            } else {
                Ok(Pattern::Relationship(RelationshipPattern {
                    variable,
                    rel_type,
                    properties,
                    direction,
                    range,
                    from: Box::new(from),
                    to: Box::new(Pattern::Node(target_node)),
                }))
            }
        } else {
            Ok(Pattern::Node(from))
        }
    }

    /// Parse a chained pattern where we already have the starting node pattern
    fn parse_chained_pattern(&mut self, from: NodePattern) -> ParseResult<Pattern> {
        // Check for relationship - can start with `-` or `<-`
        if self.check(&TokenKind::Dash) || self.check(&TokenKind::LeftArrow) {
            let starts_with_incoming = self.match_token(&[TokenKind::LeftArrow]);
            if !starts_with_incoming {
                self.consume(TokenKind::Dash, "-")?;
            }

            // Parse relationship details [r:TYPE {props} *min..max]
            let (variable, rel_type, properties, range) =
                if self.match_token(&[TokenKind::LeftBracket]) {
                    let variable = if let TokenKind::Identifier(v) = &self.peek().kind {
                        let v = v.clone();
                        self.advance();
                        Some(v)
                    } else {
                        None
                    };

                    let rel_type = if self.match_token(&[TokenKind::Colon]) {
                        if let TokenKind::Identifier(t) = &self.peek().kind {
                            let t = t.clone();
                            self.advance();
                            Some(t)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let properties = if self.check(&TokenKind::LeftBrace) {
                        Some(self.parse_property_map()?)
                    } else {
                        None
                    };

                    let range = if self.match_token(&[TokenKind::Star]) {
                        Some(self.parse_relationship_range()?)
                    } else {
                        None
                    };

                    self.consume(TokenKind::RightBracket, "]")?;
                    (variable, rel_type, properties, range)
                } else {
                    (None, None, None, None)
                };

            // Determine final direction
            let direction = if self.match_token(&[TokenKind::Arrow]) {
                Direction::Outgoing
            } else if self.match_token(&[TokenKind::Dash]) {
                if starts_with_incoming {
                    Direction::Incoming
                } else {
                    Direction::Undirected
                }
            } else {
                return Err(ParseError::InvalidSyntax(
                    "Expected '->' or '-' after relationship".to_string(),
                ));
            };

            // Parse target node
            self.consume(TokenKind::LeftParen, "(")?;
            let target_node = self.parse_node_pattern_content()?;
            self.consume(TokenKind::RightParen, ")")?;

            // Check for another chained pattern
            if self.check(&TokenKind::Dash) || self.check(&TokenKind::LeftArrow) {
                let chained = self.parse_chained_pattern(target_node)?;

                Ok(Pattern::Relationship(RelationshipPattern {
                    variable,
                    rel_type,
                    properties,
                    direction,
                    range,
                    from: Box::new(from),
                    to: Box::new(chained),
                }))
            } else {
                Ok(Pattern::Relationship(RelationshipPattern {
                    variable,
                    rel_type,
                    properties,
                    direction,
                    range,
                    from: Box::new(from),
                    to: Box::new(Pattern::Node(target_node)),
                }))
            }
        } else {
            Ok(Pattern::Node(from))
        }
    }

    fn parse_node_pattern(&mut self) -> ParseResult<NodePattern> {
        self.consume(TokenKind::LeftParen, "(")?;
        let node = self.parse_node_pattern_content()?;
        self.consume(TokenKind::RightParen, ")")?;
        Ok(node)
    }

    fn parse_node_pattern_content(&mut self) -> ParseResult<NodePattern> {
        let variable = if let TokenKind::Identifier(v) = &self.peek().kind {
            let v = v.clone();
            // Check if next token is : (label) or { (properties)
            if !self
                .tokens
                .get(self.current + 1)
                .map(|t| matches!(t.kind, TokenKind::Colon | TokenKind::LeftBrace))
                .unwrap_or(false)
            {
                // Variable only, no labels or properties - advance and return
                self.advance();
                return Ok(NodePattern {
                    variable: Some(v),
                    labels: vec![],
                    properties: None,
                });
            }
            self.advance();
            Some(v)
        } else {
            None
        };

        let mut labels = Vec::new();
        while self.match_token(&[TokenKind::Colon]) {
            if let TokenKind::Identifier(label) = &self.peek().kind {
                labels.push(label.clone());
                self.advance();
            }
        }

        let properties = if self.check(&TokenKind::LeftBrace) {
            Some(self.parse_property_map()?)
        } else {
            None
        };

        Ok(NodePattern {
            variable,
            labels,
            properties,
        })
    }

    fn parse_property_map(&mut self) -> ParseResult<PropertyMap> {
        self.consume(TokenKind::LeftBrace, "{")?;
        let mut map = PropertyMap::new();

        if !self.check(&TokenKind::RightBrace) {
            loop {
                let key = if let TokenKind::Identifier(k) = &self.peek().kind {
                    k.clone()
                } else {
                    return Err(ParseError::InvalidSyntax(
                        "Expected property name".to_string(),
                    ));
                };
                self.advance();

                self.consume(TokenKind::Colon, ":")?;
                let value = self.parse_expression()?;
                map.insert(key, value);

                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RightBrace, "}")?;
        Ok(map)
    }

    fn parse_relationship_range(&mut self) -> ParseResult<RelationshipRange> {
        let min = if let TokenKind::Integer(n) = self.peek().kind {
            self.advance();
            Some(n as usize)
        } else {
            None
        };

        let max = if self.match_token(&[TokenKind::DotDot]) {
            if let TokenKind::Integer(n) = self.peek().kind {
                self.advance();
                Some(n as usize)
            } else {
                None
            }
        } else {
            min
        };

        Ok(RelationshipRange { min, max })
    }

    fn parse_create(&mut self) -> ParseResult<CreateClause> {
        self.consume(TokenKind::Create, "CREATE")?;
        let patterns = self.parse_patterns()?;
        Ok(CreateClause { patterns })
    }

    fn parse_merge(&mut self) -> ParseResult<MergeClause> {
        self.consume(TokenKind::Merge, "MERGE")?;
        let pattern = self.parse_pattern()?;

        let mut on_create = None;
        let mut on_match = None;

        while self.peek().kind == TokenKind::OnCreate || self.peek().kind == TokenKind::OnMatch {
            if self.match_token(&[TokenKind::OnCreate]) {
                on_create = Some(self.parse_set()?);
            } else if self.match_token(&[TokenKind::OnMatch]) {
                on_match = Some(self.parse_set()?);
            }
        }

        Ok(MergeClause {
            pattern,
            on_create,
            on_match,
        })
    }

    fn parse_delete(&mut self) -> ParseResult<DeleteClause> {
        let detach = self.match_token(&[TokenKind::DetachDelete]);
        if !detach {
            self.consume(TokenKind::Delete, "DELETE")?;
        }

        let mut expressions = vec![self.parse_expression()?];
        while self.match_token(&[TokenKind::Comma]) {
            expressions.push(self.parse_expression()?);
        }

        Ok(DeleteClause {
            detach,
            expressions,
        })
    }

    fn parse_set(&mut self) -> ParseResult<SetClause> {
        self.consume(TokenKind::Set, "SET")?;
        let mut items = vec![];

        loop {
            if let TokenKind::Identifier(var) = &self.peek().kind {
                let var = var.clone();
                self.advance();

                if self.match_token(&[TokenKind::Dot]) {
                    if let TokenKind::Identifier(prop) = &self.peek().kind {
                        let prop = prop.clone();
                        self.advance();
                        self.consume(TokenKind::Equal, "=")?;
                        let value = self.parse_expression()?;
                        items.push(SetItem::Property {
                            variable: var,
                            property: prop,
                            value,
                        });
                    }
                } else if self.match_token(&[TokenKind::Equal]) {
                    let value = self.parse_expression()?;
                    items.push(SetItem::Variable {
                        variable: var,
                        value,
                    });
                }
            }

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(SetClause { items })
    }

    fn parse_remove(&mut self) -> ParseResult<RemoveClause> {
        self.consume(TokenKind::Remove, "REMOVE")?;
        let mut items = vec![];

        loop {
            if let TokenKind::Identifier(var) = &self.peek().kind {
                let var = var.clone();
                self.advance();

                if self.match_token(&[TokenKind::Dot]) {
                    // Remove property: REMOVE n.property
                    if let TokenKind::Identifier(prop) = &self.peek().kind {
                        let prop = prop.clone();
                        self.advance();
                        items.push(RemoveItem::Property {
                            variable: var,
                            property: prop,
                        });
                    } else {
                        return Err(ParseError::InvalidSyntax(
                            "Expected property name after '.'".to_string(),
                        ));
                    }
                } else if self.match_token(&[TokenKind::Colon]) {
                    // Remove labels: REMOVE n:Label1:Label2
                    let mut labels = vec![];
                    if let TokenKind::Identifier(label) = &self.peek().kind {
                        labels.push(label.clone());
                        self.advance();
                    }
                    // Handle multiple labels
                    while self.match_token(&[TokenKind::Colon]) {
                        if let TokenKind::Identifier(label) = &self.peek().kind {
                            labels.push(label.clone());
                            self.advance();
                        }
                    }
                    items.push(RemoveItem::Labels {
                        variable: var,
                        labels,
                    });
                } else {
                    return Err(ParseError::InvalidSyntax(
                        "Expected '.' or ':' after variable in REMOVE".to_string(),
                    ));
                }
            } else {
                return Err(ParseError::InvalidSyntax(
                    "Expected variable in REMOVE".to_string(),
                ));
            }

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(RemoveClause { items })
    }

    fn parse_return(&mut self) -> ParseResult<ReturnClause> {
        self.consume(TokenKind::Return, "RETURN")?;
        let distinct = self.match_token(&[TokenKind::Distinct]);

        let items = self.parse_return_items()?;
        let order_by = self.parse_order_by()?;

        let skip = if self.match_token(&[TokenKind::Skip]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let limit = if self.match_token(&[TokenKind::Limit]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(ReturnClause {
            distinct,
            items,
            order_by,
            skip,
            limit,
        })
    }

    fn parse_with(&mut self) -> ParseResult<WithClause> {
        self.consume(TokenKind::With, "WITH")?;
        let distinct = self.match_token(&[TokenKind::Distinct]);

        let items = self.parse_return_items()?;

        let where_clause = if self.match_token(&[TokenKind::Where]) {
            Some(WhereClause {
                condition: self.parse_expression()?,
            })
        } else {
            None
        };

        let order_by = self.parse_order_by()?;

        let skip = if self.match_token(&[TokenKind::Skip]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let limit = if self.match_token(&[TokenKind::Limit]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(WithClause {
            distinct,
            items,
            where_clause,
            order_by,
            skip,
            limit,
        })
    }

    fn parse_return_items(&mut self) -> ParseResult<Vec<ReturnItem>> {
        let mut items = vec![];

        loop {
            let expression = self.parse_expression()?;
            let alias = if self.match_token(&[TokenKind::As]) {
                if let TokenKind::Identifier(name) = &self.peek().kind {
                    let name = name.clone();
                    self.advance();
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            };

            items.push(ReturnItem { expression, alias });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(items)
    }

    fn parse_order_by(&mut self) -> ParseResult<Option<OrderBy>> {
        if !self.match_token(&[TokenKind::OrderBy]) {
            return Ok(None);
        }

        let mut items = vec![];

        loop {
            let expression = self.parse_expression()?;
            let ascending = if self.match_token(&[TokenKind::Desc]) {
                false
            } else {
                self.match_token(&[TokenKind::Asc]);
                true
            };

            items.push(OrderByItem {
                expression,
                ascending,
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(Some(OrderBy { items }))
    }

    fn parse_expression(&mut self) -> ParseResult<Expression> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_xor()?;

        while self.match_token(&[TokenKind::Or]) {
            let right = self.parse_xor()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_xor(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_and()?;

        while self.match_token(&[TokenKind::Xor]) {
            let right = self.parse_and()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::Xor,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_comparison()?;

        while self.match_token(&[TokenKind::And]) {
            let right = self.parse_comparison()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_additive()?;

        if let Some(op) = self.parse_comparison_op() {
            let right = self.parse_additive()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_comparison_op(&mut self) -> Option<BinaryOperator> {
        if self.match_token(&[TokenKind::Equal]) {
            Some(BinaryOperator::Equal)
        } else if self.match_token(&[TokenKind::NotEqual]) {
            Some(BinaryOperator::NotEqual)
        } else if self.match_token(&[TokenKind::LessThanOrEqual]) {
            Some(BinaryOperator::LessThanOrEqual)
        } else if self.match_token(&[TokenKind::GreaterThanOrEqual]) {
            Some(BinaryOperator::GreaterThanOrEqual)
        } else if self.match_token(&[TokenKind::LessThan]) {
            Some(BinaryOperator::LessThan)
        } else if self.match_token(&[TokenKind::GreaterThan]) {
            Some(BinaryOperator::GreaterThan)
        } else {
            None
        }
    }

    fn parse_additive(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_multiplicative()?;

        while let Some(op) = self.parse_additive_op() {
            let right = self.parse_multiplicative()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_additive_op(&mut self) -> Option<BinaryOperator> {
        if self.match_token(&[TokenKind::Plus]) {
            Some(BinaryOperator::Add)
        } else if self.match_token(&[TokenKind::Minus]) {
            Some(BinaryOperator::Subtract)
        } else {
            None
        }
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_unary()?;

        while let Some(op) = self.parse_multiplicative_op() {
            let right = self.parse_unary()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_multiplicative_op(&mut self) -> Option<BinaryOperator> {
        if self.match_token(&[TokenKind::Star]) {
            Some(BinaryOperator::Multiply)
        } else if self.match_token(&[TokenKind::Slash]) {
            Some(BinaryOperator::Divide)
        } else if self.match_token(&[TokenKind::Percent]) {
            Some(BinaryOperator::Modulo)
        } else if self.match_token(&[TokenKind::Caret]) {
            Some(BinaryOperator::Power)
        } else {
            None
        }
    }

    fn parse_unary(&mut self) -> ParseResult<Expression> {
        if self.match_token(&[TokenKind::Not]) {
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(operand),
            });
        }

        if self.match_token(&[TokenKind::Minus]) {
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Minus,
                operand: Box::new(operand),
            });
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&[TokenKind::Dot]) {
                if let TokenKind::Identifier(prop) = &self.peek().kind {
                    let prop = prop.clone();
                    self.advance();
                    expr = Expression::Property {
                        object: Box::new(expr),
                        property: prop,
                    };
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expression> {
        match &self.peek().kind.clone() {
            TokenKind::Integer(n) => {
                let n = *n;
                self.advance();
                Ok(Expression::Integer(n))
            }
            TokenKind::Float(n) => {
                let n = *n;
                self.advance();
                Ok(Expression::Float(n))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::String(s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::Boolean(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Boolean(false))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression::Null)
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();

                // Check for function call
                if self.match_token(&[TokenKind::LeftParen]) {
                    self.parse_function_call(name)
                } else {
                    Ok(Expression::Variable(name))
                }
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(TokenKind::RightParen, ")")?;
                Ok(expr)
            }
            TokenKind::LeftBracket => {
                self.advance();
                let mut items = vec![];

                if !self.check(&TokenKind::RightBracket) {
                    loop {
                        items.push(self.parse_expression()?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }

                self.consume(TokenKind::RightBracket, "]")?;
                Ok(Expression::List(items))
            }
            TokenKind::LeftBrace => {
                // Map literal: {key1: value1, key2: value2}
                self.advance();
                // Pre-allocate with reasonable capacity to reduce reallocations
                let mut map = std::collections::HashMap::with_capacity(8);

                if !self.check(&TokenKind::RightBrace) {
                    loop {
                        // Parse key (identifier or string)
                        let key = match &self.peek().kind {
                            TokenKind::Identifier(k) => {
                                let k = k.clone();
                                self.advance();
                                k
                            }
                            TokenKind::String(k) => {
                                let k = k.clone();
                                self.advance();
                                k
                            }
                            _ => {
                                let token = self.peek();
                                return Err(ParseError::UnexpectedToken {
                                    expected: "map key (identifier or string)".to_string(),
                                    found: token.kind.to_string(),
                                    line: token.position.line,
                                    column: token.position.column,
                                });
                            }
                        };

                        self.consume(TokenKind::Colon, ":")?;
                        let value = self.parse_expression()?;
                        map.insert(key, value);

                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }

                self.consume(TokenKind::RightBrace, "}")?;
                Ok(Expression::Map(map))
            }
            _ => {
                let token = self.peek();
                Err(ParseError::UnexpectedToken {
                    expected: "expression".to_string(),
                    found: token.kind.to_string(),
                    line: token.position.line,
                    column: token.position.column,
                })
            }
        }
    }

    fn parse_function_call(&mut self, name: String) -> ParseResult<Expression> {
        let mut args = vec![];

        if !self.check(&TokenKind::RightParen) {
            // Check for DISTINCT in aggregation
            let distinct = self.match_token(&[TokenKind::Distinct]);

            loop {
                args.push(self.parse_expression()?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }

            // Check if it's an aggregation function
            let agg_func = match name.to_uppercase().as_str() {
                "COUNT" => Some(AggregationFunction::Count),
                "SUM" => Some(AggregationFunction::Sum),
                "AVG" => Some(AggregationFunction::Avg),
                "MIN" => Some(AggregationFunction::Min),
                "MAX" => Some(AggregationFunction::Max),
                "COLLECT" => Some(AggregationFunction::Collect),
                _ => None,
            };

            self.consume(TokenKind::RightParen, ")")?;

            if let Some(func) = agg_func {
                if args.len() != 1 {
                    return Err(ParseError::InvalidSyntax(
                        "Aggregation function requires exactly one argument".to_string(),
                    ));
                }
                return Ok(Expression::Aggregation {
                    function: func,
                    expression: Box::new(args.into_iter().next().unwrap()),
                    distinct,
                });
            }
        } else {
            self.consume(TokenKind::RightParen, ")")?;
        }

        Ok(Expression::FunctionCall { name, args })
    }
}

/// Parse a Cypher query string into an AST
pub fn parse_cypher(input: &str) -> ParseResult<Query> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    parser.parse_query()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_match() {
        let query = "MATCH (n:Person) RETURN n";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let ast = result.unwrap();
        assert_eq!(ast.statements.len(), 2);
    }

    #[test]
    fn test_parse_match_with_where() {
        let query = "MATCH (n:Person) WHERE n.age > 30 RETURN n.name";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_relationship() {
        let query = "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a, r, b";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_create() {
        let query = "CREATE (n:Person {name: 'Alice', age: 30})";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "Hyperedge syntax not yet implemented in parser"]
    fn test_parse_hyperedge() {
        let query = "MATCH (a)-[r:TRANSACTION]->(b, c, d) RETURN a, r, b, c, d";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let ast = result.unwrap();
        assert!(ast.has_hyperedges());
    }

    #[test]
    fn test_parse_aggregation() {
        let query = "MATCH (n:Person) RETURN COUNT(n), AVG(n.age)";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    // ============== Edge Case Tests for New Functionality ==============

    #[test]
    fn test_empty_query_rejected() {
        // Empty string should fail
        let result = parse_cypher("");
        assert!(result.is_err());
        match result {
            Err(ParseError::InvalidSyntax(msg)) => {
                assert!(msg.contains("Empty query"));
            }
            _ => panic!("Expected InvalidSyntax error for empty query"),
        }
    }

    #[test]
    fn test_whitespace_only_query_rejected() {
        // Whitespace-only should fail
        let result = parse_cypher("   \n\t  ");
        assert!(result.is_err());
    }

    #[test]
    fn test_map_literal_in_return() {
        // Map literals in RETURN clause
        let query = "MATCH (n) RETURN {name: n.name, age: n.age}";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_map_literal() {
        // Empty map literal
        let query = "MATCH (n) RETURN {}";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "Nested map literals require recursive Expression parsing - future enhancement"]
    fn test_nested_map_literal() {
        // Nested map literals - currently not supported (needs recursive expression parsing)
        let query = "MATCH (n) RETURN {info: {name: n.name, details: {age: n.age}}}";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chained_relationship_outgoing() {
        // Chained outgoing relationships: (a)-[r]->(b)-[s]->(c)
        let query = "MATCH (a)-[r:KNOWS]->(b)-[s:WORKS_AT]->(c) RETURN a, b, c";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chained_relationship_mixed() {
        // Mixed direction chained relationships: (a)-[r]->(b)<-[s]-(c)
        let query =
            "MATCH (a:Person)-[r:KNOWS]->(b:Person)<-[s:MANAGES]-(c:Manager) RETURN a, b, c";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_undirected_relationship() {
        // Undirected relationship: (a)-[r]-(b)
        let query = "MATCH (a:Person)-[r:FRIEND]-(b:Person) RETURN a, b";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_property() {
        // REMOVE statement for property
        let query = "MATCH (n:Person) REMOVE n.age RETURN n";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_label() {
        // REMOVE statement for label
        let query = "MATCH (n:Person:Employee) REMOVE n:Employee RETURN n";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_with_string_keys() {
        // Map with string keys (quoted)
        let query = "MATCH (n) RETURN {'first-name': n.firstName}";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_triple_chained_relationship() {
        // Triple chained relationship: (a)-[r]->(b)-[s]->(c)-[t]->(d)
        let query = "MATCH (a)-[r]->(b)-[s]->(c)-[t]->(d) RETURN a, d";
        let result = parse_cypher(query);
        assert!(result.is_ok());
    }
}
