// SPARQL Query Executor for WASM
//
// Executes parsed SPARQL queries against an in-memory triple store.
// Simplified version for WASM environments (no async, no complex aggregates).

use super::ast::*;
use super::triple_store::{Triple, TripleStore};
use super::{SparqlError, SparqlResult};
use std::collections::HashMap;

/// Static empty HashMap for default prefixes
static EMPTY_PREFIXES: once_cell::sync::Lazy<HashMap<String, Iri>> =
    once_cell::sync::Lazy::new(HashMap::new);

/// Solution binding - maps variables to RDF terms
pub type Binding = HashMap<String, RdfTerm>;

/// Solution sequence - list of bindings
pub type Solutions = Vec<Binding>;

/// Execution context for SPARQL queries
pub struct SparqlContext<'a> {
    pub store: &'a TripleStore,
    pub base: Option<&'a Iri>,
    pub prefixes: &'a HashMap<String, Iri>,
}

impl<'a> SparqlContext<'a> {
    pub fn new(store: &'a TripleStore) -> Self {
        Self {
            store,
            base: None,
            prefixes: &EMPTY_PREFIXES,
        }
    }

    pub fn with_base(mut self, base: Option<&'a Iri>) -> Self {
        self.base = base;
        self
    }

    pub fn with_prefixes(mut self, prefixes: &'a HashMap<String, Iri>) -> Self {
        self.prefixes = prefixes;
        self
    }
}

/// Execute a SPARQL query
pub fn execute_sparql(store: &TripleStore, query: &SparqlQuery) -> SparqlResult<QueryResult> {
    let mut ctx = SparqlContext::new(store)
        .with_base(query.base.as_ref())
        .with_prefixes(&query.prefixes);

    match &query.body {
        QueryBody::Select(select) => {
            let solutions = execute_select(&mut ctx, select)?;
            Ok(QueryResult::Select(solutions))
        }
        QueryBody::Construct(construct) => {
            let triples = execute_construct(&mut ctx, construct)?;
            Ok(QueryResult::Construct(triples))
        }
        QueryBody::Ask(ask) => {
            let result = execute_ask(&mut ctx, ask)?;
            Ok(QueryResult::Ask(result))
        }
        QueryBody::Describe(describe) => {
            let triples = execute_describe(&mut ctx, describe)?;
            Ok(QueryResult::Describe(triples))
        }
        QueryBody::Update(ops) => {
            for op in ops {
                execute_update(&mut ctx, op)?;
            }
            Ok(QueryResult::Update)
        }
    }
}

/// Query result types
#[derive(Debug, Clone)]
pub enum QueryResult {
    Select(SelectResult),
    Construct(Vec<Triple>),
    Ask(bool),
    Describe(Vec<Triple>),
    Update,
}

/// SELECT query result
#[derive(Debug, Clone)]
pub struct SelectResult {
    pub variables: Vec<String>,
    pub bindings: Solutions,
}

impl SelectResult {
    pub fn new(variables: Vec<String>, bindings: Solutions) -> Self {
        Self {
            variables,
            bindings,
        }
    }
}

// ============================================================================
// SELECT Query Execution
// ============================================================================

fn execute_select(ctx: &mut SparqlContext, query: &SelectQuery) -> SparqlResult<SelectResult> {
    // Evaluate WHERE clause
    let mut solutions = evaluate_graph_pattern(ctx, &query.where_clause)?;

    // Apply solution modifiers
    solutions = apply_modifiers(solutions, &query.modifier)?;

    // Project variables
    let (variables, bindings) = project_solutions(&query.projection, solutions)?;

    Ok(SelectResult {
        variables,
        bindings,
    })
}

