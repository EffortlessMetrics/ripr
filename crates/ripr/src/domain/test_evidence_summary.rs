//! Portable test-evidence summary projected from the analyzer's internal
//! `TestGripEvidence`. Designed for consumption by cargo-allow's
//! requirement-relevance checks (#1658/#1679) and the LSP's
//! `FixInstructionSummary` (#1752).
//!
//! This is a RIPR-internal projection — the portable V1 JSON schema
//! (`TestGripSummaryV1`) waits on cargo-allow #2191 to freeze the
//! cross-tool contract. See #1783.

/// A per-related-test evidence summary entry.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestEvidenceEntry {
    /// The test function name.
    pub test_name: String,
    /// The test file path (forward-slash normalized).
    pub file: String,
    /// The test source line.
    pub line: usize,
    /// The oracle kind (exact_value, broad_error, etc.).
    pub oracle_kind: String,
    /// The oracle strength (strong, weak, etc.).
    pub oracle_strength: String,
    /// The relation reason (direct_owner_call, token_coincidence, etc.).
    pub relation_reason: String,
    /// True when a producer-owned test target identity exists.
    pub has_test_target: bool,
}

/// A summary of test-grip evidence for one seam, projected from the
/// analyzer's internal types. This is derived, not authoritative — the
/// internal `TestGripEvidence` remains the source of truth.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestEvidenceSummary {
    /// The seam identifier.
    pub seam_id: String,
    /// The related-test evidence entries, strongest first.
    pub related_tests: Vec<TestEvidenceEntry>,
    /// Count of missing discriminators.
    pub missing_discriminator_count: usize,
    /// The strongest oracle strength among related tests, or "none".
    pub strongest_oracle: String,
    /// A semantic fingerprint that changes when assertion strength changes
    /// (exact→broad) but is stable under formatting/root/order changes.
    pub fingerprint: String,
}

impl TestEvidenceSummary {
    /// Compute the semantic fingerprint. The fingerprint captures the
    /// *strength profile* of the related tests: the sorted set of
    /// (oracle_kind, oracle_strength, relation_reason) tuples. This
    /// changes when a test's assertion narrows or broadens, but is
    /// stable when test names, file paths, or ordering change.
    pub fn compute_fingerprint(entries: &[TestEvidenceEntry]) -> String {
        let mut profile: Vec<(String, String, String)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.oracle_kind.clone(),
                    entry.oracle_strength.clone(),
                    entry.relation_reason.clone(),
                )
            })
            .collect();
        profile.sort();
        let joined = profile
            .iter()
            .map(|(kind, strength, reason)| format!("{kind}:{strength}:{reason}"))
            .collect::<Vec<_>>()
            .join(";");
        format!("fp:{joined}")
    }

    /// Derive the strongest oracle strength from a list of entries.
    pub fn strongest_oracle_from(entries: &[TestEvidenceEntry]) -> String {
        entries
            .iter()
            .filter_map(|entry| {
                let strength = entry.oracle_strength.as_str();
                match strength {
                    "strong" => Some(5),
                    "medium" => Some(4),
                    "weak" => Some(3),
                    "smoke" => Some(2),
                    "none" => Some(1),
                    _ => None,
                }
            })
            .max()
            .map(|rank| match rank {
                5 => "strong",
                4 => "medium",
                3 => "weak",
                2 => "smoke",
                1 => "none",
                _ => "unknown",
            })
            .unwrap_or("none")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, strength: &str, reason: &str) -> TestEvidenceEntry {
        TestEvidenceEntry {
            test_name: "test_foo".to_string(),
            file: "src/lib.rs".to_string(),
            line: 10,
            oracle_kind: kind.to_string(),
            oracle_strength: strength.to_string(),
            relation_reason: reason.to_string(),
            has_test_target: true,
        }
    }

    #[test]
    fn fingerprint_stable_under_reordering() {
        let a = vec![
            entry("exact_value", "strong", "direct_owner_call"),
            entry("broad_error", "weak", "same_test_file"),
        ];
        let b = vec![
            entry("broad_error", "weak", "same_test_file"),
            entry("exact_value", "strong", "direct_owner_call"),
        ];
        assert_eq!(
            TestEvidenceSummary::compute_fingerprint(&a),
            TestEvidenceSummary::compute_fingerprint(&b),
        );
    }

    #[test]
    fn fingerprint_changes_when_assertion_narrows() {
        let broad = vec![entry("broad_error", "weak", "direct_owner_call")];
        let exact = vec![entry("exact_value", "strong", "direct_owner_call")];
        assert_ne!(
            TestEvidenceSummary::compute_fingerprint(&broad),
            TestEvidenceSummary::compute_fingerprint(&exact),
        );
    }

    #[test]
    fn fingerprint_stable_under_name_change() {
        let a = vec![entry("exact_value", "strong", "direct_owner_call")];
        let mut b = entry("exact_value", "strong", "direct_owner_call");
        b.test_name = "different_name".to_string();
        assert_eq!(
            TestEvidenceSummary::compute_fingerprint(&a),
            TestEvidenceSummary::compute_fingerprint(&[b]),
        );
    }

    #[test]
    fn strongest_oracle_picks_highest_rank() {
        let entries = vec![
            entry("broad_error", "weak", "direct_owner_call"),
            entry("exact_value", "strong", "direct_owner_call"),
            entry("smoke_only", "smoke", "same_test_file"),
        ];
        assert_eq!(
            TestEvidenceSummary::strongest_oracle_from(&entries),
            "strong"
        );
    }

    #[test]
    fn strongest_oracle_none_when_empty() {
        assert_eq!(TestEvidenceSummary::strongest_oracle_from(&[]), "none");
    }

    #[test]
    fn empty_entries_produce_empty_fingerprint() {
        let fp = TestEvidenceSummary::compute_fingerprint(&[]);
        assert_eq!(fp, "fp:");
    }
}
