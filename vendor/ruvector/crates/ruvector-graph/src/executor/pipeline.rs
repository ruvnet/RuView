//! Pipeline execution model with Volcano-style iterators
//!
//! Implements pull-based query execution with row batching

use crate::executor::operators::Operator;
use crate::executor::plan::Value;
use crate::executor::plan::{PhysicalPlan, QuerySchema};
use crate::executor::{ExecutionError, Result};
use std::collections::HashMap;

/// Batch size for vectorized execution
const DEFAULT_BATCH_SIZE: usize = 1024;

/// Row batch for vectorized processing
#[derive(Debug, Clone)]
pub struct RowBatch {
    pub rows: Vec<HashMap<String, Value>>,
    pub schema: QuerySchema,
}

impl RowBatch {
    /// Create a new row batch
    pub fn new(schema: QuerySchema) -> Self {
        Self {
            rows: Vec::with_capacity(DEFAULT_BATCH_SIZE),
            schema,
        }
    }

    /// Create batch with rows
    pub fn with_rows(rows: Vec<HashMap<String, Value>>, schema: QuerySchema) -> Self {
        Self { rows, schema }
    }

    /// Add a row to the batch
    pub fn add_row(&mut self, row: HashMap<String, Value>) {
        self.rows.push(row);
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.rows.len() >= DEFAULT_BATCH_SIZE
    }

    /// Get number of rows
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Clear the batch
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Merge another batch into this one
    pub fn merge(&mut self, other: RowBatch) {
        self.rows.extend(other.rows);
    }
}

/// Execution context for query pipeline
pub struct ExecutionContext {
    /// Memory limit for execution
    pub memory_limit: usize,
    /// Current memory usage
    pub memory_used: usize,
    /// Batch size
    pub batch_size: usize,
    /// Enable query profiling
    pub enable_profiling: bool,
}

impl ExecutionContext {
    /// Create new execution context
    pub fn new() -> Self {
        Self {
            memory_limit: 1024 * 1024 * 1024, // 1GB default
            memory_used: 0,
            batch_size: DEFAULT_BATCH_SIZE,
            enable_profiling: false,
        }
    }

    /// Create with custom memory limit
    pub fn with_memory_limit(memory_limit: usize) -> Self {
        Self {
            memory_limit,
            memory_used: 0,
            batch_size: DEFAULT_BATCH_SIZE,
            enable_profiling: false,
        }
    }

    /// Check if memory limit exceeded
    pub fn check_memory(&self) -> Result<()> {
        if self.memory_used > self.memory_limit {
            Err(ExecutionError::ResourceExhausted(format!(
                "Memory limit exceeded: {} > {}",
                self.memory_used, self.memory_limit
            )))
        } else {
            Ok(())
        }
    }

    /// Allocate memory
    pub fn allocate(&mut self, bytes: usize) -> Result<()> {
        self.memory_used += bytes;
        self.check_memory()
    }

    /// Free memory
    pub fn free(&mut self, bytes: usize) {
        self.memory_used = self.memory_used.saturating_sub(bytes);
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Pipeline executor using Volcano iterator model
pub struct Pipeline {
    plan: PhysicalPlan,
    operators: Vec<Box<dyn Operator>>,
    current_operator: usize,
    context: ExecutionContext,
    finished: bool,
}

impl Pipeline {
    /// Create a new pipeline from physical plan (takes ownership of operators)
    pub fn new(mut plan: PhysicalPlan) -> Self {
        let operators = std::mem::take(&mut plan.operators);
        Self {
            operators,
            plan,
            current_operator: 0,
            context: ExecutionContext::new(),
            finished: false,
        }
    }

    /// Create pipeline with custom context (takes ownership of operators)
    pub fn with_context(mut plan: PhysicalPlan, context: ExecutionContext) -> Self {
        let operators = std::mem::take(&mut plan.operators);
        Self {
            operators,
            plan,
            current_operator: 0,
            context,
            finished: false,
        }
    }

    /// Get next batch from pipeline
    pub fn next(&mut self) -> Result<Option<RowBatch>> {
        if self.finished {
            return Ok(None);
        }

        // Execute pipeline in pull-based fashion
        let result = self.execute_pipeline()?;

        if result.is_none() {
            self.finished = true;
        }

        Ok(result)
    }

    /// Execute the full pipeline
    fn execute_pipeline(&mut self) -> Result<Option<RowBatch>> {
        if self.operators.is_empty() {
            return Ok(None);
        }

        // Start with the first operator (scan)
        let mut current_batch = self.operators[0].execute(None)?;

        // Pipeline the batch through remaining operators
        for operator in &mut self.operators[1..] {
            if let Some(batch) = current_batch {
                current_batch = operator.execute(Some(batch))?;
            } else {
                return Ok(None);
            }
        }

        Ok(current_batch)
    }

    /// Reset pipeline for re-execution
    pub fn reset(&mut self) {
        self.current_operator = 0;
        self.finished = false;
        self.context = ExecutionContext::new();
    }

    /// Get execution context
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    /// Get mutable execution context
    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.context
    }
}

/// Pipeline builder for constructing execution pipelines
pub struct PipelineBuilder {
    operators: Vec<Box<dyn Operator>>,
    context: ExecutionContext,
}

impl PipelineBuilder {
    /// Create a new pipeline builder
    pub fn new() -> Self {
        Self {
            operators: Vec::new(),
            context: ExecutionContext::new(),
        }
    }

    /// Add an operator to the pipeline
    pub fn add_operator(mut self, operator: Box<dyn Operator>) -> Self {
        self.operators.push(operator);
        self
    }

    /// Set execution context
    pub fn with_context(mut self, context: ExecutionContext) -> Self {
        self.context = context;
        self
    }

    /// Build the pipeline
    pub fn build(self) -> Pipeline {
        let plan = PhysicalPlan {
            operators: self.operators,
            pipeline_breakers: Vec::new(),
            parallelism: 1,
        };

        Pipeline::with_context(plan, self.context)
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator adapter for pipeline
pub struct PipelineIterator {
    pipeline: Pipeline,
}

impl PipelineIterator {
    pub fn new(pipeline: Pipeline) -> Self {
        Self { pipeline }
    }
}

impl Iterator for PipelineIterator {
    type Item = Result<RowBatch>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.pipeline.next() {
            Ok(Some(batch)) => Some(Ok(batch)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::plan::ColumnDef;
    use crate::executor::plan::DataType;

    #[test]
    fn test_row_batch() {
        let schema = QuerySchema::new(vec![ColumnDef {
            name: "id".to_string(),
            data_type: DataType::Int64,
            nullable: false,
        }]);

        let mut batch = RowBatch::new(schema);
        assert!(batch.is_empty());

        let mut row = HashMap::new();
        row.insert("id".to_string(), Value::Int64(1));
        batch.add_row(row);

        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_execution_context() {
        let mut ctx = ExecutionContext::new();
        assert_eq!(ctx.memory_used, 0);

        ctx.allocate(1024).unwrap();
        assert_eq!(ctx.memory_used, 1024);

        ctx.free(512);
        assert_eq!(ctx.memory_used, 512);
    }

    #[test]
    fn test_memory_limit() {
        let mut ctx = ExecutionContext::with_memory_limit(1000);
        assert!(ctx.allocate(500).is_ok());
        assert!(ctx.allocate(600).is_err());
    }

    #[test]
    fn test_pipeline_builder() {
        let builder = PipelineBuilder::new();
        let pipeline = builder.build();
        assert_eq!(pipeline.operators.len(), 0);
    }
}
