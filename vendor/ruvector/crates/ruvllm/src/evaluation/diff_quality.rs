//! Diff Quality Metrics - Layer 2
//!
//! Measures whether patches behave like a senior engineer:
//! - Minimality (lines/files changed, mechanical vs semantic)
//! - Locality (edits near relevant modules)
//! - Edit similarity (compared to reference patch)
//! - Review burden (lint failures, formatting churn, followups)

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Minimality metrics for a diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Minimality {
    /// Total lines added
    pub lines_added: usize,
    /// Total lines removed
    pub lines_removed: usize,
    /// Total lines changed (added + removed)
    pub total_lines: usize,
    /// Number of files modified
    pub files_modified: usize,
    /// Number of files added
    pub files_added: usize,
    /// Number of files deleted
    pub files_deleted: usize,
    /// Estimated mechanical changes (formatting, imports, renames)
    pub mechanical_changes: usize,
    /// Estimated semantic changes (logic, behavior)
    pub semantic_changes: usize,
    /// Ratio of mechanical to total changes (0.0 to 1.0)
    pub mechanical_ratio: f64,
}

impl Minimality {
    /// Calculate minimality score (lower is better, 0.0 to 1.0 normalized)
    /// Penalizes large diffs and high mechanical ratios
    pub fn score(&self) -> f64 {
        // Ideal: small diff, low mechanical ratio
        // Score decreases with more lines and higher mechanical ratio
        let line_penalty = (self.total_lines as f64 / 100.0).min(1.0);
        let file_penalty = (self.files_modified as f64 / 10.0).min(1.0);
        let mechanical_penalty = self.mechanical_ratio;

        // Weighted combination (lower is better)
        let raw_score = 0.5 * line_penalty + 0.3 * file_penalty + 0.2 * mechanical_penalty;

        // Invert so higher is better
        1.0 - raw_score.min(1.0)
    }

    /// Check if diff is considered minimal (heuristic thresholds)
    pub fn is_minimal(&self) -> bool {
        self.total_lines < 50 && self.files_modified <= 3 && self.mechanical_ratio < 0.3
    }
}

/// Locality metrics - how scattered are the edits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditLocality {
    /// Files that were edited
    pub edited_files: Vec<String>,
    /// Modules/directories touched
    pub modules_touched: HashSet<String>,
    /// Distance from "center" of relevant code (0.0 = perfect locality)
    pub scatter_score: f64,
    /// Fraction of edits in the primary module
    pub primary_module_fraction: f64,
    /// Number of distinct directories touched
    pub directories_touched: usize,
    /// Whether edits cross package/crate boundaries
    pub crosses_boundaries: bool,
}

impl EditLocality {
    /// Calculate locality score (higher is better, 0.0 to 1.0)
    pub fn score(&self) -> f64 {
        // Penalize scatter and boundary crossing
        let scatter_penalty = self.scatter_score.min(1.0);
        let boundary_penalty = if self.crosses_boundaries { 0.2 } else { 0.0 };
        let concentration_bonus = self.primary_module_fraction;

        // Combine: high concentration and low scatter is good
        (concentration_bonus - scatter_penalty - boundary_penalty)
            .max(0.0)
            .min(1.0)
    }

    /// Check if edits are well-localized
    pub fn is_localized(&self) -> bool {
        !self.crosses_boundaries && self.primary_module_fraction > 0.7 && self.scatter_score < 0.3
    }
}

/// Review burden metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewBurden {
    /// Number of lint errors introduced
    pub lint_errors: usize,
    /// Number of lint warnings introduced
    pub lint_warnings: usize,
    /// Number of type errors
    pub type_errors: usize,
    /// Number of formatting issues (would be fixed by formatter)
    pub format_issues: usize,
    /// Number of followup patches needed to converge
    pub followup_patches: usize,
    /// Estimated review time in minutes (heuristic)
    pub estimated_review_minutes: f64,
    /// Complexity score of the diff (cyclomatic complexity delta)
    pub complexity_delta: i32,
    /// Number of new dependencies introduced
    pub new_dependencies: usize,
}

impl ReviewBurden {
    /// Calculate review burden score (lower is better, 0.0 to 1.0)
    pub fn score(&self) -> f64 {
        let error_penalty = (self.lint_errors + self.type_errors) as f64 * 0.1;
        let warning_penalty = self.lint_warnings as f64 * 0.02;
        let followup_penalty = self.followup_patches as f64 * 0.15;
        let complexity_penalty = (self.complexity_delta.max(0) as f64) * 0.05;

        let raw_burden = error_penalty + warning_penalty + followup_penalty + complexity_penalty;

        // Normalize to 0-1 (lower is better for burden, so invert for score)
        1.0 - raw_burden.min(1.0)
    }

