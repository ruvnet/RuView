// Simplified Cypher parser
// Note: This is a basic parser for demonstration. A production parser would use
// a proper parsing library like nom, pest, or lalrpop.

use super::ast::*;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Parse a Cypher query string
pub fn parse_cypher(query: &str) -> Result<CypherQuery, String> {
    let query = query.trim();

    // Very simple pattern matching for basic queries
    // Production code should use a proper parser

    if query.to_uppercase().starts_with("CREATE") {
        parse_create(query)
    } else if query.to_uppercase().starts_with("MATCH") {
        parse_match(query)
    } else {
        Err(format!("Unsupported query type: {}", query))
    }
}

fn parse_create(query: &str) -> Result<CypherQuery, String> {
    // Pattern: CREATE (n:Label {prop: value}) RETURN n
    let mut result = CypherQuery::new();

    // Extract pattern between CREATE and RETURN/end
    let create_part = if let Some(idx) = query.to_uppercase().find("RETURN") {
        &query[6..idx].trim()
    } else {
        &query[6..].trim()
    };

    let pattern = parse_pattern(create_part)?;
    result
        .clauses
        .push(Clause::Create(CreateClause::new(vec![pattern])));

    // Check for RETURN clause
    if let Some(idx) = query.to_uppercase().find("RETURN") {
        let return_part = &query[idx + 6..].trim();
        let return_clause = parse_return(return_part)?;
        result.clauses.push(Clause::Return(return_clause));
    }

    Ok(result)
}

fn parse_match(query: &str) -> Result<CypherQuery, String> {
    // Pattern: MATCH (n:Label) WHERE n.prop = value RETURN n
    let mut result = CypherQuery::new();

    // Extract MATCH pattern
    let match_start = 5; // "MATCH".len()
    let match_end = query
        .to_uppercase()
        .find("WHERE")
        .or_else(|| query.to_uppercase().find("RETURN"))
        .unwrap_or(query.len());

    let match_part = &query[match_start..match_end].trim();
    let pattern = parse_pattern(match_part)?;
    result
        .clauses
        .push(Clause::Match(MatchClause::new(vec![pattern])));

    // Check for WHERE clause
    if let Some(where_idx) = query.to_uppercase().find("WHERE") {
        let where_start = where_idx + 5; // "WHERE".len()
        let where_end = query.to_uppercase().find("RETURN").unwrap_or(query.len());

        let where_part = &query[where_start..where_end].trim();
        let where_clause = parse_where(where_part)?;
        result.clauses.push(Clause::Where(where_clause));
    }

    // Check for RETURN clause
    if let Some(return_idx) = query.to_uppercase().find("RETURN") {
        let return_part = &query[return_idx + 6..].trim();
        let return_clause = parse_return(return_part)?;
        result.clauses.push(Clause::Return(return_clause));
    }

    Ok(result)
}

fn parse_pattern(pattern_str: &str) -> Result<Pattern, String> {
    let pattern_str = pattern_str.trim();
    let mut pattern = Pattern::new();

    // Simple parser for (n:Label {prop: value})-[:TYPE]->(m)
    // This is very basic - production code needs proper parsing

    if pattern_str.starts_with('(') {
        // Node pattern
        let end = pattern_str.find(')').ok_or("Unclosed node pattern")?;

        let node_content = &pattern_str[1..end];
        let node_pattern = parse_node_pattern(node_content)?;
        pattern = pattern.with_element(PatternElement::Node(node_pattern));

        // Check for relationship
        let remaining = &pattern_str[end + 1..].trim();
        if !remaining.is_empty() && remaining.starts_with('-') {
            // Parse relationship
            let (rel_pattern, rest) = parse_relationship_pattern(remaining)?;
            pattern = pattern.with_element(PatternElement::Relationship(rel_pattern));

            // Parse target node
            if rest.starts_with('(') {
                let end = rest.find(')').ok_or("Unclosed target node pattern")?;
                let node_content = &rest[1..end];
                let node_pattern = parse_node_pattern(node_content)?;
                pattern = pattern.with_element(PatternElement::Node(node_pattern));
            }
        }
    }

    Ok(pattern)
}

