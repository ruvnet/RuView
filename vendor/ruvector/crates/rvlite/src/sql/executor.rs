// SQL executor that integrates with ruvector-core VectorDB
use super::ast::*;
use crate::{ErrorKind, RvLiteError};
use parking_lot::RwLock;
use ruvector_core::{SearchQuery, VectorDB, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Table schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<Column>,
    pub vector_column: Option<String>,
    pub vector_dimensions: Option<usize>,
}

impl TableSchema {
    /// Find the vector column in the schema
    fn find_vector_column(&self) -> Option<(String, usize)> {
        for col in &self.columns {
            if let DataType::Vector(dims) = col.data_type {
                return Some((col.name.clone(), dims));
            }
        }
        None
    }

    /// Validate that columns match the schema
    fn validate_columns(&self, columns: &[String]) -> Result<(), RvLiteError> {
        for col in columns {
            if !self.columns.iter().any(|c| &c.name == col) {
                return Err(RvLiteError {
                    message: format!("Column '{}' not found in table '{}'", col, self.name),
                    kind: ErrorKind::SqlError,
                });
            }
        }
        Ok(())
    }

    /// Get column data type
    fn get_column_type(&self, name: &str) -> Option<&DataType> {
        self.columns
            .iter()
            .find(|c| c.name == name)
            .map(|c| &c.data_type)
    }
}

/// SQL execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub rows: Vec<HashMap<String, Value>>,
    pub rows_affected: usize,
}

/// SQL Engine that manages tables and executes queries
pub struct SqlEngine {
    /// Table schemas
    schemas: RwLock<HashMap<String, TableSchema>>,
    /// Vector databases (one per table)
    databases: RwLock<HashMap<String, VectorDB>>,
}

impl SqlEngine {
    /// Create a new SQL engine
    pub fn new() -> Self {
        SqlEngine {
            schemas: RwLock::new(HashMap::new()),
            databases: RwLock::new(HashMap::new()),
        }
    }

    /// Execute a SQL statement
    pub fn execute(&self, statement: SqlStatement) -> Result<ExecutionResult, RvLiteError> {
        match statement {
            SqlStatement::CreateTable { name, columns } => self.create_table(name, columns),
            SqlStatement::Insert {
                table,
                columns,
                values,
            } => self.insert(table, columns, values),
            SqlStatement::Select {
                columns,
                from,
                where_clause,
                order_by,
                limit,
            } => self.select(columns, from, where_clause, order_by, limit),
            SqlStatement::Drop { table } => self.drop_table(table),
        }
    }

    fn create_table(
        &self,
        name: String,
        columns: Vec<Column>,
    ) -> Result<ExecutionResult, RvLiteError> {
        let mut schemas = self.schemas.write();

        if schemas.contains_key(&name) {
            return Err(RvLiteError {
                message: format!("Table '{}' already exists", name),
                kind: ErrorKind::SqlError,
            });
        }

        // Find vector column
        let (vector_column, vector_dimensions) = columns
            .iter()
            .find_map(|col| {
                if let DataType::Vector(dims) = col.data_type {
                    Some((col.name.clone(), dims))
                } else {
                    None
                }
            })
            .ok_or_else(|| RvLiteError {
                message: "Table must have at least one VECTOR column".to_string(),
                kind: ErrorKind::SqlError,
            })?;

        let schema = TableSchema {
            name: name.clone(),
            columns,
            vector_column: Some(vector_column),
            vector_dimensions: Some(vector_dimensions),
        };

        // Create vector database for this table
        let db_options = ruvector_core::types::DbOptions {
            dimensions: vector_dimensions,
            distance_metric: ruvector_core::DistanceMetric::Cosine,
            storage_path: "memory://".to_string(),
            hnsw_config: None,
            quantization: None,
        };

        let db = VectorDB::new(db_options).map_err(|e| RvLiteError {
            message: format!("Failed to create vector database: {}", e),
            kind: ErrorKind::VectorError,
        })?;

        let mut databases = self.databases.write();
        databases.insert(name.clone(), db);
        schemas.insert(name, schema);

        Ok(ExecutionResult {
            rows: Vec::new(),
            rows_affected: 0,
        })
    }

