//! Typed base/head attribution for canonical static evidence.
//!
//! This module owns identity-safe comparison vocabulary and fixture-backed
//! comparison rules. It does not decide whether an attribution blocks a gate
//! or how any consumer renders it.

use super::OracleStrength;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaAttribution {
    IntroducedByChange,
    WeakenedByChange,
    ReintroducedByChange,
    ResolvedByChange,
    ChangedSurfaceExisting,
    AdjacentPreexisting,
    BaselineExisting,
    ComparisonUnknown,
}

impl DeltaAttribution {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IntroducedByChange => "introduced_by_change",
            Self::WeakenedByChange => "weakened_by_change",
            Self::ReintroducedByChange => "reintroduced_by_change",
            Self::ResolvedByChange => "resolved_by_change",
            Self::ChangedSurfaceExisting => "changed_surface_existing",
            Self::AdjacentPreexisting => "adjacent_preexisting",
            Self::BaselineExisting => "baseline_existing",
            Self::ComparisonUnknown => "comparison_unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionBasis {
    SameCanonicalOwner,
    SameBehaviorIdentity,
    SameDiscriminator,
    OracleStrengthDecreased,
    OracleStrengthIncreased,
    GapStateChanged,
    UnchangedEvidence,
    BaseItemAbsent,
    HeadItemAbsent,
    BaseSnapshotUnavailable,
    HeadSnapshotUnavailable,
    IdentityAmbiguous,
    RenameOrMoveMapped,
    AdjacentSurface,
    BaselineReceipt,
}

impl AttributionBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SameCanonicalOwner => "same_canonical_owner",
            Self::SameBehaviorIdentity => "same_behavior_identity",
            Self::SameDiscriminator => "same_discriminator",
            Self::OracleStrengthDecreased => "oracle_strength_decreased",
            Self::OracleStrengthIncreased => "oracle_strength_increased",
            Self::GapStateChanged => "gap_state_changed",
            Self::UnchangedEvidence => "unchanged_evidence",
            Self::BaseItemAbsent => "base_item_absent",
            Self::HeadItemAbsent => "head_item_absent",
            Self::BaseSnapshotUnavailable => "base_snapshot_unavailable",
            Self::HeadSnapshotUnavailable => "head_snapshot_unavailable",
            Self::IdentityAmbiguous => "identity_ambiguous",
            Self::RenameOrMoveMapped => "rename_or_move_mapped",
            Self::AdjacentSurface => "adjacent_surface",
            Self::BaselineReceipt => "baseline_receipt",
        }
    }
}

/// Closed evidence-state vocabulary used by the causal comparison model.
///
/// Unknown is intentionally explicit: opaque or unsupported producer state
/// must remain non-causal rather than being coerced into an actionable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapState {
    Actionable,
    AlreadyObserved,
    Resolved,
    Unknown,
}

impl GapState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Actionable => "actionable",
            Self::AlreadyObserved => "already_observed",
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
        }
    }
}

