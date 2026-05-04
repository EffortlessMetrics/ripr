//! Voice B seam classification per RIPR-SPEC-0005, v1.
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

use super::seams::{RepoSeam, SeamGripClass};
use super::test_grip_evidence::TestGripEvidence;
use crate::domain::StageState;

/// A seam paired with its evidence and the resulting grip class.
/// Crate-private; the report PR consumes `Vec<ClassifiedSeam>` directly.
#[derive(Clone, Debug)]
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

/// Classify each (seam, evidence) pair. Inputs must be aligned by index;
/// the inventory walker constructs them that way.
pub(crate) fn classify_seams(
    seams: &[RepoSeam],
    evidence: &[TestGripEvidence],
) -> Vec<ClassifiedSeam> {
    debug_assert_eq!(seams.len(), evidence.len());
    seams
        .iter()
        .zip(evidence.iter())
        .map(|(seam, ev)| ClassifiedSeam {
            seam: seam.clone(),
            evidence: ev.clone(),
            class: classify_seam(seam, ev),
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
    fn classify_seams_pairs_inputs_by_index() {
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
}
