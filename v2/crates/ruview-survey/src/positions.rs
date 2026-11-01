//! Parse the sensing server's `--node-positions` string.
//!
//! Same grammar as `field_bridge::parse_node_position_entries`:
//! `node_id:x,y,z;node_id:x,y,z` or, without the prefix, `x,y,z;...` keyed by
//! list index from 0. The server skips bad entries with a warning; a checker
//! should not, so this parser returns an error instead.

use std::collections::BTreeMap;

use crate::SurveyError;

/// Most node positions accepted.
pub const MAX_NODES: usize = 64;

/// Configured positions keyed by node id, in metres.
pub type NodePositions = BTreeMap<u8, [f64; 3]>;

/// Parse a `--node-positions` value into a keyed map.
pub fn parse_node_positions(input: &str) -> Result<NodePositions, SurveyError> {
    let entries: Vec<&str> = input
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    if entries.len() > MAX_NODES {
        return Err(SurveyError::TooMany {
            what: "node positions",
            got: entries.len(),
            max: MAX_NODES,
        });
    }
    let mut map = NodePositions::new();
    for (idx, entry) in entries.iter().enumerate() {
        let (id, triplet) = match entry.split_once(':') {
            Some((id, rest)) => {
                let id = id.trim().parse::<u8>().map_err(|_| {
                    SurveyError::InvalidPositions(format!(
                        "entry {idx} '{entry}': node id must be 0..=255"
                    ))
                })?;
                (id, rest)
            }
            None => (
                u8::try_from(idx).map_err(|_| {
                    SurveyError::InvalidPositions(format!("entry {idx}: index exceeds 255"))
                })?,
                *entry,
            ),
        };
        let parts: Vec<&str> = triplet.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(SurveyError::InvalidPositions(format!(
                "entry {idx} '{entry}': expected x,y,z"
            )));
        }
        let mut xyz = [0.0f64; 3];
        for (slot, part) in xyz.iter_mut().zip(&parts) {
            let v = part.parse::<f64>().map_err(|_| {
                SurveyError::InvalidPositions(format!(
                    "entry {idx} '{entry}': '{part}' is not a number"
                ))
            })?;
            if !v.is_finite() {
                return Err(SurveyError::InvalidPositions(format!(
                    "entry {idx} '{entry}': non-finite coordinate"
                )));
            }
            *slot = v;
        }
        if map.insert(id, xyz).is_some() {
            return Err(SurveyError::InvalidPositions(format!(
                "node {id} is given twice"
            )));
        }
    }
    Ok(map)
}

/// Euclidean distance between two positions.
pub(crate) fn distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keyed_and_indexed_forms() {
        let keyed = parse_node_positions("3:0,0,1.5; 7:4.5,0,1.5").unwrap();
        assert_eq!(keyed.len(), 2);
        assert_eq!(keyed[&7], [4.5, 0.0, 1.5]);

        let indexed = parse_node_positions("0,0,1;2,0,1").unwrap();
        assert_eq!(indexed[&1], [2.0, 0.0, 1.0]);
    }

    #[test]
    fn rejects_malformed_entries() {
        assert!(parse_node_positions("1:0,0").is_err());
        assert!(parse_node_positions("x:0,0,0").is_err());
        assert!(parse_node_positions("1:0,a,0").is_err());
        assert!(parse_node_positions("1:0,0,inf").is_err());
        assert!(parse_node_positions("1:0,0,0;1:1,1,1").is_err());
    }

    #[test]
    fn empty_input_is_empty_map() {
        assert!(parse_node_positions("").unwrap().is_empty());
    }

    #[test]
    fn rejects_too_many_entries() {
        let s = (0..=MAX_NODES)
            .map(|i| format!("{i}:0,0,0"))
            .collect::<Vec<_>>()
            .join(";");
        assert!(matches!(
            parse_node_positions(&s),
            Err(SurveyError::TooMany { .. })
        ));
    }
}
