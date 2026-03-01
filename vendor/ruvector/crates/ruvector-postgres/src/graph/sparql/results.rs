// SPARQL Result Formatting
//
// Formats query results in standard SPARQL formats:
// - JSON (SPARQL 1.1 Query Results JSON Format)
// - XML (SPARQL Query Results XML Format)
// - CSV/TSV (SPARQL 1.1 Query Results CSV and TSV Formats)

use super::ast::RdfTerm;
use super::executor::QueryResult;
use super::triple_store::Triple;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result format type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultFormat {
    Json,
    Xml,
    Csv,
    Tsv,
}

/// SPARQL results wrapper for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparqlResults {
    pub head: ResultHead,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<ResultBindings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boolean: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultHead {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub link: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultBindings {
    pub bindings: Vec<HashMap<String, ResultValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultValue {
    #[serde(rename = "type")]
    pub value_type: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datatype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "xml:lang")]
    pub lang: Option<String>,
}

impl ResultValue {
    pub fn from_term(term: &RdfTerm) -> Self {
        match term {
            RdfTerm::Iri(iri) => Self {
                value_type: "uri".to_string(),
                value: iri.as_str().to_string(),
                datatype: None,
                lang: None,
            },
            RdfTerm::Literal(lit) => {
                let datatype = if lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string"
                    && lit.language.is_none()
                {
                    Some(lit.datatype.as_str().to_string())
                } else {
                    None
                };

                Self {
                    value_type: "literal".to_string(),
                    value: lit.value.clone(),
                    datatype,
                    lang: lit.language.clone(),
                }
            }
            RdfTerm::BlankNode(id) => Self {
                value_type: "bnode".to_string(),
                value: id.clone(),
                datatype: None,
                lang: None,
            },
        }
    }
}

/// Format query results in the specified format
pub fn format_results(result: &QueryResult, format: ResultFormat) -> String {
    match format {
        ResultFormat::Json => format_json(result),
        ResultFormat::Xml => format_xml(result),
        ResultFormat::Csv => format_csv(result),
        ResultFormat::Tsv => format_tsv(result),
    }
}

// ============================================================================
// JSON Format
// ============================================================================

fn format_json(result: &QueryResult) -> String {
    let sparql_results = match result {
        QueryResult::Select(select) => {
            let bindings: Vec<HashMap<String, ResultValue>> = select
                .bindings
                .iter()
                .map(|binding| {
                    binding
                        .iter()
                        .map(|(k, v)| (k.clone(), ResultValue::from_term(v)))
                        .collect()
                })
                .collect();

            SparqlResults {
                head: ResultHead {
                    vars: select.variables.clone(),
                    link: vec![],
                },
                results: Some(ResultBindings { bindings }),
                boolean: None,
            }
        }

        QueryResult::Ask(value) => SparqlResults {
            head: ResultHead {
                vars: vec![],
                link: vec![],
            },
            results: None,
            boolean: Some(*value),
        },

        QueryResult::Construct(triples) | QueryResult::Describe(triples) => {
            // For CONSTRUCT/DESCRIBE, return as JSON-LD-like format
            let bindings: Vec<HashMap<String, ResultValue>> = triples
                .iter()
                .map(|triple| {
                    let mut binding = HashMap::new();
                    binding.insert(
                        "subject".to_string(),
                        ResultValue::from_term(&triple.subject),
                    );
                    binding.insert(
                        "predicate".to_string(),
                        ResultValue {
                            value_type: "uri".to_string(),
                            value: triple.predicate.as_str().to_string(),
                            datatype: None,
                            lang: None,
                        },
                    );
                    binding.insert("object".to_string(), ResultValue::from_term(&triple.object));
                    binding
                })
                .collect();

            SparqlResults {
                head: ResultHead {
                    vars: vec![
                        "subject".to_string(),
                        "predicate".to_string(),
                        "object".to_string(),
                    ],
                    link: vec![],
                },
                results: Some(ResultBindings { bindings }),
                boolean: None,
            }
        }

        QueryResult::Update => SparqlResults {
            head: ResultHead {
                vars: vec![],
                link: vec![],
            },
            results: None,
            boolean: Some(true),
        },
    };

    serde_json::to_string_pretty(&sparql_results).unwrap_or_else(|_| "{}".to_string())
}

// ============================================================================
// XML Format
// ============================================================================

