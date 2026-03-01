// SPARQL Built-in Functions
//
// Implementation of SPARQL 1.1 built-in functions:
// https://www.w3.org/TR/sparql11-query/#SparqlOps

use super::ast::{Iri, Literal, RdfTerm};
use super::{SparqlError, SparqlResult};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Evaluate a SPARQL function call
pub fn evaluate_function(name: &str, args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let name_upper = name.to_uppercase();

    match name_upper.as_str() {
        // String functions
        "STRLEN" => fn_strlen(args),
        "SUBSTR" | "SUBSTRING" => fn_substr(args),
        "UCASE" => fn_ucase(args),
        "LCASE" => fn_lcase(args),
        "STRSTARTS" => fn_strstarts(args),
        "STRENDS" => fn_strends(args),
        "CONTAINS" => fn_contains(args),
        "STRBEFORE" => fn_strbefore(args),
        "STRAFTER" => fn_strafter(args),
        "ENCODE_FOR_URI" => fn_encode_for_uri(args),
        "CONCAT" => fn_concat(args),
        "REPLACE" => fn_replace(args),

        // Numeric functions
        "ABS" => fn_abs(args),
        "ROUND" => fn_round(args),
        "CEIL" => fn_ceil(args),
        "FLOOR" => fn_floor(args),
        "RAND" => fn_rand(args),

        // Date/time functions
        "NOW" => fn_now(args),
        "YEAR" => fn_year(args),
        "MONTH" => fn_month(args),
        "DAY" => fn_day(args),
        "HOURS" => fn_hours(args),
        "MINUTES" => fn_minutes(args),
        "SECONDS" => fn_seconds(args),
        "TIMEZONE" => fn_timezone(args),
        "TZ" => fn_tz(args),

        // Hash functions
        "MD5" => fn_hash(args, "md5"),
        "SHA1" => fn_hash(args, "sha1"),
        "SHA256" => fn_hash(args, "sha256"),
        "SHA384" => fn_hash(args, "sha384"),
        "SHA512" => fn_hash(args, "sha512"),

        // Constructor functions
        "STRUUID" => fn_struuid(args),
        "UUID" => fn_uuid(args),
        "BNODE" => fn_bnode(args),
        "STRDT" => fn_strdt(args),
        "STRLANG" => fn_strlang(args),

        // Type conversion
        "STR" => fn_str(args),

        // RuVector extensions
        "RUVECTOR_SIMILARITY" => fn_vector_similarity(args),
        "RUVECTOR_DISTANCE" => fn_vector_distance(args),

        _ => Err(SparqlError::UnsupportedOperation(format!(
            "Unknown function: {}",
            name
        ))),
    }
}

// ============================================================================
// String Functions
// ============================================================================

fn fn_strlen(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    Ok(Some(RdfTerm::Literal(Literal::integer(s.len() as i64))))
}

fn fn_substr(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let start = get_integer_arg(&args, 1)? as usize;
    let length = args
        .get(2)
        .and_then(|a| a.as_ref())
        .and_then(|t| term_to_integer(t))
        .map(|n| n as usize);

    // SPARQL uses 1-based indexing
    let start_idx = start.saturating_sub(1);

    let result: String = if let Some(len) = length {
        s.chars().skip(start_idx).take(len).collect()
    } else {
        s.chars().skip(start_idx).collect()
    };

    Ok(Some(RdfTerm::literal(result)))
}

fn fn_ucase(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    Ok(Some(RdfTerm::literal(s.to_uppercase())))
}

fn fn_lcase(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    Ok(Some(RdfTerm::literal(s.to_lowercase())))
}

fn fn_strstarts(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let prefix = get_string_arg(&args, 1)?;
    Ok(Some(RdfTerm::Literal(Literal::boolean(
        s.starts_with(&prefix),
    ))))
}

fn fn_strends(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let suffix = get_string_arg(&args, 1)?;
    Ok(Some(RdfTerm::Literal(Literal::boolean(
        s.ends_with(&suffix),
    ))))
}

