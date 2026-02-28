// SPARQL Query Parser
//
// Parses SPARQL 1.1 query strings into AST representation.
// Note: This is a functional parser for common patterns. A production
// parser would use a proper parsing library like nom, pest, or lalrpop.

use super::ast::*;
use super::SparqlError;
use std::collections::HashMap;

/// Parse a SPARQL query string
pub fn parse_sparql(query: &str) -> Result<SparqlQuery, SparqlError> {
    let mut parser = SparqlParser::new(query);
    parser.parse()
}

/// SPARQL Parser state
struct SparqlParser<'a> {
    input: &'a str,
    pos: usize,
    prefixes: HashMap<String, Iri>,
    base: Option<Iri>,
}

impl<'a> SparqlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            prefixes: HashMap::new(),
            base: None,
        }
    }

    fn parse(&mut self) -> Result<SparqlQuery, SparqlError> {
        self.skip_whitespace();

        // Parse prologue (BASE and PREFIX declarations)
        self.parse_prologue()?;

        // Determine query type
        let body = self.parse_query_body()?;

        Ok(SparqlQuery {
            base: self.base.clone(),
            prefixes: self.prefixes.clone(),
            body,
        })
    }

    fn parse_prologue(&mut self) -> Result<(), SparqlError> {
        loop {
            self.skip_whitespace();

            if self.match_keyword("BASE") {
                self.skip_whitespace();
                let iri = self.parse_iri_ref()?;
                self.base = Some(iri);
            } else if self.match_keyword("PREFIX") {
                self.skip_whitespace();
                let prefix = self.parse_prefix_name()?;
                self.skip_whitespace();
                let iri = self.parse_iri_ref()?;
                self.prefixes.insert(prefix, iri);
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_query_body(&mut self) -> Result<QueryBody, SparqlError> {
        self.skip_whitespace();

        if self.match_keyword("SELECT") {
            Ok(QueryBody::Select(self.parse_select_query()?))
        } else if self.match_keyword("CONSTRUCT") {
            Ok(QueryBody::Construct(self.parse_construct_query()?))
        } else if self.match_keyword("ASK") {
            Ok(QueryBody::Ask(self.parse_ask_query()?))
        } else if self.match_keyword("DESCRIBE") {
            Ok(QueryBody::Describe(self.parse_describe_query()?))
        } else if self.match_keyword("INSERT")
            || self.match_keyword("DELETE")
            || self.match_keyword("LOAD")
            || self.match_keyword("CLEAR")
            || self.match_keyword("CREATE")
            || self.match_keyword("DROP")
        {
            self.pos = self.pos.saturating_sub(6); // Backtrack
            Ok(QueryBody::Update(self.parse_update()?))
        } else {
            Err(SparqlError::ParseError(format!(
                "Expected SELECT, CONSTRUCT, ASK, DESCRIBE, or update keyword at position {}",
                self.pos
            )))
        }
    }

    fn parse_select_query(&mut self) -> Result<SelectQuery, SparqlError> {
        self.skip_whitespace();

        // Parse projection
        let projection = self.parse_projection()?;

        // Parse dataset clauses
        let dataset = self.parse_dataset_clauses()?;

        // Parse WHERE clause
        self.skip_whitespace();
        let where_clause = if self.match_keyword("WHERE") {
            self.skip_whitespace();
            self.parse_group_graph_pattern()?
        } else if self.peek_char() == Some('{') {
            // WHERE is optional if followed by {
            self.parse_group_graph_pattern()?
        } else {
            GraphPattern::Empty
        };

        // Parse solution modifiers
        let modifier = self.parse_solution_modifiers()?;

        // Parse VALUES clause
        let values = self.parse_values_clause()?;

        Ok(SelectQuery {
            projection,
            dataset,
            where_clause,
            modifier,
            values,
        })
    }

    fn parse_projection(&mut self) -> Result<Projection, SparqlError> {
        self.skip_whitespace();

        // Check for DISTINCT or REDUCED
        let (distinct, reduced) = if self.match_keyword("DISTINCT") {
            (true, false)
        } else if self.match_keyword("REDUCED") {
            (false, true)
        } else {
            (false, false)
        };

        self.skip_whitespace();

        // Check for *
        if self.match_char('*') {
            return Ok(Projection::All);
        }

        // Parse variable list
        let mut vars = Vec::new();
        loop {
            self.skip_whitespace();

            // Check for ( expression AS ?var )
            if self.match_char('(') {
                self.skip_whitespace();
                let expr = self.parse_expression()?;
                self.skip_whitespace();

                if !self.match_keyword("AS") {
                    return Err(SparqlError::ParseError(
                        "Expected AS in projection".to_string(),
                    ));
                }

                self.skip_whitespace();
                let var_name = self.parse_variable_name()?;
                self.skip_whitespace();

                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(
                        "Expected ) in projection".to_string(),
                    ));
                }

                vars.push(ProjectionVar::expr_as(expr, var_name));
            } else if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
                let var_name = self.parse_variable_name()?;
                vars.push(ProjectionVar::variable(var_name));
            } else {
                break;
            }
        }

        if vars.is_empty() {
            return Err(SparqlError::ParseError(
                "Expected variables in SELECT".to_string(),
            ));
        }

        if distinct {
            Ok(Projection::Distinct(vars))
        } else if reduced {
            Ok(Projection::Reduced(vars))
        } else {
            Ok(Projection::Variables(vars))
        }
    }

    fn parse_dataset_clauses(&mut self) -> Result<Vec<DatasetClause>, SparqlError> {
        let mut clauses = Vec::new();

        loop {
            self.skip_whitespace();

            if self.match_keyword("FROM") {
                self.skip_whitespace();
                let named = self.match_keyword("NAMED");
                self.skip_whitespace();
                let iri = self.parse_iri_ref()?;
                clauses.push(DatasetClause { iri, named });
            } else {
                break;
            }
        }

        Ok(clauses)
    }

    fn parse_group_graph_pattern(&mut self) -> Result<GraphPattern, SparqlError> {
        self.skip_whitespace();

        if !self.match_char('{') {
            return Err(SparqlError::ParseError(
                "Expected { for graph pattern".to_string(),
            ));
        }

        let pattern = self.parse_graph_pattern_inner()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError(
                "Expected } for graph pattern".to_string(),
            ));
        }

        Ok(pattern)
    }

    fn parse_graph_pattern_inner(&mut self) -> Result<GraphPattern, SparqlError> {
        let mut patterns: Vec<GraphPattern> = Vec::new();
        let mut filters: Vec<Expression> = Vec::new();

        loop {
            self.skip_whitespace();

            // Check for end of pattern
            if self.peek_char() == Some('}') || self.is_at_end() {
                break;
            }

            // Parse different pattern types
            if self.match_keyword("OPTIONAL") {
                self.skip_whitespace();
                let optional = self.parse_group_graph_pattern()?;
                if let Some(last) = patterns.pop() {
                    patterns.push(GraphPattern::LeftJoin(
                        Box::new(last),
                        Box::new(optional),
                        None,
                    ));
                } else {
                    patterns.push(GraphPattern::LeftJoin(
                        Box::new(GraphPattern::Empty),
                        Box::new(optional),
                        None,
                    ));
                }
            } else if self.match_keyword("UNION") {
                self.skip_whitespace();
                let right = self.parse_group_graph_pattern()?;
                if let Some(last) = patterns.pop() {
                    patterns.push(GraphPattern::Union(Box::new(last), Box::new(right)));
                }
            } else if self.match_keyword("MINUS") {
                self.skip_whitespace();
                let minus = self.parse_group_graph_pattern()?;
                if let Some(last) = patterns.pop() {
                    patterns.push(GraphPattern::Minus(Box::new(last), Box::new(minus)));
                }
            } else if self.match_keyword("GRAPH") {
                self.skip_whitespace();
                let graph_name = self.parse_var_or_iri()?;
                self.skip_whitespace();
                let inner = self.parse_group_graph_pattern()?;
                patterns.push(GraphPattern::Graph(graph_name, Box::new(inner)));
            } else if self.match_keyword("FILTER") {
                self.skip_whitespace();
                let expr = self.parse_filter_expression()?;
                filters.push(expr);
            } else if self.match_keyword("BIND") {
                self.skip_whitespace();
                if !self.match_char('(') {
                    return Err(SparqlError::ParseError("Expected ( after BIND".to_string()));
                }
                let expr = self.parse_expression()?;
                self.skip_whitespace();
                if !self.match_keyword("AS") {
                    return Err(SparqlError::ParseError("Expected AS in BIND".to_string()));
                }
                self.skip_whitespace();
                let var = self.parse_variable_name()?;
                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError("Expected ) after BIND".to_string()));
                }
                let prev = patterns.pop().unwrap_or(GraphPattern::Empty);
                patterns.push(GraphPattern::Bind(expr, var, Box::new(prev)));
            } else if self.match_keyword("VALUES") {
                let values = self.parse_inline_values()?;
                patterns.push(GraphPattern::Values(values));
            } else if self.match_keyword("SERVICE") {
                let silent = self.match_keyword("SILENT");
                self.skip_whitespace();
                let service_iri = self.parse_iri_ref()?;
                self.skip_whitespace();
                let inner = self.parse_group_graph_pattern()?;
                patterns.push(GraphPattern::Service(service_iri, Box::new(inner), silent));
            } else if self.peek_char() == Some('{') {
                // Nested group
                let inner = self.parse_group_graph_pattern()?;
                patterns.push(inner);
            } else if self.match_keyword("SELECT") {
                // Subquery
                let subquery = self.parse_select_query()?;
                patterns.push(GraphPattern::SubSelect(Box::new(subquery)));
            } else {
                // Try to parse triple patterns
                let triples = self.parse_triples_block()?;
                if !triples.is_empty() {
                    patterns.push(GraphPattern::Bgp(triples));
                } else {
                    // Skip unknown token
                    if !self.skip_to_next_pattern() {
                        break;
                    }
                }
            }
        }

        // Combine patterns
        let mut result = if patterns.is_empty() {
            GraphPattern::Empty
        } else {
            patterns
                .into_iter()
                .reduce(|a, b| GraphPattern::Join(Box::new(a), Box::new(b)))
                .unwrap()
        };

        // Apply filters
        for filter in filters {
            result = GraphPattern::Filter(Box::new(result), filter);
        }

        Ok(result)
    }

    fn parse_triples_block(&mut self) -> Result<Vec<TriplePattern>, SparqlError> {
        let mut triples = Vec::new();

        loop {
            self.skip_whitespace();

            // Check if we should stop parsing triples
            if self.is_at_end()
                || self.peek_char() == Some('}')
                || self.peek_keyword("OPTIONAL")
                || self.peek_keyword("UNION")
                || self.peek_keyword("MINUS")
                || self.peek_keyword("GRAPH")
                || self.peek_keyword("FILTER")
                || self.peek_keyword("BIND")
                || self.peek_keyword("VALUES")
                || self.peek_keyword("SERVICE")
            {
                break;
            }

            // Parse subject
            let subject = self.parse_term_or_variable()?;

            // Parse predicate-object list
            loop {
                self.skip_whitespace();

                // Check for end or separator
                if self.peek_char() == Some('.') || self.peek_char() == Some('}') {
                    break;
                }

                // Parse predicate
                let predicate = self.parse_property_path()?;

                // Parse object list
                loop {
                    self.skip_whitespace();
                    let object = self.parse_term_or_variable()?;

                    triples.push(TriplePattern::new(
                        subject.clone(),
                        predicate.clone(),
                        object,
                    ));

                    self.skip_whitespace();
                    if !self.match_char(',') {
                        break;
                    }
                }

                self.skip_whitespace();
                if !self.match_char(';') {
                    break;
                }

                self.skip_whitespace();
                // Allow trailing semicolon
                if self.peek_char() == Some('.') || self.peek_char() == Some('}') {
                    break;
                }
            }

            // Skip period separator
            self.skip_whitespace();
            self.match_char('.');
        }

        Ok(triples)
    }

    fn parse_term_or_variable(&mut self) -> Result<TermOrVariable, SparqlError> {
        self.skip_whitespace();

        if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
            Ok(TermOrVariable::Variable(self.parse_variable_name()?))
        } else if self.peek_char() == Some('_') && self.peek_char_at(1) == Some(':') {
            Ok(TermOrVariable::BlankNode(self.parse_blank_node()?))
        } else if self.peek_char() == Some('[') {
            // Anonymous blank node
            self.next_char();
            self.skip_whitespace();
            if self.match_char(']') {
                Ok(TermOrVariable::BlankNode(format!("b{}", self.pos)))
            } else {
                Err(SparqlError::ParseError(
                    "Expected ] for blank node".to_string(),
                ))
            }
        } else {
            Ok(TermOrVariable::Term(self.parse_rdf_term()?))
        }
    }

    fn parse_rdf_term(&mut self) -> Result<RdfTerm, SparqlError> {
        self.skip_whitespace();

        if self.peek_char() == Some('<') {
            // Full IRI
            Ok(RdfTerm::Iri(self.parse_iri_ref()?))
        } else if self.peek_char() == Some('"') || self.peek_char() == Some('\'') {
            // Literal
            Ok(RdfTerm::Literal(self.parse_literal()?))
        } else if self.peek_char() == Some('_') && self.peek_char_at(1) == Some(':') {
            // Blank node
            Ok(RdfTerm::BlankNode(self.parse_blank_node()?))
        } else if self.match_keyword("true") {
            Ok(RdfTerm::Literal(Literal::boolean(true)))
        } else if self.match_keyword("false") {
            Ok(RdfTerm::Literal(Literal::boolean(false)))
        } else if self
            .peek_char()
            .map(|c| c.is_ascii_digit() || c == '+' || c == '-')
            .unwrap_or(false)
        {
            // Numeric literal
            Ok(RdfTerm::Literal(self.parse_numeric_literal()?))
        } else {
            // Prefixed name
            let iri = self.parse_prefixed_name()?;
            Ok(RdfTerm::Iri(iri))
        }
    }

    fn parse_property_path(&mut self) -> Result<PropertyPath, SparqlError> {
        self.skip_whitespace();

        // Handle 'a' shorthand for rdf:type
        if self.match_keyword("a")
            && !self
                .peek_char()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false)
        {
            return Ok(PropertyPath::Iri(Iri::rdf_type()));
        }

        // Check for variable predicate
        if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
            return Ok(PropertyPath::Variable(self.parse_variable_name()?));
        }

        self.parse_path_alternative()
    }

    fn parse_path_alternative(&mut self) -> Result<PropertyPath, SparqlError> {
        let mut left = self.parse_path_sequence()?;

        loop {
            self.skip_whitespace();
            if self.match_char('|') {
                let right = self.parse_path_sequence()?;
                left = PropertyPath::Alternative(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_path_sequence(&mut self) -> Result<PropertyPath, SparqlError> {
        let mut left = self.parse_path_element()?;

        loop {
            self.skip_whitespace();
            if self.match_char('/') {
                let right = self.parse_path_element()?;
                left = PropertyPath::Sequence(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_path_element(&mut self) -> Result<PropertyPath, SparqlError> {
        self.skip_whitespace();

        // Check for inverse
        let inverse = self.match_char('^');

        let mut path = self.parse_path_primary()?;

        if inverse {
            path = PropertyPath::Inverse(Box::new(path));
        }

        // Check for modifiers (*, +, ?)
        // IMPORTANT: For '?', we must NOT consume it if followed by an alphanumeric character,
        // as that indicates a variable name (e.g., ?s, ?p, ?o) not a zero-or-one modifier
        self.skip_whitespace();
        if self.match_char('*') {
            path = PropertyPath::ZeroOrMore(Box::new(path));
        } else if self.match_char('+') {
            path = PropertyPath::OneOrMore(Box::new(path));
        } else if self.peek_char() == Some('?') {
            // Check if this is a variable (? followed by alphanumeric/underscore) or a modifier
            let is_variable = self
                .peek_char_at(1)
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
            if !is_variable {
                self.next_char(); // Consume the '?' only if it's a modifier
                path = PropertyPath::ZeroOrOne(Box::new(path));
            }
        }

        Ok(path)
    }

    fn parse_path_primary(&mut self) -> Result<PropertyPath, SparqlError> {
        self.skip_whitespace();

        if self.match_char('(') {
            let path = self.parse_path_alternative()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(SparqlError::ParseError(
                    "Expected ) in property path".to_string(),
                ));
            }
            Ok(path)
        } else if self.match_char('!') {
            // Negated property set
            let negated = self.parse_negated_property_set()?;
            Ok(negated)
        } else if self.peek_char() == Some('<') {
            Ok(PropertyPath::Iri(self.parse_iri_ref()?))
        } else if self.match_keyword("a") {
            Ok(PropertyPath::Iri(Iri::rdf_type()))
        } else {
            Ok(PropertyPath::Iri(self.parse_prefixed_name()?))
        }
    }

    fn parse_negated_property_set(&mut self) -> Result<PropertyPath, SparqlError> {
        self.skip_whitespace();

        let mut iris = Vec::new();

        if self.match_char('(') {
            loop {
                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                }

                let iri = if self.peek_char() == Some('<') {
                    self.parse_iri_ref()?
                } else {
                    self.parse_prefixed_name()?
                };
                iris.push(iri);

                self.skip_whitespace();
                if !self.match_char('|') {
                    self.skip_whitespace();
                    if !self.match_char(')') {
                        return Err(SparqlError::ParseError(
                            "Expected ) in negated property set".to_string(),
                        ));
                    }
                    break;
                }
            }
        } else {
            let iri = if self.peek_char() == Some('<') {
                self.parse_iri_ref()?
            } else {
                self.parse_prefixed_name()?
            };
            iris.push(iri);
        }

        Ok(PropertyPath::NegatedPropertySet(iris))
    }

    fn parse_expression(&mut self) -> Result<Expression, SparqlError> {
        self.parse_conditional_or_expression()
    }

    fn parse_conditional_or_expression(&mut self) -> Result<Expression, SparqlError> {
        let mut left = self.parse_conditional_and_expression()?;

        loop {
            self.skip_whitespace();
            if self.match_keyword("||") {
                let right = self.parse_conditional_and_expression()?;
                left = Expression::or(left, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_conditional_and_expression(&mut self) -> Result<Expression, SparqlError> {
        let mut left = self.parse_relational_expression()?;

        loop {
            self.skip_whitespace();
            if self.match_keyword("&&") {
                let right = self.parse_relational_expression()?;
                left = Expression::and(left, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_relational_expression(&mut self) -> Result<Expression, SparqlError> {
        let left = self.parse_additive_expression()?;

        self.skip_whitespace();

        if self.match_keyword("=") {
            let right = self.parse_additive_expression()?;
            Ok(Expression::eq(left, right))
        } else if self.match_keyword("!=") {
            let right = self.parse_additive_expression()?;
            Ok(Expression::neq(left, right))
        } else if self.match_keyword("<=") {
            let right = self.parse_additive_expression()?;
            Ok(Expression::binary(left, BinaryOp::LtEq, right))
        } else if self.match_keyword(">=") {
            let right = self.parse_additive_expression()?;
            Ok(Expression::binary(left, BinaryOp::GtEq, right))
        } else if self.match_keyword("<") && self.peek_char() != Some('=') {
            let right = self.parse_additive_expression()?;
            Ok(Expression::lt(left, right))
        } else if self.match_keyword(">") && self.peek_char() != Some('=') {
            let right = self.parse_additive_expression()?;
            Ok(Expression::gt(left, right))
        } else if self.match_keyword("IN") {
            self.skip_whitespace();
            if !self.match_char('(') {
                return Err(SparqlError::ParseError("Expected ( after IN".to_string()));
            }
            let list = self.parse_expression_list()?;
            if !self.match_char(')') {
                return Err(SparqlError::ParseError(
                    "Expected ) after IN list".to_string(),
                ));
            }
            Ok(Expression::In(Box::new(left), list))
        } else if self.match_keyword("NOT") {
            self.skip_whitespace();
            if self.match_keyword("IN") {
                self.skip_whitespace();
                if !self.match_char('(') {
                    return Err(SparqlError::ParseError(
                        "Expected ( after NOT IN".to_string(),
                    ));
                }
                let list = self.parse_expression_list()?;
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(
                        "Expected ) after NOT IN list".to_string(),
                    ));
                }
                Ok(Expression::NotIn(Box::new(left), list))
            } else {
                Err(SparqlError::ParseError("Expected IN after NOT".to_string()))
            }
        } else {
            Ok(left)
        }
    }

    fn parse_additive_expression(&mut self) -> Result<Expression, SparqlError> {
        let mut left = self.parse_multiplicative_expression()?;

        loop {
            self.skip_whitespace();
            if self.match_char('+') && self.peek_char() != Some('+') {
                let right = self.parse_multiplicative_expression()?;
                left = Expression::binary(left, BinaryOp::Add, right);
            } else if self.match_char('-') && self.peek_char() != Some('-') {
                let right = self.parse_multiplicative_expression()?;
                left = Expression::binary(left, BinaryOp::Sub, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_multiplicative_expression(&mut self) -> Result<Expression, SparqlError> {
        let mut left = self.parse_unary_expression()?;

        loop {
            self.skip_whitespace();
            if self.match_char('*') {
                let right = self.parse_unary_expression()?;
                left = Expression::binary(left, BinaryOp::Mul, right);
            } else if self.match_char('/') {
                let right = self.parse_unary_expression()?;
                left = Expression::binary(left, BinaryOp::Div, right);
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_unary_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();

        if self.match_char('!') {
            let expr = self.parse_primary_expression()?;
            Ok(Expression::unary(UnaryOp::Not, expr))
        } else if self.match_char('+') {
            let expr = self.parse_primary_expression()?;
            Ok(Expression::unary(UnaryOp::Plus, expr))
        } else if self.match_char('-') {
            let expr = self.parse_primary_expression()?;
            Ok(Expression::unary(UnaryOp::Minus, expr))
        } else {
            self.parse_primary_expression()
        }
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();

        // Parenthesized expression
        if self.match_char('(') {
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(SparqlError::ParseError(
                    "Expected ) in expression".to_string(),
                ));
            }
            return Ok(expr);
        }

        // Built-in functions and expressions
        if self.match_keyword("BOUND") {
            self.skip_whitespace();
            if !self.match_char('(') {
                return Err(SparqlError::ParseError(
                    "Expected ( after BOUND".to_string(),
                ));
            }
            let var = self.parse_variable_name()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(SparqlError::ParseError(
                    "Expected ) after BOUND".to_string(),
                ));
            }
            return Ok(Expression::Bound(var));
        }

        if self.match_keyword("IF") {
            return self.parse_if_expression();
        }

        if self.match_keyword("COALESCE") {
            return self.parse_coalesce_expression();
        }

        if self.match_keyword("EXISTS") {
            self.skip_whitespace();
            let pattern = self.parse_group_graph_pattern()?;
            return Ok(Expression::Exists(Box::new(pattern)));
        }

        if self.match_keyword("NOT") {
            self.skip_whitespace();
            if self.match_keyword("EXISTS") {
                self.skip_whitespace();
                let pattern = self.parse_group_graph_pattern()?;
                return Ok(Expression::NotExists(Box::new(pattern)));
            }
        }

        // Aggregate functions
        if let Some(agg) = self.try_parse_aggregate()? {
            return Ok(Expression::Aggregate(agg));
        }

        // Built-in test functions
        for (keyword, constructor) in &[
            (
                "isIRI",
                Expression::IsIri as fn(Box<Expression>) -> Expression,
            ),
            (
                "isURI",
                Expression::IsIri as fn(Box<Expression>) -> Expression,
            ),
            (
                "isBLANK",
                Expression::IsBlank as fn(Box<Expression>) -> Expression,
            ),
            (
                "isLITERAL",
                Expression::IsLiteral as fn(Box<Expression>) -> Expression,
            ),
            (
                "isNUMERIC",
                Expression::IsNumeric as fn(Box<Expression>) -> Expression,
            ),
        ] {
            if self.match_keyword(keyword) {
                self.skip_whitespace();
                if !self.match_char('(') {
                    return Err(SparqlError::ParseError(format!(
                        "Expected ( after {}",
                        keyword
                    )));
                }
                let arg = self.parse_expression()?;
                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(format!(
                        "Expected ) after {}",
                        keyword
                    )));
                }
                return Ok(constructor(Box::new(arg)));
            }
        }

        // STR, LANG, DATATYPE
        if self.match_keyword("STR") {
            return self.parse_single_arg_function(|e| Expression::Str(Box::new(e)));
        }
        if self.match_keyword("LANG") {
            return self.parse_single_arg_function(|e| Expression::Lang(Box::new(e)));
        }
        if self.match_keyword("DATATYPE") {
            return self.parse_single_arg_function(|e| Expression::Datatype(Box::new(e)));
        }
        if self.match_keyword("IRI") || self.match_keyword("URI") {
            return self.parse_single_arg_function(|e| Expression::Iri(Box::new(e)));
        }

        // REGEX
        if self.match_keyword("REGEX") {
            return self.parse_regex_expression();
        }

        // Other functions
        if let Some(func) = self.try_parse_function_call()? {
            return Ok(Expression::Function(func));
        }

        // Variable
        if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
            return Ok(Expression::var(self.parse_variable_name()?));
        }

        // Literal or IRI
        let term = self.parse_rdf_term()?;
        Ok(Expression::term(term))
    }

    fn parse_single_arg_function<F>(&mut self, constructor: F) -> Result<Expression, SparqlError>
    where
        F: FnOnce(Expression) -> Expression,
    {
        self.skip_whitespace();
        if !self.match_char('(') {
            return Err(SparqlError::ParseError(
                "Expected ( for function".to_string(),
            ));
        }
        let arg = self.parse_expression()?;
        self.skip_whitespace();
        if !self.match_char(')') {
            return Err(SparqlError::ParseError(
                "Expected ) for function".to_string(),
            ));
        }
        Ok(constructor(arg))
    }

    fn parse_if_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();
        if !self.match_char('(') {
            return Err(SparqlError::ParseError("Expected ( after IF".to_string()));
        }

        let condition = self.parse_expression()?;
        self.skip_whitespace();
        if !self.match_char(',') {
            return Err(SparqlError::ParseError("Expected , in IF".to_string()));
        }

        let then_expr = self.parse_expression()?;
        self.skip_whitespace();
        if !self.match_char(',') {
            return Err(SparqlError::ParseError("Expected , in IF".to_string()));
        }

        let else_expr = self.parse_expression()?;
        self.skip_whitespace();
        if !self.match_char(')') {
            return Err(SparqlError::ParseError("Expected ) after IF".to_string()));
        }

        Ok(Expression::If(
            Box::new(condition),
            Box::new(then_expr),
            Box::new(else_expr),
        ))
    }

    fn parse_coalesce_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();
        if !self.match_char('(') {
            return Err(SparqlError::ParseError(
                "Expected ( after COALESCE".to_string(),
            ));
        }

        let exprs = self.parse_expression_list()?;

        self.skip_whitespace();
        if !self.match_char(')') {
            return Err(SparqlError::ParseError(
                "Expected ) after COALESCE".to_string(),
            ));
        }

        Ok(Expression::Coalesce(exprs))
    }

    fn parse_regex_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();
        if !self.match_char('(') {
            return Err(SparqlError::ParseError(
                "Expected ( after REGEX".to_string(),
            ));
        }

        let text = self.parse_expression()?;
        self.skip_whitespace();
        if !self.match_char(',') {
            return Err(SparqlError::ParseError("Expected , in REGEX".to_string()));
        }

        let pattern = self.parse_expression()?;

        self.skip_whitespace();
        let flags = if self.match_char(',') {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.skip_whitespace();
        if !self.match_char(')') {
            return Err(SparqlError::ParseError(
                "Expected ) after REGEX".to_string(),
            ));
        }

        Ok(Expression::Regex(Box::new(text), Box::new(pattern), flags))
    }

    fn try_parse_aggregate(&mut self) -> Result<Option<Aggregate>, SparqlError> {
        let saved_pos = self.pos;

        for keyword in &[
            "COUNT",
            "SUM",
            "AVG",
            "MIN",
            "MAX",
            "GROUP_CONCAT",
            "SAMPLE",
        ] {
            if self.match_keyword(keyword) {
                self.skip_whitespace();
                if !self.match_char('(') {
                    self.pos = saved_pos;
                    continue;
                }

                self.skip_whitespace();
                let distinct = self.match_keyword("DISTINCT");

                self.skip_whitespace();

                let agg = match *keyword {
                    "COUNT" => {
                        let expr = if self.match_char('*') {
                            None
                        } else {
                            Some(Box::new(self.parse_expression()?))
                        };
                        Aggregate::Count { expr, distinct }
                    }
                    "SUM" => Aggregate::Sum {
                        expr: Box::new(self.parse_expression()?),
                        distinct,
                    },
                    "AVG" => Aggregate::Avg {
                        expr: Box::new(self.parse_expression()?),
                        distinct,
                    },
                    "MIN" => Aggregate::Min {
                        expr: Box::new(self.parse_expression()?),
                    },
                    "MAX" => Aggregate::Max {
                        expr: Box::new(self.parse_expression()?),
                    },
                    "GROUP_CONCAT" => {
                        let expr = Box::new(self.parse_expression()?);
                        self.skip_whitespace();
                        let separator = if self.match_char(';') {
                            self.skip_whitespace();
                            if self.match_keyword("SEPARATOR") {
                                self.skip_whitespace();
                                if !self.match_char('=') {
                                    return Err(SparqlError::ParseError(
                                        "Expected = after SEPARATOR".to_string(),
                                    ));
                                }
                                let sep = self.parse_literal()?;
                                Some(sep.value)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        Aggregate::GroupConcat {
                            expr,
                            separator,
                            distinct,
                        }
                    }
                    "SAMPLE" => Aggregate::Sample {
                        expr: Box::new(self.parse_expression()?),
                    },
                    _ => unreachable!(),
                };

                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(
                        "Expected ) after aggregate".to_string(),
                    ));
                }

                return Ok(Some(agg));
            }
        }

        Ok(None)
    }

    fn try_parse_function_call(&mut self) -> Result<Option<FunctionCall>, SparqlError> {
        // Try to parse a function call: name(args)
        let saved_pos = self.pos;

        // Try IRI function
        if self.peek_char() == Some('<') {
            let iri = self.parse_iri_ref()?;
            self.skip_whitespace();
            if self.match_char('(') {
                let args = self.parse_expression_list()?;
                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(
                        "Expected ) after function".to_string(),
                    ));
                }
                return Ok(Some(FunctionCall::new(iri.as_str(), args)));
            } else {
                self.pos = saved_pos;
                return Ok(None);
            }
        }

        // Try prefixed name function
        if let Ok(name) = self.try_parse_function_name() {
            self.skip_whitespace();
            if self.match_char('(') {
                let args = self.parse_expression_list()?;
                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(SparqlError::ParseError(
                        "Expected ) after function".to_string(),
                    ));
                }
                return Ok(Some(FunctionCall::new(name, args)));
            } else {
                self.pos = saved_pos;
            }
        }

        Ok(None)
    }

    fn try_parse_function_name(&mut self) -> Result<String, SparqlError> {
        // Parse built-in function names
        let builtin_functions = [
            "STRLEN",
            "SUBSTR",
            "UCASE",
            "LCASE",
            "STRSTARTS",
            "STRENDS",
            "CONTAINS",
            "STRBEFORE",
            "STRAFTER",
            "ENCODE_FOR_URI",
            "CONCAT",
            "LANGMATCHES",
            "REPLACE",
            "ABS",
            "ROUND",
            "CEIL",
            "FLOOR",
            "RAND",
            "NOW",
            "YEAR",
            "MONTH",
            "DAY",
            "HOURS",
            "MINUTES",
            "SECONDS",
            "TIMEZONE",
            "TZ",
            "MD5",
            "SHA1",
            "SHA256",
            "SHA384",
            "SHA512",
            "STRUUID",
            "UUID",
            "BNODE",
        ];

        for func in &builtin_functions {
            if self.match_keyword(func) {
                return Ok(func.to_string());
            }
        }

        // Try prefixed name
        let iri = self.parse_prefixed_name()?;
        Ok(iri.as_str().to_string())
    }

    fn parse_expression_list(&mut self) -> Result<Vec<Expression>, SparqlError> {
        let mut exprs = Vec::new();

        self.skip_whitespace();
        if self.peek_char() == Some(')') {
            return Ok(exprs);
        }

        loop {
            exprs.push(self.parse_expression()?);
            self.skip_whitespace();
            if !self.match_char(',') {
                break;
            }
        }

        Ok(exprs)
    }

    fn parse_filter_expression(&mut self) -> Result<Expression, SparqlError> {
        self.skip_whitespace();

        if self.match_char('(') {
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(SparqlError::ParseError(
                    "Expected ) after FILTER".to_string(),
                ));
            }
            Ok(expr)
        } else {
            // Built-in call without parens
            self.parse_expression()
        }
    }

    fn parse_solution_modifiers(&mut self) -> Result<SolutionModifier, SparqlError> {
        let mut modifier = SolutionModifier::default();

        // GROUP BY
        self.skip_whitespace();
        if self.match_keyword("GROUP") {
            self.skip_whitespace();
            if !self.match_keyword("BY") {
                return Err(SparqlError::ParseError(
                    "Expected BY after GROUP".to_string(),
                ));
            }
            // Skip GROUP BY for now - would need to handle in modifier
        }

        // HAVING
        self.skip_whitespace();
        if self.match_keyword("HAVING") {
            self.skip_whitespace();
            let expr = self.parse_expression()?;
            modifier.having = Some(expr);
        }

        // ORDER BY
        self.skip_whitespace();
        if self.match_keyword("ORDER") {
            self.skip_whitespace();
            if !self.match_keyword("BY") {
                return Err(SparqlError::ParseError(
                    "Expected BY after ORDER".to_string(),
                ));
            }

            loop {
                self.skip_whitespace();

                let ascending = if self.match_keyword("ASC") {
                    true
                } else if self.match_keyword("DESC") {
                    false
                } else {
                    true // Default ascending
                };

                self.skip_whitespace();

                // Check for parenthesized expression
                let expr = if self.match_char('(') {
                    let e = self.parse_expression()?;
                    self.skip_whitespace();
                    if !self.match_char(')') {
                        return Err(SparqlError::ParseError(
                            "Expected ) in ORDER BY".to_string(),
                        ));
                    }
                    e
                } else if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
                    Expression::var(self.parse_variable_name()?)
                } else {
                    break;
                };

                modifier.order_by.push(OrderCondition {
                    expression: expr,
                    ascending,
                });

                self.skip_whitespace();
                if self.peek_char() == Some('?')
                    || self.peek_char() == Some('$')
                    || self.peek_keyword("ASC")
                    || self.peek_keyword("DESC")
                {
                    continue;
                }
                break;
            }
        }

        // LIMIT
        self.skip_whitespace();
        if self.match_keyword("LIMIT") {
            self.skip_whitespace();
            let limit = self.parse_integer()?;
            modifier.limit = Some(limit as usize);
        }

        // OFFSET
        self.skip_whitespace();
        if self.match_keyword("OFFSET") {
            self.skip_whitespace();
            let offset = self.parse_integer()?;
            modifier.offset = Some(offset as usize);
        }

        Ok(modifier)
    }

    fn parse_values_clause(&mut self) -> Result<Option<ValuesClause>, SparqlError> {
        self.skip_whitespace();
        if !self.match_keyword("VALUES") {
            return Ok(None);
        }

        Ok(Some(self.parse_inline_values()?))
    }

    fn parse_inline_values(&mut self) -> Result<ValuesClause, SparqlError> {
        self.skip_whitespace();

        let mut variables = Vec::new();

        // Single variable or list
        if self.match_char('(') {
            loop {
                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                }
                variables.push(self.parse_variable_name()?);
            }
        } else {
            variables.push(self.parse_variable_name()?);
        }

        self.skip_whitespace();
        if !self.match_char('{') {
            return Err(SparqlError::ParseError("Expected { in VALUES".to_string()));
        }

        let mut bindings = Vec::new();

        loop {
            self.skip_whitespace();
            if self.match_char('}') {
                break;
            }

            if self.match_char('(') {
                let mut row = Vec::new();
                loop {
                    self.skip_whitespace();
                    if self.match_char(')') {
                        break;
                    }

                    if self.match_keyword("UNDEF") {
                        row.push(None);
                    } else {
                        row.push(Some(self.parse_rdf_term()?));
                    }
                }
                bindings.push(row);
            } else {
                // Single value per row
                if self.match_keyword("UNDEF") {
                    bindings.push(vec![None]);
                } else {
                    bindings.push(vec![Some(self.parse_rdf_term()?)]);
                }
            }
        }

        Ok(ValuesClause {
            variables,
            bindings,
        })
    }

    fn parse_construct_query(&mut self) -> Result<ConstructQuery, SparqlError> {
        self.skip_whitespace();

        // Parse template
        if !self.match_char('{') {
            return Err(SparqlError::ParseError(
                "Expected { for CONSTRUCT template".to_string(),
            ));
        }

        let template = self.parse_triples_block()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError(
                "Expected } for CONSTRUCT template".to_string(),
            ));
        }

        // Dataset clauses
        let dataset = self.parse_dataset_clauses()?;

        // WHERE clause
        self.skip_whitespace();
        let where_clause = if self.match_keyword("WHERE") {
            self.skip_whitespace();
            self.parse_group_graph_pattern()?
        } else {
            GraphPattern::Empty
        };

        // Solution modifiers
        let modifier = self.parse_solution_modifiers()?;

        Ok(ConstructQuery {
            template,
            dataset,
            where_clause,
            modifier,
        })
    }

    fn parse_ask_query(&mut self) -> Result<AskQuery, SparqlError> {
        // Dataset clauses
        let dataset = self.parse_dataset_clauses()?;

        // WHERE clause
        self.skip_whitespace();
        let where_clause = if self.match_keyword("WHERE") {
            self.skip_whitespace();
            self.parse_group_graph_pattern()?
        } else {
            self.parse_group_graph_pattern()?
        };

        Ok(AskQuery {
            dataset,
            where_clause,
        })
    }

    fn parse_describe_query(&mut self) -> Result<DescribeQuery, SparqlError> {
        self.skip_whitespace();

        let mut resources = Vec::new();

        // Parse resource list
        if self.match_char('*') {
            // DESCRIBE * - all resources (handled during execution)
        } else {
            loop {
                self.skip_whitespace();

                if self.peek_keyword("FROM")
                    || self.peek_keyword("WHERE")
                    || self.peek_char() == Some('{')
                {
                    break;
                }

                resources.push(self.parse_var_or_iri()?);
            }
        }

        // Dataset clauses
        let dataset = self.parse_dataset_clauses()?;

        // Optional WHERE clause
        self.skip_whitespace();
        let where_clause = if self.match_keyword("WHERE") || self.peek_char() == Some('{') {
            if self.peek_char() == Some('{') {
                Some(self.parse_group_graph_pattern()?)
            } else {
                self.skip_whitespace();
                Some(self.parse_group_graph_pattern()?)
            }
        } else {
            None
        };

        Ok(DescribeQuery {
            resources,
            dataset,
            where_clause,
        })
    }

    fn parse_update(&mut self) -> Result<Vec<UpdateOperation>, SparqlError> {
        let mut operations = Vec::new();

        loop {
            self.skip_whitespace();

            if self.is_at_end() {
                break;
            }

            let op = if self.match_keyword("INSERT") {
                self.skip_whitespace();
                if self.match_keyword("DATA") {
                    self.parse_insert_data()?
                } else {
                    // INSERT { } WHERE { }
                    self.parse_modify_insert()?
                }
            } else if self.match_keyword("DELETE") {
                self.skip_whitespace();
                if self.match_keyword("DATA") {
                    self.parse_delete_data()?
                } else if self.match_keyword("WHERE") {
                    // DELETE WHERE { }
                    self.parse_delete_where()?
                } else {
                    // DELETE { } INSERT { } WHERE { }
                    self.parse_modify_delete()?
                }
            } else if self.match_keyword("LOAD") {
                self.parse_load()?
            } else if self.match_keyword("CLEAR") {
                self.parse_clear()?
            } else if self.match_keyword("CREATE") {
                self.parse_create()?
            } else if self.match_keyword("DROP") {
                self.parse_drop()?
            } else {
                break;
            };

            operations.push(op);

            // Check for separator
            self.skip_whitespace();
            if !self.match_char(';') {
                break;
            }
        }

        Ok(operations)
    }

    fn parse_insert_data(&mut self) -> Result<UpdateOperation, SparqlError> {
        self.skip_whitespace();
        if !self.match_char('{') {
            return Err(SparqlError::ParseError(
                "Expected { for INSERT DATA".to_string(),
            ));
        }

        let quads = self.parse_quads()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError(
                "Expected } for INSERT DATA".to_string(),
            ));
        }

        Ok(UpdateOperation::InsertData(InsertData { quads }))
    }

    fn parse_delete_data(&mut self) -> Result<UpdateOperation, SparqlError> {
        self.skip_whitespace();
        if !self.match_char('{') {
            return Err(SparqlError::ParseError(
                "Expected { for DELETE DATA".to_string(),
            ));
        }

        let quads = self.parse_quads()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError(
                "Expected } for DELETE DATA".to_string(),
            ));
        }

        Ok(UpdateOperation::DeleteData(DeleteData { quads }))
    }

    fn parse_quads(&mut self) -> Result<Vec<Quad>, SparqlError> {
        let mut quads = Vec::new();

        loop {
            self.skip_whitespace();

            if self.peek_char() == Some('}') {
                break;
            }

            // Check for GRAPH
            let graph = if self.match_keyword("GRAPH") {
                self.skip_whitespace();
                let graph_iri = self.parse_iri_ref()?;
                self.skip_whitespace();
                if !self.match_char('{') {
                    return Err(SparqlError::ParseError(
                        "Expected { after GRAPH".to_string(),
                    ));
                }
                Some(graph_iri)
            } else {
                None
            };

            // Parse triples
            let triple_patterns = self.parse_triples_block()?;

            for tp in triple_patterns {
                if let (TermOrVariable::Term(s), PropertyPath::Iri(p), TermOrVariable::Term(o)) =
                    (tp.subject, tp.predicate, tp.object)
                {
                    quads.push(Quad {
                        subject: s,
                        predicate: p,
                        object: o,
                        graph: graph.clone(),
                    });
                }
            }

            if graph.is_some() {
                self.skip_whitespace();
                if !self.match_char('}') {
                    return Err(SparqlError::ParseError(
                        "Expected } after GRAPH triples".to_string(),
                    ));
                }
            }
        }

        Ok(quads)
    }

    fn parse_modify_insert(&mut self) -> Result<UpdateOperation, SparqlError> {
        // INSERT { pattern } WHERE { pattern }
        self.skip_whitespace();
        if !self.match_char('{') {
            return Err(SparqlError::ParseError("Expected { for INSERT".to_string()));
        }

        let insert_patterns = self.parse_quad_patterns()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError("Expected } for INSERT".to_string()));
        }

        self.skip_whitespace();
        if !self.match_keyword("WHERE") {
            return Err(SparqlError::ParseError(
                "Expected WHERE after INSERT".to_string(),
            ));
        }

        self.skip_whitespace();
        let where_pattern = self.parse_group_graph_pattern()?;

        Ok(UpdateOperation::Modify(Modify {
            with_graph: None,
            delete_pattern: None,
            insert_pattern: Some(insert_patterns),
            using: Vec::new(),
            where_pattern,
        }))
    }

    fn parse_modify_delete(&mut self) -> Result<UpdateOperation, SparqlError> {
        // DELETE { pattern } [INSERT { pattern }] WHERE { pattern }
        self.skip_whitespace();
        if !self.match_char('{') {
            return Err(SparqlError::ParseError("Expected { for DELETE".to_string()));
        }

        let delete_patterns = self.parse_quad_patterns()?;

        self.skip_whitespace();
        if !self.match_char('}') {
            return Err(SparqlError::ParseError("Expected } for DELETE".to_string()));
        }

        // Optional INSERT
        self.skip_whitespace();
        let insert_patterns = if self.match_keyword("INSERT") {
            self.skip_whitespace();
            if !self.match_char('{') {
                return Err(SparqlError::ParseError("Expected { for INSERT".to_string()));
            }

            let patterns = self.parse_quad_patterns()?;

            self.skip_whitespace();
            if !self.match_char('}') {
                return Err(SparqlError::ParseError("Expected } for INSERT".to_string()));
            }

            Some(patterns)
        } else {
            None
        };

        self.skip_whitespace();
        if !self.match_keyword("WHERE") {
            return Err(SparqlError::ParseError(
                "Expected WHERE after DELETE".to_string(),
            ));
        }

        self.skip_whitespace();
        let where_pattern = self.parse_group_graph_pattern()?;

        Ok(UpdateOperation::Modify(Modify {
            with_graph: None,
            delete_pattern: Some(delete_patterns),
            insert_pattern: insert_patterns,
            using: Vec::new(),
            where_pattern,
        }))
    }

    fn parse_delete_where(&mut self) -> Result<UpdateOperation, SparqlError> {
        self.skip_whitespace();
        let where_pattern = self.parse_group_graph_pattern()?;

        // Convert graph pattern to quad patterns for deletion
        let delete_patterns = self.graph_pattern_to_quad_patterns(&where_pattern);

        Ok(UpdateOperation::Modify(Modify {
            with_graph: None,
            delete_pattern: Some(delete_patterns),
            insert_pattern: None,
            using: Vec::new(),
            where_pattern,
        }))
    }

    fn parse_quad_patterns(&mut self) -> Result<Vec<QuadPattern>, SparqlError> {
        let triples = self.parse_triples_block()?;

        Ok(triples
            .into_iter()
            .map(|tp| QuadPattern {
                subject: tp.subject,
                predicate: match tp.predicate {
                    PropertyPath::Iri(iri) => VarOrIri::Iri(iri),
                    PropertyPath::Variable(v) => VarOrIri::Variable(v),
                    _ => VarOrIri::Variable("_path".to_string()),
                },
                object: tp.object,
                graph: None,
            })
            .collect())
    }

    fn graph_pattern_to_quad_patterns(&self, pattern: &GraphPattern) -> Vec<QuadPattern> {
        match pattern {
            GraphPattern::Bgp(triples) => triples
                .iter()
                .map(|tp| QuadPattern {
                    subject: tp.subject.clone(),
                    predicate: match &tp.predicate {
                        PropertyPath::Iri(iri) => VarOrIri::Iri(iri.clone()),
                        PropertyPath::Variable(v) => VarOrIri::Variable(v.clone()),
                        _ => VarOrIri::Variable("_path".to_string()),
                    },
                    object: tp.object.clone(),
                    graph: None,
                })
                .collect(),
            GraphPattern::Join(a, b) => {
                let mut result = self.graph_pattern_to_quad_patterns(a);
                result.extend(self.graph_pattern_to_quad_patterns(b));
                result
            }
            _ => Vec::new(),
        }
    }

    fn parse_load(&mut self) -> Result<UpdateOperation, SparqlError> {
        let silent = self.match_keyword("SILENT");
        self.skip_whitespace();

        let source = self.parse_iri_ref()?;

        self.skip_whitespace();
        let destination = if self.match_keyword("INTO") {
            self.skip_whitespace();
            if !self.match_keyword("GRAPH") {
                return Err(SparqlError::ParseError(
                    "Expected GRAPH after INTO".to_string(),
                ));
            }
            self.skip_whitespace();
            Some(self.parse_iri_ref()?)
        } else {
            None
        };

        Ok(UpdateOperation::Load {
            source,
            destination,
            silent,
        })
    }

    fn parse_clear(&mut self) -> Result<UpdateOperation, SparqlError> {
        let silent = self.match_keyword("SILENT");
        self.skip_whitespace();

        let target = self.parse_graph_target()?;

        Ok(UpdateOperation::Clear { target, silent })
    }

    fn parse_create(&mut self) -> Result<UpdateOperation, SparqlError> {
        let silent = self.match_keyword("SILENT");
        self.skip_whitespace();

        if !self.match_keyword("GRAPH") {
            return Err(SparqlError::ParseError(
                "Expected GRAPH after CREATE".to_string(),
            ));
        }
        self.skip_whitespace();

        let graph = self.parse_iri_ref()?;

        Ok(UpdateOperation::Create { graph, silent })
    }

    fn parse_drop(&mut self) -> Result<UpdateOperation, SparqlError> {
        let silent = self.match_keyword("SILENT");
        self.skip_whitespace();

        let target = self.parse_graph_target()?;

        Ok(UpdateOperation::Drop { target, silent })
    }

    fn parse_graph_target(&mut self) -> Result<GraphTarget, SparqlError> {
        if self.match_keyword("DEFAULT") {
            Ok(GraphTarget::Default)
        } else if self.match_keyword("NAMED") {
            Ok(GraphTarget::AllNamed)
        } else if self.match_keyword("ALL") {
            Ok(GraphTarget::All)
        } else if self.match_keyword("GRAPH") {
            self.skip_whitespace();
            let iri = self.parse_iri_ref()?;
            Ok(GraphTarget::Named(iri))
        } else {
            Err(SparqlError::ParseError("Expected graph target".to_string()))
        }
    }

    // ========== Helper methods ==========

    fn parse_iri_ref(&mut self) -> Result<Iri, SparqlError> {
        self.skip_whitespace();

        if self.match_char('<') {
            let start = self.pos;
            while let Some(c) = self.peek_char() {
                if c == '>' {
                    let iri = &self.input[start..self.pos];
                    self.next_char();
                    return Ok(Iri::new(iri));
                }
                self.next_char();
            }
            Err(SparqlError::ParseError("Unclosed IRI".to_string()))
        } else {
            self.parse_prefixed_name()
        }
    }

    fn parse_prefixed_name(&mut self) -> Result<Iri, SparqlError> {
        let start = self.pos;

        // Parse prefix
        while let Some(c) = self.peek_char() {
            if c == ':' {
                break;
            }
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                break;
            }
            self.next_char();
        }

        let prefix = &self.input[start..self.pos];

        if !self.match_char(':') {
            return Err(SparqlError::ParseError(format!(
                "Expected : in prefixed name at {}",
                self.pos
            )));
        }

        // Parse local part
        let local_start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || "{}[](),;.".contains(c) {
                break;
            }
            self.next_char();
        }

        let local = &self.input[local_start..self.pos];

        // Resolve prefix
        if let Some(base) = self.prefixes.get(prefix) {
            Ok(Iri::new(format!("{}{}", base.as_str(), local)))
        } else if prefix.is_empty() {
            // Default prefix
            if let Some(base) = self.prefixes.get("") {
                Ok(Iri::new(format!("{}{}", base.as_str(), local)))
            } else {
                Ok(Iri::new(format!(":{}", local)))
            }
        } else {
            // Return unresolved for now
            Ok(Iri::new(format!("{}:{}", prefix, local)))
        }
    }

    fn parse_prefix_name(&mut self) -> Result<String, SparqlError> {
        let start = self.pos;

        while let Some(c) = self.peek_char() {
            if c == ':' {
                let prefix = self.input[start..self.pos].to_string();
                self.next_char();
                return Ok(prefix);
            }
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            self.next_char();
        }

        if self.match_char(':') {
            Ok(self.input[start..self.pos - 1].to_string())
        } else {
            Err(SparqlError::ParseError("Expected prefix name".to_string()))
        }
    }

    fn parse_variable_name(&mut self) -> Result<String, SparqlError> {
        if !self.match_char('?') && !self.match_char('$') {
            return Err(SparqlError::ParseError(
                "Expected ? or $ for variable".to_string(),
            ));
        }

        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            self.next_char();
        }

        let name = &self.input[start..self.pos];
        if name.is_empty() {
            return Err(SparqlError::ParseError("Empty variable name".to_string()));
        }

        Ok(name.to_string())
    }

    fn parse_blank_node(&mut self) -> Result<String, SparqlError> {
        if !self.match_char('_') || !self.match_char(':') {
            return Err(SparqlError::ParseError(
                "Expected _: for blank node".to_string(),
            ));
        }

        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' {
                break;
            }
            self.next_char();
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_literal(&mut self) -> Result<Literal, SparqlError> {
        let quote = self
            .next_char()
            .ok_or_else(|| SparqlError::ParseError("Expected quote".to_string()))?;

        if quote != '"' && quote != '\'' {
            return Err(SparqlError::ParseError(
                "Expected \" or ' for literal".to_string(),
            ));
        }

        // Check for long literal (""" or ''')
        let long = if self.peek_char() == Some(quote) && self.peek_char_at(1) == Some(quote) {
            self.next_char();
            self.next_char();
            true
        } else {
            false
        };

        let mut value = String::new();
        let end_pattern = if long {
            format!("{}{}{}", quote, quote, quote)
        } else {
            quote.to_string()
        };

        loop {
            if self.is_at_end() {
                return Err(SparqlError::ParseError("Unclosed literal".to_string()));
            }

            // Check for end
            if long {
                if self.peek_char() == Some(quote)
                    && self.peek_char_at(1) == Some(quote)
                    && self.peek_char_at(2) == Some(quote)
                {
                    self.next_char();
                    self.next_char();
                    self.next_char();
                    break;
                }
            } else if self.peek_char() == Some(quote) {
                self.next_char();
                break;
            }

            // Handle escape sequences
            if self.peek_char() == Some('\\') {
                self.next_char();
                match self.next_char() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('\'') => value.push('\''),
                    Some(c) => {
                        value.push('\\');
                        value.push(c);
                    }
                    None => {
                        return Err(SparqlError::ParseError(
                            "Unexpected end in escape".to_string(),
                        ))
                    }
                }
            } else {
                value.push(self.next_char().unwrap());
            }
        }

        // Check for language tag or datatype
        if self.match_char('@') {
            let start = self.pos;
            while let Some(c) = self.peek_char() {
                if !c.is_alphanumeric() && c != '-' {
                    break;
                }
                self.next_char();
            }
            let lang = self.input[start..self.pos].to_string();
            Ok(Literal::language(value, lang))
        } else if self.match_char('^') && self.match_char('^') {
            let datatype = self.parse_iri_ref()?;
            Ok(Literal::typed(value, datatype))
        } else {
            Ok(Literal::simple(value))
        }
    }

    fn parse_numeric_literal(&mut self) -> Result<Literal, SparqlError> {
        let start = self.pos;

        // Optional sign
        if self.peek_char() == Some('+') || self.peek_char() == Some('-') {
            self.next_char();
        }

        // Integer part
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }

        // Check for decimal/double
        if self.peek_char() == Some('.')
            && self
                .peek_char_at(1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            self.next_char();
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    self.next_char();
                } else {
                    break;
                }
            }

            // Check for exponent
            if self.peek_char() == Some('e') || self.peek_char() == Some('E') {
                self.next_char();
                if self.peek_char() == Some('+') || self.peek_char() == Some('-') {
                    self.next_char();
                }
                while let Some(c) = self.peek_char() {
                    if c.is_ascii_digit() {
                        self.next_char();
                    } else {
                        break;
                    }
                }
                Ok(Literal::typed(
                    &self.input[start..self.pos],
                    Iri::xsd_double(),
                ))
            } else {
                Ok(Literal::typed(
                    &self.input[start..self.pos],
                    Iri::xsd_decimal(),
                ))
            }
        } else if self.peek_char() == Some('e') || self.peek_char() == Some('E') {
            self.next_char();
            if self.peek_char() == Some('+') || self.peek_char() == Some('-') {
                self.next_char();
            }
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    self.next_char();
                } else {
                    break;
                }
            }
            Ok(Literal::typed(
                &self.input[start..self.pos],
                Iri::xsd_double(),
            ))
        } else {
            Ok(Literal::typed(
                &self.input[start..self.pos],
                Iri::xsd_integer(),
            ))
        }
    }

    fn parse_integer(&mut self) -> Result<i64, SparqlError> {
        let start = self.pos;

        if self.peek_char() == Some('-') || self.peek_char() == Some('+') {
            self.next_char();
        }

        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }

        let num_str = &self.input[start..self.pos];
        num_str
            .parse()
            .map_err(|_| SparqlError::ParseError(format!("Invalid integer: {}", num_str)))
    }

    fn parse_var_or_iri(&mut self) -> Result<VarOrIri, SparqlError> {
        self.skip_whitespace();

        if self.peek_char() == Some('?') || self.peek_char() == Some('$') {
            Ok(VarOrIri::Variable(self.parse_variable_name()?))
        } else {
            Ok(VarOrIri::Iri(self.parse_iri_ref()?))
        }
    }

    // ========== Low-level helpers ==========

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
            } else if c == '#' {
                // Comment - skip to end of line
                while let Some(c) = self.next_char() {
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.input[self.pos..].chars().nth(offset)
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.next_char();
            true
        } else {
            false
        }
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if remaining.len() < keyword.len() {
            return false;
        }

        let potential = &remaining[..keyword.len()];
        if potential.eq_ignore_ascii_case(keyword) {
            // Make sure it's not part of a longer identifier
            let after = remaining.chars().nth(keyword.len());
            if after
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false)
            {
                return false;
            }
            self.pos += keyword.len();
            true
        } else {
            false
        }
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if remaining.len() < keyword.len() {
            return false;
        }

        let potential = &remaining[..keyword.len()];
        if potential.eq_ignore_ascii_case(keyword) {
            let after = remaining.chars().nth(keyword.len());
            !after
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_to_next_pattern(&mut self) -> bool {
        // Skip to next meaningful token
        while let Some(c) = self.peek_char() {
            if c == '.' || c == ';' || c == '}' || c == '{' {
                if c == '.' || c == ';' {
                    self.next_char();
                }
                return true;
            }
            self.next_char();
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
        let result = parse_sparql(query);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(matches!(parsed.body, QueryBody::Select(_)));
    }

    #[test]
    fn test_parse_select_with_filter() {
        let query = r#"
            PREFIX ex: <http://example.org/>
            SELECT ?name WHERE {
                ?person ex:name ?name .
                FILTER(?name = "Alice")
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_select_distinct() {
        let query = "SELECT DISTINCT ?type WHERE { ?s a ?type }";
        let result = parse_sparql(query);
        assert!(result.is_ok());

        if let QueryBody::Select(select) = result.unwrap().body {
            assert!(matches!(select.projection, Projection::Distinct(_)));
        } else {
            panic!("Expected SELECT query");
        }
    }

    #[test]
    fn test_parse_optional() {
        let query = r#"
            SELECT ?name ?email WHERE {
                ?person <http://example.org/name> ?name .
                OPTIONAL { ?person <http://example.org/email> ?email }
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_union() {
        let query = r#"
            SELECT ?name WHERE {
                { ?s <http://example.org/firstName> ?name }
                UNION
                { ?s <http://example.org/lastName> ?name }
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_ask() {
        let query = "ASK { ?s ?p ?o }";
        let result = parse_sparql(query);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().body, QueryBody::Ask(_)));
    }

    #[test]
    fn test_parse_construct() {
        let query = r#"
            CONSTRUCT { ?s <http://example.org/knows> ?o }
            WHERE { ?s <http://example.org/friend> ?o }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().body, QueryBody::Construct(_)));
    }

    #[test]
    fn test_parse_describe() {
        let query = "DESCRIBE <http://example.org/person/1>";
        let result = parse_sparql(query);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().body, QueryBody::Describe(_)));
    }

    #[test]
    fn test_parse_property_path() {
        let query = r#"
            SELECT ?ancestor WHERE {
                <http://example.org/person/1> <http://example.org/parent>+ ?ancestor
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_aggregates() {
        let query = "SELECT (COUNT(?s) AS ?count) WHERE { ?s ?p ?o }";
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_order_limit() {
        let query = "SELECT ?name WHERE { ?s <http://example.org/name> ?name } ORDER BY ?name LIMIT 10 OFFSET 5";
        let result = parse_sparql(query);
        assert!(result.is_ok());

        if let QueryBody::Select(select) = result.unwrap().body {
            assert_eq!(select.modifier.limit, Some(10));
            assert_eq!(select.modifier.offset, Some(5));
            assert!(!select.modifier.order_by.is_empty());
        }
    }

    #[test]
    fn test_parse_insert_data() {
        let query = r#"
            INSERT DATA {
                <http://example.org/person/1> <http://example.org/name> "Alice" .
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().body, QueryBody::Update(_)));
    }

    #[test]
    fn test_parse_delete_data() {
        let query = r#"
            DELETE DATA {
                <http://example.org/person/1> <http://example.org/name> "Alice" .
            }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_values() {
        let query = r#"
            SELECT ?name WHERE {
                ?person <http://example.org/name> ?name
            }
            VALUES ?name { "Alice" "Bob" }
        "#;
        let result = parse_sparql(query);
        assert!(result.is_ok());
    }
}
