// SPARQL Abstract Syntax Tree (AST) types
//
// Provides type-safe representation of SPARQL 1.1 queries following
// the W3C specification: https://www.w3.org/TR/sparql11-query/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete SPARQL query or update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparqlQuery {
    /// Base IRI for relative IRI resolution
    pub base: Option<Iri>,
    /// PREFIX declarations
    pub prefixes: HashMap<String, Iri>,
    /// The query form (SELECT, CONSTRUCT, ASK, DESCRIBE) or update operation
    pub body: QueryBody,
}

impl SparqlQuery {
    pub fn new(body: QueryBody) -> Self {
        Self {
            base: None,
            prefixes: HashMap::new(),
            body,
        }
    }

    pub fn with_base(mut self, base: Iri) -> Self {
        self.base = Some(base);
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>, iri: Iri) -> Self {
        self.prefixes.insert(prefix.into(), iri);
        self
    }
}

impl Default for SparqlQuery {
    fn default() -> Self {
        Self::new(QueryBody::Select(SelectQuery::default()))
    }
}

/// Query body - either a query form or update operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryBody {
    Select(SelectQuery),
    Construct(ConstructQuery),
    Ask(AskQuery),
    Describe(DescribeQuery),
    Update(Vec<UpdateOperation>),
}

/// Query form type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryForm {
    Select,
    Construct,
    Ask,
    Describe,
}

/// SELECT query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectQuery {
    /// Result variables or expressions
    pub projection: Projection,
    /// Dataset clauses (FROM, FROM NAMED)
    pub dataset: Vec<DatasetClause>,
    /// WHERE clause graph pattern
    pub where_clause: GraphPattern,
    /// Solution modifiers
    pub modifier: SolutionModifier,
    /// VALUES clause for inline data
    pub values: Option<ValuesClause>,
}

impl Default for SelectQuery {
    fn default() -> Self {
        Self {
            projection: Projection::All,
            dataset: Vec::new(),
            where_clause: GraphPattern::Empty,
            modifier: SolutionModifier::default(),
            values: None,
        }
    }
}

/// Projection in SELECT clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Projection {
    /// SELECT * - all variables
    All,
    /// SELECT DISTINCT ...
    Distinct(Vec<ProjectionVar>),
    /// SELECT REDUCED ...
    Reduced(Vec<ProjectionVar>),
    /// SELECT var1 var2 ...
    Variables(Vec<ProjectionVar>),
}

/// Variable or expression in projection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionVar {
    pub expression: Expression,
    pub alias: Option<String>,
}

impl ProjectionVar {
    pub fn variable(name: impl Into<String>) -> Self {
        Self {
            expression: Expression::Variable(name.into()),
            alias: None,
        }
    }

    pub fn expr_as(expr: Expression, alias: impl Into<String>) -> Self {
        Self {
            expression: expr,
            alias: Some(alias.into()),
        }
    }
}

/// CONSTRUCT query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructQuery {
    /// Template for constructing triples
    pub template: Vec<TriplePattern>,
    /// Dataset clauses
    pub dataset: Vec<DatasetClause>,
    /// WHERE clause
    pub where_clause: GraphPattern,
    /// Solution modifiers
    pub modifier: SolutionModifier,
}

impl Default for ConstructQuery {
    fn default() -> Self {
        Self {
            template: Vec::new(),
            dataset: Vec::new(),
            where_clause: GraphPattern::Empty,
            modifier: SolutionModifier::default(),
        }
    }
}

/// ASK query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskQuery {
    /// Dataset clauses
    pub dataset: Vec<DatasetClause>,
    /// WHERE clause
    pub where_clause: GraphPattern,
}

impl Default for AskQuery {
    fn default() -> Self {
        Self {
            dataset: Vec::new(),
            where_clause: GraphPattern::Empty,
        }
    }
}

/// DESCRIBE query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeQuery {
    /// Resources to describe
    pub resources: Vec<VarOrIri>,
    /// Dataset clauses
    pub dataset: Vec<DatasetClause>,
    /// Optional WHERE clause
    pub where_clause: Option<GraphPattern>,
}

