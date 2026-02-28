//! Cypher query language parser and execution engine
//!
//! This module provides a complete Cypher query language implementation including:
//! - Lexical analysis (tokenization)
//! - Syntax parsing (AST generation)
//! - Semantic analysis and type checking
//! - Query optimization
//! - Support for hyperedges (N-ary relationships)

pub mod ast;
pub mod lexer;
pub mod optimizer;
pub mod parser;
pub mod semantic;

pub use ast::{Query, Statement};
pub use lexer::{Token, TokenKind};
pub use optimizer::{OptimizationPlan, QueryOptimizer};
pub use parser::{parse_cypher, ParseError};
pub use semantic::{SemanticAnalyzer, SemanticError};