fn fn_contains(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let pattern = get_string_arg(&args, 1)?;
    Ok(Some(RdfTerm::Literal(Literal::boolean(
        s.contains(&pattern),
    ))))
}

fn fn_strbefore(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let pattern = get_string_arg(&args, 1)?;

    let result = if pattern.is_empty() {
        String::new()
    } else if let Some(idx) = s.find(&pattern) {
        s[..idx].to_string()
    } else {
        String::new()
    };

    Ok(Some(RdfTerm::literal(result)))
}

fn fn_strafter(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let pattern = get_string_arg(&args, 1)?;

    let result = if pattern.is_empty() {
        s
    } else if let Some(idx) = s.find(&pattern) {
        s[idx + pattern.len()..].to_string()
    } else {
        String::new()
    };

    Ok(Some(RdfTerm::literal(result)))
}

fn fn_encode_for_uri(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;

    let encoded: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect();

    Ok(Some(RdfTerm::literal(encoded)))
}

fn fn_concat(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let mut result = String::new();

    for arg in args {
        if let Some(term) = arg {
            result.push_str(&term_to_string(&term));
        }
    }

    Ok(Some(RdfTerm::literal(result)))
}

fn fn_replace(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;
    let pattern = get_string_arg(&args, 1)?;
    let replacement = get_string_arg(&args, 2)?;
    // Note: Full regex support would require the regex crate
    let result = s.replace(&pattern, &replacement);
    Ok(Some(RdfTerm::literal(result)))
}

// ============================================================================
// Numeric Functions
// ============================================================================

fn fn_abs(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let n = get_number_arg(&args, 0)?;
    Ok(Some(RdfTerm::Literal(Literal::decimal(n.abs()))))
}

fn fn_round(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let n = get_number_arg(&args, 0)?;
    Ok(Some(RdfTerm::Literal(Literal::decimal(n.round()))))
}

fn fn_ceil(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let n = get_number_arg(&args, 0)?;
    Ok(Some(RdfTerm::Literal(Literal::decimal(n.ceil()))))
}

fn fn_floor(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let n = get_number_arg(&args, 0)?;
    Ok(Some(RdfTerm::Literal(Literal::decimal(n.floor()))))
}

fn fn_rand(_args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    // Simple pseudo-random using hash of current time
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    nanos.hash(&mut hasher);
    let hash = hasher.finish();

    let random = (hash as f64) / (u64::MAX as f64);
    Ok(Some(RdfTerm::Literal(Literal::double(random))))
}

// ============================================================================
// Date/Time Functions
// ============================================================================

fn fn_now(_args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| SparqlError::ExecutionError(e.to_string()))?;

    let secs = duration.as_secs();
    // Simple ISO 8601 format
    let datetime = format!("{}Z", secs);

    Ok(Some(RdfTerm::typed_literal(datetime, Iri::xsd_datetime())))
}

fn fn_year(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    // Simple parsing - expects YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS format
    if dt.len() >= 4 {
        if let Ok(year) = dt[..4].parse::<i64>() {
            return Ok(Some(RdfTerm::Literal(Literal::integer(year))));
        }
    }
    Ok(None)
}

fn fn_month(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    if dt.len() >= 7 && dt.chars().nth(4) == Some('-') {
        if let Ok(month) = dt[5..7].parse::<i64>() {
            return Ok(Some(RdfTerm::Literal(Literal::integer(month))));
        }
    }
    Ok(None)
}

fn fn_day(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    if dt.len() >= 10 && dt.chars().nth(7) == Some('-') {
        if let Ok(day) = dt[8..10].parse::<i64>() {
            return Ok(Some(RdfTerm::Literal(Literal::integer(day))));
        }
    }
    Ok(None)
}