impl Default for DescribeQuery {
    fn default() -> Self {
        Self {
            resources: Vec::new(),
            dataset: Vec::new(),
            where_clause: None,
        }
    }
}

/// Dataset clause (FROM / FROM NAMED)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetClause {
    pub iri: Iri,
    pub named: bool,
}

/// VALUES clause for inline data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuesClause {
    pub variables: Vec<String>,
    pub bindings: Vec<Vec<Option<RdfTerm>>>,
}

/// Graph pattern - the WHERE clause body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphPattern {
    /// Empty pattern
    Empty,
    /// Basic Graph Pattern - set of triple patterns
    Bgp(Vec<TriplePattern>),
    /// Join of patterns (implicit AND)
    Join(Box<GraphPattern>, Box<GraphPattern>),
    /// Left outer join (OPTIONAL)
    LeftJoin(Box<GraphPattern>, Box<GraphPattern>, Option<Expression>),
    /// Union of patterns (UNION)
    Union(Box<GraphPattern>, Box<GraphPattern>),
    /// Filter (FILTER)
    Filter(Box<GraphPattern>, Expression),
    /// Named graph (GRAPH)
    Graph(VarOrIri, Box<GraphPattern>),
    /// Service (FEDERATED query)
    Service(Iri, Box<GraphPattern>, bool),
    /// MINUS pattern
    Minus(Box<GraphPattern>, Box<GraphPattern>),
    /// EXISTS or NOT EXISTS
    Exists(Box<GraphPattern>, bool),
    /// BIND assignment
    Bind(Expression, String, Box<GraphPattern>),
    /// GROUP BY aggregation
    Group(
        Box<GraphPattern>,
        Vec<GroupCondition>,
        Vec<(Aggregate, String)>,
    ),
    /// Subquery
    SubSelect(Box<SelectQuery>),
    /// VALUES inline data
    Values(ValuesClause),
}

/// Triple pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriplePattern {
    pub subject: TermOrVariable,
    pub predicate: PropertyPath,
    pub object: TermOrVariable,
}

impl TriplePattern {
    pub fn new(subject: TermOrVariable, predicate: PropertyPath, object: TermOrVariable) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }

    /// Simple triple pattern with IRI predicate
    pub fn simple(subject: TermOrVariable, predicate: Iri, object: TermOrVariable) -> Self {
        Self {
            subject,
            predicate: PropertyPath::Iri(predicate),
            object,
        }
    }
}

/// Term or variable in triple pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TermOrVariable {
    Term(RdfTerm),
    Variable(String),
    BlankNode(String),
}

impl TermOrVariable {
    pub fn var(name: impl Into<String>) -> Self {
        Self::Variable(name.into())
    }

    pub fn iri(iri: Iri) -> Self {
        Self::Term(RdfTerm::Iri(iri))
    }

    pub fn literal(value: impl Into<String>) -> Self {
        Self::Term(RdfTerm::Literal(Literal::simple(value)))
    }

    pub fn blank(id: impl Into<String>) -> Self {
        Self::BlankNode(id.into())
    }
}

/// Variable or IRI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VarOrIri {
    Variable(String),
    Iri(Iri),
}

/// Property path expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyPath {
    /// Simple IRI predicate
    Iri(Iri),
    /// Variable predicate
    Variable(String),
    /// Inverse path (^path)
    Inverse(Box<PropertyPath>),
    /// Sequence path (path1/path2)
    Sequence(Box<PropertyPath>, Box<PropertyPath>),
    /// Alternative path (path1|path2)
    Alternative(Box<PropertyPath>, Box<PropertyPath>),
    /// Zero or more (*path)
    ZeroOrMore(Box<PropertyPath>),
    /// One or more (+path)
    OneOrMore(Box<PropertyPath>),
    /// Zero or one (?path)
    ZeroOrOne(Box<PropertyPath>),
    /// Negated property set (!(path1|path2))
    NegatedPropertySet(Vec<Iri>),
    /// Fixed length path {n}
    FixedLength(Box<PropertyPath>, usize),
    /// Range length path {n,m}
    RangeLength(Box<PropertyPath>, usize, Option<usize>),
}

