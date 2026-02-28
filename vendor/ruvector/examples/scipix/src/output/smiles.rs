//! SMILES (Simplified Molecular Input Line Entry System) generator
//!
//! Converts chemical structure representations to SMILES notation.
//! This is a simplified implementation - full chemistry support requires
//! dedicated chemistry libraries like RDKit or OpenBabel.

use super::OcrResult;

/// SMILES notation generator for chemical structures
pub struct SmilesGenerator {
    canonical: bool,
    include_stereochemistry: bool,
}

impl SmilesGenerator {
    pub fn new() -> Self {
        Self {
            canonical: true,
            include_stereochemistry: true,
        }
    }

    pub fn canonical(mut self, canonical: bool) -> Self {
        self.canonical = canonical;
        self
    }

    pub fn stereochemistry(mut self, include: bool) -> Self {
        self.include_stereochemistry = include;
        self
    }

    /// Generate SMILES from OCR result
    pub fn generate_from_result(&self, result: &OcrResult) -> Result<String, String> {
        // Check if SMILES already available
        if let Some(smiles) = &result.formats.smiles {
            return Ok(smiles.clone());
        }

        // Check for chemistry-related content in line data
        if let Some(line_data) = &result.line_data {
            for line in line_data {
                if line.line_type == "chemistry" || line.line_type == "molecule" {
                    return self.parse_chemical_notation(&line.text);
                }
            }
        }

        Err("No chemical structure data found".to_string())
    }

    /// Parse chemical notation to SMILES
    /// This is a placeholder - real implementation needs chemistry parsing
    fn parse_chemical_notation(&self, notation: &str) -> Result<String, String> {
        // Check if already SMILES format
        if self.is_smiles(notation) {
            return Ok(notation.to_string());
        }

        // Try to parse common chemical formulas
        if let Some(smiles) = self.simple_formula_to_smiles(notation) {
            return Ok(smiles);
        }

        Err(format!("Cannot convert '{}' to SMILES", notation))
    }

    /// Check if string is already SMILES notation
    fn is_smiles(&self, s: &str) -> bool {
        // Basic SMILES characters
        let smiles_chars = "CNOPSFClBrI[]()=#@+-0123456789cnops";
        s.chars().all(|c| smiles_chars.contains(c))
    }

    /// Convert simple chemical formulas to SMILES
    fn simple_formula_to_smiles(&self, formula: &str) -> Option<String> {
        // Common chemical formulas
        match formula.trim() {
            "H2O" | "water" => Some("O".to_string()),
            "CO2" | "carbon dioxide" => Some("O=C=O".to_string()),
            "CH4" | "methane" => Some("C".to_string()),
            "C2H6" | "ethane" => Some("CC".to_string()),
            "C2H5OH" | "ethanol" => Some("CCO".to_string()),
            "CH3COOH" | "acetic acid" => Some("CC(=O)O".to_string()),
            "C6H6" | "benzene" => Some("c1ccccc1".to_string()),
            "C6H12O6" | "glucose" => Some("OC[C@H]1OC(O)[C@H](O)[C@@H](O)[C@@H]1O".to_string()),
            "NH3" | "ammonia" => Some("N".to_string()),
            "H2SO4" | "sulfuric acid" => Some("OS(=O)(=O)O".to_string()),
            "NaCl" | "sodium chloride" => Some("[Na+].[Cl-]".to_string()),
            _ => None,
        }
    }

    /// Validate SMILES notation
    pub fn validate(&self, smiles: &str) -> Result<(), String> {
        // Basic validation checks

        // Check parentheses balance
        let mut depth = 0;
        for c in smiles.chars() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err("Unbalanced parentheses".to_string());
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err("Unbalanced parentheses".to_string());
        }

        // Check brackets balance
        let mut depth = 0;
        for c in smiles.chars() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err("Unbalanced brackets".to_string());
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err("Unbalanced brackets".to_string());
        }

        Ok(())
    }

    /// Convert SMILES to molecular formula
    pub fn to_molecular_formula(&self, smiles: &str) -> Result<String, String> {
        self.validate(smiles)?;

        // Simplified formula extraction
        // Real implementation would parse the SMILES properly
        let mut counts: std::collections::HashMap<char, usize> = std::collections::HashMap::new();

        for c in smiles.chars() {
            if c.is_alphabetic() && c.is_uppercase() {
                *counts.entry(c).or_insert(0) += 1;
            }
        }

        let mut formula = String::new();
        // Only use single-character elements for simplicity
        for element in &['C', 'H', 'N', 'O', 'S', 'P', 'F'] {
            if let Some(&count) = counts.get(element) {
                formula.push(*element);
                if count > 1 {
                    formula.push_str(&count.to_string());
                }
            }
        }

        if formula.is_empty() {
            Err("Could not determine molecular formula".to_string())
        } else {
            Ok(formula)
        }
    }

    /// Calculate molecular weight (approximate)
    pub fn molecular_weight(&self, smiles: &str) -> Result<f32, String> {
        self.validate(smiles)?;

        // Simplified atomic weights
        let weights: std::collections::HashMap<char, f32> = [
            ('C', 12.01),
            ('H', 1.008),
            ('N', 14.01),
            ('O', 16.00),
            ('S', 32.07),
            ('P', 30.97),
            ('F', 19.00),
        ]
        .iter()
        .cloned()
        .collect();

        let mut total_weight = 0.0;

        for c in smiles.chars() {
            if let Some(&weight) = weights.get(&c) {
                total_weight += weight;
            }
        }

        Ok(total_weight)
    }
}

