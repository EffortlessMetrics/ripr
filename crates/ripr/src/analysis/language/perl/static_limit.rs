//! Projection of producer-owned Perl boundaries into RIPR's shared static-limit taxonomy.
//!
//! This module does not inspect Perl source or parse human-readable limitation messages.
//! It maps typed `ripr-perl-facts-v1` boundary facts and stable limitation codes after
//! packet ingestion, preserving the producer as the semantic authority.

use super::{BoundaryKind, ChangeFact, PerlFactPacket, PerlRelatedTestEvidence};
use crate::domain::StaticLimitKind;
use std::collections::BTreeSet;

/// Conservative disposition plus the strongest earned shared category.
///
/// `blocks` remains independent from `kind` because the v1 packet mixes semantic
/// boundaries with operational states such as `missing_test_runner`. Those states
/// must continue to block actionability, but assigning a semantic label to them
/// would fabricate evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Projection {
    /// Blocks strict actionability (any operational limitation or boundary).
    pub(super) blocks: bool,
    /// Caps the exposure CLASS. Pre-#3520 the class was capped only by
    /// semantic dynamic-dispatch evidence — boundaries, or a limitation
    /// explicitly typed `dynamic_dispatch`. Operational states such as
    /// `partial_emitter` keep findings advisory but never mask an earned
    /// class: hiding evidence the packet actually carries is the
    /// analyzer-cannot-resolve vs missing-proof inversion (#3215).
    pub(super) blocks_class: bool,
    pub(super) kind: Option<StaticLimitKind>,
}

pub(super) fn for_change(
    packet: &PerlFactPacket,
    change: &ChangeFact,
    related_evidence: &[PerlRelatedTestEvidence],
) -> Projection {
    let test_file_ids = related_evidence
        .iter()
        .filter_map(|evidence| packet.test(&evidence.test_id))
        .map(|test| test.file_id.as_str())
        .collect::<BTreeSet<_>>();

    let mut projection = Projection::default();

    for boundary in &packet.dynamic_boundaries {
        let applies = match boundary.owner_id.as_deref() {
            Some(owner_id) => owner_id == change.owner_id,
            None => {
                boundary.file_id == change.file_id
                    || test_file_ids.contains(boundary.file_id.as_str())
            }
        };
        if !applies {
            continue;
        }

        projection.blocks = true;
        projection.blocks_class = true;
        if let Some(kind) = boundary_kind(boundary.kind) {
            select_more_specific(&mut projection.kind, kind);
        }
    }

    let mut relevant_refs = packet.change_evidence_ids(change);
    for evidence in related_evidence {
        relevant_refs.extend(packet.actionability_evidence_ids(change, evidence));
    }

    for limitation in &packet.limitations {
        let applies = limitation.evidence_refs.is_empty()
            || limitation
                .evidence_refs
                .iter()
                .any(|evidence_ref| relevant_refs.contains(evidence_ref));
        if !applies {
            continue;
        }

        // Operational limitations remain fail-closed even though they do not
        // earn a semantic shared taxonomy label.
        if super::limitation_kind_blocks_strict_actionability(&limitation.kind) {
            projection.blocks = true;
        }
        if let Some(kind) = limitation_kind(&limitation.kind) {
            projection.blocks = true;
            if kind == StaticLimitKind::DynamicDispatch {
                projection.blocks_class = true;
            }
            select_more_specific(&mut projection.kind, kind);
        }
    }

    projection
}

/// Map a producer boundary enum without consulting fixture names or messages.
///
/// Broad Perl concepts map to the strongest shared category they actually earn.
/// In particular, `monkeypatch_or_symbol_patch` is not narrowed to
/// `mocked_module`: a symbol-table patch is metaprogramming, but it does not prove
/// that a module was mocked.
fn boundary_kind(kind: BoundaryKind) -> Option<StaticLimitKind> {
    match kind {
        BoundaryKind::DynamicDispatch => Some(StaticLimitKind::DynamicDispatch),
        BoundaryKind::ModuleResolutionUnknown => Some(StaticLimitKind::MissingImportGraph),
        BoundaryKind::GeneratedSymbol
        | BoundaryKind::RoleComposition
        | BoundaryKind::MonkeypatchOrSymbolPatch
        | BoundaryKind::EvalOrStringCode
        | BoundaryKind::SymbolTableMutation => Some(StaticLimitKind::Metaprogramming),
        BoundaryKind::FrameworkIndirection => Some(StaticLimitKind::DecoratorIndirection),
        BoundaryKind::UnknownHelper => Some(StaticLimitKind::OpaqueCustomAssertionHelper),
        BoundaryKind::UnsupportedSyntax => Some(StaticLimitKind::UnsupportedSyntax),
        BoundaryKind::MissingTestRunner
        | BoundaryKind::MissingDiffOwner
        | BoundaryKind::PacketIncomplete
        | BoundaryKind::PartialEmitter
        | BoundaryKind::Unknown => None,
    }
}