fn fn_hours(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    if let Some(t_pos) = dt.find('T') {
        if dt.len() >= t_pos + 3 {
            if let Ok(hours) = dt[t_pos + 1..t_pos + 3].parse::<i64>() {
                return Ok(Some(RdfTerm::Literal(Literal::integer(hours))));
            }
        }
    }
    Ok(None)
}

fn fn_minutes(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    if let Some(t_pos) = dt.find('T') {
        if dt.len() >= t_pos + 6 {
            if let Ok(minutes) = dt[t_pos + 4..t_pos + 6].parse::<i64>() {
                return Ok(Some(RdfTerm::Literal(Literal::integer(minutes))));
            }
        }
    }
    Ok(None)
}

fn fn_seconds(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    if let Some(t_pos) = dt.find('T') {
        if dt.len() >= t_pos + 9 {
            // Handle both integer and decimal seconds
            let sec_str = &dt[t_pos + 7..];
            let end_pos = sec_str
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(sec_str.len());
            if let Ok(seconds) = sec_str[..end_pos].parse::<f64>() {
                return Ok(Some(RdfTerm::Literal(Literal::decimal(seconds))));
            }
        }
    }
    Ok(None)
}

fn fn_timezone(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;
    // Look for timezone at end
    if dt.ends_with('Z') {
        return Ok(Some(RdfTerm::literal("PT0S")));
    }

    // Look for +/-HH:MM
    if let Some(tz_pos) = dt.rfind('+').or_else(|| dt.rfind('-')) {
        if tz_pos > 10 {
            // After date part
            let tz = &dt[tz_pos..];
            if tz.len() >= 6 {
                let sign = if tz.starts_with('-') { "-" } else { "" };
                let hours: i64 = tz[1..3].parse().unwrap_or(0);
                let minutes: i64 = tz[4..6].parse().unwrap_or(0);
                let duration = format!("{}PT{}H{}M", sign, hours, minutes);
                return Ok(Some(RdfTerm::literal(duration)));
            }
        }
    }

    Ok(None)
}

fn fn_tz(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let dt = get_string_arg(&args, 0)?;

    if dt.ends_with('Z') {
        return Ok(Some(RdfTerm::literal("Z")));
    }

    if let Some(tz_pos) = dt.rfind('+').or_else(|| dt.rfind('-')) {
        if tz_pos > 10 {
            return Ok(Some(RdfTerm::literal(&dt[tz_pos..])));
        }
    }

    Ok(Some(RdfTerm::literal("")))
}

// ============================================================================
// Hash Functions
// ============================================================================

fn fn_hash(args: Vec<Option<RdfTerm>>, algorithm: &str) -> SparqlResult<Option<RdfTerm>> {
    let s = get_string_arg(&args, 0)?;

    // Simple hash implementation using Rust's hasher
    // In production, use proper crypto hashes
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    algorithm.hash(&mut hasher);
    let hash = hasher.finish();

    // Format as hex string
    let hex = format!("{:016x}", hash);

    Ok(Some(RdfTerm::literal(hex)))
}

// ============================================================================
// Constructor Functions
// ============================================================================

fn fn_struuid(_args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    // Generate UUID-like string
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    nanos.hash(&mut hasher);
    let hash1 = hasher.finish();

    hasher = DefaultHasher::new();
    (nanos + 1).hash(&mut hasher);
    let hash2 = hasher.finish();

    let uuid = format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (hash1 >> 32) as u32,
        (hash1 >> 16) as u16,
        (hash1 as u16) & 0x0FFF,
        ((hash2 >> 48) as u16 & 0x3FFF) | 0x8000,
        hash2 & 0xFFFFFFFFFFFF
    );

    Ok(Some(RdfTerm::literal(uuid)))
}

fn fn_uuid(_args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let struuid = fn_struuid(vec![])?;
    if let Some(RdfTerm::Literal(lit)) = struuid {
        Ok(Some(RdfTerm::Iri(Iri::new(format!(
            "urn:uuid:{}",
            lit.value
        )))))
    } else {
        Ok(None)
    }
}