fn project_solutions(
    projection: &Projection,
    solutions: Solutions,
) -> SparqlResult<(Vec<String>, Solutions)> {
    match projection {
        Projection::All => {
            // Get all unique variables
            let mut vars: Vec<String> = Vec::new();
            for binding in &solutions {
                for var in binding.keys() {
                    if !vars.contains(var) {
                        vars.push(var.clone());
                    }
                }
            }
            vars.sort();
            Ok((vars, solutions))
        }
        Projection::Variables(vars) | Projection::Distinct(vars) | Projection::Reduced(vars) => {
            let var_names: Vec<String> = vars
                .iter()
                .map(|v| {
                    v.alias.clone().unwrap_or_else(|| {
                        if let Expression::Variable(name) = &v.expression {
                            name.clone()
                        } else {
                            "_expr".to_string()
                        }
                    })
                })
                .collect();

            let mut projected: Solutions = Vec::new();

            for binding in solutions {
                let mut new_binding = Binding::new();

                for (i, pv) in vars.iter().enumerate() {
                    if let Some(value) = evaluate_expression(&pv.expression, &binding)? {
                        new_binding.insert(var_names[i].clone(), value);
                    }
                }

                // For DISTINCT, check if this binding already exists
                if matches!(projection, Projection::Distinct(_)) {
                    if !projected.iter().any(|b| bindings_equal(b, &new_binding)) {
                        projected.push(new_binding);
                    }
                } else {
                    projected.push(new_binding);
                }
            }

            Ok((var_names, projected))
        }
    }
}

fn bindings_equal(a: &Binding, b: &Binding) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().all(|(k, v)| b.get(k) == Some(v))
}

// ============================================================================
// Graph Pattern Evaluation
// ============================================================================

fn evaluate_graph_pattern(ctx: &SparqlContext, pattern: &GraphPattern) -> SparqlResult<Solutions> {
    match pattern {
        GraphPattern::Empty => Ok(vec![Binding::new()]),

        GraphPattern::Bgp(triples) => evaluate_bgp(ctx, triples),

        GraphPattern::Join(left, right) => {
            let left_solutions = evaluate_graph_pattern(ctx, left)?;
            let right_solutions = evaluate_graph_pattern(ctx, right)?;
            join_solutions(left_solutions, right_solutions)
        }

        GraphPattern::LeftJoin(left, right, condition) => {
            let left_solutions = evaluate_graph_pattern(ctx, left)?;
            let right_solutions = evaluate_graph_pattern(ctx, right)?;
            left_join_solutions(left_solutions, right_solutions, condition.as_ref())
        }

        GraphPattern::Union(left, right) => {
            let mut left_solutions = evaluate_graph_pattern(ctx, left)?;
            let right_solutions = evaluate_graph_pattern(ctx, right)?;
            left_solutions.extend(right_solutions);
            Ok(left_solutions)
        }

        GraphPattern::Filter(inner, condition) => {
            let solutions = evaluate_graph_pattern(ctx, inner)?;
            filter_solutions(solutions, condition)
        }

        GraphPattern::Minus(left, right) => {
            let left_solutions = evaluate_graph_pattern(ctx, left)?;
            let right_solutions = evaluate_graph_pattern(ctx, right)?;
            minus_solutions(left_solutions, right_solutions)
        }

        GraphPattern::Bind(expr, var, inner) => {
            let mut solutions = evaluate_graph_pattern(ctx, inner)?;
            for binding in &mut solutions {
                if let Some(value) = evaluate_expression(expr, binding)? {
                    binding.insert(var.clone(), value);
                }
            }
            Ok(solutions)
        }

        GraphPattern::Values(values) => {
            let mut solutions = Vec::new();
            for row in &values.bindings {
                let mut binding = Binding::new();
                for (i, var) in values.variables.iter().enumerate() {
                    if let Some(Some(term)) = row.get(i) {
                        binding.insert(var.clone(), term.clone());
                    }
                }
                solutions.push(binding);
            }
            Ok(solutions)
        }

        _ => Err(SparqlError::UnsupportedOperation(format!(
            "Graph pattern not supported in WASM build: {:?}",
            pattern
        ))),
    }
}

fn evaluate_bgp(ctx: &SparqlContext, patterns: &[TriplePattern]) -> SparqlResult<Solutions> {
    let mut solutions = vec![Binding::new()];

    for pattern in patterns {
        let mut new_solutions = Vec::new();

        for binding in &solutions {
            let matches = match_triple_pattern(ctx, pattern, binding)?;
            new_solutions.extend(matches);
        }

        solutions = new_solutions;

        if solutions.is_empty() {
            break;
        }
    }

    Ok(solutions)
}

