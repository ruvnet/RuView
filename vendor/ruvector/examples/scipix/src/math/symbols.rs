//! Mathematical symbol definitions and Unicode to LaTeX mappings
//!
//! This module provides comprehensive mappings between Unicode mathematical
//! symbols and their LaTeX representations, covering Greek letters, operators,
//! relations, arrows, and special symbols.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A mathematical symbol with its properties
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MathSymbol {
    /// Unicode character
    pub unicode: char,
    /// LaTeX command (without backslash)
    pub latex: String,
    /// Symbol category
    pub category: SymbolCategory,
    /// Alternative representations
    pub alternatives: Vec<String>,
}

/// Categories of mathematical symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolCategory {
    /// Greek letters (α, β, γ, etc.)
    Greek,
    /// Binary operators (+, -, ×, etc.)
    Operator,
    /// Relations (=, <, >, etc.)
    Relation,
    /// Arrows (→, ⇒, etc.)
    Arrow,
    /// Delimiters (parentheses, brackets, etc.)
    Delimiter,
    /// Set theory symbols (∈, ⊂, ∪, etc.)
    SetTheory,
    /// Logic symbols (∀, ∃, ∧, etc.)
    Logic,
    /// Calculus symbols (∫, ∂, ∇, etc.)
    Calculus,
    /// Geometry symbols (∠, ⊥, ∥, etc.)
    Geometry,
    /// Miscellaneous symbols
    Misc,
}

