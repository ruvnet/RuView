//! Federation policy for selective sharing.
//!
//! Controls what learning is exported, quality gates, rate limits,
//! and privacy budget constraints.

use std::collections::HashSet;

/// Controls what a user shares in federated exports.
#[derive(Clone, Debug)]
pub struct FederationPolicy {
    /// Segment types allowed for export (empty = all allowed).
    pub allowed_segments: HashSet<u8>,
    /// Segment types explicitly denied for export.
    pub denied_segments: HashSet<u8>,
    /// Domain IDs allowed for export (empty = all allowed).
    pub allowed_domains: HashSet<String>,
    /// Domain IDs denied for export.
    pub denied_domains: HashSet<String>,
    /// Minimum quality score for exported trajectories (0.0 - 1.0).
    pub quality_threshold: f64,
    /// Minimum observations per prior entry for export.
    pub min_observations: u64,
    /// Maximum exports per hour.
    pub max_exports_per_hour: u32,
    /// Maximum cumulative privacy budget (epsilon).
    pub privacy_budget_limit: f64,
    /// Whether to include policy kernel snapshots.
    pub export_kernels: bool,
    /// Whether to include cost curve data.
    pub export_cost_curves: bool,
}

impl Default for FederationPolicy {
    fn default() -> Self {
        Self {
            allowed_segments: HashSet::new(),
            denied_segments: HashSet::new(),
            allowed_domains: HashSet::new(),
            denied_domains: HashSet::new(),
            quality_threshold: 0.5,
            min_observations: 12,
            max_exports_per_hour: 100,
            privacy_budget_limit: 10.0,
            export_kernels: true,
            export_cost_curves: true,
        }
    }
}

impl FederationPolicy {
    /// Create a restrictive policy (deny all by default).
    pub fn restrictive() -> Self {
        Self {
            quality_threshold: 0.8,
            min_observations: 50,
            max_exports_per_hour: 10,
            privacy_budget_limit: 5.0,
            export_kernels: false,
            export_cost_curves: false,
            ..Default::default()
        }
    }

    /// Create a permissive policy (share everything).
    pub fn permissive() -> Self {
        Self {
            quality_threshold: 0.0,
            min_observations: 1,
            max_exports_per_hour: 1000,
            privacy_budget_limit: 100.0,
            export_kernels: true,
            export_cost_curves: true,
            ..Default::default()
        }
    }

    /// Check if a segment type is allowed for export.
    pub fn is_segment_allowed(&self, seg_type: u8) -> bool {
        if self.denied_segments.contains(&seg_type) {
            return false;
        }
        if self.allowed_segments.is_empty() {
            return true;
        }
        self.allowed_segments.contains(&seg_type)
    }

    /// Check if a domain is allowed for export.
    pub fn is_domain_allowed(&self, domain_id: &str) -> bool {
        if self.denied_domains.contains(domain_id) {
            return false;
        }
        if self.allowed_domains.is_empty() {
            return true;
        }
        self.allowed_domains.contains(domain_id)
    }

    /// Allow a specific segment type.
    pub fn allow_segment(mut self, seg_type: u8) -> Self {
        self.allowed_segments.insert(seg_type);
        self
    }

    /// Deny a specific segment type.
    pub fn deny_segment(mut self, seg_type: u8) -> Self {
        self.denied_segments.insert(seg_type);
        self
    }

    /// Allow a specific domain.
    pub fn allow_domain(mut self, domain_id: &str) -> Self {
        self.allowed_domains.insert(domain_id.to_string());
        self
    }

    /// Deny a specific domain.
    pub fn deny_domain(mut self, domain_id: &str) -> Self {
        self.denied_domains.insert(domain_id.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy() {
        let p = FederationPolicy::default();
        assert_eq!(p.quality_threshold, 0.5);
        assert_eq!(p.min_observations, 12);
        assert!(p.is_segment_allowed(0x33));
        assert!(p.is_domain_allowed("anything"));
    }

    #[test]
    fn restrictive_policy() {
        let p = FederationPolicy::restrictive();
        assert_eq!(p.quality_threshold, 0.8);
        assert_eq!(p.min_observations, 50);
        assert!(!p.export_kernels);
        assert!(!p.export_cost_curves);
    }

    #[test]
    fn permissive_policy() {
        let p = FederationPolicy::permissive();
        assert_eq!(p.quality_threshold, 0.0);
        assert_eq!(p.min_observations, 1);
    }

    #[test]
    fn segment_allowlist() {
        let p = FederationPolicy::default().allow_segment(0x33).allow_segment(0x34);
        assert!(p.is_segment_allowed(0x33));
        assert!(p.is_segment_allowed(0x34));
        assert!(!p.is_segment_allowed(0x35)); // not in allowlist
    }

    #[test]
    fn segment_denylist() {
        let p = FederationPolicy::default().deny_segment(0x36);
        assert!(p.is_segment_allowed(0x33));
        assert!(!p.is_segment_allowed(0x36)); // denied
    }

    #[test]
    fn deny_takes_precedence() {
        let p = FederationPolicy::default()
            .allow_segment(0x33)
            .deny_segment(0x33);
        assert!(!p.is_segment_allowed(0x33)); // deny wins
    }

    #[test]
    fn domain_filtering() {
        let p = FederationPolicy::default()
            .allow_domain("genomics")
            .deny_domain("secret_project");
        assert!(p.is_domain_allowed("genomics"));
        assert!(!p.is_domain_allowed("secret_project"));
        assert!(!p.is_domain_allowed("trading")); // not in allowlist
    }

    #[test]
    fn empty_allowlist_allows_all() {
        let p = FederationPolicy::default();
        assert!(p.is_segment_allowed(0x33));
        assert!(p.is_segment_allowed(0xFF));
        assert!(p.is_domain_allowed("any_domain"));
    }
}
