use crate::agent::loop_commands::display_path;
use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::output::agent_seam_packets::suggested_assertion_for_classified_seam;
use std::cmp::Ordering;

pub(crate) fn top_actionable_seams(
    classified: &[ClassifiedSeam],
    max_seams: usize,
) -> Vec<&ClassifiedSeam> {
    let mut actionable = classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .collect::<Vec<_>>();
    actionable.sort_by(|left, right| compare_ranked_seams(left, right));
    actionable.truncate(max_seams);
    actionable
}

pub(super) fn actionable_total(classified: &[ClassifiedSeam]) -> usize {
    classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .count()
}

fn compare_ranked_seams(left: &ClassifiedSeam, right: &ClassifiedSeam) -> Ordering {
    class_rank(left.class)
        .cmp(&class_rank(right.class))
        .then(
            bool_rank(!left.evidence.missing_discriminators.is_empty()).cmp(&bool_rank(
                !right.evidence.missing_discriminators.is_empty(),
            )),
        )
        .then(
            bool_rank(!left.evidence.related_tests.is_empty())
                .cmp(&bool_rank(!right.evidence.related_tests.is_empty())),
        )
        .then(
            bool_rank(suggested_assertion_for_classified_seam(left).is_some()).cmp(&bool_rank(
                suggested_assertion_for_classified_seam(right).is_some(),
            )),
        )
        .then(display_path(left.seam.file()).cmp(&display_path(right.seam.file())))
        .then(left.seam.display_line().cmp(&right.seam.display_line()))
        .then(left.seam.kind().as_str().cmp(right.seam.kind().as_str()))
        .then(left.seam.id().as_str().cmp(right.seam.id().as_str()))
}

fn class_rank(class: SeamGripClass) -> Option<u8> {
    Some(match class {
        SeamGripClass::WeaklyGripped => 0,
        SeamGripClass::Ungripped => 1,
        SeamGripClass::ReachableUnrevealed => 2,
        SeamGripClass::ActivationUnknown
        | SeamGripClass::PropagationUnknown
        | SeamGripClass::ObservationUnknown
        | SeamGripClass::DiscriminationUnknown => 3,
        SeamGripClass::Opaque => 4,
        SeamGripClass::StronglyGripped | SeamGripClass::Intentional | SeamGripClass::Suppressed => {
            return None;
        }
    })
}

fn bool_rank(value: bool) -> u8 {
    if value { 0 } else { 1 }
}