fn format_xml(result: &QueryResult) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0"?>
<sparql xmlns="http://www.w3.org/2005/sparql-results#">
"#,
    );

    match result {
        QueryResult::Select(select) => {
            // Head
            xml.push_str("  <head>\n");
            for var in &select.variables {
                xml.push_str(&format!("    <variable name=\"{}\"/>\n", escape_xml(var)));
            }
            xml.push_str("  </head>\n");

            // Results
            xml.push_str("  <results>\n");
            for binding in &select.bindings {
                xml.push_str("    <result>\n");
                for (var, value) in binding {
                    xml.push_str(&format!("      <binding name=\"{}\">\n", escape_xml(var)));
                    xml.push_str(&format_term_xml(value));
                    xml.push_str("      </binding>\n");
                }
                xml.push_str("    </result>\n");
            }
            xml.push_str("  </results>\n");
        }

        QueryResult::Ask(value) => {
            xml.push_str("  <head/>\n");
            xml.push_str(&format!("  <boolean>{}</boolean>\n", value));
        }

        QueryResult::Construct(triples) | QueryResult::Describe(triples) => {
            xml.push_str("  <head>\n");
            xml.push_str("    <variable name=\"subject\"/>\n");
            xml.push_str("    <variable name=\"predicate\"/>\n");
            xml.push_str("    <variable name=\"object\"/>\n");
            xml.push_str("  </head>\n");

            xml.push_str("  <results>\n");
            for triple in triples {
                xml.push_str("    <result>\n");
                xml.push_str("      <binding name=\"subject\">\n");
                xml.push_str(&format_term_xml(&triple.subject));
                xml.push_str("      </binding>\n");
                xml.push_str("      <binding name=\"predicate\">\n");
                xml.push_str(&format!(
                    "        <uri>{}</uri>\n",
                    escape_xml(triple.predicate.as_str())
                ));
                xml.push_str("      </binding>\n");
                xml.push_str("      <binding name=\"object\">\n");
                xml.push_str(&format_term_xml(&triple.object));
                xml.push_str("      </binding>\n");
                xml.push_str("    </result>\n");
            }
            xml.push_str("  </results>\n");
        }

        QueryResult::Update => {
            xml.push_str("  <head/>\n");
            xml.push_str("  <boolean>true</boolean>\n");
        }
    }

    xml.push_str("</sparql>");
    xml
}