fn fn_bnode(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    if args.is_empty() || args[0].is_none() {
        // Generate new blank node
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Ok(Some(RdfTerm::BlankNode(format!("b{}", nanos))))
    } else {
        // Create blank node with given ID
        let id = get_string_arg(&args, 0)?;
        Ok(Some(RdfTerm::BlankNode(id)))
    }
}

fn fn_strdt(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let value = get_string_arg(&args, 0)?;
    let datatype = get_iri_arg(&args, 1)?;
    Ok(Some(RdfTerm::typed_literal(value, datatype)))
}

fn fn_strlang(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let value = get_string_arg(&args, 0)?;
    let lang = get_string_arg(&args, 1)?;
    Ok(Some(RdfTerm::lang_literal(value, lang)))
}

fn fn_str(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let term = args.get(0).and_then(|a| a.clone());
    Ok(term.map(|t| RdfTerm::literal(term_to_string(&t))))
}

// ============================================================================
// RuVector Extension Functions
// ============================================================================

/// Compute cosine similarity between two vector literals
fn fn_vector_similarity(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let v1 = get_vector_arg(&args, 0)?;
    let v2 = get_vector_arg(&args, 1)?;

    if v1.len() != v2.len() {
        return Err(SparqlError::TypeMismatch {
            expected: format!("vectors of same dimension"),
            actual: format!("dimensions {} and {}", v1.len(), v2.len()),
        });
    }

    // Cosine similarity
    let dot: f64 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f64 = v1.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm2: f64 = v2.iter().map(|x| x * x).sum::<f64>().sqrt();

    let similarity = if norm1 > 0.0 && norm2 > 0.0 {
        dot / (norm1 * norm2)
    } else {
        0.0
    };

    Ok(Some(RdfTerm::Literal(Literal::double(similarity))))
}