    fn insert(
        &self,
        table: String,
        columns: Vec<String>,
        values: Vec<Value>,
    ) -> Result<ExecutionResult, RvLiteError> {
        let schemas = self.schemas.read();
        let schema = schemas.get(&table).ok_or_else(|| RvLiteError {
            message: format!("Table '{}' not found", table),
            kind: ErrorKind::SqlError,
        })?;

        // Validate columns
        schema.validate_columns(&columns)?;

        if columns.len() != values.len() {
            return Err(RvLiteError {
                message: format!(
                    "Column count ({}) does not match value count ({})",
                    columns.len(),
                    values.len()
                ),
                kind: ErrorKind::SqlError,
            });
        }

        // Extract vector and metadata
        let mut vector: Option<Vec<f32>> = None;
        let mut metadata = HashMap::new();
        let mut id: Option<String> = None;

        for (col, val) in columns.iter().zip(values.iter()) {
            if let Some(DataType::Vector(_)) = schema.get_column_type(col) {
                if let Value::Vector(v) = val {
                    vector = Some(v.clone());
                } else {
                    return Err(RvLiteError {
                        message: format!("Expected vector value for column '{}'", col),
                        kind: ErrorKind::SqlError,
                    });
                }
            } else {
                // Store as metadata
                metadata.insert(col.clone(), val.to_json());

                // Use 'id' column as vector ID if present
                if col == "id" {
                    if let Value::Text(s) = val {
                        id = Some(s.clone());
                    }
                }
            }
        }

        let vector = vector.ok_or_else(|| RvLiteError {
            message: "No vector value provided".to_string(),
            kind: ErrorKind::SqlError,
        })?;

        // Validate vector dimensions
        if let Some(expected_dims) = schema.vector_dimensions {
            if vector.len() != expected_dims {
                return Err(RvLiteError {
                    message: format!(
                        "Vector dimension mismatch: expected {}, got {}",
                        expected_dims,
                        vector.len()
                    ),
                    kind: ErrorKind::SqlError,
                });
            }
        }

        // Insert into vector database
        let entry = VectorEntry {
            id,
            vector,
            metadata: Some(metadata),
        };

        let databases = self.databases.read();
        let db = databases.get(&table).ok_or_else(|| RvLiteError {
            message: format!("Database for table '{}' not found", table),
            kind: ErrorKind::SqlError,
        })?;

        db.insert(entry).map_err(|e| RvLiteError {
            message: format!("Failed to insert: {}", e),
            kind: ErrorKind::VectorError,
        })?;

        Ok(ExecutionResult {
            rows: Vec::new(),
            rows_affected: 1,
        })
    }

    fn select(
        &self,
        _columns: Vec<SelectColumn>,
        from: String,
        where_clause: Option<Expression>,
        order_by: Option<OrderBy>,
        limit: Option<usize>,
    ) -> Result<ExecutionResult, RvLiteError> {
        let schemas = self.schemas.read();
        let schema = schemas.get(&from).ok_or_else(|| RvLiteError {
            message: format!("Table '{}' not found", from),
            kind: ErrorKind::SqlError,
        })?;

        let databases = self.databases.read();
        let db = databases.get(&from).ok_or_else(|| RvLiteError {
            message: format!("Database for table '{}' not found", from),
            kind: ErrorKind::SqlError,
        })?;

        // Handle vector similarity search
        if let Some(order_by) = order_by {
            if let Expression::Distance {
                column: _,
                metric: _,
                vector,
            } = order_by.expression
            {
                let k = limit.unwrap_or(10);

                // Build filter from WHERE clause
                let filter = if let Some(where_expr) = where_clause {
                    Some(self.build_filter(where_expr)?)
                } else {
                    None
                };

                let query = SearchQuery {
                    vector,
                    k,
                    filter,
                    ef_search: None,
                };

                let results = db.search(query).map_err(|e| RvLiteError {
                    message: format!("Search failed: {}", e),
                    kind: ErrorKind::VectorError,
                })?;

                // Convert results to rows
                let rows: Vec<HashMap<String, Value>> = results
                    .into_iter()
                    .map(|result| {
                        let mut row = HashMap::new();

                        // Add vector if present
                        if let Some(vec_col) = &schema.vector_column {
                            if let Some(vector) = result.vector {
                                row.insert(vec_col.clone(), Value::Vector(vector));
                            }
                        }

                        // Add metadata
                        if let Some(metadata) = result.metadata {
                            for (key, val) in metadata {
                                row.insert(key, Value::from_json(&val));
                            }
                        }

                        // Add distance score
                        row.insert("_distance".to_string(), Value::Real(result.score as f64));

                        row
                    })
                    .collect();

                return Ok(ExecutionResult {
                    rows,
                    rows_affected: 0,
                });
            }
        }

        // Non-vector query - return all rows (scan all vectors)
        // This is essentially a table scan through the vector database
        let k = limit.unwrap_or(1000); // Default to 1000 rows max

        // Create a zero vector for exhaustive search
        let dims = schema.vector_dimensions.unwrap_or(3);
        let query_vector = vec![0.0f32; dims];

        // Build filter from WHERE clause
        let filter = if let Some(where_expr) = where_clause {
            Some(self.build_filter(where_expr)?)
        } else {
            None
        };

        let query = SearchQuery {
            vector: query_vector,
            k,
            filter,
            ef_search: None,
        };

        let results = db.search(query).map_err(|e| RvLiteError {
            message: format!("Search failed: {}", e),
            kind: ErrorKind::VectorError,
        })?;

        // Convert results to rows
        let rows: Vec<HashMap<String, Value>> = results
            .into_iter()
            .map(|result| {
                let mut row = HashMap::new();

                // Add vector if present
                if let Some(vec_col) = &schema.vector_column {
                    if let Some(vector) = result.vector {
                        row.insert(vec_col.clone(), Value::Vector(vector));
                    }
                }

                // Add metadata
                if let Some(metadata) = result.metadata {
                    for (key, val) in metadata {
                        row.insert(key, Value::from_json(&val));
                    }
                }

                row
            })
            .collect();

        Ok(ExecutionResult {
            rows,
            rows_affected: 0,
        })
    }