fn match_triple_pattern(
    ctx: &SparqlContext,
    pattern: &TriplePattern,
    binding: &Binding,
) -> SparqlResult<Solutions> {
    // Resolve pattern components
    let subject = resolve_term_or_var(&pattern.subject, binding);
    let object = resolve_term_or_var(&pattern.object, binding);

    // Handle simple IRI predicate (most common case)
    if let PropertyPath::Iri(iri) = &pattern.predicate {
        return match_simple_triple(
            ctx,
            subject,
            Some(iri),
            object,
            &pattern.subject,
            &pattern.object,
            binding,
        );
    }

    // For now, only support simple IRI predicates in WASM
    Err(SparqlError::PropertyPathError(
        "Complex property paths not yet supported in WASM build".to_string(),
    ))
}

fn resolve_term_or_var(tov: &TermOrVariable, binding: &Binding) -> Option<RdfTerm> {
    match tov {
        TermOrVariable::Term(t) => Some(t.clone()),
        TermOrVariable::Variable(v) => binding.get(v).cloned(),
        TermOrVariable::BlankNode(id) => Some(RdfTerm::BlankNode(id.clone())),
    }
}

fn match_simple_triple(
    ctx: &SparqlContext,
    subject: Option<RdfTerm>,
    predicate: Option<&Iri>,
    object: Option<RdfTerm>,
    subj_pattern: &TermOrVariable,
    obj_pattern: &TermOrVariable,
    binding: &Binding,
) -> SparqlResult<Solutions> {
    let triples = ctx
        .store
        .query(subject.as_ref(), predicate, object.as_ref());

    let mut solutions = Vec::new();

    for triple in triples {
        let mut new_binding = binding.clone();
        let mut matches = true;

        // Bind subject variable
        if let TermOrVariable::Variable(var) = subj_pattern {
            if let Some(existing) = new_binding.get(var) {
                if existing != &triple.subject {
                    matches = false;
                }
            } else {
                new_binding.insert(var.clone(), triple.subject.clone());
            }
        }

        // Bind object variable
        if matches {
            if let TermOrVariable::Variable(var) = obj_pattern {
                if let Some(existing) = new_binding.get(var) {
                    if existing != &triple.object {
                        matches = false;
                    }
                } else {
                    new_binding.insert(var.clone(), triple.object.clone());
                }
            }
        }

        if matches {
            solutions.push(new_binding);
        }
    }

    Ok(solutions)
}

// ============================================================================
// Solution Operations
// ============================================================================

fn join_solutions(left: Solutions, right: Solutions) -> SparqlResult<Solutions> {
    if left.is_empty() || right.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();

    for l in &left {
        for r in &right {
            if let Some(merged) = merge_bindings(l, r) {
                result.push(merged);
            }
        }
    }

    Ok(result)
}

fn left_join_solutions(
    left: Solutions,
    right: Solutions,
    condition: Option<&Expression>,
) -> SparqlResult<Solutions> {
    let mut result = Vec::new();

    for l in &left {
        let mut found_match = false;

        for r in &right {
            if let Some(merged) = merge_bindings(l, r) {
                // Check condition if present
                let include = if let Some(cond) = condition {
                    evaluate_expression_as_bool(cond, &merged)?
                } else {
                    true
                };

                if include {
                    result.push(merged);
                    found_match = true;
                }
            }
        }

        if !found_match {
            result.push(l.clone());
        }
    }

    Ok(result)
}

fn minus_solutions(left: Solutions, right: Solutions) -> SparqlResult<Solutions> {
    let mut result = Vec::new();

    for l in &left {
        let mut has_compatible = false;

        for r in &right {
            if bindings_compatible(l, r) {
                has_compatible = true;
                break;
            }
        }

        if !has_compatible {
            result.push(l.clone());
        }
    }

    Ok(result)
}

fn merge_bindings(a: &Binding, b: &Binding) -> Option<Binding> {
    let mut result = a.clone();

    for (k, v) in b {
        if let Some(existing) = result.get(k) {
            if existing != v {
                return None;
            }
        } else {
            result.insert(k.clone(), v.clone());
        }
    }

    Some(result)
}

fn bindings_compatible(a: &Binding, b: &Binding) -> bool {
    for (k, v) in a {
        if let Some(bv) = b.get(k) {
            if v != bv {
                return false;
            }
        }
    }
    true
}

