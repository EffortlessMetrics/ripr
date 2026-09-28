//! Projection-only controls derived from the pinned real-producer packet.
//!
//! The in-memory variants below are synthetic, not new producer receipts. They
//! isolate runner availability from semantic evidence without modifying the
//! byte-pinned migration corpus or claiming ingestion/runner execution proof.

use super::super::{Confidence, DynamicBoundaryFact, PacketStatus, packet_to_findings};
use super::*;
use crate::domain::{ExposureClass, Finding, LanguageStatus};

const REAL_PRODUCER_PACKET: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/perl_packet_contract_migration/producer-packets/v1/ordinary_discount.json"
));

/// Isolate the boundary under test from the real packet's operational blockers.
fn isolated_packet() -> Result<PerlFactPacket, String> {
    let mut packet: PerlFactPacket = serde_json::from_str(REAL_PRODUCER_PACKET)
        .map_err(|error| format!("decode pinned producer packet: {error}"))?;
    assert_eq!(packet.changes.len(), 1);
    assert!(packet.dynamic_boundaries.is_empty());
    packet.packet_status = PacketStatus::Complete;
    packet.limitations.clear();
    packet.verify_commands.clear();
    Ok(packet)
}

/// Add an owner-scoped boundary without duplicating the production classifier.
fn boundary(packet: &PerlFactPacket, kind: BoundaryKind) -> Result<DynamicBoundaryFact, String> {
    let change = packet
        .changes
        .first()
        .ok_or_else(|| "missing changed owner in the control packet".to_string())?;
    Ok(DynamicBoundaryFact {
        boundary_id: format!("boundary:runner-control:{kind:?}"),
        kind,
        file_id: change.file_id.clone(),
        owner_id: Some(change.owner_id.clone()),
        range: change.range.clone(),
        confidence: Confidence::High,
        provenance_refs: Vec::new(),
    })
}

/// Require a nonempty production projection before inspecting its disposition.
fn finding(packet: &PerlFactPacket) -> Result<Finding, String> {
    let mut findings = packet_to_findings(packet);
    assert_eq!(findings.len(), 1);
    findings
        .pop()
        .ok_or_else(|| "production projection omitted the control finding".to_string())
}

/// Exercise the same boundary authority consumed by the production projection.
fn projection(packet: &PerlFactPacket) -> Result<Projection, String> {
    let change = packet
        .changes
        .first()
        .ok_or_else(|| "missing change for boundary projection".to_string())?;
    let related = packet.related_test_evidence_for_change(&change.change_id);
    assert!(!related.is_empty());
    Ok(for_change(packet, change, &related))
}

#[test]
fn perl_static_limit_missing_runner_keeps_observation() -> Result<(), String> {
    let mut packet = isolated_packet()?;
    assert_eq!(projection(&packet)?, Projection::default());
    assert_eq!(finding(&packet)?.class, ExposureClass::Exposed);

    let missing_runner = boundary(&packet, BoundaryKind::MissingTestRunner)?;
    packet.dynamic_boundaries.push(missing_runner);
    let projected = projection(&packet)?;
    assert!(projected.blocks);
    assert!(!projected.blocks_class);
    assert_eq!(projected.kind, None);

    let observed = finding(&packet)?;
    assert_eq!(observed.class, ExposureClass::Exposed);
    assert_eq!(observed.language_status, Some(LanguageStatus::Preview));
    assert!(observed.canonical_gap.is_none());
    assert!(
        observed
            .evidence
            .iter()
            .any(|evidence| evidence.starts_with("perl_already_discriminated:"))
    );
    assert!(
        !observed
            .evidence
            .iter()
            .any(|evidence| evidence.starts_with("perl_suggested_"))
    );
    assert!(packet.verify_commands.is_empty());
    Ok(())
}

