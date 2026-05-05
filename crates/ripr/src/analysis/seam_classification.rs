//! Seam classification per RIPR-SPEC-0005, v1.
//!
//! Maps `TestGripEvidence` produced by `analysis::test_grip_evidence`
//! into a single `SeamGripClass` per seam. Pure data transformation:
//! no I/O, no rendering, no badge policy. The
//! `output/repo-exposure-report-v1` work item is the first consumer.
//!
//! Classification rules (priority order, top wins):
//!
//! 1. `reach == No`                                     → `Ungripped`
//! 2. any stage `== Opaque`                              → `Opaque`
//! 3. all five stages `== Yes`                           → `StronglyGripped`
//! 4. `discriminate == No`                               → `ReachableUnrevealed`
//! 5. `discriminate == Weak`
//!    or `missing_discriminators` non-empty              → `WeaklyGripped`
//! 6. `activate == Unknown`                              → `ActivationUnknown`
//! 7. `propagate == Unknown`                             → `PropagationUnknown`
//! 8. `observe == Unknown`                               → `ObservationUnknown`
//! 9. `discriminate == Unknown`                          → `DiscriminationUnknown`
//! 10. fallback                                          → `Opaque`
//!
//! `Intentional` and `Suppressed` are reserved variants. The classifier
//! does not emit them today; a follow-up PR will consult declared test
//! intent (`.ripr/intents.toml`-style) and reasoned suppressions
//! (`.ripr/suppressions.toml`) and post-process the natural class
//! into one of those two.

use super::seams::{RepoSeam, SeamGripClass, SeamId};
use super::test_grip_evidence::TestGripEvidence;
use crate::domain::StageState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A seam paired with its evidence and the resulting grip class.
/// Crate-private; the report PR consumes `Vec<ClassifiedSeam>` directly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ClassifiedSeam {
    pub(crate) seam: RepoSeam,
    pub(crate) evidence: TestGripEvidence,
    pub(crate) class: SeamGripClass,
}

/// Apply the classification rules in priority order.
pub(crate) fn classify_seam(_seam: &RepoSeam, evidence: &TestGripEvidence) -> SeamGripClass {
    if evidence.reach.state == StageState::No {
        return SeamGripClass::Ungripped;
    }

    if any_stage_opaque(evidence) {
        return SeamGripClass::Opaque;
    }

    if all_stages_yes(evidence) {
        return SeamGripClass::StronglyGripped;
    }

    if evidence.discriminate.state == StageState::No {
        return SeamGripClass::ReachableUnrevealed;
    }

    if evidence.discriminate.state == StageState::Weak
        || !evidence.missing_discriminators.is_empty()
    {
        return SeamGripClass::WeaklyGripped;
    }

    if evidence.activate.state == StageState::Unknown {
        return SeamGripClass::ActivationUnknown;
    }
    if evidence.propagate.state == StageState::Unknown {
        return SeamGripClass::PropagationUnknown;
    }
    if evidence.observe.state == StageState::Unknown {
        return SeamGripClass::ObservationUnknown;
    }
    if evidence.discriminate.state == StageState::Unknown {
        return SeamGripClass::DiscriminationUnknown;
    }

    SeamGripClass::Opaque
}

/// Classify each seam by pairing it with its matching evidence record.
///
/// Pairing is done by `seam_id`, not by index, because
/// `evidence_for_seams` sorts its output by `seam_id` while the
/// inventory walker sorts seams by `(file, byte_offset, kind, owner)`.
/// Index-alignment would silently misattribute evidence to seams once
/// the input has more than one seam — caught by chatgpt-codex during
/// review and pinned by `classify_seams_pairs_evidence_by_seam_id`.
///
/// Seams without a matching evidence record are skipped. The inventory
/// walker always builds evidence for every seam, so this only filters
/// out genuinely orphaned input.
pub(crate) fn classify_seams(
    seams: &[RepoSeam],
    evidence: &[TestGripEvidence],
) -> Vec<ClassifiedSeam> {
    let evidence_by_id: HashMap<&SeamId, &TestGripEvidence> =
        evidence.iter().map(|ev| (&ev.seam_id, ev)).collect();
    seams
        .iter()
        .filter_map(|seam| {
            let ev = evidence_by_id.get(seam.id())?;
            Some(ClassifiedSeam {
                seam: seam.clone(),
                evidence: (*ev).clone(),
                class: classify_seam(seam, ev),
            })
        })
        .collect()
}

fn any_stage_opaque(evidence: &TestGripEvidence) -> bool {
    matches!(evidence.reach.state, StageState::Opaque)
        || matches!(evidence.activate.state, StageState::Opaque)
        || matches!(evidence.propagate.state, StageState::Opaque)
        || matches!(evidence.observe.state, StageState::Opaque)
        || matches!(evidence.discriminate.state, StageState::Opaque)
}