/// RDF term
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RdfTerm {
    /// IRI reference
    Iri(Iri),
    /// Literal value
    Literal(Literal),
    /// Blank node
    BlankNode(String),
}

impl RdfTerm {
    pub fn iri(value: impl Into<String>) -> Self {
        Self::Iri(Iri::new(value))
    }

    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(Literal::simple(value))
    }

    pub fn typed_literal(value: impl Into<String>, datatype: Iri) -> Self {
        Self::Literal(Literal::typed(value, datatype))
    }

    pub fn lang_literal(value: impl Into<String>, lang: impl Into<String>) -> Self {
        Self::Literal(Literal::language(value, lang))
    }

    pub fn blank(id: impl Into<String>) -> Self {
        Self::BlankNode(id.into())
    }

    /// Check if this is an IRI
    pub fn is_iri(&self) -> bool {
        matches!(self, Self::Iri(_))
    }

    /// Check if this is a literal
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(_))
    }

    /// Check if this is a blank node
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Self::BlankNode(_))
    }
}

/// IRI (Internationalized Resource Identifier)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Iri(pub String);

impl Iri {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Common RDF namespace IRIs
    pub fn rdf_type() -> Self {
        Self::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    }

    pub fn rdfs_label() -> Self {
        Self::new("http://www.w3.org/2000/01/rdf-schema#label")
    }

    pub fn rdfs_comment() -> Self {
        Self::new("http://www.w3.org/2000/01/rdf-schema#comment")
    }

    pub fn xsd_string() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#string")
    }

    pub fn xsd_integer() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#integer")
    }

    pub fn xsd_decimal() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#decimal")
    }

    pub fn xsd_double() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#double")
    }

    pub fn xsd_boolean() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#boolean")
    }

    pub fn xsd_date() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#date")
    }

    pub fn xsd_datetime() -> Self {
        Self::new("http://www.w3.org/2001/XMLSchema#dateTime")
    }
}

/// RDF Literal
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Literal {
    /// Lexical form (string value)
    pub value: String,
    /// Optional language tag
    pub language: Option<String>,
    /// Datatype IRI (defaults to xsd:string)
    pub datatype: Iri,
}

impl Literal {
    /// Simple string literal
    pub fn simple(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            language: None,
            datatype: Iri::xsd_string(),
        }
    }

    /// Typed literal
    pub fn typed(value: impl Into<String>, datatype: Iri) -> Self {
        Self {
            value: value.into(),
            language: None,
            datatype,
        }
    }

    /// Language-tagged literal
    pub fn language(value: impl Into<String>, lang: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            language: Some(lang.into()),
            datatype: Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString"),
        }
    }

    /// Integer literal
    pub fn integer(value: i64) -> Self {
        Self::typed(value.to_string(), Iri::xsd_integer())
    }

    /// Decimal literal
    pub fn decimal(value: f64) -> Self {
        Self::typed(value.to_string(), Iri::xsd_decimal())
    }

    /// Double literal
    pub fn double(value: f64) -> Self {
        Self::typed(value.to_string(), Iri::xsd_double())
    }

    /// Boolean literal
    pub fn boolean(value: bool) -> Self {
        Self::typed(if value { "true" } else { "false" }, Iri::xsd_boolean())
    }

    /// Try to parse as integer
    pub fn as_integer(&self) -> Option<i64> {
        self.value.parse().ok()
    }

    /// Try to parse as double
    pub fn as_double(&self) -> Option<f64> {
        self.value.parse().ok()
    }

    /// Try to parse as boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self.value.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }
}