    fn drop_table(&self, table: String) -> Result<ExecutionResult, RvLiteError> {
        let mut schemas = self.schemas.write();
        let mut databases = self.databases.write();

        schemas.remove(&table).ok_or_else(|| RvLiteError {
            message: format!("Table '{}' not found", table),
            kind: ErrorKind::SqlError,
        })?;

        databases.remove(&table);

        Ok(ExecutionResult {
            rows: Vec::new(),
            rows_affected: 0,
        })
    }

    /// Build metadata filter from WHERE expression
    fn build_filter(
        &self,
        expr: Expression,
    ) -> Result<HashMap<String, serde_json::Value>, RvLiteError> {
        let mut filter = HashMap::new();

        match expr {
            Expression::BinaryOp { left, op, right } => {
                if let (Expression::Column(col), Expression::Literal(val)) = (*left, *right) {
                    if op == BinaryOperator::Eq {
                        filter.insert(col, val.to_json());
                    } else {
                        return Err(RvLiteError {
                            message: "Only equality filters supported in WHERE clause".to_string(),
                            kind: ErrorKind::NotImplemented,
                        });
                    }
                }
            }
            Expression::And(left, right) => {
                let left_filter = self.build_filter(*left)?;
                let right_filter = self.build_filter(*right)?;
                filter.extend(left_filter);
                filter.extend(right_filter);
            }
            _ => {
                return Err(RvLiteError {
                    message: "Unsupported WHERE clause expression".to_string(),
                    kind: ErrorKind::NotImplemented,
                });
            }
        }

        Ok(filter)
    }

    /// List all tables
    pub fn list_tables(&self) -> Vec<String> {
        self.schemas.read().keys().cloned().collect()
    }

    /// Get table schema
    pub fn get_schema(&self, table: &str) -> Option<TableSchema> {
        self.schemas.read().get(table).cloned()
    }
}

impl Default for SqlEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_insert() {
        let engine = SqlEngine::new();

        // Create table
        let create = SqlStatement::CreateTable {
            name: "docs".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    data_type: DataType::Text,
                },
                Column {
                    name: "content".to_string(),
                    data_type: DataType::Text,
                },
                Column {
                    name: "embedding".to_string(),
                    data_type: DataType::Vector(3),
                },
            ],
        };
        engine.execute(create).unwrap();

        // Insert row
        let insert = SqlStatement::Insert {
            table: "docs".to_string(),
            columns: vec![
                "id".to_string(),
                "content".to_string(),
                "embedding".to_string(),
            ],
            values: vec![
                Value::Text("1".to_string()),
                Value::Text("hello".to_string()),
                Value::Vector(vec![1.0, 2.0, 3.0]),
            ],
        };
        let result = engine.execute(insert).unwrap();
        assert_eq!(result.rows_affected, 1);
    }

    #[test]
    fn test_vector_search() {
        let engine = SqlEngine::new();

        // Create table
        let create = SqlStatement::CreateTable {
            name: "docs".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    data_type: DataType::Text,
                },
                Column {
                    name: "embedding".to_string(),
                    data_type: DataType::Vector(3),
                },
            ],
        };
        engine.execute(create).unwrap();

        // Insert rows
        for i in 0..5 {
            let insert = SqlStatement::Insert {
                table: "docs".to_string(),
                columns: vec!["id".to_string(), "embedding".to_string()],
                values: vec![
                    Value::Text(format!("{}", i)),
                    Value::Vector(vec![i as f32, i as f32 * 2.0, i as f32 * 3.0]),
                ],
            };
            engine.execute(insert).unwrap();
        }

        // Search
        let select = SqlStatement::Select {
            columns: vec![SelectColumn::Wildcard],
            from: "docs".to_string(),
            where_clause: None,
            order_by: Some(OrderBy {
                expression: Expression::Distance {
                    column: "embedding".to_string(),
                    metric: DistanceMetric::L2,
                    vector: vec![2.0, 4.0, 6.0],
                },
                direction: OrderDirection::Asc,
            }),
            limit: Some(3),
        };

        let result = engine.execute(select).unwrap();
        assert_eq!(result.rows.len(), 3);
    }
}