    /// Check if review burden is acceptable
    pub fn is_acceptable(&self) -> bool {
        self.lint_errors == 0
            && self.type_errors == 0
            && self.followup_patches <= 1
            && self.format_issues == 0
    }
}

/// Aggregated diff quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffQualityMetrics {
    /// Minimality score (0.0 to 1.0, higher is better)
    pub minimality_score: f64,
    /// Locality score (0.0 to 1.0, higher is better)
    pub locality_score: f64,
    /// Edit similarity to reference (0.0 to 1.0, if reference available)
    pub edit_similarity: Option<f64>,
    /// Review burden score (0.0 to 1.0, higher means lower burden)
    pub review_burden_score: f64,
    /// Combined quality score
    pub combined_score: f64,
    /// Raw minimality data
    pub minimality: Minimality,
    /// Raw locality data
    pub locality: EditLocality,
    /// Raw review burden data
    pub review_burden: ReviewBurden,
}

impl DiffQualityMetrics {
    /// Calculate combined quality score
    pub fn calculate_combined(&mut self) {
        // Weighted combination of all scores
        let sim_score = self.edit_similarity.unwrap_or(0.5);
        self.combined_score = 0.3 * self.minimality_score
            + 0.25 * self.locality_score
            + 0.25 * sim_score
            + 0.2 * self.review_burden_score;
    }

    /// Check if diff meets quality bar
    pub fn meets_quality_bar(&self, threshold: f64) -> bool {
        self.combined_score >= threshold
    }
}

/// Diff analyzer that computes quality metrics
pub struct DiffAnalyzer {
    /// Known mechanical patterns (regex patterns for imports, formatting, etc.)
    mechanical_patterns: Vec<String>,
    /// Module boundary markers
    boundary_markers: Vec<String>,
}

impl Default for DiffAnalyzer {
    fn default() -> Self {
        Self {
            mechanical_patterns: vec![
                r"^[-+]\s*use\s+".to_string(), // import changes
                r"^[-+]\s*$".to_string(),      // blank line changes
                r"^[-+]\s*//".to_string(),     // comment changes
                r"^[-+]\s*#\[".to_string(),    // attribute changes
            ],
            boundary_markers: vec![
                "Cargo.toml".to_string(),
                "package.json".to_string(),
                "go.mod".to_string(),
            ],
        }
    }
}

impl DiffAnalyzer {
    /// Analyze a unified diff and compute quality metrics
    pub fn analyze(&self, diff: &str, reference_diff: Option<&str>) -> DiffQualityMetrics {
        let minimality = self.analyze_minimality(diff);
        let locality = self.analyze_locality(diff);
        let review_burden = self.analyze_review_burden(diff);
        let edit_similarity = reference_diff.map(|r| self.compute_edit_similarity(diff, r));

        let mut metrics = DiffQualityMetrics {
            minimality_score: minimality.score(),
            locality_score: locality.score(),
            edit_similarity,
            review_burden_score: review_burden.score(),
            combined_score: 0.0,
            minimality,
            locality,
            review_burden,
        };

        metrics.calculate_combined();
        metrics
    }

    /// Analyze minimality of a diff
    fn analyze_minimality(&self, diff: &str) -> Minimality {
        let mut lines_added = 0;
        let mut lines_removed = 0;
        let mut mechanical = 0;
        let mut files: HashSet<String> = HashSet::new();
        let mut current_file = String::new();

        for line in diff.lines() {
            // Track files
            if line.starts_with("+++ ") || line.starts_with("--- ") {
                if line.len() > 4 {
                    current_file = line[4..].trim_start_matches("b/").to_string();
                    files.insert(current_file.clone());
                }
                continue;
            }

            // Count changes
            if line.starts_with('+') && !line.starts_with("+++") {
                lines_added += 1;
                if self.is_mechanical_change(line) {
                    mechanical += 1;
                }
            } else if line.starts_with('-') && !line.starts_with("---") {
                lines_removed += 1;
                if self.is_mechanical_change(line) {
                    mechanical += 1;
                }
            }
        }

        let total = lines_added + lines_removed;
        let mechanical_ratio = if total > 0 {
            mechanical as f64 / total as f64
        } else {
            0.0
        };

        Minimality {
            lines_added,
            lines_removed,
            total_lines: total,
            files_modified: files.len(),
            files_added: 0, // Would need git status
            files_deleted: 0,
            mechanical_changes: mechanical,
            semantic_changes: total.saturating_sub(mechanical),
            mechanical_ratio,
        }
    }