fn filter_solutions(solutions: Solutions, condition: &Expression) -> SparqlResult<Solutions> {
    let mut result = Vec::new();

    for binding in solutions {
        if evaluate_expression_as_bool(condition, &binding)? {
            result.push(binding);
        }
    }

    Ok(result)
}

// ============================================================================
// Solution Modifiers
// ============================================================================

fn apply_modifiers(
    mut solutions: Solutions,
    modifier: &SolutionModifier,
) -> SparqlResult<Solutions> {
    // ORDER BY
    if !modifier.order_by.is_empty() {
        solutions.sort_by(|a, b| {
            for cond in &modifier.order_by {
                let va = evaluate_expression(&cond.expression, a).ok().flatten();
                let vb = evaluate_expression(&cond.expression, b).ok().flatten();

                let ord = match (va, vb) {
                    (Some(ta), Some(tb)) => compare_terms(&ta, &tb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };

                let ord = if cond.ascending { ord } else { ord.reverse() };

                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    // OFFSET
    if let Some(offset) = modifier.offset {
        if offset < solutions.len() {
            solutions = solutions.into_iter().skip(offset).collect();
        } else {
            solutions.clear();
        }
    }

    // LIMIT
    if let Some(limit) = modifier.limit {
        solutions.truncate(limit);
    }

    Ok(solutions)
}

fn compare_terms(a: &RdfTerm, b: &RdfTerm) -> std::cmp::Ordering {
    match (a, b) {
        (RdfTerm::Literal(la), RdfTerm::Literal(lb)) => {
            if let (Some(na), Some(nb)) = (la.as_double(), lb.as_double()) {
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                la.value.cmp(&lb.value)
            }
        }
        (RdfTerm::Iri(ia), RdfTerm::Iri(ib)) => ia.as_str().cmp(ib.as_str()),
        _ => std::cmp::Ordering::Equal,
    }
}

// ============================================================================
// Expression Evaluation
// ============================================================================

fn evaluate_expression(expr: &Expression, binding: &Binding) -> SparqlResult<Option<RdfTerm>> {
    match expr {
        Expression::Variable(var) => Ok(binding.get(var).cloned()),

        Expression::Term(term) => Ok(Some(term.clone())),

        Expression::Binary(left, op, right) => {
            let lv = evaluate_expression(left, binding)?;
            let rv = evaluate_expression(right, binding)?;
            evaluate_binary_op(lv, *op, rv)
        }

        Expression::Unary(op, inner) => {
            let v = evaluate_expression(inner, binding)?;
            evaluate_unary_op(*op, v)
        }

        Expression::Bound(var) => Ok(Some(RdfTerm::Literal(Literal::boolean(
            binding.contains_key(var),
        )))),

        Expression::If(cond, then_expr, else_expr) => {
            if evaluate_expression_as_bool(cond, binding)? {
                evaluate_expression(then_expr, binding)
            } else {
                evaluate_expression(else_expr, binding)
            }
        }

        Expression::Coalesce(exprs) => {
            for e in exprs {
                if let Some(v) = evaluate_expression(e, binding)? {
                    return Ok(Some(v));
                }
            }
            Ok(None)
        }

        Expression::IsIri(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(Some(RdfTerm::Literal(Literal::boolean(
                v.map(|t| t.is_iri()).unwrap_or(false),
            ))))
        }

        Expression::IsBlank(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(Some(RdfTerm::Literal(Literal::boolean(
                v.map(|t| t.is_blank_node()).unwrap_or(false),
            ))))
        }

        Expression::IsLiteral(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(Some(RdfTerm::Literal(Literal::boolean(
                v.map(|t| t.is_literal()).unwrap_or(false),
            ))))
        }

        Expression::Str(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(v.map(|t| RdfTerm::literal(term_to_string(&t))))
        }

        Expression::Lang(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(v.and_then(|t| {
                if let RdfTerm::Literal(lit) = t {
                    Some(RdfTerm::literal(lit.language.unwrap_or_default()))
                } else {
                    None
                }
            }))
        }

        Expression::Datatype(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(v.and_then(|t| {
                if let RdfTerm::Literal(lit) = t {
                    Some(RdfTerm::Iri(lit.datatype))
                } else {
                    None
                }
            }))
        }

        _ => Err(SparqlError::UnsupportedOperation(
            "Complex expressions not yet supported in WASM build".to_string(),
        )),
    }
}

fn evaluate_expression_as_bool(expr: &Expression, binding: &Binding) -> SparqlResult<bool> {
    let value = evaluate_expression(expr, binding)?;

    Ok(match value {
        None => false,
        Some(RdfTerm::Literal(lit)) => {
            if let Some(b) = lit.as_boolean() {
                b
            } else if let Some(n) = lit.as_double() {
                n != 0.0
            } else {
                !lit.value.is_empty()
            }
        }
        Some(_) => true,
    })
}

fn evaluate_binary_op(
    left: Option<RdfTerm>,
    op: BinaryOp,
    right: Option<RdfTerm>,
) -> SparqlResult<Option<RdfTerm>> {
    match op {
        BinaryOp::And => {
            let lb = left.map(|t| term_to_bool(&t)).unwrap_or(false);
            let rb = right.map(|t| term_to_bool(&t)).unwrap_or(false);
            Ok(Some(RdfTerm::Literal(Literal::boolean(lb && rb))))
        }

        BinaryOp::Or => {
            let lb = left.map(|t| term_to_bool(&t)).unwrap_or(false);
            let rb = right.map(|t| term_to_bool(&t)).unwrap_or(false);
            Ok(Some(RdfTerm::Literal(Literal::boolean(lb || rb))))
        }

        BinaryOp::Eq => Ok(Some(RdfTerm::Literal(Literal::boolean(left == right)))),

        BinaryOp::NotEq => Ok(Some(RdfTerm::Literal(Literal::boolean(left != right)))),

        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
            let cmp = match (&left, &right) {
                (Some(l), Some(r)) => compare_terms(l, r),
                _ => return Ok(None),
            };

            let result = match op {
                BinaryOp::Lt => cmp == std::cmp::Ordering::Less,
                BinaryOp::LtEq => cmp != std::cmp::Ordering::Greater,
                BinaryOp::Gt => cmp == std::cmp::Ordering::Greater,
                BinaryOp::GtEq => cmp != std::cmp::Ordering::Less,
                _ => unreachable!(),
            };

            Ok(Some(RdfTerm::Literal(Literal::boolean(result))))
        }

        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
            let ln = left.and_then(|t| term_to_number(&t));
            let rn = right.and_then(|t| term_to_number(&t));

            match (ln, rn) {
                (Some(l), Some(r)) => {
                    let result = match op {
                        BinaryOp::Add => l + r,
                        BinaryOp::Sub => l - r,
                        BinaryOp::Mul => l * r,
                        BinaryOp::Div => {
                            if r == 0.0 {
                                return Ok(None);
                            }
                            l / r
                        }
                        _ => unreachable!(),
                    };
                    Ok(Some(RdfTerm::Literal(Literal::decimal(result))))
                }
                _ => Ok(None),
            }
        }

        _ => Err(SparqlError::UnsupportedOperation(format!(
            "Binary operator not supported: {:?}",
            op
        ))),
    }
}

fn evaluate_unary_op(op: UnaryOp, value: Option<RdfTerm>) -> SparqlResult<Option<RdfTerm>> {
    match op {
        UnaryOp::Not => {
            let b = value.map(|t| term_to_bool(&t)).unwrap_or(false);
            Ok(Some(RdfTerm::Literal(Literal::boolean(!b))))
        }

        UnaryOp::Plus => Ok(value),

        UnaryOp::Minus => {
            let n = value.and_then(|t| term_to_number(&t));
            Ok(n.map(|v| RdfTerm::Literal(Literal::decimal(-v))))
        }
    }
}

fn term_to_string(term: &RdfTerm) -> String {
    match term {
        RdfTerm::Iri(iri) => iri.as_str().to_string(),
        RdfTerm::Literal(lit) => lit.value.clone(),
        RdfTerm::BlankNode(id) => format!("_:{}", id),
    }
}

fn term_to_number(term: &RdfTerm) -> Option<f64> {
    match term {
        RdfTerm::Literal(lit) => lit.as_double(),
        _ => None,
    }
}

fn term_to_bool(term: &RdfTerm) -> bool {
    match term {
        RdfTerm::Literal(lit) => {
            if let Some(b) = lit.as_boolean() {
                b
            } else if let Some(n) = lit.as_double() {
                n != 0.0
            } else {
                !lit.value.is_empty()
            }
        }
        _ => true,
    }
}

// ============================================================================
// Other Query Forms
// ============================================================================

fn execute_construct(ctx: &SparqlContext, query: &ConstructQuery) -> SparqlResult<Vec<Triple>> {
    let solutions = evaluate_graph_pattern(ctx, &query.where_clause)?;
    let solutions = apply_modifiers(solutions, &query.modifier)?;

    let mut triples = Vec::new();

    for binding in solutions {
        for pattern in &query.template {
            if let (Some(s), Some(o)) = (
                resolve_term_or_var(&pattern.subject, &binding),
                resolve_term_or_var(&pattern.object, &binding),
            ) {
                if let PropertyPath::Iri(p) = &pattern.predicate {
                    triples.push(Triple::new(s, p.clone(), o));
                }
            }
        }
    }

    Ok(triples)
}

fn execute_ask(ctx: &SparqlContext, query: &AskQuery) -> SparqlResult<bool> {
    let solutions = evaluate_graph_pattern(ctx, &query.where_clause)?;
    Ok(!solutions.is_empty())
}

fn execute_describe(ctx: &SparqlContext, query: &DescribeQuery) -> SparqlResult<Vec<Triple>> {
    let mut resources: Vec<RdfTerm> = Vec::new();

    // Get resources from query
    for r in &query.resources {
        match r {
            VarOrIri::Iri(iri) => resources.push(RdfTerm::Iri(iri.clone())),
            VarOrIri::Variable(var) => {
                if let Some(pattern) = &query.where_clause {
                    let solutions = evaluate_graph_pattern(ctx, pattern)?;
                    for binding in solutions {
                        if let Some(term) = binding.get(var) {
                            if !resources.contains(term) {
                                resources.push(term.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Get all triples about each resource
    let mut triples = Vec::new();
    for resource in resources {
        // Triples where resource is subject
        triples.extend(ctx.store.query(Some(&resource), None, None));
        // Triples where resource is object
        triples.extend(ctx.store.query(None, None, Some(&resource)));
    }

    Ok(triples)
}

// ============================================================================
// Update Operations (Simplified)
// ============================================================================

fn execute_update(_ctx: &SparqlContext, _op: &UpdateOperation) -> SparqlResult<()> {
    // Simplified: Updates not fully implemented in WASM build
    Err(SparqlError::UnsupportedOperation(
        "Update operations not yet supported in WASM build".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparql::parser::parse_sparql;

    fn setup_test_store() -> TripleStore {
        let store = TripleStore::new();

        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::new("http://example.org/name"),
            RdfTerm::literal("Alice"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::new("http://example.org/age"),
            RdfTerm::Literal(Literal::integer(30)),
        ));

        store
    }

    #[test]
    fn test_simple_select() {
        let store = setup_test_store();
        let query = parse_sparql("SELECT ?s ?p ?o WHERE { ?s ?p ?o }").unwrap();
        let result = execute_sparql(&store, &query).unwrap();

        if let QueryResult::Select(select) = result {
            assert!(!select.bindings.is_empty());
        } else {
            panic!("Expected SELECT result");
        }
    }

    #[test]
    fn test_select_with_filter() {
        let store = setup_test_store();
        let query = parse_sparql(
            r#"
            SELECT ?name WHERE {
                ?s <http://example.org/name> ?name .
                FILTER(?name = "Alice")
            }
        "#,
        )
        .unwrap();
        let result = execute_sparql(&store, &query).unwrap();

        if let QueryResult::Select(select) = result {
            assert_eq!(select.bindings.len(), 1);
        }
    }

    #[test]
    fn test_ask_query() {
        let store = setup_test_store();

        let query = parse_sparql(
            r#"
            ASK { <http://example.org/person/1> <http://example.org/name> "Alice" }
        "#,
        )
        .unwrap();
        let result = execute_sparql(&store, &query).unwrap();

        assert!(matches!(result, QueryResult::Ask(true)));
    }
}