fn all_stages_yes(evidence: &TestGripEvidence) -> bool {
    evidence.reach.state == StageState::Yes
        && evidence.activate.state == StageState::Yes
        && evidence.propagate.state == StageState::Yes
        && evidence.observe.state == StageState::Yes
        && evidence.discriminate.state == StageState::Yes
        && evidence.missing_discriminators.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, StageEvidence, StageState, ValueFact,
    };

    fn sample_seam() -> RepoSeam {
        RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            7,
            "amount >= threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn evidence_with(
        reach: StageState,
        activate: StageState,
        propagate: StageState,
        observe: StageState,
        discriminate: StageState,
        missing: Vec<MissingDiscriminatorFact>,
    ) -> TestGripEvidence {
        TestGripEvidence {
            seam_id: sample_seam().id().clone(),
            related_tests: Vec::<RelatedTestGrip>::new(),
            reach: stage(reach),
            activate: stage(activate),
            propagate: stage(propagate),
            observe: stage(observe),
            discriminate: stage(discriminate),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: missing,
        }
    }

    fn no_missing() -> Vec<MissingDiscriminatorFact> {
        Vec::new()
    }

    fn one_missing() -> Vec<MissingDiscriminatorFact> {
        vec![MissingDiscriminatorFact {
            value: "threshold (equality boundary)".to_string(),
            reason: "observed values do not include the equality-boundary case".to_string(),
            flow_sink: None,
        }]
    }

    #[test]
    fn given_all_ripr_stages_yes_and_strong_oracle_then_seam_is_strongly_gripped() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::StronglyGripped
        );
    }

    #[test]
    fn given_related_tests_with_missing_boundary_discriminator_then_seam_is_weakly_gripped() {
        // Reach + activate + observe are all Yes; the only gap is a
        // missing equality-boundary discriminator hypothesis.
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            one_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::WeaklyGripped
        );
    }

    #[test]
    fn given_no_related_tests_then_seam_is_ungripped() {
        let evidence = evidence_with(
            StageState::No,
            StageState::No,
            StageState::No,
            StageState::No,
            StageState::No,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::Ungripped
        );
    }

    #[test]
    fn given_reach_yes_but_discriminate_no_then_seam_is_reachable_unrevealed() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::No,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::ReachableUnrevealed
        );
    }

    #[test]
    fn given_activation_unknown_then_seam_class_is_activation_unknown() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Unknown,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::ActivationUnknown
        );
    }

    #[test]
    fn given_propagate_unknown_then_seam_class_is_propagation_unknown() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Unknown,
            StageState::Yes,
            StageState::Yes,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::PropagationUnknown
        );
    }

    #[test]
    fn given_observe_unknown_then_seam_class_is_observation_unknown() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Unknown,
            StageState::Yes,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::ObservationUnknown
        );
    }

    #[test]
    fn given_discriminate_unknown_then_seam_class_is_discrimination_unknown() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Unknown,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::DiscriminationUnknown
        );
    }

    #[test]
    fn given_opaque_static_limitation_then_seam_class_is_opaque() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Opaque,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::Opaque
        );
    }

    #[test]
    fn weak_discriminate_maps_to_weakly_gripped_even_without_missing_discriminators() {
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Weak,
            no_missing(),
        );
        assert_eq!(
            classify_seam(&sample_seam(), &evidence),
            SeamGripClass::WeaklyGripped
        );
    }

    #[test]
    fn headline_eligibility_matches_spec_table() {
        let headline = [
            SeamGripClass::Ungripped,
            SeamGripClass::WeaklyGripped,
            SeamGripClass::ReachableUnrevealed,
            SeamGripClass::ActivationUnknown,
            SeamGripClass::PropagationUnknown,
            SeamGripClass::ObservationUnknown,
            SeamGripClass::DiscriminationUnknown,
        ];
        for class in headline {
            assert!(
                class.is_headline_eligible(),
                "{} should be headline-eligible",
                class.as_str()
            );
        }
        let visible_only = [
            SeamGripClass::StronglyGripped,
            SeamGripClass::Opaque,
            SeamGripClass::Intentional,
            SeamGripClass::Suppressed,
        ];
        for class in visible_only {
            assert!(
                !class.is_headline_eligible(),
                "{} should not be headline-eligible",
                class.as_str()
            );
        }
    }

    #[test]
    fn intentional_and_suppressed_render_their_strings() {
        // Variant placeholder: classification PR does not emit these
        // automatically; declared-intent / suppression PRs do. Pin the
        // string table so that future logic stays consistent.
        assert_eq!(SeamGripClass::Intentional.as_str(), "intentional");
        assert_eq!(SeamGripClass::Suppressed.as_str(), "suppressed");
    }

    #[test]
    fn classify_seams_returns_one_classified_seam_per_input() {
        let seam = sample_seam();
        let evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            no_missing(),
        );
        let classified =
            classify_seams(std::slice::from_ref(&seam), std::slice::from_ref(&evidence));
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].class, SeamGripClass::StronglyGripped);
        assert_eq!(classified[0].seam.id(), seam.id());
    }

    #[test]
    fn classify_seams_pairs_evidence_by_seam_id_not_index() -> Result<(), String> {
        // Regression for chatgpt-codex review on PR #237: evidence is
        // sorted by seam_id while seams are sorted by (file, byte_offset,
        // kind, owner). Index-alignment misattributes evidence; pairing
        // must be by seam_id.
        let strong_seam = RepoSeam::new(
            "src/a.rs",
            "a::strong",
            SeamKind::PredicateBoundary,
            10,
            1,
            "x >= 1",
            RequiredDiscriminator::BoundaryValue {
                description: "x >= 1".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let ungripped_seam = RepoSeam::new(
            "src/b.rs",
            "b::ungripped",
            SeamKind::ReturnValue,
            20,
            2,
            "answer",
            RequiredDiscriminator::ReturnValue {
                description: "answer".to_string(),
            },
            ExpectedSink::ReturnValue,
        );

        let strong_evidence = TestGripEvidence {
            seam_id: strong_seam.id().clone(),
            related_tests: Vec::<RelatedTestGrip>::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: no_missing(),
        };
        let ungripped_evidence = TestGripEvidence {
            seam_id: ungripped_seam.id().clone(),
            related_tests: Vec::<RelatedTestGrip>::new(),
            reach: stage(StageState::No),
            activate: stage(StageState::No),
            propagate: stage(StageState::No),
            observe: stage(StageState::No),
            discriminate: stage(StageState::No),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: no_missing(),
        };

        // Seams in one order, evidence in the OPPOSITE order. With
        // index-alignment this would swap their classifications.
        let seams = [strong_seam.clone(), ungripped_seam.clone()];
        let evidence = [ungripped_evidence, strong_evidence];

        let classified = classify_seams(&seams, &evidence);
        assert_eq!(classified.len(), 2);

        let strong_record = classified
            .iter()
            .find(|c| c.seam.id() == strong_seam.id())
            .ok_or_else(|| "strong seam not classified".to_string())?;
        let ungripped_record = classified
            .iter()
            .find(|c| c.seam.id() == ungripped_seam.id())
            .ok_or_else(|| "ungripped seam not classified".to_string())?;

        assert_eq!(strong_record.class, SeamGripClass::StronglyGripped);
        assert_eq!(ungripped_record.class, SeamGripClass::Ungripped);
        Ok(())
    }
    #[test]
    fn classify_seams_skips_seams_without_matching_evidence() {
        let seam = sample_seam();
        let unrelated_seam = RepoSeam::new(
            "src/other.rs",
            "other::path",
            SeamKind::ReturnValue,
            99,
            1,
            "value",
            RequiredDiscriminator::ReturnValue {
                description: "value".to_string(),
            },
            ExpectedSink::ReturnValue,
        );

        let evidence = TestGripEvidence {
            seam_id: unrelated_seam.id().clone(),
            related_tests: Vec::<RelatedTestGrip>::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: no_missing(),
        };

        let classified = classify_seams(std::slice::from_ref(&seam), &[evidence]);
        assert!(classified.is_empty());
    }

    #[test]
    fn classify_seams_uses_matching_evidence_when_orphan_evidence_is_present() {
        let seam = sample_seam();
        let orphan = RepoSeam::new(
            "src/orphan.rs",
            "orphan::path",
            SeamKind::ReturnValue,
            111,
            3,
            "orphan",
            RequiredDiscriminator::ReturnValue {
                description: "orphan".to_string(),
            },
            ExpectedSink::ReturnValue,
        );

        let orphan_evidence = TestGripEvidence {
            seam_id: orphan.id().clone(),
            related_tests: Vec::<RelatedTestGrip>::new(),
            reach: stage(StageState::No),
            activate: stage(StageState::No),
            propagate: stage(StageState::No),
            observe: stage(StageState::No),
            discriminate: stage(StageState::No),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: no_missing(),
        };
        let matching_evidence = evidence_with(
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            StageState::Weak,
            no_missing(),
        );

        let classified = classify_seams(
            std::slice::from_ref(&seam),
            &[orphan_evidence, matching_evidence],
        );
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].seam.id(), seam.id());
        assert_eq!(classified[0].class, SeamGripClass::WeaklyGripped);
    }
}