impl Default for SmilesGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// SMILES parser for extracting structure information
pub struct SmilesParser;

impl SmilesParser {
    pub fn new() -> Self {
        Self
    }

    /// Count atoms in SMILES notation
    pub fn count_atoms(&self, smiles: &str) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();

        let mut i = 0;
        let chars: Vec<char> = smiles.chars().collect();

        while i < chars.len() {
            if chars[i].is_uppercase() {
                let mut atom = String::from(chars[i]);

                // Check for two-letter atoms (Cl, Br, etc.)
                if i + 1 < chars.len() && chars[i + 1].is_lowercase() {
                    atom.push(chars[i + 1]);
                    i += 1;
                }

                *counts.entry(atom).or_insert(0) += 1;
            }
            i += 1;
        }

        counts
    }

    /// Extract ring information
    pub fn find_rings(&self, smiles: &str) -> Vec<usize> {
        let mut rings = Vec::new();

        for (_i, c) in smiles.chars().enumerate() {
            if c.is_numeric() {
                if let Some(digit) = c.to_digit(10) {
                    rings.push(digit as usize);
                }
            }
        }

        rings
    }
}

impl Default for SmilesParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_smiles() {
        let gen = SmilesGenerator::new();

        assert!(gen.is_smiles("CCO"));
        assert!(gen.is_smiles("c1ccccc1"));
        assert!(gen.is_smiles("CC(=O)O"));
        assert!(!gen.is_smiles("not smiles!"));
    }

    #[test]
    fn test_simple_formula_conversion() {
        let gen = SmilesGenerator::new();

        assert_eq!(gen.simple_formula_to_smiles("H2O"), Some("O".to_string()));
        assert_eq!(
            gen.simple_formula_to_smiles("CO2"),
            Some("O=C=O".to_string())
        );
        assert_eq!(gen.simple_formula_to_smiles("CH4"), Some("C".to_string()));
        assert_eq!(
            gen.simple_formula_to_smiles("benzene"),
            Some("c1ccccc1".to_string())
        );
    }

    #[test]
    fn test_validate_smiles() {
        let gen = SmilesGenerator::new();

        assert!(gen.validate("CCO").is_ok());
        assert!(gen.validate("CC(O)C").is_ok());
        assert!(gen.validate("c1ccccc1").is_ok());

        assert!(gen.validate("CC(O").is_err()); // Unbalanced
        assert!(gen.validate("CC)O").is_err()); // Unbalanced
    }

    #[test]
    fn test_molecular_formula() {
        let gen = SmilesGenerator::new();

        let formula = gen.to_molecular_formula("CCO").unwrap();
        assert!(formula.contains('C'));
        assert!(formula.contains('O'));
    }

    #[test]
    fn test_molecular_weight() {
        let gen = SmilesGenerator::new();

        // Water: H2O (but SMILES is just "O", representing OH2)
        let weight = gen.molecular_weight("O").unwrap();
        assert!(weight > 0.0);

        // Ethanol: C2H6O
        let weight = gen.molecular_weight("CCO").unwrap();
        assert!(weight > 30.0); // Should be around 46
    }

    #[test]
    fn test_count_atoms() {
        let parser = SmilesParser::new();

        let counts = parser.count_atoms("CCO");
        assert_eq!(counts.get("C"), Some(&2));
        assert_eq!(counts.get("O"), Some(&1));

        let counts = parser.count_atoms("CC(=O)O");
        assert_eq!(counts.get("C"), Some(&2));
        assert_eq!(counts.get("O"), Some(&2));
    }

    #[test]
    fn test_find_rings() {
        let parser = SmilesParser::new();

        let rings = parser.find_rings("c1ccccc1");
        assert_eq!(rings, vec![1, 1]);

        let rings = parser.find_rings("C1CC1");
        assert_eq!(rings, vec![1, 1]);
    }
}