/// Global symbol mapping from Unicode to LaTeX
pub static SYMBOL_MAP: Lazy<HashMap<char, MathSymbol>> = Lazy::new(|| {
    let mut map = HashMap::new();

    // Greek lowercase letters
    map.insert(
        'α',
        MathSymbol {
            unicode: 'α',
            latex: "alpha".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'β',
        MathSymbol {
            unicode: 'β',
            latex: "beta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'γ',
        MathSymbol {
            unicode: 'γ',
            latex: "gamma".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'δ',
        MathSymbol {
            unicode: 'δ',
            latex: "delta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'ε',
        MathSymbol {
            unicode: 'ε',
            latex: "epsilon".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["varepsilon".to_string()],
        },
    );
    map.insert(
        'ζ',
        MathSymbol {
            unicode: 'ζ',
            latex: "zeta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'η',
        MathSymbol {
            unicode: 'η',
            latex: "eta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'θ',
        MathSymbol {
            unicode: 'θ',
            latex: "theta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["vartheta".to_string()],
        },
    );
    map.insert(
        'ι',
        MathSymbol {
            unicode: 'ι',
            latex: "iota".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'κ',
        MathSymbol {
            unicode: 'κ',
            latex: "kappa".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'λ',
        MathSymbol {
            unicode: 'λ',
            latex: "lambda".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'μ',
        MathSymbol {
            unicode: 'μ',
            latex: "mu".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'ν',
        MathSymbol {
            unicode: 'ν',
            latex: "nu".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'ξ',
        MathSymbol {
            unicode: 'ξ',
            latex: "xi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'π',
        MathSymbol {
            unicode: 'π',
            latex: "pi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["varpi".to_string()],
        },
    );
    map.insert(
        'ρ',
        MathSymbol {
            unicode: 'ρ',
            latex: "rho".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["varrho".to_string()],
        },
    );
    map.insert(
        'σ',
        MathSymbol {
            unicode: 'σ',
            latex: "sigma".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["varsigma".to_string()],
        },
    );
    map.insert(
        'τ',
        MathSymbol {
            unicode: 'τ',
            latex: "tau".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'υ',
        MathSymbol {
            unicode: 'υ',
            latex: "upsilon".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'φ',
        MathSymbol {
            unicode: 'φ',
            latex: "phi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec!["varphi".to_string()],
        },
    );
    map.insert(
        'χ',
        MathSymbol {
            unicode: 'χ',
            latex: "chi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'ψ',
        MathSymbol {
            unicode: 'ψ',
            latex: "psi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'ω',
        MathSymbol {
            unicode: 'ω',
            latex: "omega".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );

    // Greek uppercase letters
    map.insert(
        'Γ',
        MathSymbol {
            unicode: 'Γ',
            latex: "Gamma".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Δ',
        MathSymbol {
            unicode: 'Δ',
            latex: "Delta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Θ',
        MathSymbol {
            unicode: 'Θ',
            latex: "Theta".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Λ',
        MathSymbol {
            unicode: 'Λ',
            latex: "Lambda".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Ξ',
        MathSymbol {
            unicode: 'Ξ',
            latex: "Xi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Π',
        MathSymbol {
            unicode: 'Π',
            latex: "Pi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Σ',
        MathSymbol {
            unicode: 'Σ',
            latex: "Sigma".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Υ',
        MathSymbol {
            unicode: 'Υ',
            latex: "Upsilon".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Φ',
        MathSymbol {
            unicode: 'Φ',
            latex: "Phi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Ψ',
        MathSymbol {
            unicode: 'Ψ',
            latex: "Psi".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );
    map.insert(
        'Ω',
        MathSymbol {
            unicode: 'Ω',
            latex: "Omega".to_string(),
            category: SymbolCategory::Greek,
            alternatives: vec![],
        },
    );

    // Binary operators
    map.insert(
        '±',
        MathSymbol {
            unicode: '±',
            latex: "pm".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '∓',
        MathSymbol {
            unicode: '∓',
            latex: "mp".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '×',
        MathSymbol {
            unicode: '×',
            latex: "times".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec!["cdot".to_string()],
        },
    );
    map.insert(
        '÷',
        MathSymbol {
            unicode: '÷',
            latex: "div".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '∗',
        MathSymbol {
            unicode: '∗',
            latex: "ast".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '⋆',
        MathSymbol {
            unicode: '⋆',
            latex: "star".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '∘',
        MathSymbol {
            unicode: '∘',
            latex: "circ".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '∙',
        MathSymbol {
            unicode: '∙',
            latex: "bullet".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊕',
        MathSymbol {
            unicode: '⊕',
            latex: "oplus".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊗',
        MathSymbol {
            unicode: '⊗',
            latex: "otimes".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊙',
        MathSymbol {
            unicode: '⊙',
            latex: "odot".to_string(),
            category: SymbolCategory::Operator,
            alternatives: vec![],
        },
    );

    // Relations
    map.insert(
        '=',
        MathSymbol {
            unicode: '=',
            latex: "=".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≠',
        MathSymbol {
            unicode: '≠',
            latex: "neq".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec!["ne".to_string()],
        },
    );
    map.insert(
        '<',
        MathSymbol {
            unicode: '<',
            latex: "<".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '>',
        MathSymbol {
            unicode: '>',
            latex: ">".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≤',
        MathSymbol {
            unicode: '≤',
            latex: "leq".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec!["le".to_string()],
        },
    );
    map.insert(
        '≥',
        MathSymbol {
            unicode: '≥',
            latex: "geq".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec!["ge".to_string()],
        },
    );
    map.insert(
        '≪',
        MathSymbol {
            unicode: '≪',
            latex: "ll".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≫',
        MathSymbol {
            unicode: '≫',
            latex: "gg".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≈',
        MathSymbol {
            unicode: '≈',
            latex: "approx".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≡',
        MathSymbol {
            unicode: '≡',
            latex: "equiv".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '∼',
        MathSymbol {
            unicode: '∼',
            latex: "sim".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '≅',
        MathSymbol {
            unicode: '≅',
            latex: "cong".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '∝',
        MathSymbol {
            unicode: '∝',
            latex: "propto".to_string(),
            category: SymbolCategory::Relation,
            alternatives: vec![],
        },
    );
    map.insert(
        '∈',
        MathSymbol {
            unicode: '∈',
            latex: "in".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '∉',
        MathSymbol {
            unicode: '∉',
            latex: "notin".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊂',
        MathSymbol {
            unicode: '⊂',
            latex: "subset".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊃',
        MathSymbol {
            unicode: '⊃',
            latex: "supset".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊆',
        MathSymbol {
            unicode: '⊆',
            latex: "subseteq".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊇',
        MathSymbol {
            unicode: '⊇',
            latex: "supseteq".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );

    // Set theory
    map.insert(
        '∪',
        MathSymbol {
            unicode: '∪',
            latex: "cup".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '∩',
        MathSymbol {
            unicode: '∩',
            latex: "cap".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        '∅',
        MathSymbol {
            unicode: '∅',
            latex: "emptyset".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec!["varnothing".to_string()],
        },
    );
    map.insert(
        'ℕ',
        MathSymbol {
            unicode: 'ℕ',
            latex: "mathbb{N}".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℤ',
        MathSymbol {
            unicode: 'ℤ',
            latex: "mathbb{Z}".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℚ',
        MathSymbol {
            unicode: 'ℚ',
            latex: "mathbb{Q}".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℝ',
        MathSymbol {
            unicode: 'ℝ',
            latex: "mathbb{R}".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℂ',
        MathSymbol {
            unicode: 'ℂ',
            latex: "mathbb{C}".to_string(),
            category: SymbolCategory::SetTheory,
            alternatives: vec![],
        },
    );

    // Logic
    map.insert(
        '∀',
        MathSymbol {
            unicode: '∀',
            latex: "forall".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec![],
        },
    );
    map.insert(
        '∃',
        MathSymbol {
            unicode: '∃',
            latex: "exists".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec![],
        },
    );
    map.insert(
        '∄',
        MathSymbol {
            unicode: '∄',
            latex: "nexists".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec![],
        },
    );
    map.insert(
        '∧',
        MathSymbol {
            unicode: '∧',
            latex: "land".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec!["wedge".to_string()],
        },
    );
    map.insert(
        '∨',
        MathSymbol {
            unicode: '∨',
            latex: "lor".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec!["vee".to_string()],
        },
    );
    map.insert(
        '¬',
        MathSymbol {
            unicode: '¬',
            latex: "neg".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec!["lnot".to_string()],
        },
    );
    map.insert(
        '⇒',
        MathSymbol {
            unicode: '⇒',
            latex: "Rightarrow".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec!["implies".to_string()],
        },
    );
    map.insert(
        '⇐',
        MathSymbol {
            unicode: '⇐',
            latex: "Leftarrow".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec![],
        },
    );
    map.insert(
        '⇔',
        MathSymbol {
            unicode: '⇔',
            latex: "Leftrightarrow".to_string(),
            category: SymbolCategory::Logic,
            alternatives: vec!["iff".to_string()],
        },
    );

    // Arrows
    map.insert(
        '→',
        MathSymbol {
            unicode: '→',
            latex: "to".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec!["rightarrow".to_string()],
        },
    );
    map.insert(
        '←',
        MathSymbol {
            unicode: '←',
            latex: "leftarrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec!["gets".to_string()],
        },
    );
    map.insert(
        '↔',
        MathSymbol {
            unicode: '↔',
            latex: "leftrightarrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↑',
        MathSymbol {
            unicode: '↑',
            latex: "uparrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↓',
        MathSymbol {
            unicode: '↓',
            latex: "downarrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↗',
        MathSymbol {
            unicode: '↗',
            latex: "nearrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↘',
        MathSymbol {
            unicode: '↘',
            latex: "searrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↙',
        MathSymbol {
            unicode: '↙',
            latex: "swarrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↖',
        MathSymbol {
            unicode: '↖',
            latex: "nwarrow".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );
    map.insert(
        '↦',
        MathSymbol {
            unicode: '↦',
            latex: "mapsto".to_string(),
            category: SymbolCategory::Arrow,
            alternatives: vec![],
        },
    );

    // Calculus
    map.insert(
        '∫',
        MathSymbol {
            unicode: '∫',
            latex: "int".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∬',
        MathSymbol {
            unicode: '∬',
            latex: "iint".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∭',
        MathSymbol {
            unicode: '∭',
            latex: "iiint".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∮',
        MathSymbol {
            unicode: '∮',
            latex: "oint".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∂',
        MathSymbol {
            unicode: '∂',
            latex: "partial".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∇',
        MathSymbol {
            unicode: '∇',
            latex: "nabla".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∑',
        MathSymbol {
            unicode: '∑',
            latex: "sum".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∏',
        MathSymbol {
            unicode: '∏',
            latex: "prod".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );
    map.insert(
        '∐',
        MathSymbol {
            unicode: '∐',
            latex: "coprod".to_string(),
            category: SymbolCategory::Calculus,
            alternatives: vec![],
        },
    );

    // Geometry
    map.insert(
        '∠',
        MathSymbol {
            unicode: '∠',
            latex: "angle".to_string(),
            category: SymbolCategory::Geometry,
            alternatives: vec![],
        },
    );
    map.insert(
        '∡',
        MathSymbol {
            unicode: '∡',
            latex: "measuredangle".to_string(),
            category: SymbolCategory::Geometry,
            alternatives: vec![],
        },
    );
    map.insert(
        '⊥',
        MathSymbol {
            unicode: '⊥',
            latex: "perp".to_string(),
            category: SymbolCategory::Geometry,
            alternatives: vec![],
        },
    );
    map.insert(
        '∥',
        MathSymbol {
            unicode: '∥',
            latex: "parallel".to_string(),
            category: SymbolCategory::Geometry,
            alternatives: vec![],
        },
    );
    map.insert(
        '△',
        MathSymbol {
            unicode: '△',
            latex: "triangle".to_string(),
            category: SymbolCategory::Geometry,
            alternatives: vec![],
        },
    );

    // Miscellaneous
    map.insert(
        '∞',
        MathSymbol {
            unicode: '∞',
            latex: "infty".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℓ',
        MathSymbol {
            unicode: 'ℓ',
            latex: "ell".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℏ',
        MathSymbol {
            unicode: 'ℏ',
            latex: "hbar".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '℘',
        MathSymbol {
            unicode: '℘',
            latex: "wp".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℜ',
        MathSymbol {
            unicode: 'ℜ',
            latex: "Re".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        'ℑ',
        MathSymbol {
            unicode: 'ℑ',
            latex: "Im".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '√',
        MathSymbol {
            unicode: '√',
            latex: "sqrt".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '∛',
        MathSymbol {
            unicode: '∛',
            latex: "sqrt[3]".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '∜',
        MathSymbol {
            unicode: '∜',
            latex: "sqrt[4]".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '†',
        MathSymbol {
            unicode: '†',
            latex: "dagger".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '‡',
        MathSymbol {
            unicode: '‡',
            latex: "ddagger".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '…',
        MathSymbol {
            unicode: '…',
            latex: "ldots".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec!["dots".to_string()],
        },
    );
    map.insert(
        '⋮',
        MathSymbol {
            unicode: '⋮',
            latex: "vdots".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '⋯',
        MathSymbol {
            unicode: '⋯',
            latex: "cdots".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );
    map.insert(
        '⋱',
        MathSymbol {
            unicode: '⋱',
            latex: "ddots".to_string(),
            category: SymbolCategory::Misc,
            alternatives: vec![],
        },
    );

    map
});

/// Get LaTeX representation for a Unicode character
pub fn unicode_to_latex(c: char) -> Option<&'static str> {
    SYMBOL_MAP.get(&c).map(|s| s.latex.as_str())
}

/// Get symbol by Unicode character
pub fn get_symbol(c: char) -> Option<&'static MathSymbol> {
    SYMBOL_MAP.get(&c)
}

/// Get all symbols in a category
pub fn symbols_by_category(category: SymbolCategory) -> Vec<&'static MathSymbol> {
    SYMBOL_MAP
        .values()
        .filter(|s| s.category == category)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greek_letters() {
        assert_eq!(unicode_to_latex('α'), Some("alpha"));
        assert_eq!(unicode_to_latex('β'), Some("beta"));
        assert_eq!(unicode_to_latex('Γ'), Some("Gamma"));
        assert_eq!(unicode_to_latex('Δ'), Some("Delta"));
    }

    #[test]
    fn test_operators() {
        assert_eq!(unicode_to_latex('±'), Some("pm"));
        assert_eq!(unicode_to_latex('×'), Some("times"));
        assert_eq!(unicode_to_latex('÷'), Some("div"));
    }

    #[test]
    fn test_relations() {
        assert_eq!(unicode_to_latex('≠'), Some("neq"));
        assert_eq!(unicode_to_latex('≤'), Some("leq"));
        assert_eq!(unicode_to_latex('≥'), Some("geq"));
        assert_eq!(unicode_to_latex('≈'), Some("approx"));
    }

    #[test]
    fn test_set_theory() {
        assert_eq!(unicode_to_latex('∈'), Some("in"));
        assert_eq!(unicode_to_latex('∪'), Some("cup"));
        assert_eq!(unicode_to_latex('∩'), Some("cap"));
        assert_eq!(unicode_to_latex('∅'), Some("emptyset"));
    }

    #[test]
    fn test_calculus() {
        assert_eq!(unicode_to_latex('∫'), Some("int"));
        assert_eq!(unicode_to_latex('∂'), Some("partial"));
        assert_eq!(unicode_to_latex('∇'), Some("nabla"));
        assert_eq!(unicode_to_latex('∑'), Some("sum"));
    }

    #[test]
    fn test_arrows() {
        assert_eq!(unicode_to_latex('→'), Some("to"));
        assert_eq!(unicode_to_latex('←'), Some("leftarrow"));
        assert_eq!(unicode_to_latex('⇒'), Some("Rightarrow"));
    }

    #[test]
    fn test_symbol_category() {
        let greek_symbols = symbols_by_category(SymbolCategory::Greek);
        assert!(!greek_symbols.is_empty());
        assert!(greek_symbols.iter().any(|s| s.unicode == 'α'));

        let calc_symbols = symbols_by_category(SymbolCategory::Calculus);
        assert!(calc_symbols.iter().any(|s| s.unicode == '∫'));
    }

    #[test]
    fn test_get_symbol() {
        let sym = get_symbol('π').unwrap();
        assert_eq!(sym.latex, "pi");
        assert_eq!(sym.category, SymbolCategory::Greek);
        assert!(sym.alternatives.contains(&"varpi".to_string()));
    }

    #[test]
    fn test_symbol_map_count() {
        // Ensure we have a substantial number of symbols
        assert!(SYMBOL_MAP.len() > 100);
    }
}