fn parse_node_pattern(content: &str) -> Result<NodePattern, String> {
    let content = content.trim();
    let mut pattern = NodePattern::new();

    if content.is_empty() {
        return Ok(pattern);
    }

    // Parse: n:Label {prop: value}
    let mut parts = content.splitn(2, '{');
    let var_label = parts.next().unwrap_or("").trim();

    // Parse variable and labels
    if let Some((var, labels)) = var_label.split_once(':') {
        let var = var.trim();
        if !var.is_empty() {
            pattern = pattern.with_variable(var);
        }

        let labels = labels.trim();
        for label in labels.split(':') {
            let label = label.trim();
            if !label.is_empty() {
                pattern = pattern.with_label(label);
            }
        }
    } else if !var_label.is_empty() {
        // Just a variable
        pattern = pattern.with_variable(var_label);
    }

    // Parse properties
    if let Some(props_str) = parts.next() {
        let props_str = props_str.trim_end_matches('}').trim();
        let properties = parse_properties(props_str)?;
        for (key, value) in properties {
            pattern = pattern.with_property(key, Expression::Literal(value));
        }
    }

    Ok(pattern)
}

fn parse_relationship_pattern(content: &str) -> Result<(RelationshipPattern, &str), String> {
    let content = content.trim();

    // Determine direction
    let (direction, start_idx) = if content.starts_with("<-") {
        (Direction::Incoming, 2)
    } else if content.starts_with("->") {
        (Direction::Outgoing, 2)
    } else if content.starts_with('-') {
        (Direction::Both, 1)
    } else {
        return Err("Invalid relationship pattern".to_string());
    };

    let mut pattern = RelationshipPattern::new(direction);

    // Find relationship end
    let _end_markers = if direction == Direction::Incoming {
        vec!["-", "-("]
    } else {
        vec!["->", "-"]
    };

    let mut rel_content = "";
    let mut rest_start = start_idx;

    // Parse relationship details if present
    if content[start_idx..].starts_with('[') {
        if let Some(end) = content[start_idx..].find(']') {
            rel_content = &content[start_idx + 1..start_idx + end];
            rest_start = start_idx + end + 1;

            // Skip closing arrow
            let rest = &content[rest_start..];
            if rest.starts_with("->") {
                rest_start += 2;
            } else if rest.starts_with('-') {
                rest_start += 1;
            }
        }
    }

    // Parse relationship content: r:TYPE {prop: value}
    if !rel_content.is_empty() {
        let mut parts = rel_content.splitn(2, '{');
        let var_type = parts.next().unwrap_or("").trim();

        if let Some((var, rel_type)) = var_type.split_once(':') {
            let var = var.trim();
            if !var.is_empty() {
                pattern = pattern.with_variable(var);
            }

            let rel_type = rel_type.trim();
            if !rel_type.is_empty() {
                pattern = pattern.with_type(rel_type);
            }
        } else if !var_type.is_empty() {
            // Could be variable or type
            if var_type.chars().next().unwrap_or(' ').is_lowercase() {
                pattern = pattern.with_variable(var_type);
            } else {
                pattern = pattern.with_type(var_type);
            }
        }

        // Parse properties
        if let Some(props_str) = parts.next() {
            let props_str = props_str.trim_end_matches('}').trim();
            let properties = parse_properties(props_str)?;
            for (key, value) in properties {
                pattern = pattern.with_property(key, Expression::Literal(value));
            }
        }
    }

    Ok((pattern, &content[rest_start..]))
}