fn format_term_xml(term: &RdfTerm) -> String {
    match term {
        RdfTerm::Iri(iri) => {
            format!("        <uri>{}</uri>\n", escape_xml(iri.as_str()))
        }
        RdfTerm::Literal(lit) => {
            let mut s = String::from("        <literal");
            if let Some(lang) = &lit.language {
                s.push_str(&format!(" xml:lang=\"{}\"", escape_xml(lang)));
            } else if lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                s.push_str(&format!(
                    " datatype=\"{}\"",
                    escape_xml(lit.datatype.as_str())
                ));
            }
            s.push_str(&format!(">{}</literal>\n", escape_xml(&lit.value)));
            s
        }
        RdfTerm::BlankNode(id) => {
            format!("        <bnode>{}</bnode>\n", escape_xml(id))
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ============================================================================
// CSV Format
// ============================================================================

fn format_csv(result: &QueryResult) -> String {
    format_delimited(result, ',')
}

// ============================================================================
// TSV Format
// ============================================================================

fn format_tsv(result: &QueryResult) -> String {
    format_delimited(result, '\t')
}

fn format_delimited(result: &QueryResult, delimiter: char) -> String {
    let mut output = String::new();

    match result {
        QueryResult::Select(select) => {
            // Header
            output.push_str(&select.variables.join(&delimiter.to_string()));
            output.push('\n');

            // Rows
            for binding in &select.bindings {
                let row: Vec<String> = select
                    .variables
                    .iter()
                    .map(|var| {
                        binding
                            .get(var)
                            .map(|term| format_term_csv(term, delimiter))
                            .unwrap_or_default()
                    })
                    .collect();
                output.push_str(&row.join(&delimiter.to_string()));
                output.push('\n');
            }
        }

        QueryResult::Ask(value) => {
            output.push_str(&format!("{}\n", value));
        }

        QueryResult::Construct(triples) | QueryResult::Describe(triples) => {
            output.push_str(&format!(
                "subject{}predicate{}object\n",
                delimiter, delimiter
            ));
            for triple in triples {
                output.push_str(&format!(
                    "{}{}{}{}{}",
                    format_term_csv(&triple.subject, delimiter),
                    delimiter,
                    escape_csv(triple.predicate.as_str(), delimiter),
                    delimiter,
                    format_term_csv(&triple.object, delimiter),
                ));
                output.push('\n');
            }
        }

        QueryResult::Update => {
            output.push_str("success\ntrue\n");
        }
    }

    output
}

fn format_term_csv(term: &RdfTerm, delimiter: char) -> String {
    match term {
        RdfTerm::Iri(iri) => escape_csv(iri.as_str(), delimiter),
        RdfTerm::Literal(lit) => {
            if lit.language.is_some()
                || lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string"
            {
                // Use N-Triples-like format for typed/language literals
                let mut s = format!("\"{}\"", lit.value.replace('"', "\\\""));
                if let Some(lang) = &lit.language {
                    s.push_str(&format!("@{}", lang));
                } else {
                    s.push_str(&format!("^^<{}>", lit.datatype.as_str()));
                }
                escape_csv(&s, delimiter)
            } else {
                escape_csv(&lit.value, delimiter)
            }
        }
        RdfTerm::BlankNode(id) => escape_csv(&format!("_:{}", id), delimiter),
    }
}

fn escape_csv(s: &str, delimiter: char) -> String {
    if s.contains(delimiter) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ============================================================================
// N-Triples Format (for CONSTRUCT/DESCRIBE)
// ============================================================================

/// Format triples as N-Triples
pub fn format_ntriples(triples: &[Triple]) -> String {
    let mut output = String::new();

    for triple in triples {
        output.push_str(&format_term_nt(&triple.subject));
        output.push(' ');
        output.push_str(&format!("<{}>", triple.predicate.as_str()));
        output.push(' ');
        output.push_str(&format_term_nt(&triple.object));
        output.push_str(" .\n");
    }

    output
}

fn format_term_nt(term: &RdfTerm) -> String {
    match term {
        RdfTerm::Iri(iri) => format!("<{}>", iri.as_str()),
        RdfTerm::Literal(lit) => {
            let escaped = lit
                .value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");

            if let Some(lang) = &lit.language {
                format!("\"{}\"@{}", escaped, lang)
            } else if lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                format!("\"{}\"^^<{}>", escaped, lit.datatype.as_str())
            } else {
                format!("\"{}\"", escaped)
            }
        }
        RdfTerm::BlankNode(id) => format!("_:{}", id),
    }
}

// ============================================================================
// Turtle Format (for CONSTRUCT/DESCRIBE)
// ============================================================================

/// Format triples as Turtle
pub fn format_turtle(triples: &[Triple]) -> String {
    let mut output = String::new();

    // Group by subject
    let mut by_subject: HashMap<String, Vec<&Triple>> = HashMap::new();
    for triple in triples {
        let key = format_term_nt(&triple.subject);
        by_subject.entry(key).or_default().push(triple);
    }

    for (subject, subject_triples) in by_subject {
        output.push_str(&subject);
        output.push('\n');

        let total = subject_triples.len();
        for (i, triple) in subject_triples.iter().enumerate() {
            output.push_str("    ");
            output.push_str(&format!("<{}>", triple.predicate.as_str()));
            output.push(' ');
            output.push_str(&format_term_nt(&triple.object));

            if i < total - 1 {
                output.push_str(" ;\n");
            } else {
                output.push_str(" .\n");
            }
        }
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::super::ast::Iri;
    use super::super::executor::SelectResult;
    use super::*;
    use std::collections::HashMap;

    fn create_test_select() -> QueryResult {
        let mut binding = HashMap::new();
        binding.insert("name".to_string(), RdfTerm::literal("Alice"));
        binding.insert("age".to_string(), RdfTerm::literal("30"));

        QueryResult::Select(SelectResult {
            variables: vec!["name".to_string(), "age".to_string()],
            bindings: vec![binding],
        })
    }

    #[test]
    fn test_json_format() {
        let result = create_test_select();
        let json = format_results(&result, ResultFormat::Json);

        assert!(json.contains("\"vars\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"Alice\""));
    }

    #[test]
    fn test_xml_format() {
        let result = create_test_select();
        let xml = format_results(&result, ResultFormat::Xml);

        assert!(xml.contains("<sparql"));
        assert!(xml.contains("<variable name=\"name\""));
        assert!(xml.contains("<literal>Alice</literal>"));
    }

    #[test]
    fn test_csv_format() {
        let result = create_test_select();
        let csv = format_results(&result, ResultFormat::Csv);

        assert!(csv.contains("name,age"));
        assert!(csv.contains("Alice"));
    }

    #[test]
    fn test_tsv_format() {
        let result = create_test_select();
        let tsv = format_results(&result, ResultFormat::Tsv);

        assert!(tsv.contains("name\tage"));
    }

    #[test]
    fn test_ask_json() {
        let result = QueryResult::Ask(true);
        let json = format_results(&result, ResultFormat::Json);

        assert!(json.contains("\"boolean\": true"));
    }

    #[test]
    fn test_ntriples() {
        let triples = vec![Triple::new(
            RdfTerm::iri("http://example.org/s"),
            Iri::new("http://example.org/p"),
            RdfTerm::literal("object"),
        )];

        let nt = format_ntriples(&triples);
        assert!(nt.contains("<http://example.org/s>"));
        assert!(nt.contains("<http://example.org/p>"));
        assert!(nt.contains("\"object\""));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