/// Map stable producer limitation codes.
///
/// `framework_indirection` is intentionally different from the boundary enum:
/// the current producer emits this limitation when a recognized Test::More/Test2
/// file has only wrapped, aliased, or dynamically generated assertions. That is
/// an opaque assertion-helper limitation, not proof of a decorator boundary.
fn limitation_kind(kind: &str) -> Option<StaticLimitKind> {
    match kind {
        "dynamic_dispatch" => Some(StaticLimitKind::DynamicDispatch),
        "module_resolution_unknown" | "missing_import_graph" => {
            Some(StaticLimitKind::MissingImportGraph)
        }
        "generated_symbol"
        | "role_composition"
        | "monkeypatch_or_symbol_patch"
        | "eval_or_string_code"
        | "symbol_table_mutation"
        | "metaprogramming" => Some(StaticLimitKind::Metaprogramming),
        "decorator_indirection" => Some(StaticLimitKind::DecoratorIndirection),
        "mocked_module" => Some(StaticLimitKind::MockedModule),
        "framework_indirection" | "unknown_helper" | "opaque_custom_assertion_helper" => {
            Some(StaticLimitKind::OpaqueCustomAssertionHelper)
        }
        "unsupported_syntax" | "parse_failure" => Some(StaticLimitKind::UnsupportedSyntax),
        _ => None,
    }
}

fn select_more_specific(selected: &mut Option<StaticLimitKind>, candidate: StaticLimitKind) {
    if selected.is_none_or(|current| priority(candidate) < priority(current)) {
        *selected = Some(candidate);
    }
}

fn priority(kind: StaticLimitKind) -> u8 {
    match kind {
        StaticLimitKind::UnsupportedSyntax => 0,
        StaticLimitKind::MockedModule => 1,
        StaticLimitKind::OpaqueCustomAssertionHelper => 2,
        StaticLimitKind::DecoratorIndirection => 3,
        StaticLimitKind::MissingImportGraph => 4,
        StaticLimitKind::Metaprogramming => 5,
        StaticLimitKind::DynamicDispatch => 6,
        _ => u8::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perl_static_limit_maps_boundary_facts_without_message_parsing() {
        let cases = [
            (
                BoundaryKind::DynamicDispatch,
                Some(StaticLimitKind::DynamicDispatch),
            ),
            (
                BoundaryKind::ModuleResolutionUnknown,
                Some(StaticLimitKind::MissingImportGraph),
            ),
            (
                BoundaryKind::GeneratedSymbol,
                Some(StaticLimitKind::Metaprogramming),
            ),
            (
                BoundaryKind::RoleComposition,
                Some(StaticLimitKind::Metaprogramming),
            ),
            (
                BoundaryKind::MonkeypatchOrSymbolPatch,
                Some(StaticLimitKind::Metaprogramming),
            ),
            (
                BoundaryKind::EvalOrStringCode,
                Some(StaticLimitKind::Metaprogramming),
            ),
            (
                BoundaryKind::SymbolTableMutation,
                Some(StaticLimitKind::Metaprogramming),
            ),
            (
                BoundaryKind::FrameworkIndirection,
                Some(StaticLimitKind::DecoratorIndirection),
            ),
            (
                BoundaryKind::UnknownHelper,
                Some(StaticLimitKind::OpaqueCustomAssertionHelper),
            ),
            (
                BoundaryKind::UnsupportedSyntax,
                Some(StaticLimitKind::UnsupportedSyntax),
            ),
            (BoundaryKind::MissingTestRunner, None),
            (BoundaryKind::MissingDiffOwner, None),
            (BoundaryKind::PacketIncomplete, None),
            (BoundaryKind::PartialEmitter, None),
            (BoundaryKind::Unknown, None),
        ];

        for (boundary, expected) in cases {
            assert_eq!(boundary_kind(boundary), expected, "{boundary:?}");
        }
    }

    #[test]
    fn perl_static_limit_maps_stable_limitation_codes() {
        let cases = [
            ("dynamic_dispatch", StaticLimitKind::DynamicDispatch),
            (
                "module_resolution_unknown",
                StaticLimitKind::MissingImportGraph,
            ),
            ("generated_symbol", StaticLimitKind::Metaprogramming),
            (
                "decorator_indirection",
                StaticLimitKind::DecoratorIndirection,
            ),
            ("mocked_module", StaticLimitKind::MockedModule),
            (
                "framework_indirection",
                StaticLimitKind::OpaqueCustomAssertionHelper,
            ),
            ("parse_failure", StaticLimitKind::UnsupportedSyntax),
        ];

        for (code, expected) in cases {
            assert_eq!(limitation_kind(code), Some(expected), "{code}");
        }
        assert_eq!(limitation_kind("narrowed_representation"), None);
        assert_eq!(limitation_kind("packet_incomplete"), None);
    }

    #[test]
    fn perl_static_limit_keeps_operational_limitations_blocking_without_a_label() {
        for code in [
            "missing_test_runner",
            "missing_diff_owner",
            "narrowed_representation",
            "packet_incomplete",
            "partial_emitter",
            "unknown",
        ] {
            assert!(super::super::limitation_kind_blocks_strict_actionability(
                code
            ));
            assert_eq!(limitation_kind(code), None, "{code} must remain untyped");
        }
    }

    #[test]
    fn perl_static_limit_prefers_the_most_specific_earned_category() {
        let mut selected = Some(StaticLimitKind::DynamicDispatch);
        select_more_specific(&mut selected, StaticLimitKind::Metaprogramming);
        select_more_specific(&mut selected, StaticLimitKind::OpaqueCustomAssertionHelper);
        select_more_specific(&mut selected, StaticLimitKind::UnsupportedSyntax);

        assert_eq!(selected, Some(StaticLimitKind::UnsupportedSyntax));
    }
}