fn parse_properties(props_str: &str) -> Result<HashMap<String, JsonValue>, String> {
    let mut properties = HashMap::new();

    if props_str.is_empty() {
        return Ok(properties);
    }

    // Very simple property parser: key: value, key2: value2
    // Production code should use proper JSON parsing
    for pair in props_str.split(',') {
        let pair = pair.trim();
        if let Some((key, value)) = pair.split_once(':') {
            let key = key.trim().trim_matches('\'').trim_matches('"');
            let value = value.trim();

            let json_value = if value.starts_with('\'') || value.starts_with('"') {
                // String
                JsonValue::String(value.trim_matches('\'').trim_matches('"').to_string())
            } else if let Ok(num) = value.parse::<i64>() {
                // Integer
                JsonValue::Number(num.into())
            } else if let Ok(num) = value.parse::<f64>() {
                // Float
                JsonValue::Number(serde_json::Number::from_f64(num).ok_or("Invalid number")?)
            } else if value == "true" || value == "false" {
                // Boolean
                JsonValue::Bool(value == "true")
            } else {
                // Default to string
                JsonValue::String(value.to_string())
            };

            properties.insert(key.to_string(), json_value);
        }
    }

    Ok(properties)
}

fn parse_where(where_str: &str) -> Result<WhereClause, String> {
    // Simple WHERE parser: n.prop = value
    let where_str = where_str.trim();

    // Parse simple equality
    if let Some((left, right)) = where_str.split_once('=') {
        let left = left.trim();
        let right = right.trim();

        let left_expr = if let Some((var, prop)) = left.split_once('.') {
            Expression::Property(var.trim().to_string(), prop.trim().to_string())
        } else {
            Expression::Variable(left.to_string())
        };

        let right_expr = if right.starts_with('\'') || right.starts_with('"') {
            Expression::Literal(JsonValue::String(
                right.trim_matches('\'').trim_matches('"').to_string(),
            ))
        } else if let Ok(num) = right.parse::<i64>() {
            Expression::Literal(JsonValue::Number(num.into()))
        } else {
            Expression::Variable(right.to_string())
        };

        Ok(WhereClause::new(Expression::BinaryOp(
            Box::new(left_expr),
            BinaryOperator::Eq,
            Box::new(right_expr),
        )))
    } else {
        Err("Unsupported WHERE clause format".to_string())
    }
}

fn parse_return(return_str: &str) -> Result<ReturnClause, String> {
    let return_str = return_str.trim();
    let mut items = Vec::new();

    // Parse return items (comma-separated)
    for item_str in return_str.split(',') {
        let item_str = item_str.trim();

        // Check for alias: expr AS alias
        if let Some((expr_str, alias)) = item_str.split_once(" AS ") {
            let expr = parse_return_expression(expr_str.trim())?;
            items.push(ReturnItem::new(expr).with_alias(alias.trim()));
        } else {
            let expr = parse_return_expression(item_str)?;
            items.push(ReturnItem::new(expr));
        }
    }

    Ok(ReturnClause::new(items))
}

fn parse_return_expression(expr_str: &str) -> Result<Expression, String> {
    let expr_str = expr_str.trim();

    // Check for property access
    if let Some((var, prop)) = expr_str.split_once('.') {
        Ok(Expression::Property(
            var.trim().to_string(),
            prop.trim().to_string(),
        ))
    } else {
        Ok(Expression::Variable(expr_str.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create() {
        let query = "CREATE (n:Person {name: 'Alice', age: 30}) RETURN n";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 2);
    }

    #[test]
    fn test_parse_match() {
        let query = "MATCH (n:Person) WHERE n.name = 'Alice' RETURN n";
        let result = parse_cypher(query);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.clauses.len(), 3);
    }

    #[test]
    fn test_parse_pattern_with_relationship() {
        let pattern_str = "(a:Person)-[:KNOWS]->(b:Person)";
        let result = parse_pattern(pattern_str);
        assert!(result.is_ok());

        let pattern = result.unwrap();
        assert_eq!(pattern.elements.len(), 3); // node, rel, node
    }

    #[test]
    fn test_parse_properties() {
        let props = "name: 'Alice', age: 30, active: true";
        let result = parse_properties(props);
        assert!(result.is_ok());

        let properties = result.unwrap();
        assert_eq!(properties.len(), 3);
        assert_eq!(properties.get("name").unwrap().as_str().unwrap(), "Alice");
        assert_eq!(properties.get("age").unwrap().as_i64().unwrap(), 30);
        assert_eq!(properties.get("active").unwrap().as_bool().unwrap(), true);
    }
}