/// SPARQL expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    /// Variable reference
    Variable(String),
    /// Constant term
    Term(RdfTerm),
    /// Binary operation
    Binary(Box<Expression>, BinaryOp, Box<Expression>),
    /// Unary operation
    Unary(UnaryOp, Box<Expression>),
    /// Function call
    Function(FunctionCall),
    /// Aggregate function
    Aggregate(Aggregate),
    /// IN expression
    In(Box<Expression>, Vec<Expression>),
    /// NOT IN expression
    NotIn(Box<Expression>, Vec<Expression>),
    /// EXISTS subquery
    Exists(Box<GraphPattern>),
    /// NOT EXISTS subquery
    NotExists(Box<GraphPattern>),
    /// Conditional (IF)
    If(Box<Expression>, Box<Expression>, Box<Expression>),
    /// COALESCE
    Coalesce(Vec<Expression>),
    /// BOUND test
    Bound(String),
    /// isIRI test
    IsIri(Box<Expression>),
    /// isBlank test
    IsBlank(Box<Expression>),
    /// isLiteral test
    IsLiteral(Box<Expression>),
    /// isNumeric test
    IsNumeric(Box<Expression>),
    /// REGEX pattern matching
    Regex(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    /// LANG function
    Lang(Box<Expression>),
    /// DATATYPE function
    Datatype(Box<Expression>),
    /// STR function
    Str(Box<Expression>),
    /// IRI constructor
    Iri(Box<Expression>),
}

impl Expression {
    pub fn var(name: impl Into<String>) -> Self {
        Self::Variable(name.into())
    }

    pub fn term(t: RdfTerm) -> Self {
        Self::Term(t)
    }

    pub fn literal(value: impl Into<String>) -> Self {
        Self::Term(RdfTerm::literal(value))
    }

    pub fn integer(value: i64) -> Self {
        Self::Term(RdfTerm::Literal(Literal::integer(value)))
    }

    pub fn binary(left: Expression, op: BinaryOp, right: Expression) -> Self {
        Self::Binary(Box::new(left), op, Box::new(right))
    }

    pub fn unary(op: UnaryOp, expr: Expression) -> Self {
        Self::Unary(op, Box::new(expr))
    }

    pub fn and(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::And, right)
    }

    pub fn or(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::Or, right)
    }

    pub fn eq(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::Eq, right)
    }

    pub fn neq(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::NotEq, right)
    }

    pub fn lt(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::Lt, right)
    }

    pub fn gt(left: Expression, right: Expression) -> Self {
        Self::binary(left, BinaryOp::Gt, right)
    }
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Logical
    And,
    Or,
    // Comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    // String
    SameTerm,
    LangMatches,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Plus,
    Minus,
}

/// Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expression>,
}

impl FunctionCall {
    pub fn new(name: impl Into<String>, args: Vec<Expression>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }
}

/// Aggregate function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aggregate {
    Count {
        expr: Option<Box<Expression>>,
        distinct: bool,
    },
    Sum {
        expr: Box<Expression>,
        distinct: bool,
    },
    Avg {
        expr: Box<Expression>,
        distinct: bool,
    },
    Min {
        expr: Box<Expression>,
    },
    Max {
        expr: Box<Expression>,
    },
    GroupConcat {
        expr: Box<Expression>,
        separator: Option<String>,
        distinct: bool,
    },
    Sample {
        expr: Box<Expression>,
    },
}

/// Filter expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub expression: Expression,
}

impl Filter {
    pub fn new(expression: Expression) -> Self {
        Self { expression }
    }
}

/// Solution modifier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolutionModifier {
    pub order_by: Vec<OrderCondition>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub having: Option<Expression>,
}

impl SolutionModifier {
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn with_order(mut self, conditions: Vec<OrderCondition>) -> Self {
        self.order_by = conditions;
        self
    }

    pub fn with_having(mut self, expr: Expression) -> Self {
        self.having = Some(expr);
        self
    }
}

/// ORDER BY condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCondition {
    pub expression: Expression,
    pub ascending: bool,
}

impl OrderCondition {
    pub fn asc(expr: Expression) -> Self {
        Self {
            expression: expr,
            ascending: true,
        }
    }