impl From<&str> for GapState {
    fn from(value: &str) -> Self {
        match value {
            "actionable" => Self::Actionable,
            "already_observed" => Self::AlreadyObserved,
            "resolved" => Self::Resolved,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonConfidence {
    FixtureBacked,
    High,
    Medium,
    Low,
    Unknown,
}

impl ComparisonConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixtureBacked => "fixture_backed",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalEvidenceState {
    pub canonical_owner: String,
    pub behavior_identity: String,
    pub discriminator_identity: String,
    pub gap_state: GapState,
    pub oracle_strength: OracleStrength,
}

impl CanonicalEvidenceState {
    pub fn new(
        canonical_owner: impl Into<String>,
        behavior_identity: impl Into<String>,
        discriminator_identity: impl Into<String>,
        gap_state: impl Into<GapState>,
        oracle_strength: OracleStrength,
    ) -> Self {
        Self {
            canonical_owner: canonical_owner.into(),
            behavior_identity: behavior_identity.into(),
            discriminator_identity: discriminator_identity.into(),
            gap_state: gap_state.into(),
            oracle_strength,
        }
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.canonical_owner == other.canonical_owner
            && self.behavior_identity == other.behavior_identity
            && self.discriminator_identity == other.discriminator_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalDelta {
    pub canonical_gap_id: String,
    pub delta_attribution: DeltaAttribution,
    pub base_state: Option<CanonicalEvidenceState>,
    pub head_state: Option<CanonicalEvidenceState>,
    pub attribution_basis: Vec<AttributionBasis>,
    pub comparison_confidence: ComparisonConfidence,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComparisonCoverage {
    pub base_items: usize,
    pub head_items: usize,
    pub matched_items: usize,
    pub ambiguous_items: usize,
    pub unknown_items: usize,
}

impl ComparisonCoverage {
    pub fn is_complete(&self) -> bool {
        self.ambiguous_items == 0 && self.unknown_items == 0
    }
}

/// Compare one fixture-backed canonical item without using line proximity.
///
/// `base_available` and `head_available` distinguish an absent item in a
/// complete snapshot from an unavailable snapshot. An unavailable base or
/// head is always `comparison_unknown`, even when the other side has a record.
pub fn compare_fixture_delta(
    canonical_gap_id: impl Into<String>,
    base_available: bool,
    head_available: bool,
    base_state: Option<CanonicalEvidenceState>,
    head_state: Option<CanonicalEvidenceState>,
) -> CanonicalDelta {
    let canonical_gap_id = canonical_gap_id.into();
    if !base_available || !head_available {
        return CanonicalDelta {
            canonical_gap_id,
            delta_attribution: DeltaAttribution::ComparisonUnknown,
            base_state,
            head_state,
            attribution_basis: match (base_available, head_available) {
                (false, false) => vec![
                    AttributionBasis::BaseSnapshotUnavailable,
                    AttributionBasis::HeadSnapshotUnavailable,
                ],
                (false, true) => vec![AttributionBasis::BaseSnapshotUnavailable],
                (true, false) => vec![AttributionBasis::HeadSnapshotUnavailable],
                (true, true) => Vec::new(),
            },
            comparison_confidence: ComparisonConfidence::Unknown,
        };
    }

    match (base_state.as_ref(), head_state.as_ref()) {
        (None, Some(_)) => CanonicalDelta {
            canonical_gap_id,
            delta_attribution: DeltaAttribution::IntroducedByChange,
            base_state,
            head_state,
            attribution_basis: vec![AttributionBasis::BaseItemAbsent],
            comparison_confidence: ComparisonConfidence::FixtureBacked,
        },
        (Some(_), None) => CanonicalDelta {
            canonical_gap_id,
            delta_attribution: DeltaAttribution::ResolvedByChange,
            base_state,
            head_state,
            attribution_basis: vec![AttributionBasis::HeadItemAbsent],
            comparison_confidence: ComparisonConfidence::FixtureBacked,
        },
        (Some(base), Some(head)) if !base.same_identity(head) => CanonicalDelta {
            canonical_gap_id,
            delta_attribution: DeltaAttribution::ComparisonUnknown,
            base_state,
            head_state,
            attribution_basis: vec![AttributionBasis::IdentityAmbiguous],
            comparison_confidence: ComparisonConfidence::Unknown,
        },
        (Some(base), Some(head)) => {
            let mut attribution_basis = vec![
                AttributionBasis::SameCanonicalOwner,
                AttributionBasis::SameBehaviorIdentity,
                AttributionBasis::SameDiscriminator,
            ];
            let (delta_attribution, comparison_confidence) = if is_reintroduced(base, head) {
                attribution_basis.push(AttributionBasis::GapStateChanged);
                (
                    DeltaAttribution::ReintroducedByChange,
                    ComparisonConfidence::FixtureBacked,
                )
            } else if is_resolved_state(head.gap_state) && !is_resolved_state(base.gap_state) {
                attribution_basis.push(AttributionBasis::GapStateChanged);
                (
                    DeltaAttribution::ResolvedByChange,
                    ComparisonConfidence::FixtureBacked,
                )
            } else if head.oracle_strength.rank() < base.oracle_strength.rank() {
                attribution_basis.push(AttributionBasis::OracleStrengthDecreased);
                (
                    DeltaAttribution::WeakenedByChange,
                    ComparisonConfidence::FixtureBacked,
                )
            } else if head.oracle_strength.rank() > base.oracle_strength.rank() {
                attribution_basis.push(AttributionBasis::OracleStrengthIncreased);
                (
                    DeltaAttribution::ResolvedByChange,
                    ComparisonConfidence::FixtureBacked,
                )
            } else if (base.gap_state == head.gap_state
                || (is_resolved_state(base.gap_state) && is_resolved_state(head.gap_state)))
                && base.oracle_strength == head.oracle_strength
            {
                attribution_basis.push(AttributionBasis::UnchangedEvidence);
                (
                    DeltaAttribution::ChangedSurfaceExisting,
                    ComparisonConfidence::FixtureBacked,
                )
            } else {
                attribution_basis.push(AttributionBasis::GapStateChanged);
                (
                    DeltaAttribution::ComparisonUnknown,
                    ComparisonConfidence::Low,
                )
            };
            CanonicalDelta {
                canonical_gap_id,
                delta_attribution,
                base_state,
                head_state,
                attribution_basis,
                comparison_confidence,
            }
        }
        (None, None) => CanonicalDelta {
            canonical_gap_id,
            delta_attribution: DeltaAttribution::ComparisonUnknown,
            base_state,
            head_state,
            attribution_basis: vec![AttributionBasis::IdentityAmbiguous],
            comparison_confidence: ComparisonConfidence::Unknown,
        },
    }
}

fn is_reintroduced(base: &CanonicalEvidenceState, head: &CanonicalEvidenceState) -> bool {
    is_resolved_state(base.gap_state) && head.gap_state == GapState::Actionable
}

fn is_resolved_state(gap_state: GapState) -> bool {
    matches!(gap_state, GapState::Resolved | GapState::AlreadyObserved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(
        gap_state: impl Into<GapState>,
        oracle_strength: OracleStrength,
    ) -> CanonicalEvidenceState {
        CanonicalEvidenceState::new(
            "crate::pricing::discount",
            "predicate:amount>=threshold",
            "amount==threshold",
            gap_state,
            oracle_strength,
        )
    }

    #[test]
    fn closed_attribution_vocabulary_serializes_stably() {
        let labels = [
            DeltaAttribution::IntroducedByChange,
            DeltaAttribution::WeakenedByChange,
            DeltaAttribution::ReintroducedByChange,
            DeltaAttribution::ResolvedByChange,
            DeltaAttribution::ChangedSurfaceExisting,
            DeltaAttribution::AdjacentPreexisting,
            DeltaAttribution::BaselineExisting,
            DeltaAttribution::ComparisonUnknown,
        ];
        assert_eq!(
            labels.map(DeltaAttribution::as_str),
            [
                "introduced_by_change",
                "weakened_by_change",
                "reintroduced_by_change",
                "resolved_by_change",
                "changed_surface_existing",
                "adjacent_preexisting",
                "baseline_existing",
                "comparison_unknown",
            ]
        );
    }

    #[test]
    fn supporting_vocabularies_and_coverage_are_typed() {
        let bases = [
            AttributionBasis::SameCanonicalOwner,
            AttributionBasis::SameBehaviorIdentity,
            AttributionBasis::SameDiscriminator,
            AttributionBasis::OracleStrengthDecreased,
            AttributionBasis::OracleStrengthIncreased,
            AttributionBasis::GapStateChanged,
            AttributionBasis::UnchangedEvidence,
            AttributionBasis::BaseItemAbsent,
            AttributionBasis::HeadItemAbsent,
            AttributionBasis::BaseSnapshotUnavailable,
            AttributionBasis::HeadSnapshotUnavailable,
            AttributionBasis::IdentityAmbiguous,
            AttributionBasis::RenameOrMoveMapped,
            AttributionBasis::AdjacentSurface,
            AttributionBasis::BaselineReceipt,
        ];
        assert_eq!(
            bases.map(AttributionBasis::as_str),
            [
                "same_canonical_owner",
                "same_behavior_identity",
                "same_discriminator",
                "oracle_strength_decreased",
                "oracle_strength_increased",
                "gap_state_changed",
                "unchanged_evidence",
                "base_item_absent",
                "head_item_absent",
                "base_snapshot_unavailable",
                "head_snapshot_unavailable",
                "identity_ambiguous",
                "rename_or_move_mapped",
                "adjacent_surface",
                "baseline_receipt",
            ]
        );

        let confidences = [
            ComparisonConfidence::FixtureBacked,
            ComparisonConfidence::High,
            ComparisonConfidence::Medium,
            ComparisonConfidence::Low,
            ComparisonConfidence::Unknown,
        ];
        assert_eq!(
            confidences.map(ComparisonConfidence::as_str),
            ["fixture_backed", "high", "medium", "low", "unknown"]
        );

        let gap_states = [
            GapState::Actionable,
            GapState::AlreadyObserved,
            GapState::Resolved,
            GapState::Unknown,
        ];
        assert_eq!(
            gap_states.map(GapState::as_str),
            ["actionable", "already_observed", "resolved", "unknown"]
        );

        let complete = ComparisonCoverage {
            base_items: 1,
            head_items: 1,
            matched_items: 1,
            ambiguous_items: 0,
            unknown_items: 0,
        };
        assert!(complete.is_complete());
        let incomplete = ComparisonCoverage {
            ambiguous_items: 1,
            ..complete
        };
        assert!(!incomplete.is_complete());
    }

    #[test]
    fn unavailable_base_never_promotes_a_head_only_item() {
        let delta = compare_fixture_delta(
            "gap:pricing",
            false,
            true,
            None,
            Some(state("actionable", OracleStrength::Weak)),
        );
        assert_eq!(delta.delta_attribution, DeltaAttribution::ComparisonUnknown);
        assert_eq!(
            delta.attribution_basis,
            vec![AttributionBasis::BaseSnapshotUnavailable]
        );
        assert_eq!(delta.comparison_confidence, ComparisonConfidence::Unknown);
    }

    #[test]
    fn fixture_rules_cover_introduced_weakened_reintroduced_and_resolved() {
        let introduced = compare_fixture_delta(
            "gap:introduced",
            true,
            true,
            None,
            Some(state("actionable", OracleStrength::Weak)),
        );
        assert_eq!(
            introduced.delta_attribution,
            DeltaAttribution::IntroducedByChange
        );

        let weakened = compare_fixture_delta(
            "gap:weakened",
            true,
            true,
            Some(state("actionable", OracleStrength::Strong)),
            Some(state("actionable", OracleStrength::Weak)),
        );
        assert_eq!(
            weakened.delta_attribution,
            DeltaAttribution::WeakenedByChange
        );
        assert!(
            weakened
                .attribution_basis
                .contains(&AttributionBasis::OracleStrengthDecreased)
        );

        let reintroduced = compare_fixture_delta(
            "gap:reintroduced",
            true,
            true,
            Some(state("resolved", OracleStrength::Strong)),
            Some(state("actionable", OracleStrength::Strong)),
        );
        assert_eq!(
            reintroduced.delta_attribution,
            DeltaAttribution::ReintroducedByChange
        );

        let resolved = compare_fixture_delta(
            "gap:resolved",
            true,
            true,
            Some(state("actionable", OracleStrength::Weak)),
            None,
        );
        assert_eq!(
            resolved.delta_attribution,
            DeltaAttribution::ResolvedByChange
        );
    }

    #[test]
    fn remaining_comparison_branches_are_conservative_and_typed() {
        let unavailable_head = compare_fixture_delta(
            "gap:head-unavailable",
            true,
            false,
            Some(state("actionable", OracleStrength::Weak)),
            None,
        );
        assert_eq!(
            unavailable_head.delta_attribution,
            DeltaAttribution::ComparisonUnknown
        );
        assert_eq!(
            unavailable_head.attribution_basis,
            vec![AttributionBasis::HeadSnapshotUnavailable]
        );

        let oracle_increased = compare_fixture_delta(
            "gap:oracle-increased",
            true,
            true,
            Some(state("actionable", OracleStrength::Weak)),
            Some(state("actionable", OracleStrength::Strong)),
        );
        assert_eq!(
            oracle_increased.delta_attribution,
            DeltaAttribution::ResolvedByChange
        );
        assert_eq!(
            oracle_increased.attribution_basis.last(),
            Some(&AttributionBasis::OracleStrengthIncreased)
        );

        let unchanged = compare_fixture_delta(
            "gap:unchanged",
            true,
            true,
            Some(state("actionable", OracleStrength::Weak)),
            Some(state("actionable", OracleStrength::Weak)),
        );
        assert_eq!(
            unchanged.delta_attribution,
            DeltaAttribution::ChangedSurfaceExisting
        );
        assert_eq!(
            unchanged.attribution_basis.last(),
            Some(&AttributionBasis::UnchangedEvidence)
        );

        let no_items = compare_fixture_delta("gap:no-items", true, true, None, None);
        assert_eq!(
            no_items.delta_attribution,
            DeltaAttribution::ComparisonUnknown
        );
        assert_eq!(
            no_items.attribution_basis,
            vec![AttributionBasis::IdentityAmbiguous]
        );
    }

    #[test]
    fn identity_mismatch_is_unknown_even_when_lines_would_be_adjacent() {
        let mut head = state("actionable", OracleStrength::Weak);
        head.canonical_owner = "crate::other::discount".to_string();
        let delta = compare_fixture_delta(
            "gap:ambiguous",
            true,
            true,
            Some(state("actionable", OracleStrength::Strong)),
            Some(head),
        );
        assert_eq!(delta.delta_attribution, DeltaAttribution::ComparisonUnknown);
        assert_eq!(
            delta.attribution_basis,
            vec![AttributionBasis::IdentityAmbiguous]
        );
    }

    #[test]
    fn resolving_gap_state_without_oracle_increase_uses_gap_state_basis() {
        let delta = compare_fixture_delta(
            "gap:resolved-by-state",
            true,
            true,
            Some(state("actionable", OracleStrength::Weak)),
            Some(state("resolved", OracleStrength::Weak)),
        );
        assert_eq!(delta.delta_attribution, DeltaAttribution::ResolvedByChange);
        assert!(
            delta
                .attribution_basis
                .contains(&AttributionBasis::GapStateChanged)
        );
        assert!(
            !delta
                .attribution_basis
                .contains(&AttributionBasis::OracleStrengthIncreased)
        );
    }

    #[test]
    fn resolved_transition_wins_over_oracle_weakening() {
        let delta = compare_fixture_delta(
            "gap:resolved-before-weakened",
            true,
            true,
            Some(state("actionable", OracleStrength::Strong)),
            Some(state("resolved", OracleStrength::Weak)),
        );
        assert_eq!(delta.delta_attribution, DeltaAttribution::ResolvedByChange);
        assert!(
            !delta
                .attribution_basis
                .contains(&AttributionBasis::OracleStrengthDecreased)
        );
    }

    #[test]
    fn resolved_state_aliases_are_unchanged_evidence() {
        let delta = compare_fixture_delta(
            "gap:resolved-alias",
            true,
            true,
            Some(state("resolved", OracleStrength::Weak)),
            Some(state("already_observed", OracleStrength::Weak)),
        );
        assert_eq!(
            delta.delta_attribution,
            DeltaAttribution::ChangedSurfaceExisting
        );
        assert_eq!(
            delta.attribution_basis.last(),
            Some(&AttributionBasis::UnchangedEvidence)
        );
    }

    #[test]
    fn same_identity_unknown_does_not_claim_identity_ambiguity() {
        let delta = compare_fixture_delta(
            "gap:unknown-state",
            true,
            true,
            Some(state("actionable", OracleStrength::Weak)),
            Some(state("unsupported", OracleStrength::Weak)),
        );
        assert_eq!(delta.delta_attribution, DeltaAttribution::ComparisonUnknown);
        assert_eq!(
            delta.attribution_basis,
            vec![
                AttributionBasis::SameCanonicalOwner,
                AttributionBasis::SameBehaviorIdentity,
                AttributionBasis::SameDiscriminator,
                AttributionBasis::GapStateChanged,
            ]
        );
    }
}