/// Compute L2 distance between two vector literals
fn fn_vector_distance(args: Vec<Option<RdfTerm>>) -> SparqlResult<Option<RdfTerm>> {
    let v1 = get_vector_arg(&args, 0)?;
    let v2 = get_vector_arg(&args, 1)?;

    if v1.len() != v2.len() {
        return Err(SparqlError::TypeMismatch {
            expected: format!("vectors of same dimension"),
            actual: format!("dimensions {} and {}", v1.len(), v2.len()),
        });
    }

    // L2 (Euclidean) distance
    let distance: f64 = v1
        .iter()
        .zip(v2.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt();

    Ok(Some(RdfTerm::Literal(Literal::double(distance))))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn get_string_arg(args: &[Option<RdfTerm>], index: usize) -> SparqlResult<String> {
    args.get(index)
        .and_then(|a| a.as_ref())
        .map(|t| term_to_string(t))
        .ok_or_else(|| SparqlError::ExecutionError(format!("Missing argument {}", index)))
}

fn get_number_arg(args: &[Option<RdfTerm>], index: usize) -> SparqlResult<f64> {
    args.get(index)
        .and_then(|a| a.as_ref())
        .and_then(|t| term_to_number(t))
        .ok_or_else(|| SparqlError::TypeMismatch {
            expected: "numeric".to_string(),
            actual: "non-numeric or missing".to_string(),
        })
}

fn get_integer_arg(args: &[Option<RdfTerm>], index: usize) -> SparqlResult<i64> {
    args.get(index)
        .and_then(|a| a.as_ref())
        .and_then(|t| term_to_integer(t))
        .ok_or_else(|| SparqlError::TypeMismatch {
            expected: "integer".to_string(),
            actual: "non-integer or missing".to_string(),
        })
}

fn get_iri_arg(args: &[Option<RdfTerm>], index: usize) -> SparqlResult<Iri> {
    args.get(index)
        .and_then(|a| a.as_ref())
        .and_then(|t| match t {
            RdfTerm::Iri(iri) => Some(iri.clone()),
            RdfTerm::Literal(lit) => Some(Iri::new(&lit.value)),
            _ => None,
        })
        .ok_or_else(|| SparqlError::TypeMismatch {
            expected: "IRI".to_string(),
            actual: "non-IRI or missing".to_string(),
        })
}

fn get_vector_arg(args: &[Option<RdfTerm>], index: usize) -> SparqlResult<Vec<f64>> {
    let s = get_string_arg(args, index)?;

    // Parse vector format: [1.0, 2.0, 3.0] or 1.0,2.0,3.0
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');

    s.split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .map_err(|_| SparqlError::TypeMismatch {
                    expected: "numeric vector".to_string(),
                    actual: format!("invalid number: {}", v),
                })
        })
        .collect()
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

fn term_to_integer(term: &RdfTerm) -> Option<i64> {
    match term {
        RdfTerm::Literal(lit) => lit.as_integer(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strlen() {
        let result = fn_strlen(vec![Some(RdfTerm::literal("hello"))]).unwrap();
        assert!(matches!(result, Some(RdfTerm::Literal(l)) if l.as_integer() == Some(5)));
    }

    #[test]
    fn test_substr() {
        let result = fn_substr(vec![
            Some(RdfTerm::literal("hello")),
            Some(RdfTerm::Literal(Literal::integer(2))),
            Some(RdfTerm::Literal(Literal::integer(3))),
        ])
        .unwrap();
        assert!(matches!(result, Some(RdfTerm::Literal(l)) if l.value == "ell"));
    }

    #[test]
    fn test_ucase_lcase() {
        let upper = fn_ucase(vec![Some(RdfTerm::literal("hello"))]).unwrap();
        assert!(matches!(upper, Some(RdfTerm::Literal(l)) if l.value == "HELLO"));

        let lower = fn_lcase(vec![Some(RdfTerm::literal("HELLO"))]).unwrap();
        assert!(matches!(lower, Some(RdfTerm::Literal(l)) if l.value == "hello"));
    }

    #[test]
    fn test_contains() {
        let result = fn_contains(vec![
            Some(RdfTerm::literal("hello world")),
            Some(RdfTerm::literal("world")),
        ])
        .unwrap();
        assert!(matches!(result, Some(RdfTerm::Literal(l)) if l.as_boolean() == Some(true)));
    }

    #[test]
    fn test_abs() {
        let result = fn_abs(vec![Some(RdfTerm::Literal(Literal::decimal(-5.5)))]).unwrap();
        assert!(matches!(result, Some(RdfTerm::Literal(l)) if l.as_double() == Some(5.5)));
    }

    #[test]
    fn test_concat() {
        let result = fn_concat(vec![
            Some(RdfTerm::literal("hello")),
            Some(RdfTerm::literal(" ")),
            Some(RdfTerm::literal("world")),
        ])
        .unwrap();
        assert!(matches!(result, Some(RdfTerm::Literal(l)) if l.value == "hello world"));
    }

    #[test]
    fn test_vector_similarity() {
        let result = fn_vector_similarity(vec![
            Some(RdfTerm::literal("[1.0, 0.0, 0.0]")),
            Some(RdfTerm::literal("[1.0, 0.0, 0.0]")),
        ])
        .unwrap();

        if let Some(RdfTerm::Literal(l)) = result {
            let sim = l.as_double().unwrap();
            assert!((sim - 1.0).abs() < 0.001);
        } else {
            panic!("Expected literal result");
        }
    }

    #[test]
    fn test_vector_distance() {
        let result = fn_vector_distance(vec![
            Some(RdfTerm::literal("[0.0, 0.0]")),
            Some(RdfTerm::literal("[3.0, 4.0]")),
        ])
        .unwrap();

        if let Some(RdfTerm::Literal(l)) = result {
            let dist = l.as_double().unwrap();
            assert!((dist - 5.0).abs() < 0.001);
        } else {
            panic!("Expected literal result");
        }
    }
}
