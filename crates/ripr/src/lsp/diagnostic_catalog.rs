//! Governed diagnostic-code catalog for RIPR LSP diagnostics.
//!
//! RIPR-SPEC-0127 (issue #1662 slice A). One registry owns the closed set of
//! stable `code` values every RIPR diagnostic may carry. The message text is
//! presentation; the catalog owns the stable identity of the code.
//!
//! This slice establishes the typed registry, the current-code inventory, the
//! single-source constructors that build every emitted code, and fail-closed
//! resolution: a code that is not a governed entry does not resolve and, for
//! the one family whose input is open (`ripr-gap-*`), is not emitted at all.
//!
//! Out of scope for this slice (tracked by #1662 PR B/C/D): the human title,
//! summary, category, `codeDescription.href`, and generated documentation of
//! each code (PR B); typed recovery / actionability / non-claim metadata
//! (PR C); and migrating the remaining renderer-local code strings across
//! `output/` (PR D). Those slices extend the entry with the metadata they
//! consume; this slice carries only the code identity actually used today, so
//! the module has no dead surface and needs no lint suppression.

use crate::analysis::seams::SeamGripClass;
use crate::domain::ExposureClass;

/// One governed diagnostic code and the deprecated aliases that still resolve
/// to it. Later slices extend this entry with documentation and recovery
/// metadata; the code identity never changes.
pub(crate) struct CatalogEntry {
    /// The stable, emitted diagnostic code.
    pub(crate) code: &'static str,
    /// Deprecated codes that still resolve to this entry but are no longer
    /// emitted. Empty unless current output already requires an alias.
    pub(crate) aliases: &'static [&'static str],
}

const fn entry(code: &'static str) -> CatalogEntry {
    CatalogEntry { code, aliases: &[] }
}

/// The governed catalog: exactly one entry per emitted code.
///
/// Finding codes are the verbatim `ExposureClass` labels; seam codes are
/// `ripr-seam-{SeamGripClass}` for every grip class; gap codes are
/// `ripr-gap-{GapRecord.kind}` for every known kind. `MissingArtifact` is a
/// value contract an external gap-decision-ledger producer may supply and is
/// catalogued so it never emits an unregistered code.
const CATALOG: &[CatalogEntry] = &[
    // Finding / exposure codes (verbatim ExposureClass labels).
    entry("exposed"),
    entry("weakly_exposed"),
    entry("reachable_unrevealed"),
    entry("no_static_path"),
    entry("infection_unknown"),
    entry("propagation_unknown"),
    entry("static_unknown"),
    // Seam grip codes (ripr-seam-{SeamGripClass}).
    entry("ripr-seam-strongly-gripped"),
    entry("ripr-seam-weakly-gripped"),
    entry("ripr-seam-ungripped"),
    entry("ripr-seam-reachable-unrevealed"),
    entry("ripr-seam-activation-unknown"),
    entry("ripr-seam-propagation-unknown"),
    entry("ripr-seam-observation-unknown"),
    entry("ripr-seam-discrimination-unknown"),
    entry("ripr-seam-opaque"),
    entry("ripr-seam-intentional"),
    entry("ripr-seam-suppressed"),
    // Gap decision codes (ripr-gap-{GapRecord.kind}).
    entry("ripr-gap-NoActionAlreadyObserved"),
    entry("ripr-gap-NoActionInternal"),
    entry("ripr-gap-NoActionNoRelatedTest"),
    entry("ripr-gap-NoActionHeuristicOnly"),
    entry("ripr-gap-StaticLimitation"),
    entry("ripr-gap-MissingOutputContract"),
    entry("ripr-gap-MissingBoundaryAssertion"),
    entry("ripr-gap-MissingErrorDiscriminator"),
    entry("ripr-gap-MissingValueAssertion"),
    entry("ripr-gap-MissingSideEffectObserver"),
    entry("ripr-gap-Unknown"),
    entry("ripr-gap-MissingArtifact"),
    // Scope-limitation codes (workspace-scoped guard disclosures).
    entry("ripr-scope-diff-oversized"),
];

/// The workspace-scoped warning code emitted when the fail-closed
/// diff-scope guard converts to a limited snapshot (#2299).
pub(crate) const DIFF_SCOPE_OVERSIZED_CODE: &str = "ripr-scope-diff-oversized";

/// The governed catalog of diagnostic codes.
pub(crate) fn catalog() -> &'static [CatalogEntry] {
    CATALOG
}