#[test]
fn perl_static_limit_missing_runner_does_not_invent_observation() -> Result<(), String> {
    let mut packet = isolated_packet()?;
    let change = packet
        .changes
        .first_mut()
        .ok_or_else(|| "missing control change".to_string())?;
    change.changed_observable = None;
    change.changed_text_digest = "discriminator:assert the changed return value".to_string();
    let before = finding(&packet)?;
    assert_eq!(before.class, ExposureClass::WeaklyExposed);
    assert!(
        before
            .evidence
            .iter()
            .any(|evidence| evidence.starts_with("perl_suggested_test_location:"))
    );

    let missing_runner = boundary(&packet, BoundaryKind::MissingTestRunner)?;
    packet.dynamic_boundaries.push(missing_runner);
    assert!(projection(&packet)?.blocks);
    let after = finding(&packet)?;
    assert_eq!(after.class, ExposureClass::WeaklyExposed);
    assert!(after.canonical_gap.is_none());
    assert!(!after.evidence.iter().any(|evidence| {
        evidence.starts_with("perl_suggested_")
            || evidence.starts_with("perl_already_discriminated:")
    }));
    Ok(())
}

#[test]
fn perl_static_limit_other_boundaries_still_cap_sink_observation() -> Result<(), String> {
    for kind in [
        BoundaryKind::DynamicDispatch,
        BoundaryKind::ModuleResolutionUnknown,
        BoundaryKind::GeneratedSymbol,
        BoundaryKind::RoleComposition,
        BoundaryKind::MonkeypatchOrSymbolPatch,
        BoundaryKind::EvalOrStringCode,
        BoundaryKind::SymbolTableMutation,
        BoundaryKind::FrameworkIndirection,
        BoundaryKind::UnknownHelper,
        BoundaryKind::UnsupportedSyntax,
        BoundaryKind::MissingDiffOwner,
        BoundaryKind::PacketIncomplete,
        BoundaryKind::PartialEmitter,
        BoundaryKind::Unknown,
    ] {
        let mut packet = isolated_packet()?;
        assert_eq!(finding(&packet)?.class, ExposureClass::Exposed);
        let fact = boundary(&packet, kind)?;
        packet.dynamic_boundaries.push(fact);
        let projected = projection(&packet)?;
        assert!(projected.blocks, "{kind:?}");
        assert!(projected.blocks_class, "{kind:?}");
        assert_eq!(
            finding(&packet)?.class,
            ExposureClass::StaticUnknown,
            "{kind:?}"
        );
    }
    Ok(())
}

#[test]
fn perl_static_limit_missing_runner_never_clears_another_boundary_cap() -> Result<(), String> {
    for kinds in [
        [
            BoundaryKind::DynamicDispatch,
            BoundaryKind::MissingTestRunner,
        ],
        [
            BoundaryKind::MissingTestRunner,
            BoundaryKind::DynamicDispatch,
        ],
    ] {
        let mut packet = isolated_packet()?;
        for kind in kinds {
            let fact = boundary(&packet, kind)?;
            packet.dynamic_boundaries.push(fact);
        }
        let projected = projection(&packet)?;
        assert!(projected.blocks);
        assert!(projected.blocks_class);
        assert_eq!(projected.kind, Some(StaticLimitKind::DynamicDispatch));
        assert_eq!(finding(&packet)?.class, ExposureClass::StaticUnknown);
    }
    Ok(())
}

#[test]
fn perl_static_limit_missing_runner_respects_scope() -> Result<(), String> {
    let packet = isolated_packet()?;
    let mut unrelated = boundary(&packet, BoundaryKind::MissingTestRunner)?;
    let other_owner = packet
        .owners
        .iter()
        .find(|owner| Some(&owner.owner_id) != unrelated.owner_id.as_ref())
        .ok_or_else(|| "control packet needs a different owner".to_string())?;
    unrelated.owner_id = Some(other_owner.owner_id.clone());
    let mut other = packet.clone();
    other.dynamic_boundaries.push(unrelated);
    assert_eq!(projection(&other)?, Projection::default());

    let test = packet
        .tests
        .first()
        .ok_or_else(|| "control packet needs a related test".to_string())?;
    let mut test_boundary = boundary(&packet, BoundaryKind::MissingTestRunner)?;
    test_boundary.owner_id = None;
    test_boundary.file_id = test.file_id.clone();
    test_boundary.range = test.range.clone();
    let mut related = packet;
    related.dynamic_boundaries.push(test_boundary);
    let projected = projection(&related)?;
    assert!(projected.blocks);
    assert!(!projected.blocks_class);
    assert_eq!(finding(&related)?.class, ExposureClass::Exposed);
    Ok(())
}
