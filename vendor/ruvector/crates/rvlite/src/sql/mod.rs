// SQL query engine module for rvlite
// Provides SQL interface for vector database operations with WASM compatibility

mod ast;
mod executor;
mod parser;

pub use ast::*;
pub use executor::{ExecutionResult, SqlEngine};
pub use parser::{ParseError, SqlParser};

#[cfg(test)]
mod tests;
