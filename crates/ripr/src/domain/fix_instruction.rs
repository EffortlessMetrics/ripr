//! A closed, producer-backed fix-instruction state derived from the
//! [`DiagnosticWitness`]. Every surface (diagnostics, hover, code actions,
//! repair packets) projects from this single summary so that the
//! fix-readiness vocabulary is consistent and never re-inferred per surface.
//!
//! See #1663 / #1752. This is NOT a second repair-readiness authority —
//! [`crate::analysis::repair_route::RepairRouteReadiness`] is the route
//! readiness gate; this type is the projection of witness availability.

use super::DiagnosticWitness;

/// The closed 5-state fix-instruction vocabulary. Each state is derived
/// from existing producer-owned witness fields; no field is fabricated.
///
/// States not yet derivable (`exact_edit_ready`, `exact_assertion_ready`,
/// `verification_ready`) are reserved for when #1658/#1660/#1571 land;
/// they are intentionally absent from the current derivation.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum FixInstructionState {
    /// A producer-owned fix site (file/line/test) is available, but no
    /// exact assertion or edit is ready. The user can navigate to the
    /// test and inspect the current oracle.
    FixSiteReady,
    /// The diagnostic is a static limitation — the producer could not
    /// resolve a route. The user can inspect the evidence but no bounded
    /// repair instruction is available.
    StaticLimitation,
    /// The snapshot is stale — the analysis inputs have changed since the
    /// diagnostic was produced. The user should refresh before acting.
    Stale,
    /// The diagnostic is informational only (no fix site, no limitation,
    /// no actionable route). The user can inspect but there is nothing
    /// to fix.
    InspectOnly,
    /// No producer-owned witness is available for this diagnostic at all.
    Unavailable,
}

/// A bounded fix-instruction summary projected from the diagnostic witness.
/// Not a second repair-readiness authority — derived, not authoritative.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FixInstructionSummary {
    pub state: FixInstructionState,
    /// True when a producer-owned fix site (file/line/test) exists.
    pub has_fix_site: bool,
    /// True when a producer-owned suggested assertion exists.
    pub has_suggested_assertion: bool,
    /// The named limitation kinds from the witness, if any.
    pub limitation_kinds: Vec<String>,
}

impl FixInstructionSummary {
    /// Derive the summary from a [`DiagnosticWitness`]. The derivation is
    /// conservative: when in doubt, prefer a more limited state.
    pub fn from_witness(witness: &DiagnosticWitness) -> Self {
        let has_fix_site = witness.fix_site.is_some();
        let has_suggested_assertion = witness.suggested_assertion.is_some();
        let limitation_kinds = witness
            .limitations
            .iter()
            .map(|limitation| limitation.kind.clone())
            .collect::<Vec<_>>();

        let is_stale = witness
            .limitations
            .iter()
            .any(|limitation| limitation.kind.contains("stale"));

        let is_static_limitation = witness.limitations.iter().any(|limitation| {
            limitation.kind.contains("missing_discriminator")
                || limitation.kind.contains("static_limit")
        });

        let state = if is_stale {
            FixInstructionState::Stale
        } else if is_static_limitation {
            FixInstructionState::StaticLimitation
        } else if has_fix_site {
            FixInstructionState::FixSiteReady
        } else if limitation_kinds.is_empty() {
            // No limitations and no fix site — informational only.
            FixInstructionState::InspectOnly
        } else {
            FixInstructionState::Unavailable
        };

        Self {
            state,
            has_fix_site,
            has_suggested_assertion,
            limitation_kinds,
        }
    }

    /// Derive the summary for a diagnostic with no witness at all.
    pub fn unavailable() -> Self {
        Self {
            state: FixInstructionState::Unavailable,
            has_fix_site: false,
            has_suggested_assertion: false,
            limitation_kinds: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DiagnosticConfidence, DiagnosticWitness, DiagnosticWitnessLimitation};

    fn witness_with_fix_site() -> DiagnosticWitness {
        DiagnosticWitness {
            kind: "static_discriminator_gap".to_string(),
            probe_family: "test".to_string(),
            changed_expression: "expr".to_string(),
            before: None,
            after: None,
            expected_sink: None,
            missing_discriminators: Vec::new(),
            fix_site: Some(crate::domain::DiagnosticFixSite {
                file: "src/lib.rs".to_string(),
                line: 10,
                test_name: "test_foo".to_string(),
                current_oracle: None,
                oracle_kind: "exact_value".to_string(),
                oracle_strength: "strong".to_string(),
                oracle_location: None,
            }),
            suggested_assertion: None,
            explain_command: "ripr explain".to_string(),
            confidence: DiagnosticConfidence {
                value: Some(0.8),
                basis: "static_only".to_string(),
            },
            limitations: Vec::new(),
        }
    }

    fn witness_with_limitation(kind: &str) -> DiagnosticWitness {
        let mut witness = witness_with_fix_site();
        witness.fix_site = None;
        witness.limitations = vec![DiagnosticWitnessLimitation {
            kind: kind.to_string(),
            detail: "test limitation".to_string(),
        }];
        witness
    }

    fn bare_witness() -> DiagnosticWitness {
        let mut witness = witness_with_fix_site();
        witness.fix_site = None;
        witness.limitations = Vec::new();
        witness
    }

    #[test]
    fn fix_site_ready_when_fix_site_present_and_no_limitation() {
        let summary = FixInstructionSummary::from_witness(&witness_with_fix_site());
        assert_eq!(summary.state, FixInstructionState::FixSiteReady);
        assert!(summary.has_fix_site);
    }

    #[test]
    fn static_limitation_when_missing_discriminator() {
        let witness = witness_with_limitation("missing_discriminator_unavailable");
        let summary = FixInstructionSummary::from_witness(&witness);
        assert_eq!(summary.state, FixInstructionState::StaticLimitation);
    }

    #[test]
    fn stale_when_stale_limitation_present() {
        let witness = witness_with_limitation("snapshot_stale");
        let summary = FixInstructionSummary::from_witness(&witness);
        assert_eq!(summary.state, FixInstructionState::Stale);
    }

    #[test]
    fn inspect_only_when_no_fix_site_and_no_limitations() {
        let summary = FixInstructionSummary::from_witness(&bare_witness());
        assert_eq!(summary.state, FixInstructionState::InspectOnly);
    }

    #[test]
    fn unavailable_when_no_witness() {
        let summary = FixInstructionSummary::unavailable();
        assert_eq!(summary.state, FixInstructionState::Unavailable);
        assert!(!summary.has_fix_site);
    }

    #[test]
    fn stale_takes_precedence_over_static_limitation() {
        let mut witness = witness_with_limitation("missing_discriminator_unavailable");
        witness.limitations.push(DiagnosticWitnessLimitation {
            kind: "snapshot_stale".to_string(),
            detail: "stale".to_string(),
        });
        let summary = FixInstructionSummary::from_witness(&witness);
        assert_eq!(summary.state, FixInstructionState::Stale);
    }
}
