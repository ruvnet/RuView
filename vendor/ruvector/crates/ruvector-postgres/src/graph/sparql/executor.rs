// SPARQL Query Executor
//
// Executes parsed SPARQL queries against a triple store.

use super::ast::*;
use super::functions::evaluate_function;
use super::triple_store::{Triple, TripleStore};
use super::{SparqlError, SparqlResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Solution binding - maps variables to RDF terms
pub type Binding = HashMap<String, RdfTerm>;

/// Solution sequence - list of bindings
pub type Solutions = Vec<Binding>;

/// Static empty HashMap for default prefixes
static EMPTY_PREFIXES: Lazy<HashMap<String, Iri>> = Lazy::new(HashMap::new);

/// Execution context for SPARQL queries
pub struct SparqlContext<'a> {
    pub store: &'a TripleStore,
    pub default_graph: Option<String>,
    pub named_graphs: Vec<&'a str>,
    pub base: Option<&'a Iri>,
    pub prefixes: &'a HashMap<String, Iri>,
    #[allow(dead_code)]
    blank_node_counter: u64,
}

impl<'a> SparqlContext<'a> {
    pub fn new(store: &'a TripleStore) -> Self {
        Self {
            store,
            default_graph: None,
            named_graphs: Vec::new(),
            base: None,
            prefixes: &EMPTY_PREFIXES,
            blank_node_counter: 0,
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

    #[allow(dead_code)]
    fn new_blank_node(&mut self) -> String {
        self.blank_node_counter += 1;
        format!("b{}", self.blank_node_counter)
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

    pub fn empty() -> Self {
        Self {
            variables: Vec::new(),
            bindings: Vec::new(),
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

    // Apply VALUES clause
    if let Some(values) = &query.values {
        solutions = join_values(solutions, values)?;
    }

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
                            format!("_expr{}", 0)
                        }
                    })
                })
                .collect();

            let mut projected: Solutions = Vec::new();

