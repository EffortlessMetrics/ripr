use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::output::agent_seam_packets::suggested_assertion_for_classified_seam;
use crate::output::path::display_path;
use std::collections::BTreeMap;
use std::path::Path;

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
    spread_across_owners(&mut actionable);
    actionable.truncate(max_seams);
    actionable
}

/// Within each actionable class, rank every function's first seam ahead of
/// any function's second (#5770). Adjacent seams of one owner share its
/// tests, so when those tests are good every one of them is wrong together;
/// ranked by location alone they filled the top ten (8 of semver's 10 sat in
/// two `Identifier` methods). The class still leads, so a weak seam is never
/// pushed below an unknown one, and `RankKey` order holds inside each round.
///
/// Rounds count across classes on purpose: a function already listed for a
/// weak seam does not get a fresh first pick among the unknown ones, so one
/// function cannot claim a slot per class.
fn spread_across_owners(ranked: &mut Vec<&ClassifiedSeam>) {
    let mut taken: BTreeMap<(&Path, &str), usize> = BTreeMap::new();
    let mut keyed = ranked
        .drain(..)
        .map(|entry| {
            let round = taken
                .entry((entry.seam.file(), entry.seam.owner()))
                .or_default();
            let key = (class_rank(entry.class), *round);
            *round += 1;
            (key, entry)
        })
        .collect::<Vec<_>>();
    // Stable, so seams with the same class and round keep `RankKey` order.
    keyed.sort_by_key(|(key, _)| *key);
    ranked.extend(keyed.into_iter().map(|(_, entry)| entry));
}

/// Actionable seams that share an owning function with `entry`, itself
/// included, so a renderer can say how many more a ranked seam stands for.
/// Pass the same slice `top_actionable_seams` ranked: the renderer subtracts
/// the listed seams from this count, so a narrower slice would undercount.
pub(super) fn actionable_in_owner(classified: &[ClassifiedSeam], entry: &ClassifiedSeam) -> usize {
    classified
        .iter()
        .filter(|other| {
            class_rank(other.class).is_some()
                && other.seam.file() == entry.seam.file()
                && other.seam.owner() == entry.seam.owner()
        })
        .count()
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