    /// Check if a line change is mechanical
    fn is_mechanical_change(&self, line: &str) -> bool {
        for pattern in &self.mechanical_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(line) {
                    return true;
                }
            }
        }
        false
    }

    /// Analyze locality of edits
    fn analyze_locality(&self, diff: &str) -> EditLocality {
        let mut files: Vec<String> = Vec::new();
        let mut modules: HashSet<String> = HashSet::new();
        let mut directories: HashSet<String> = HashSet::new();
        let mut crosses_boundaries = false;

        for line in diff.lines() {
            if line.starts_with("+++ ") && line.len() > 4 {
                let file = line[4..].trim_start_matches("b/").to_string();
                files.push(file.clone());

                // Extract directory/module
                if let Some(dir) = file.rsplit('/').nth(1) {
                    directories.insert(dir.to_string());
                }
                if let Some(module) = file.split('/').next() {
                    modules.insert(module.to_string());
                }

                // Check boundary crossing
                for marker in &self.boundary_markers {
                    if file.ends_with(marker) {
                        crosses_boundaries = true;
                    }
                }
            }
        }

        // Calculate scatter score based on module spread
        let scatter_score = if modules.len() <= 1 {
            0.0
        } else {
            (modules.len() as f64 - 1.0) / 5.0 // Normalize
        };

        // Calculate primary module fraction
        let primary_fraction = if files.is_empty() {
            1.0
        } else {
            // Count files in most common module
            let mut module_counts: HashMap<String, usize> = HashMap::new();
            for file in &files {
                if let Some(module) = file.split('/').next() {
                    *module_counts.entry(module.to_string()).or_insert(0) += 1;
                }
            }
            let max_count = module_counts.values().max().copied().unwrap_or(0);
            max_count as f64 / files.len() as f64
        };

        EditLocality {
            edited_files: files,
            modules_touched: modules,
            scatter_score: scatter_score.min(1.0),
            primary_module_fraction: primary_fraction,
            directories_touched: directories.len(),
            crosses_boundaries,
        }
    }

    /// Analyze review burden
    fn analyze_review_burden(&self, _diff: &str) -> ReviewBurden {
        // In a real implementation, this would:
        // 1. Run linters on the patched code
        // 2. Run type checker
        // 3. Run formatter in check mode
        // 4. Compute complexity metrics

        ReviewBurden {
            lint_errors: 0,
            lint_warnings: 0,
            type_errors: 0,
            format_issues: 0,
            followup_patches: 0,
            estimated_review_minutes: 0.0,
            complexity_delta: 0,
            new_dependencies: 0,
        }
    }

    /// Compute edit similarity between two diffs (0.0 to 1.0)
    /// Uses a simplified Jaccard-like similarity on changed lines
    pub fn compute_edit_similarity(&self, diff1: &str, diff2: &str) -> f64 {
        let lines1: HashSet<&str> = diff1
            .lines()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .filter(|l| !l.starts_with("+++") && !l.starts_with("---"))
            .collect();

        let lines2: HashSet<&str> = diff2
            .lines()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .filter(|l| !l.starts_with("+++") && !l.starts_with("---"))
            .collect();

        if lines1.is_empty() && lines2.is_empty() {
            return 1.0;
        }

        let intersection = lines1.intersection(&lines2).count();
        let union = lines1.union(&lines2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimality_score() {
        let minimal = Minimality {
            lines_added: 5,
            lines_removed: 3,
            total_lines: 8,
            files_modified: 1,
            files_added: 0,
            files_deleted: 0,
            mechanical_changes: 1,
            semantic_changes: 7,
            mechanical_ratio: 0.125,
        };

        assert!(minimal.is_minimal());
        assert!(minimal.score() > 0.8);
    }

    #[test]
    fn test_locality_score() {
        let localized = EditLocality {
            edited_files: vec!["src/lib.rs".into(), "src/utils.rs".into()],
            modules_touched: HashSet::from(["src".into()]),
            scatter_score: 0.0,
            primary_module_fraction: 1.0,
            directories_touched: 1,
            crosses_boundaries: false,
        };

        assert!(localized.is_localized());
        assert!(localized.score() > 0.8);
    }

    #[test]
    fn test_edit_similarity() {
        let analyzer = DiffAnalyzer::default();

        let diff1 = "+line1\n+line2\n-line3";
        let diff2 = "+line1\n+line2\n-line3";

        assert_eq!(analyzer.compute_edit_similarity(diff1, diff2), 1.0);

        let diff3 = "+line1\n+different\n-other";
        assert!(analyzer.compute_edit_similarity(diff1, diff3) < 1.0);
    }

    #[test]
    fn test_review_burden_acceptable() {
        let acceptable = ReviewBurden {
            lint_errors: 0,
            lint_warnings: 2,
            type_errors: 0,
            format_issues: 0,
            followup_patches: 0,
            estimated_review_minutes: 5.0,
            complexity_delta: 2,
            new_dependencies: 0,
        };

        assert!(acceptable.is_acceptable());
        assert!(acceptable.score() > 0.8);
    }
}
