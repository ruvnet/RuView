//! Pre-built invariant library.
//!
//! Registers RuVector's core type declarations into a lean-agentic
//! proof environment so that verification functions can reference them.

/// Well-known symbol names used throughout the verification layer.
pub mod symbols {
    pub const NAT: &str = "Nat";
    pub const RUVEC: &str = "RuVec";
    pub const EQ: &str = "Eq";
    pub const EQ_REFL: &str = "Eq.refl";
    pub const DISTANCE_METRIC: &str = "DistanceMetric";
    pub const L2: &str = "DistanceMetric.L2";
    pub const COSINE: &str = "DistanceMetric.Cosine";
    pub const DOT: &str = "DistanceMetric.Dot";
    pub const HNSW_INDEX: &str = "HnswIndex";
    pub const INSERT_RESULT: &str = "InsertResult";
    pub const PIPELINE_STAGE: &str = "PipelineStage";
    pub const TYPE_UNIVERSE: &str = "Type";
}

/// Pre-registered type declarations available after calling `register_builtins`.
///
/// These mirror the RuVector domain:
/// - `Nat` : Type (natural numbers for dimensions)
/// - `RuVec` : Nat -> Type (dimension-indexed vectors)
/// - `Eq` : {A : Type} -> A -> A -> Type (propositional equality)
/// - `Eq.refl` : {A : Type} -> (a : A) -> Eq a a (reflexivity proof)
/// - `DistanceMetric` : Type (L2, Cosine, Dot)
/// - `HnswIndex` : Nat -> DistanceMetric -> Type
/// - `InsertResult` : Type
/// - `PipelineStage` : Type -> Type -> Type
pub fn builtin_declarations() -> Vec<BuiltinDecl> {
    vec![
        BuiltinDecl {
            name: symbols::NAT,
            arity: 0,
            doc: "Natural numbers",
        },
        BuiltinDecl {
            name: symbols::RUVEC,
            arity: 1,
            doc: "Dimension-indexed vector",
        },
        BuiltinDecl {
            name: symbols::EQ,
            arity: 2,
            doc: "Propositional equality",
        },
        BuiltinDecl {
            name: symbols::EQ_REFL,
            arity: 1,
            doc: "Reflexivity proof",
        },
        BuiltinDecl {
            name: symbols::DISTANCE_METRIC,
            arity: 0,
            doc: "Distance metric enum",
        },
        BuiltinDecl {
            name: symbols::L2,
            arity: 0,
            doc: "L2 Euclidean distance",
        },
        BuiltinDecl {
            name: symbols::COSINE,
            arity: 0,
            doc: "Cosine distance",
        },
        BuiltinDecl {
            name: symbols::DOT,
            arity: 0,
            doc: "Dot product distance",
        },
        BuiltinDecl {
            name: symbols::HNSW_INDEX,
            arity: 2,
            doc: "HNSW index type",
        },
        BuiltinDecl {
            name: symbols::INSERT_RESULT,
            arity: 0,
            doc: "Insert result type",
        },
        BuiltinDecl {
            name: symbols::PIPELINE_STAGE,
            arity: 2,
            doc: "Typed pipeline stage",
        },
    ]
}

/// A built-in type declaration to register in the proof environment.
#[derive(Debug, Clone)]
pub struct BuiltinDecl {
    /// Symbol name.
    pub name: &'static str,
    /// Number of type parameters.
    pub arity: u32,
    /// Documentation.
    pub doc: &'static str,
}

/// Register all built-in RuVector types into the proof environment's symbol table.
///
/// This is called once during `ProofEnvironment::new()` to make domain types
/// available for proof construction.
pub fn register_builtin_symbols(symbols: &mut Vec<String>) {
    for decl in builtin_declarations() {
        if !symbols.contains(&decl.name.to_string()) {
            symbols.push(decl.name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_declarations_complete() {
        let decls = builtin_declarations();
        assert!(
            decls.len() >= 11,
            "expected at least 11 builtins, got {}",
            decls.len()
        );
    }

    #[test]
    fn all_builtins_have_names() {
        for decl in builtin_declarations() {
            assert!(!decl.name.is_empty());
            assert!(!decl.doc.is_empty());
        }
    }

    #[test]
    fn register_symbols_no_duplicates() {
        let mut syms = vec!["Nat".to_string()]; // pre-existing
        register_builtin_symbols(&mut syms);
        let nat_count = syms.iter().filter(|s| *s == "Nat").count();
        assert_eq!(nat_count, 1, "Nat should not be duplicated");
    }

    #[test]
    fn symbol_constants_valid() {
        assert_eq!(symbols::NAT, "Nat");
        assert_eq!(symbols::RUVEC, "RuVec");
        assert_eq!(symbols::EQ_REFL, "Eq.refl");
    }
}