    pub fn desc(expr: Expression) -> Self {
        Self {
            expression: expr,
            ascending: false,
        }
    }
}

/// GROUP BY condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GroupCondition {
    Variable(String),
    Expression(Expression, Option<String>),
}

// ============================================================================
// SPARQL Update Operations
// ============================================================================

/// SPARQL Update operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateOperation {
    /// INSERT DATA { triples }
    InsertData(InsertData),
    /// DELETE DATA { triples }
    DeleteData(DeleteData),
    /// DELETE { pattern } INSERT { pattern } WHERE { pattern }
    Modify(Modify),
    /// LOAD <iri> INTO GRAPH <iri>
    Load {
        source: Iri,
        destination: Option<Iri>,
        silent: bool,
    },
    /// CLEAR GRAPH <iri>
    Clear { target: GraphTarget, silent: bool },
    /// CREATE GRAPH <iri>
    Create { graph: Iri, silent: bool },
    /// DROP GRAPH <iri>
    Drop { target: GraphTarget, silent: bool },
    /// COPY source TO destination
    Copy {
        source: GraphTarget,
        destination: GraphTarget,
        silent: bool,
    },
    /// MOVE source TO destination
    Move {
        source: GraphTarget,
        destination: GraphTarget,
        silent: bool,
    },
    /// ADD source TO destination
    Add {
        source: GraphTarget,
        destination: GraphTarget,
        silent: bool,
    },
}

/// INSERT DATA operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertData {
    pub quads: Vec<Quad>,
}

/// DELETE DATA operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteData {
    pub quads: Vec<Quad>,
}

/// DELETE/INSERT with WHERE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modify {
    pub with_graph: Option<Iri>,
    pub delete_pattern: Option<Vec<QuadPattern>>,
    pub insert_pattern: Option<Vec<QuadPattern>>,
    pub using: Vec<DatasetClause>,
    pub where_pattern: GraphPattern,
}

/// Quad (triple with optional graph)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quad {
    pub subject: RdfTerm,
    pub predicate: Iri,
    pub object: RdfTerm,
    pub graph: Option<Iri>,
}

/// Quad pattern (for DELETE/INSERT templates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadPattern {
    pub subject: TermOrVariable,
    pub predicate: VarOrIri,
    pub object: TermOrVariable,
    pub graph: Option<VarOrIri>,
}

/// Graph target for management operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphTarget {
    Default,
    Named(Iri),
    All,
    AllNamed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdf_term_creation() {
        let iri = RdfTerm::iri("http://example.org/resource");
        assert!(iri.is_iri());

        let lit = RdfTerm::literal("hello");
        assert!(lit.is_literal());

        let blank = RdfTerm::blank("b0");
        assert!(blank.is_blank_node());
    }

    #[test]
    fn test_literal_parsing() {
        let int_lit = Literal::integer(42);
        assert_eq!(int_lit.as_integer(), Some(42));

        let double_lit = Literal::double(3.14);
        assert!((double_lit.as_double().unwrap() - 3.14).abs() < 0.001);

        let bool_lit = Literal::boolean(true);
        assert_eq!(bool_lit.as_boolean(), Some(true));
    }

    #[test]
    fn test_expression_builder() {
        let expr = Expression::and(
            Expression::eq(Expression::var("x"), Expression::integer(10)),
            Expression::gt(Expression::var("y"), Expression::integer(5)),
        );

        match expr {
            Expression::Binary(_, BinaryOp::And, _) => (),
            _ => panic!("Expected AND expression"),
        }
    }

    #[test]
    fn test_triple_pattern() {
        let pattern = TriplePattern::simple(
            TermOrVariable::var("s"),
            Iri::rdf_type(),
            TermOrVariable::iri(Iri::new("http://example.org/Person")),
        );

        assert!(matches!(pattern.subject, TermOrVariable::Variable(_)));
        assert!(matches!(pattern.predicate, PropertyPath::Iri(_)));
    }
}