            for binding in solutions {
                let mut new_binding = Binding::new();

                for (i, pv) in vars.iter().enumerate() {
                    let value = evaluate_expression(&pv.expression, &binding)?;
                    if let Some(term) = value {
                        new_binding.insert(var_names[i].clone(), term);
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

fn evaluate_graph_pattern(
    ctx: &mut SparqlContext,
    pattern: &GraphPattern,
) -> SparqlResult<Solutions> {
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

        GraphPattern::Graph(graph_name, inner) => {
            // Execute pattern against specific graph
            let graph_iri = match graph_name {
                VarOrIri::Iri(iri) => Some(iri.as_str().to_string()),
                VarOrIri::Variable(_) => None, // Query all named graphs
            };

            if let Some(graph) = graph_iri {
                // Temporarily set the graph context
                let old_default = ctx.default_graph.clone();
                ctx.default_graph = Some(graph);
                let result = evaluate_graph_pattern(ctx, inner);
                ctx.default_graph = old_default;
                result
            } else {
                // Union over all named graphs
                let mut all_solutions = Vec::new();
                for graph in ctx.store.list_graphs() {
                    ctx.default_graph = Some(graph.clone());
                    let solutions = evaluate_graph_pattern(ctx, inner)?;

                    // Add graph variable binding
                    if let VarOrIri::Variable(var) = graph_name {
                        for mut sol in solutions {
                            sol.insert(var.clone(), RdfTerm::Iri(Iri::new(&graph)));
                            all_solutions.push(sol);
                        }
                    } else {
                        all_solutions.extend(solutions);
                    }
                }
                Ok(all_solutions)
            }
        }

        GraphPattern::Minus(left, right) => {
            let left_solutions = evaluate_graph_pattern(ctx, left)?;
            let right_solutions = evaluate_graph_pattern(ctx, right)?;
            minus_solutions(left_solutions, right_solutions)
        }

        GraphPattern::Exists(inner, positive) => {
            let solutions = evaluate_graph_pattern(ctx, inner)?;
            if *positive {
                Ok(if solutions.is_empty() {
                    vec![]
                } else {
                    vec![Binding::new()]
                })
            } else {
                Ok(if solutions.is_empty() {
                    vec![Binding::new()]
                } else {
                    vec![]
                })
            }
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

        GraphPattern::Group(inner, group_by, aggregates) => {
            let solutions = evaluate_graph_pattern(ctx, inner)?;
            evaluate_group(solutions, group_by, aggregates)
        }

        GraphPattern::SubSelect(subquery) => execute_select(ctx, subquery).map(|r| r.bindings),

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

        GraphPattern::Service(_, _, _) => Err(SparqlError::UnsupportedOperation(
            "SERVICE queries not supported".to_string(),
        )),
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

    // Handle property paths
    match &pattern.predicate {
        PropertyPath::Iri(iri) => match_simple_triple(
            ctx,
            subject,
            Some(iri),
            object,
            &pattern.subject,
            &pattern.object,
            binding,
        ),
        PropertyPath::Variable(var) => {
            let pred = binding.get(var).and_then(|t| {
                if let RdfTerm::Iri(iri) = t {
                    Some(iri.clone())
                } else {
                    None
                }
            });
            match_simple_triple_with_var_pred(
                ctx,
                subject,
                pred.as_ref(),
                object,
                &pattern.subject,
                var,
                &pattern.object,
                binding,
            )
        }
        path => evaluate_property_path(
            ctx,
            subject,
            path,
            object,
            &pattern.subject,
            &pattern.object,
            binding,
        ),
    }
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

fn match_simple_triple_with_var_pred(
    ctx: &SparqlContext,
    subject: Option<RdfTerm>,
    predicate: Option<&Iri>,
    object: Option<RdfTerm>,
    subj_pattern: &TermOrVariable,
    pred_var: &str,
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

        // Bind predicate variable
        if matches {
            if let Some(existing) = new_binding.get(pred_var) {
                if let RdfTerm::Iri(existing_iri) = existing {
                    if existing_iri != &triple.predicate {
                        matches = false;
                    }
                } else {
                    matches = false;
                }
            } else {
                new_binding.insert(pred_var.to_string(), RdfTerm::Iri(triple.predicate.clone()));
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

fn evaluate_property_path(
    ctx: &SparqlContext,
    subject: Option<RdfTerm>,
    path: &PropertyPath,
    object: Option<RdfTerm>,
    subj_pattern: &TermOrVariable,
    obj_pattern: &TermOrVariable,
    binding: &Binding,
) -> SparqlResult<Solutions> {
    match path {
        PropertyPath::Iri(iri) => match_simple_triple(
            ctx,
            subject,
            Some(iri),
            object,
            subj_pattern,
            obj_pattern,
            binding,
        ),

        PropertyPath::Inverse(inner) => {
            // Swap subject and object
            evaluate_property_path(
                ctx,
                object,
                inner,
                subject,
                obj_pattern,
                subj_pattern,
                binding,
            )
        }

        PropertyPath::Sequence(first, second) => {
            // ?s path1/path2 ?o means ?s path1 ?mid . ?mid path2 ?o
            let mid_var = format!("_path_mid_{}", binding.len());
            let mid_pattern = TermOrVariable::Variable(mid_var.clone());

            let first_solutions = evaluate_property_path(
                ctx,
                subject,
                first,
                None,
                subj_pattern,
                &mid_pattern,
                binding,
            )?;

            let mut solutions = Vec::new();
            for sol in first_solutions {
                let mid_value = sol.get(&mid_var).cloned();
                let second_solutions = evaluate_property_path(
                    ctx,
                    mid_value,
                    second,
                    object.clone(),
                    &mid_pattern,
                    obj_pattern,
                    &sol,
                )?;
                solutions.extend(second_solutions);
            }

            Ok(solutions)
        }

        PropertyPath::Alternative(left, right) => {
            let mut left_solutions = evaluate_property_path(
                ctx,
                subject.clone(),
                left,
                object.clone(),
                subj_pattern,
                obj_pattern,
                binding,
            )?;
            let right_solutions = evaluate_property_path(
                ctx,
                subject,
                right,
                object,
                subj_pattern,
                obj_pattern,
                binding,
            )?;
            left_solutions.extend(right_solutions);
            Ok(left_solutions)
        }

        PropertyPath::ZeroOrMore(inner) => evaluate_transitive_path(
            ctx,
            subject,
            inner,
            object,
            subj_pattern,
            obj_pattern,
            binding,
            true,
        ),

        PropertyPath::OneOrMore(inner) => evaluate_transitive_path(
            ctx,
            subject,
            inner,
            object,
            subj_pattern,
            obj_pattern,
            binding,
            false,
        ),

        PropertyPath::ZeroOrOne(inner) => {
            let mut solutions = Vec::new();

            // Zero case - subject equals object
            if let (Some(s), Some(o)) = (&subject, &object) {
                if s == o {
                    solutions.push(binding.clone());
                }
            } else if subject.is_some() {
                // Bind object to subject
                let mut sol = binding.clone();
                if let TermOrVariable::Variable(var) = obj_pattern {
                    sol.insert(var.clone(), subject.clone().unwrap());
                }
                solutions.push(sol);
            }

            // One case
            let one_solutions = evaluate_property_path(
                ctx,
                subject,
                inner,
                object,
                subj_pattern,
                obj_pattern,
                binding,
            )?;
            solutions.extend(one_solutions);

            Ok(solutions)
        }

        PropertyPath::NegatedPropertySet(iris) => {
            // Match any predicate NOT in the set
            let all_triples = ctx.store.query(subject.as_ref(), None, object.as_ref());

            let mut solutions = Vec::new();
            for triple in all_triples {
                if !iris.iter().any(|i| i == &triple.predicate) {
                    let mut new_binding = binding.clone();

                    if let TermOrVariable::Variable(var) = subj_pattern {
                        new_binding.insert(var.clone(), triple.subject.clone());
                    }
                    if let TermOrVariable::Variable(var) = obj_pattern {
                        new_binding.insert(var.clone(), triple.object.clone());
                    }

                    solutions.push(new_binding);
                }
            }

            Ok(solutions)
        }

        _ => Err(SparqlError::PropertyPathError(
            "Unsupported property path".to_string(),
        )),
    }
}

fn evaluate_transitive_path(
    ctx: &SparqlContext,
    subject: Option<RdfTerm>,
    path: &PropertyPath,
    object: Option<RdfTerm>,
    subj_pattern: &TermOrVariable,
    obj_pattern: &TermOrVariable,
    binding: &Binding,
    include_zero: bool,
) -> SparqlResult<Solutions> {
    let mut solutions = Vec::new();
    let mut visited: std::collections::HashSet<RdfTerm> = std::collections::HashSet::new();
    let mut frontier: Vec<RdfTerm> = Vec::new();

    // Initialize frontier
    if let Some(s) = &subject {
        frontier.push(s.clone());
        visited.insert(s.clone());

        // Zero length path
        if include_zero {
            if let Some(o) = &object {
                if s == o {
                    solutions.push(binding.clone());
                }
            } else {
                let mut sol = binding.clone();
                if let TermOrVariable::Variable(var) = obj_pattern {
                    sol.insert(var.clone(), s.clone());
                }
                solutions.push(sol);
            }
        }
    }

    // BFS traversal
    let max_depth = 100; // Prevent infinite loops
    let mut depth = 0;

    while !frontier.is_empty() && depth < max_depth {
        let mut next_frontier = Vec::new();

        for current in &frontier {
            let current_pattern = TermOrVariable::Term(current.clone());
            let step_solutions = evaluate_property_path(
                ctx,
                Some(current.clone()),
                path,
                None,
                &current_pattern,
                obj_pattern,
                binding,
            )?;

            for sol in step_solutions {
                if let TermOrVariable::Variable(var) = obj_pattern {
                    if let Some(next) = sol.get(var) {
                        if !visited.contains(next) {
                            visited.insert(next.clone());
                            next_frontier.push(next.clone());

                            // Check if we reached the target
                            if let Some(o) = &object {
                                if next == o {
                                    solutions.push(sol.clone());
                                }
                            } else {
                                solutions.push(sol.clone());
                            }
                        }
                    }
                }
            }
        }

        frontier = next_frontier;
        depth += 1;
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
            if bindings_compatible(l, r) && shares_variables(l, r) {
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

fn shares_variables(a: &Binding, b: &Binding) -> bool {
    a.keys().any(|k| b.contains_key(k))
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

fn join_values(solutions: Solutions, values: &ValuesClause) -> SparqlResult<Solutions> {
    let value_solutions: Solutions = values
        .bindings
        .iter()
        .map(|row| {
            let mut binding = Binding::new();
            for (i, var) in values.variables.iter().enumerate() {
                if let Some(Some(term)) = row.get(i) {
                    binding.insert(var.clone(), term.clone());
                }
            }
            binding
        })
        .collect();

    join_solutions(solutions, value_solutions)
}

// ============================================================================
// Aggregation
// ============================================================================

fn evaluate_group(
    solutions: Solutions,
    group_by: &[GroupCondition],
    aggregates: &[(Aggregate, String)],
) -> SparqlResult<Solutions> {
    // Group solutions by GROUP BY keys
    let mut groups: HashMap<Vec<Option<RdfTerm>>, Solutions> = HashMap::new();

    for binding in solutions {
        let mut key = Vec::new();
        for cond in group_by {
            let value = match cond {
                GroupCondition::Variable(var) => binding.get(var).cloned(),
                GroupCondition::Expression(expr, _) => evaluate_expression(expr, &binding)?,
            };
            key.push(value);
        }

        groups.entry(key).or_insert_with(Vec::new).push(binding);
    }

    // Compute aggregates for each group
    let mut result = Vec::new();

    for (key, group) in groups {
        let mut binding = Binding::new();

        // Add GROUP BY variables
        for (i, cond) in group_by.iter().enumerate() {
            if let Some(value) = &key[i] {
                let var_name = match cond {
                    GroupCondition::Variable(var) => var.clone(),
                    GroupCondition::Expression(_, Some(alias)) => alias.clone(),
                    GroupCondition::Expression(_, None) => format!("_group{}", i),
                };
                binding.insert(var_name, value.clone());
            }
        }

        // Compute aggregates
        for (agg, var) in aggregates {
            let value = compute_aggregate(agg, &group)?;
            if let Some(v) = value {
                binding.insert(var.clone(), v);
            }
        }

        result.push(binding);
    }

    Ok(result)
}

fn compute_aggregate(agg: &Aggregate, group: &Solutions) -> SparqlResult<Option<RdfTerm>> {
    match agg {
        Aggregate::Count { expr, distinct } => {
            let mut count = 0i64;
            let mut seen: std::collections::HashSet<RdfTerm> = std::collections::HashSet::new();

            for binding in group {
                let value = if let Some(e) = expr {
                    evaluate_expression(e, binding)?
                } else {
                    Some(RdfTerm::literal("*"))
                };

                if let Some(v) = value {
                    if *distinct {
                        if !seen.contains(&v) {
                            seen.insert(v);
                            count += 1;
                        }
                    } else {
                        count += 1;
                    }
                }
            }

            Ok(Some(RdfTerm::Literal(Literal::integer(count))))
        }

        Aggregate::Sum { expr, distinct } => {
            let mut sum = 0.0f64;
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    if let RdfTerm::Literal(lit) = &term {
                        if let Some(n) = lit.as_double() {
                            let key = lit.value.clone();
                            if *distinct {
                                if !seen.contains(&key) {
                                    seen.insert(key);
                                    sum += n;
                                }
                            } else {
                                sum += n;
                            }
                        }
                    }
                }
            }

            Ok(Some(RdfTerm::Literal(Literal::decimal(sum))))
        }

        Aggregate::Avg { expr, distinct } => {
            let mut sum = 0.0f64;
            let mut count = 0i64;
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    if let RdfTerm::Literal(lit) = &term {
                        if let Some(n) = lit.as_double() {
                            let key = lit.value.clone();
                            if *distinct {
                                if !seen.contains(&key) {
                                    seen.insert(key);
                                    sum += n;
                                    count += 1;
                                }
                            } else {
                                sum += n;
                                count += 1;
                            }
                        }
                    }
                }
            }

            if count > 0 {
                Ok(Some(RdfTerm::Literal(Literal::decimal(sum / count as f64))))
            } else {
                Ok(None)
            }
        }

        Aggregate::Min { expr } => {
            let mut min: Option<RdfTerm> = None;

            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    min = Some(match min {
                        None => term,
                        Some(current) => {
                            if compare_terms(&term, &current) == std::cmp::Ordering::Less {
                                term
                            } else {
                                current
                            }
                        }
                    });
                }
            }

            Ok(min)
        }

        Aggregate::Max { expr } => {
            let mut max: Option<RdfTerm> = None;

            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    max = Some(match max {
                        None => term,
                        Some(current) => {
                            if compare_terms(&term, &current) == std::cmp::Ordering::Greater {
                                term
                            } else {
                                current
                            }
                        }
                    });
                }
            }

            Ok(max)
        }

        Aggregate::GroupConcat {
            expr,
            separator,
            distinct,
        } => {
            let sep = separator.as_deref().unwrap_or(" ");
            let mut values: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    let s = term_to_string(&term);
                    if *distinct {
                        if !seen.contains(&s) {
                            seen.insert(s.clone());
                            values.push(s);
                        }
                    } else {
                        values.push(s);
                    }
                }
            }

            Ok(Some(RdfTerm::literal(values.join(sep))))
        }

        Aggregate::Sample { expr } => {
            for binding in group {
                if let Some(term) = evaluate_expression(expr, binding)? {
                    return Ok(Some(term));
                }
            }
            Ok(None)
        }
    }
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

    // HAVING
    if let Some(having) = &modifier.having {
        solutions = filter_solutions(solutions, having)?;
    }

    Ok(solutions)
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

        Expression::Function(func) => {
            let args: Vec<Option<RdfTerm>> = func
                .args
                .iter()
                .map(|a| evaluate_expression(a, binding))
                .collect::<SparqlResult<Vec<_>>>()?;
            evaluate_function(&func.name, args)
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

        Expression::In(expr, list) => {
            let v = evaluate_expression(expr, binding)?;
            for item in list {
                let iv = evaluate_expression(item, binding)?;
                if v == iv {
                    return Ok(Some(RdfTerm::Literal(Literal::boolean(true))));
                }
            }
            Ok(Some(RdfTerm::Literal(Literal::boolean(false))))
        }

        Expression::NotIn(expr, list) => {
            let v = evaluate_expression(expr, binding)?;
            for item in list {
                let iv = evaluate_expression(item, binding)?;
                if v == iv {
                    return Ok(Some(RdfTerm::Literal(Literal::boolean(false))));
                }
            }
            Ok(Some(RdfTerm::Literal(Literal::boolean(true))))
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

        Expression::IsNumeric(e) => {
            let v = evaluate_expression(e, binding)?;
            let is_numeric = v
                .map(|t| {
                    if let RdfTerm::Literal(lit) = t {
                        lit.as_double().is_some()
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            Ok(Some(RdfTerm::Literal(Literal::boolean(is_numeric))))
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

        Expression::Iri(e) => {
            let v = evaluate_expression(e, binding)?;
            Ok(v.map(|t| RdfTerm::Iri(Iri::new(term_to_string(&t)))))
        }

        Expression::Regex(text, pattern, flags) => {
            let text_val = evaluate_expression(text, binding)?
                .map(|t| term_to_string(&t))
                .unwrap_or_default();
            let pattern_val = evaluate_expression(pattern, binding)?
                .map(|t| term_to_string(&t))
                .unwrap_or_default();
            let flags_val = flags
                .as_ref()
                .and_then(|f| evaluate_expression(f, binding).ok().flatten())
                .map(|t| term_to_string(&t))
                .unwrap_or_default();

            // Simple regex matching (limited without regex crate)
            let matches = if flags_val.contains('i') {
                text_val
                    .to_lowercase()
                    .contains(&pattern_val.to_lowercase())
            } else {
                text_val.contains(&pattern_val)
            };

            Ok(Some(RdfTerm::Literal(Literal::boolean(matches))))
        }

        Expression::Aggregate(_) => {
            // Aggregates are handled separately in GROUP BY
            Err(SparqlError::AggregateError(
                "Aggregate in non-aggregate context".to_string(),
            ))
        }

        Expression::Exists(pattern) | Expression::NotExists(pattern) => {
            // Would need context to evaluate
            Err(SparqlError::UnsupportedOperation(
                "EXISTS requires context".to_string(),
            ))
        }
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

        BinaryOp::SameTerm => Ok(Some(RdfTerm::Literal(Literal::boolean(left == right)))),

        BinaryOp::LangMatches => {
            let lang = left.map(|t| term_to_string(&t)).unwrap_or_default();
            let range = right.map(|t| term_to_string(&t)).unwrap_or_default();

            let matches = if range == "*" {
                !lang.is_empty()
            } else {
                lang.eq_ignore_ascii_case(&range)
                    || lang
                        .to_lowercase()
                        .starts_with(&format!("{}-", range.to_lowercase()))
            };

            Ok(Some(RdfTerm::Literal(Literal::boolean(matches))))
        }
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

fn execute_construct(ctx: &mut SparqlContext, query: &ConstructQuery) -> SparqlResult<Vec<Triple>> {
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

fn execute_ask(ctx: &mut SparqlContext, query: &AskQuery) -> SparqlResult<bool> {
    let solutions = evaluate_graph_pattern(ctx, &query.where_clause)?;
    Ok(!solutions.is_empty())
}

fn execute_describe(ctx: &mut SparqlContext, query: &DescribeQuery) -> SparqlResult<Vec<Triple>> {
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
// Update Operations
// ============================================================================

fn execute_update(ctx: &mut SparqlContext, op: &UpdateOperation) -> SparqlResult<()> {
    match op {
        UpdateOperation::InsertData(data) => {
            for quad in &data.quads {
                let triple = Triple::new(
                    quad.subject.clone(),
                    quad.predicate.clone(),
                    quad.object.clone(),
                );
                if let Some(graph) = &quad.graph {
                    ctx.store.insert_into_graph(triple, Some(graph.as_str()));
                } else {
                    ctx.store.insert(triple);
                }
            }
            Ok(())
        }

        UpdateOperation::DeleteData(data) => {
            for quad in &data.quads {
                let matches = ctx.store.query(
                    Some(&quad.subject),
                    Some(&quad.predicate),
                    Some(&quad.object),
                );
                // Note: We'd need triple IDs to actually remove
            }
            Ok(())
        }

        UpdateOperation::Modify(modify) => {
            let solutions = evaluate_graph_pattern(ctx, &modify.where_pattern)?;

            // Delete matching patterns
            if let Some(delete_patterns) = &modify.delete_pattern {
                for binding in &solutions {
                    for pattern in delete_patterns {
                        // Resolve pattern and delete matching triples
                    }
                }
            }

            // Insert new triples
            if let Some(insert_patterns) = &modify.insert_pattern {
                for binding in &solutions {
                    for pattern in insert_patterns {
                        if let (Some(s), Some(o)) = (
                            resolve_quad_term(&pattern.subject, binding),
                            resolve_quad_term(&pattern.object, binding),
                        ) {
                            if let VarOrIri::Iri(p) = &pattern.predicate {
                                let triple = Triple::new(s, p.clone(), o);
                                ctx.store.insert(triple);
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        UpdateOperation::Clear { target, silent } => {
            match target {
                GraphTarget::Default => ctx.store.clear_graph(None),
                GraphTarget::Named(iri) => ctx.store.clear_graph(Some(iri.as_str())),
                GraphTarget::All | GraphTarget::AllNamed => ctx.store.clear(),
            }
            Ok(())
        }

        UpdateOperation::Drop { target, silent } => {
            // Same as CLEAR for in-memory store
            match target {
                GraphTarget::Default => ctx.store.clear_graph(None),
                GraphTarget::Named(iri) => ctx.store.clear_graph(Some(iri.as_str())),
                GraphTarget::All | GraphTarget::AllNamed => ctx.store.clear(),
            }
            Ok(())
        }

        UpdateOperation::Load {
            source,
            destination,
            silent,
        } => Err(SparqlError::UnsupportedOperation(
            "LOAD not supported".to_string(),
        )),

        UpdateOperation::Create { graph, silent } => {
            // Named graphs are created automatically
            Ok(())
        }

        UpdateOperation::Copy {
            source,
            destination,
            silent,
        }
        | UpdateOperation::Move {
            source,
            destination,
            silent,
        }
        | UpdateOperation::Add {
            source,
            destination,
            silent,
        } => Err(SparqlError::UnsupportedOperation(
            "Graph management not fully supported".to_string(),
        )),
    }
}

fn resolve_quad_term(tov: &TermOrVariable, binding: &Binding) -> Option<RdfTerm> {
    match tov {
        TermOrVariable::Term(t) => Some(t.clone()),
        TermOrVariable::Variable(v) => binding.get(v).cloned(),
        TermOrVariable::BlankNode(id) => Some(RdfTerm::BlankNode(id.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::sparql::parser::parse_sparql;

    fn setup_test_store() -> TripleStore {
        let store = TripleStore::new();

        // Add test data
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
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/2"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/2"),
            Iri::new("http://example.org/name"),
            RdfTerm::literal("Bob"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::new("http://example.org/knows"),
            RdfTerm::iri("http://example.org/person/2"),
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
            let binding = &select.bindings[0];
            let name = binding.get("name").unwrap();
            assert!(matches!(name, RdfTerm::Literal(l) if l.value == "Alice"));
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

    #[test]
    fn test_count_aggregate() {
        let store = setup_test_store();
        let query = parse_sparql(
            r#"
            SELECT (COUNT(?s) AS ?count) WHERE {
                ?s a <http://example.org/Person>
            }
        "#,
        )
        .unwrap();
        let result = execute_sparql(&store, &query).unwrap();

        if let QueryResult::Select(select) = result {
            assert!(!select.bindings.is_empty());
        }
    }

    #[test]
    fn test_optional_pattern() {
        let store = setup_test_store();
        let query = parse_sparql(
            r#"
            SELECT ?name ?age WHERE {
                ?s <http://example.org/name> ?name .
                OPTIONAL { ?s <http://example.org/age> ?age }
            }
        "#,
        )
        .unwrap();
        let result = execute_sparql(&store, &query).unwrap();

        if let QueryResult::Select(select) = result {
            assert_eq!(select.bindings.len(), 2);
            // One binding should have age, one should not
            let with_age = select
                .bindings
                .iter()
                .filter(|b| b.contains_key("age"))
                .count();
            assert_eq!(with_age, 1);
        }
    }
}
