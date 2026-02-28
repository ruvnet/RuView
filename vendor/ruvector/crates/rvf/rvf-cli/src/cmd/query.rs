//! `rvf query` -- Query nearest neighbors.

use clap::Args;
use std::path::Path;

use rvf_runtime::filter::FilterExpr;
use rvf_runtime::{QueryOptions, RvfStore};

use super::map_rvf_err;

#[derive(Args)]
pub struct QueryArgs {
    /// Path to the RVF store
    path: String,
    /// Query vector as comma-separated floats (e.g. "1.0,0.0,0.5")
    #[arg(short, long)]
    vector: String,
    /// Number of nearest neighbors to return
    #[arg(short, long, default_value = "10")]
    k: usize,
    /// Optional filter as JSON (e.g. '{"eq":{"field":0,"value":{"u64":10}}}')
    #[arg(short, long)]
    filter: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(args: QueryArgs) -> Result<(), Box<dyn std::error::Error>> {
    let vector: Vec<f32> = args
        .vector
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f32>()
                .map_err(|e| format!("Invalid vector component '{s}': {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let filter = match &args.filter {
        Some(f) => Some(parse_filter_json(f)?),
        None => None,
    };

    let query_opts = QueryOptions {
        filter,
        ..Default::default()
    };

    let store = RvfStore::open_readonly(Path::new(&args.path)).map_err(map_rvf_err)?;
    let results = store
        .query(&vector, args.k, &query_opts)
        .map_err(map_rvf_err)?;

    if args.json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "distance": r.distance,
                })
            })
            .collect();
        crate::output::print_json(&serde_json::json!({
            "results": json_results,
            "count": results.len(),
        }));
    } else {
        println!("Query results ({} neighbors):", results.len());
        for (i, r) in results.iter().enumerate() {
            println!("  [{i}] id={} distance={:.6}", r.id, r.distance);
        }
    }
    Ok(())
}

/// Parse a JSON string into a FilterExpr.
///
/// Supported format:
///   {"eq": {"field": 0, "value": {"u64": 42}}}
///   {"ne": {"field": 0, "value": {"string": "cat_a"}}}
///   {"gt": {"field": 1, "value": {"f64": 3.14}}}
///   {"lt": {"field": 1, "value": {"i64": -5}}}
///   {"ge": {"field": 1, "value": {"u64": 100}}}
///   {"le": {"field": 1, "value": {"u64": 100}}}
///   {"and": [<expr>, <expr>, ...]}
///   {"or": [<expr>, <expr>, ...]}
///   {"not": <expr>}
pub fn parse_filter_json(json_str: &str) -> Result<FilterExpr, Box<dyn std::error::Error>> {
    let v: serde_json::Value = serde_json::from_str(json_str)?;
    parse_filter_value(&v)
}

fn parse_filter_value(v: &serde_json::Value) -> Result<FilterExpr, Box<dyn std::error::Error>> {
    let obj = v.as_object().ok_or("filter must be a JSON object")?;

    if let Some(inner) = obj.get("eq") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Eq(field, val));
    }
    if let Some(inner) = obj.get("ne") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Ne(field, val));
    }
    if let Some(inner) = obj.get("lt") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Lt(field, val));
    }
    if let Some(inner) = obj.get("le") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Le(field, val));
    }
    if let Some(inner) = obj.get("gt") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Gt(field, val));
    }
    if let Some(inner) = obj.get("ge") {
        let (field, val) = parse_field_value(inner)?;
        return Ok(FilterExpr::Ge(field, val));
    }
    if let Some(inner) = obj.get("and") {
        let arr = inner.as_array().ok_or("'and' value must be an array")?;
        let exprs: Result<Vec<_>, _> = arr.iter().map(parse_filter_value).collect();
        return Ok(FilterExpr::And(exprs?));
    }
    if let Some(inner) = obj.get("or") {
        let arr = inner.as_array().ok_or("'or' value must be an array")?;
        let exprs: Result<Vec<_>, _> = arr.iter().map(parse_filter_value).collect();
        return Ok(FilterExpr::Or(exprs?));
    }
    if let Some(inner) = obj.get("not") {
        let expr = parse_filter_value(inner)?;
        return Ok(FilterExpr::Not(Box::new(expr)));
    }

    Err("unrecognized filter operator; expected: eq, ne, lt, le, gt, ge, and, or, not".into())
}

fn parse_field_value(
    v: &serde_json::Value,
) -> Result<(u16, rvf_runtime::filter::FilterValue), Box<dyn std::error::Error>> {
    let obj = v
        .as_object()
        .ok_or("comparison must be a JSON object with 'field' and 'value'")?;
    let field = obj
        .get("field")
        .and_then(|f| f.as_u64())
        .ok_or("missing or invalid 'field' (must be u16)")? as u16;
    let value_obj = obj.get("value").ok_or("missing 'value' in comparison")?;
    let filter_val = parse_filter_val(value_obj)?;
    Ok((field, filter_val))
}

fn parse_filter_val(
    v: &serde_json::Value,
) -> Result<rvf_runtime::filter::FilterValue, Box<dyn std::error::Error>> {
    use rvf_runtime::filter::FilterValue;

    if let Some(obj) = v.as_object() {
        if let Some(val) = obj.get("u64") {
            return Ok(FilterValue::U64(
                val.as_u64().ok_or("u64 value must be a number")?,
            ));
        }
        if let Some(val) = obj.get("i64") {
            return Ok(FilterValue::I64(
                val.as_i64().ok_or("i64 value must be a number")?,
            ));
        }
        if let Some(val) = obj.get("f64") {
            return Ok(FilterValue::F64(
                val.as_f64().ok_or("f64 value must be a number")?,
            ));
        }
        if let Some(val) = obj.get("string") {
            return Ok(FilterValue::String(
                val.as_str()
                    .ok_or("string value must be a string")?
                    .to_string(),
            ));
        }
        if let Some(val) = obj.get("bool") {
            return Ok(FilterValue::Bool(
                val.as_bool().ok_or("bool value must be a boolean")?,
            ));
        }
    }

    // Fallback: infer type from JSON value directly
    if let Some(n) = v.as_u64() {
        return Ok(FilterValue::U64(n));
    }
    if let Some(n) = v.as_i64() {
        return Ok(FilterValue::I64(n));
    }
    if let Some(n) = v.as_f64() {
        return Ok(FilterValue::F64(n));
    }
    if let Some(s) = v.as_str() {
        return Ok(FilterValue::String(s.to_string()));
    }
    if let Some(b) = v.as_bool() {
        return Ok(FilterValue::Bool(b));
    }

    Err("cannot parse filter value; expected {\"u64\": N}, {\"string\": \"...\"}, etc.".into())
}