/// Resolve a code (or a deprecated alias) to its catalog entry, if any. An
/// unknown code resolves to `None` and must not be emitted as a diagnostic.
pub(crate) fn resolve(code: &str) -> Option<&'static CatalogEntry> {
    catalog()
        .iter()
        .find(|entry| entry.code == code || entry.aliases.contains(&code))
}

/// Build the catalog-backed diagnostic code for a finding exposure class.
///
/// `ExposureClass` is a closed enum whose every label is a governed code, so
/// this is infallible.
pub(crate) fn finding_code(class: &ExposureClass) -> String {
    class.as_str().to_string()
}

/// Build the catalog-backed diagnostic code for a seam grip class.
///
/// `SeamGripClass` is a closed enum whose every label is a governed code, so
/// this is infallible.
pub(crate) fn seam_code(class: SeamGripClass) -> String {
    format!("ripr-seam-{}", class.as_str().replace('_', "-"))
}

/// Build the catalog-backed diagnostic code for a gap-decision record kind.
///
/// `GapRecord.kind` is an open string that may come from an external ledger, so
/// this fails closed: it returns `Some` only when the resulting code is a
/// governed catalog entry, and `None` for an unregistered kind so the caller
/// under-emits rather than surfacing an unknown code.
pub(crate) fn gap_code(kind: &str) -> Option<String> {
    let code = format!("ripr-gap-{}", kind.replace('_', "-"));
    resolve(&code).map(|entry| entry.code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPOSURE_CLASSES: [ExposureClass; 7] = [
        ExposureClass::Exposed,
        ExposureClass::WeaklyExposed,
        ExposureClass::ReachableUnrevealed,
        ExposureClass::NoStaticPath,
        ExposureClass::InfectionUnknown,
        ExposureClass::PropagationUnknown,
        ExposureClass::StaticUnknown,
    ];

    // Every `GapRecord.kind` this crate produces plus the external-producer
    // `MissingArtifact` contract; see crates/ripr/src/output/gap_decision_ledger.rs.
    const KNOWN_GAP_KINDS: [&str; 12] = [
        "NoActionAlreadyObserved",
        "NoActionInternal",
        "NoActionNoRelatedTest",
        "NoActionHeuristicOnly",
        "StaticLimitation",
        "MissingOutputContract",
        "MissingBoundaryAssertion",
        "MissingErrorDiscriminator",
        "MissingValueAssertion",
        "MissingSideEffectObserver",
        "Unknown",
        "MissingArtifact",
    ];

    #[test]
    fn every_entry_has_a_nonempty_code() {
        for entry in catalog() {
            assert!(!entry.code.is_empty(), "catalog entry has an empty code");
        }
    }

    #[test]
    fn codes_and_aliases_are_globally_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for entry in catalog() {
            assert!(seen.insert(entry.code), "duplicate code {}", entry.code);
            for alias in entry.aliases {
                assert!(
                    seen.insert(alias),
                    "alias {alias} collides with a code or alias"
                );
            }
        }
    }

    #[test]
    fn every_code_resolves_to_exactly_one_entry() {
        for entry in catalog() {
            let mut matches = catalog()
                .iter()
                .filter(|candidate| candidate.code == entry.code);
            assert!(matches.next().is_some(), "{} does not resolve", entry.code);
            assert!(
                matches.next().is_none(),
                "{} resolves to more than one entry",
                entry.code
            );
        }
    }

    #[test]
    fn every_emitted_finding_code_is_catalogued() {
        for class in EXPOSURE_CLASSES {
            let code = finding_code(&class);
            assert!(
                resolve(&code).is_some(),
                "finding code {code} is not in the catalog"
            );
        }
    }

    #[test]
    fn every_emitted_seam_code_is_catalogued() {
        for class in SeamGripClass::ALL {
            let code = seam_code(class);
            assert!(
                resolve(&code).is_some(),
                "seam code {code} is not in the catalog"
            );
        }
    }

    #[test]
    fn every_emitted_gap_code_is_catalogued() {
        for kind in KNOWN_GAP_KINDS {
            let code = gap_code(kind);
            assert!(
                code.is_some(),
                "gap kind {kind} did not produce a catalogued code"
            );
        }
    }

    #[test]
    fn gap_code_fails_closed_for_unknown_kind() {
        assert!(gap_code("TotallyUnregisteredKind").is_none());
        assert!(gap_code("").is_none());
    }

    #[test]
    fn unknown_code_does_not_resolve() {
        assert!(resolve("ripr-bogus-code").is_none());
        assert!(resolve("").is_none());
        assert!(resolve("ripr-gap-NotAGapKind").is_none());
    }
}
