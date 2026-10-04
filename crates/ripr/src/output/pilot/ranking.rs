use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::output::agent_seam_packets::suggested_assertion_for_classified_seam;
use crate::output::path::display_path;

pub(crate) fn top_actionable_seams(
    classified: &[ClassifiedSeam],
    max_seams: usize,
) -> Vec<&ClassifiedSeam> {
    let mut actionable = classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .collect::<Vec<_>>();
    // The rank key holds `suggested_assertion_for_classified_seam`, which
    // derives repair-route readiness from the seam's evidence; computing it
    // inside a comparator re-derived it O(n log n) times and dominated a
    // warm `ripr pilot` on a 900-seam crate (#5348). Compute it once per
    // seam; `sort_by_cached_key` is stable, like the `sort_by` it replaces.
    actionable.sort_by_cached_key(|entry| RankKey::of(entry));
    actionable.truncate(max_seams);
    actionable
}

/// The pilot ranking order, ascending: actionable class first, then seams
/// with missing discriminators, with related tests, and with a suggested
/// assertion, then file, line, kind and id as stable tie-breakers.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RankKey<'a> {
    class: Option<u8>,
    missing_discriminators: u8,
    related_tests: u8,
    suggested_assertion: u8,
    file: String,
    line: usize,
    kind: &'a str,
    id: &'a str,
}

impl<'a> RankKey<'a> {
    fn of(entry: &'a ClassifiedSeam) -> Self {
        Self {
            class: class_rank(entry.class),
            missing_discriminators: bool_rank(!entry.evidence.missing_discriminators.is_empty()),
            related_tests: bool_rank(!entry.evidence.related_tests.is_empty()),
            suggested_assertion: bool_rank(
                suggested_assertion_for_classified_seam(entry).is_some(),
            ),
            file: display_path(entry.seam.file()),
            line: entry.seam.display_line(),
            kind: entry.seam.kind().as_str(),
            id: entry.seam.id().as_str(),
        }
    }
}

pub(super) fn actionable_total(classified: &[ClassifiedSeam]) -> usize {
    classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .count()
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
